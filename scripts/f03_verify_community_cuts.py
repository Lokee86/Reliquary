"""Reconstruct retained Community history at each pinned physical frontier."""
import argparse
from collections import defaultdict
from fractions import Fraction
import hashlib
import json
import math
from pathlib import Path
from struct import unpack_from

from f03_legacy_frame_clock import LegacyFrameClock
from f03_verify_archived_oracle import PINS, require
from f03_verify_independent_repeat import (
    MANIFEST, SCHEMAS, open_header, read_document, read_frame, regular, snapshot, take_stream,
)


DOMAIN = b"reliquary:f03:clock-neutral-community:v1\0"
NATIVE_DOMAIN = "clock_neutral_history"
FAMILIES = {"community_snapshot", "community_lineage", "community_name"}


def encode(value):
    return json.dumps(value, sort_keys=True, separators=(",", ":")).encode("ascii")


def sha256(value):
    return hashlib.sha256(value).hexdigest()


def decode_snapshot(payload, owner_uuid):
    require(len(payload) >= 60, "truncated Community snapshot header")
    schema, generation, graph, algorithm, seed, resolution, quality, count = unpack_from(
        "<IQQIQddI", payload, 8
    )
    require(schema == 1 and generation > 0 and algorithm in (1, 2)
            and math.isfinite(resolution) and resolution > 0 and math.isfinite(quality),
            "invalid Community snapshot header")
    communities = []
    previous = b""
    seen = set()
    offset = 60
    for _ in range(count):
        require(offset + 36 <= len(payload), "truncated Community header")
        identity, size = unpack_from("<32sI", payload, offset)
        offset += 36
        end = offset + 32 * size
        require(size > 0 and end <= len(payload) and previous < identity,
                "invalid Community count or identity order")
        members = [payload[index:index + 32] for index in range(offset, end, 32)]
        require(all(left < right for left, right in zip(members, members[1:]))
                and not seen.intersection(members), "invalid Community member ordering or duplication")
        preimage = b"reliquary-community-v1\0" + owner_uuid + size.to_bytes(8, "little")
        require(hashlib.sha256(preimage + payload[offset:end]).digest() == identity,
                "Community ID does not match its exact membership")
        communities.append({"id": identity.hex(), "members": [item.hex() for item in members]})
        seen.update(members)
        previous = identity
        offset = end
    require(offset == len(payload), "Community snapshot trailing bytes")
    return {
        "generation": generation, "derived_graph_version": graph, "algorithm_version": algorithm,
        "seed": seed, "resolution_f64_le": payload[40:48].hex(), "quality_f64_le": payload[48:56].hex(),
        "communities": communities,
    }


def unique_best(options):
    maximum = max(score for score, _ in options)
    choices = [identity for score, identity in options if score == maximum]
    return choices[0] if len(choices) == 1 else None


def derive_lineage(previous, current):
    old = {row["id"]: set(row["members"]) for row in previous["communities"]}
    new = {row["id"]: set(row["members"]) for row in current["communities"]}
    prior_by_member = {member: identity for identity, members in old.items() for member in members}
    overlaps = defaultdict(int)
    for successor, members in new.items():
        for member in members:
            if member in prior_by_member:
                overlaps[(prior_by_member[member], successor)] += 1
    from_options = defaultdict(list)
    to_options = defaultdict(list)
    for (prior, successor), overlap in overlaps.items():
        score = Fraction(overlap, len(old[prior]) + len(new[successor]) - overlap)
        from_options[prior].append((score, successor))
        to_options[successor].append((score, prior))
    from_best = {identity: unique_best(options) for identity, options in from_options.items()}
    to_best = {identity: unique_best(options) for identity, options in to_options.items()}
    links = [{
        "predecessor": prior, "successor": successor, "intersection": overlap,
        "predecessor_members": len(old[prior]), "successor_members": len(new[successor]),
        "continuation": from_best[prior] == successor and to_best[successor] == prior,
        "split": len(from_options[prior]) > 1, "merge": len(to_options[successor]) > 1,
    } for (prior, successor), overlap in sorted(overlaps.items())]
    return {
        "from_generation": previous["generation"], "to_generation": current["generation"],
        "links": links, "new": sorted(new.keys() - to_options.keys()),
        "ended": sorted(old.keys() - from_options.keys()),
    }


def expected_records(output, artifacts, owner, schema):
    stream = take_stream(output, artifacts, owner, "records", owner["native"]["record_stream"],
                         schema_id=schema)
    records = {}
    for _, row in stream:
        if row["projection_domain"] == NATIVE_DOMAIN and row["family"] in FAMILIES:
            key = row["family"], row["stable_key_sha256"]
            require(key not in records and row["availability"] == "retained",
                    "duplicate or unavailable Community commitment")
            records[key] = row["canonical_value_sha256"], row["canonical_byte_length"]
    return records


