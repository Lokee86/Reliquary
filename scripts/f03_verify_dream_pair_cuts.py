"""Verify Dream pair membership at every retained global cut from pinned frames."""
import argparse
import hashlib
import json
from pathlib import Path

from f03_legacy_frame_clock import LegacyFrameClock
from f03_verify_archived_oracle import PINS, require
from f03_verify_independent_repeat import (
    MANIFEST, SCHEMAS, open_header, read_document, read_frame, regular, snapshot, take_stream,
)

PAIR_DOMAIN = b"reliquary:f03:clock-neutral-dream-pairs:v1\0"
NATIVE_DOMAIN = b"reliquary:historical-owner-state:v1\0"


def encode(value):
    return json.dumps(value, sort_keys=True, separators=(",", ":")).encode("ascii")


def framed(value):
    return len(value).to_bytes(8, "little") + value


def native_head(owner, pairs):
    hasher = hashlib.sha256(NATIVE_DOMAIN + (1).to_bytes(4, "little"))
    hasher.update(bytes.fromhex(owner["owner_uuid"]) + framed(b"dream_pairs"))
    hasher.update(b"\0" * 9 + len(pairs).to_bytes(8, "little"))
    for left, right in sorted(pairs):
        value = encode({"left": list(left), "right": list(right)})
        hasher.update(framed(b"pair") + framed(left + right) + framed(value))
    return hasher.hexdigest()


def admit_mapping(payload, mappings, ids):
    require(len(payload) == 44, "invalid Dream Memory mapping")
    slot = int.from_bytes(payload[8:12], "little")
    identity = payload[12:]
    require(slot > 0, "zero Dream Memory mapping")
    if slot in mappings:
        require(mappings[slot] == identity, "conflicting Dream Memory mapping")
    else:
        require(slot == len(mappings) + 1 and identity not in ids, "noncanonical Dream Memory mapping")
        mappings[slot] = identity
        ids.add(identity)


def resolve_pair(payload, mappings):
    require(len(payload) == 16, "invalid Dream pair length")
    left = int.from_bytes(payload[8:12], "little")
    right = int.from_bytes(payload[12:16], "little")
    require(left in mappings and right in mappings and mappings[left] < mappings[right],
            "missing or noncanonical Dream pair identity")
    return mappings[left], mappings[right]


def observe(output, artifacts, owner, schema, path, kind):
    before = regular(path)
    mappings = {}
    ids = set()
    pairs = {}
    pair_frames = 0
    clock = LegacyFrameClock()
    with path.open("rb") as source:
        source_hash = open_header(source, kind, owner)
        for ordinal, frame in take_stream(output, artifacts, owner, "frames", owner["frame_stream"], schema_id=schema):
            payload = read_frame(source, frame, source_hash, before.st_size)
            global_ = int(frame["latest_global"])
            require(global_ == clock.consume(payload),
                    "Dream pair frontier differs from reconstructed physical clock")
            marker = payload[:8]
            if marker == b"CVADRM01":
                admit_mapping(payload, mappings, ids)
            elif marker == b"CVADRP02":
                pair = resolve_pair(payload, mappings)
                pair_frames += 1
                pairs.setdefault(pair, (global_, ordinal))
            elif marker.startswith((b"CVADRM", b"CVADRP")):
                raise ValueError("unknown Dream pair carrier")
        require(source.tell() == before.st_size and source_hash.hexdigest() == owner["source_sha256"]
                and snapshot(before) == snapshot(regular(path)), "Dream pair source differs from its pins")
    native = [row for row in owner["native"]["native_history_projections"] if row["domain"] == "dream_pairs"]
    digest = native_head(owner, pairs)
    require(len(native) == 1 and native[0]["sha256"] == digest and native[0]["rows"] == str(len(pairs)),
            "retained physical Dream pairs differ from the native head")
    head = clock.head
    intervals = []
    for pair, (first, ordinal) in sorted(pairs.items(), key=lambda item: item[1]):
        left, right = pair
        value = encode({"left": list(left), "right": list(right)})
        intervals.append({
            "first_global": str(first), "last_global": str(head), "first_physical_ordinal": str(ordinal),
            "stable_key_sha256": hashlib.sha256(left + right).hexdigest(),
            "value_sha256": hashlib.sha256(value).hexdigest(),
        })
    chain = hashlib.sha256(PAIR_DOMAIN)
    position = 0
    for global_ in range(head + 1):
        while position < len(intervals) and int(intervals[position]["first_global"]) == global_:
            chain.update(framed(encode(intervals[position])))
            position += 1
        yield {"global_version": str(global_), "pair_count": str(position), "prefix_sha256": chain.hexdigest()}
    require(position == len(pairs), "Dream pair interval exceeds the retained global head")
    yield {"native_head_sha256": digest, "pair_frames": pair_frames, "pairs": len(pairs), "intervals": intervals}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-copies", type=Path, required=True)
    parser.add_argument("--source-coverage", type=Path, required=True)
    parser.add_argument("--private-output", type=Path, required=True,
                        help="fresh directory under the caller's protected evidence root")
    args = parser.parse_args()
    output = args.source_coverage.parent
    coverage, coverage_sha = read_document(args.source_coverage, 2 * 1024 * 1024)
    manifest, _ = read_document(output / MANIFEST, 2 * 1024 * 1024)
    schema = coverage["schema_id"]
    require(schema in SCHEMAS and manifest["schema_id"] == schema
            and manifest["source_coverage_sha256"] == coverage_sha, "invalid source coverage provenance")
    owners = {row["source_sha256"]: row for row in coverage["sources"]}
    require(len(coverage["sources"]) == len(PINS) and owners.keys() == set(PINS.values()),
            "Dream pair cohort differs from reviewed pins")
    results = []
    regular(args.private_output.parent, directory=True)
    args.private_output.mkdir()
    for relative, pinned in sorted(PINS.items()):
        owner = owners[pinned]
        rows = observe(output, manifest["artifact_sha256"], owner, schema, args.source_copies / relative,
                       1 if relative.endswith(".rel") else 2)
        path = args.private_output / f'{owner["copy_id"]}-dream-pair-cuts.jsonl'
        digest = hashlib.sha256()
        points = 0
        with path.open("xb") as witness:
            for row in rows:
                if "intervals" in row:
                    summary = row
                    break
                line = encode(row) + b"\n"
                witness.write(line)
                digest.update(line)
                points += 1
        interval_path = args.private_output / f'{owner["copy_id"]}-dream-pair-intervals.jsonl'
        interval_digest = hashlib.sha256()
        with interval_path.open("xb") as witness:
            for row in summary.pop("intervals"):
                line = encode(row) + b"\n"
                witness.write(line)
                interval_digest.update(line)
        results.append({
            "fixture": relative, "source_sha256": pinned, "global_points": points,
            "cuts_sha256": digest.hexdigest(), "intervals_sha256": interval_digest.hexdigest(), **summary,
        })
    print(json.dumps({
        "status": "PASS", "scope": "retained clock-neutral Dream pair memberships",
        "authentic_admission": False, "source_coverage_sha256": coverage_sha, "fixtures": results,
    }, indent=2))


if __name__ == "__main__":
    main()