def observe(path, output, artifacts, owner, schema, kind):
    expected = expected_records(output, artifacts, owner, schema)
    clock = LegacyFrameClock()
    before = regular(path)
    uuid = bytes.fromhex(owner["owner_uuid"])
    transitions = []
    previous = None
    with path.open("rb") as source:
        source_hash = open_header(source, kind, owner)
        frames = take_stream(output, artifacts, owner, "frames", owner["frame_stream"], schema_id=schema)
        for ordinal, frame in frames:
            payload = read_frame(source, frame, source_hash, before.st_size)
            global_ = clock.consume(payload)
            require(str(global_) == frame["latest_global"], "Community physical clock mismatch")
            marker = payload[:8]
            if marker != b"CVACOMM1":
                require(not marker.startswith((b"CVACOMM", b"CVACNAM")),
                        "Community carrier or semantic name outside verified schema")
                continue
            current = decode_snapshot(payload, uuid)
            generation = current["generation"]
            require(generation == (1 if previous is None else previous["generation"] + 1),
                    "Community generation discontinuity")
            records = {"community_snapshot": current}
            if previous is not None:
                records["community_lineage"] = derive_lineage(previous, current)
            commitments = {}
            for family, value in records.items():
                canonical = encode(value)
                digest = sha256(canonical)
                key = family, sha256(generation.to_bytes(8, "big"))
                require(expected.pop(key, None) == (digest, str(len(canonical))),
                        "Community history differs from its typed native commitment")
                commitments[family] = digest
            transitions.append({
                "first_global": str(global_), "physical_ordinal": str(ordinal), "generation": str(generation),
                "derived_memory_graph_version": str(current["derived_graph_version"]),
                "records": commitments,
            })
            previous = current
        require(source.tell() == before.st_size and source_hash.hexdigest() == owner["source_sha256"]
                and snapshot(before) == snapshot(regular(path)), "Community source differs from its pins")
    require(not expected, "native Community commitments were omitted by the physical traversal")

    digest = hashlib.sha256(DOMAIN)
    digest.update(uuid)
    cuts = [{"first_global": "0", "visible_generations": 0, "sha256": digest.hexdigest()}]
    for count, transition in enumerate(transitions, 1):
        encoded = encode(transition)
        digest.update(len(encoded).to_bytes(8, "little") + encoded)
        cut = {"first_global": transition["first_global"], "visible_generations": count,
               "sha256": digest.hexdigest()}
        if cut["first_global"] == cuts[-1]["first_global"]:
            cuts[-1] = cut
        else:
            cuts.append(cut)
    for cut, next_cut in zip(cuts, cuts[1:]):
        cut["last_global"] = str(int(next_cut["first_global"]) - 1)
    cuts[-1]["last_global"] = str(clock.head)
    return {
        "source_sha256": owner["source_sha256"], "copy_id": owner["copy_id"],
        "global_cuts": clock.head + 1, "snapshot_rows": len(transitions),
        "lineage_rows": max(0, len(transitions) - 1), "name_rows": 0,
        "physical_transitions": transitions, "retained_history_cut_intervals": cuts,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-copies", type=Path, required=True)
    parser.add_argument("--source-coverage", type=Path, required=True)
    args = parser.parse_args()
    output = args.source_coverage.parent
    coverage, coverage_sha = read_document(args.source_coverage, 2 * 1024 * 1024)
    manifest, manifest_sha = read_document(output / MANIFEST, 2 * 1024 * 1024)
    schema = coverage["schema_id"]
    require(schema in SCHEMAS and manifest["schema_id"] == schema
            and manifest["source_coverage_sha256"] == coverage_sha, "invalid source coverage provenance")
    owners = {row["source_sha256"]: row for row in coverage["sources"]}
    require(len(coverage["sources"]) == len(PINS) and owners.keys() == set(PINS.values()),
            "Community cohort differs from reviewed pins")
    artifacts = manifest["artifact_sha256"].copy()
    results = [
        observe(args.source_copies / fixture, output, artifacts, owners[digest], schema,
                1 if fixture.endswith(".rel") else 2)
        for fixture, digest in PINS.items()
    ]
    print(json.dumps({
        "status": "PASS", "scope": "retained Community snapshot and lineage membership at pinned physical frontiers",
        "source_coverage_sha256": coverage_sha, "source_manifest_sha256": manifest_sha,
        "snapshot_rows": sum(row["snapshot_rows"] for row in results),
        "lineage_rows": sum(row["lineage_rows"] for row in results),
        "global_cuts": sum(row["global_cuts"] for row in results),
        "owners": results,
    }, indent=2))


if __name__ == "__main__":
    main()
