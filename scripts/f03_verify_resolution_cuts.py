"""Verify retained Entity Resolution revisions and current state at pinned physical cuts."""
import argparse
from collections import Counter
from dataclasses import dataclass
import hashlib
import json
from pathlib import Path
from struct import Struct

from f03_legacy_frame_clock import LegacyFrameClock
from f03_verify_archived_oracle import PINS, require
from f03_verify_independent_repeat import (
    MANIFEST, SCHEMA_V2, decimal, open_header, read_document, read_frame, regular, snapshot, take_stream,
)


NATIVE_DOMAIN = "clock_neutral_history"
REVISION = "entity_resolution_revision"
INERT = "retained_inert_resolution"
DELTA = b"CVAERS03"
LITERAL = b"CVAERL01"
FULL = b"CVAERS01"
HEADER = Struct("<32sBIIIqB")
HEADER_END = 8 + HEADER.size
STATUS = ("tombstoned", "resolved", "rejected", "pending", "dormant")
RETAINED_DOMAIN = b"reliquary:f03:entity-resolution-retained:v1\0"
CURRENT_DOMAIN = b"reliquary:f03:entity-resolution-current:v1\0"


def encode(value):
    return json.dumps(value, sort_keys=True, separators=(",", ":")).encode("ascii")


def sha256(value):
    return hashlib.sha256(value).hexdigest()


class Reader:
    def __init__(self, data):
        self.data = data
        self.offset = 0

    def take(self, size):
        end = self.offset + size
        require(end <= len(self.data), "truncated Entity Resolution field")
        value = self.data[self.offset:end]
        self.offset = end
        return value

    def candidates(self):
        count = self.take(1)[0]
        require(count <= 8, "Entity Resolution has too many candidates")
        body = self.take(count * 32)
        identities = [body[index:index + 32] for index in range(0, len(body), 32)]
        require(all(left < right for left, right in zip(identities, identities[1:])),
                "Entity Resolution candidates are repeated or unordered")
        return bytes([count]) + body

    def finish(self):
        require(self.offset == len(self.data), "trailing Entity Resolution bytes")


@dataclass(frozen=True)
class Decision:
    revision: int
    status: int
    evidence: tuple
    value_sha256: str


def signed(field):
    return int.from_bytes(field, "little", signed=True)


def unresolved(reader, status, previous):
    mask = reader.take(1)[0]
    require(status == 4 or not mask & 128, "Pending delta contains Dormant timestamp")
    prior = previous.evidence if previous is not None and previous.status in (3, 4) else ()
    parts = []
    for index, size in enumerate((0, 1, 32, 32, 8, 8, 2, 8)[:7 + (status == 4)]):
        if mask & (1 << index):
            parts.append(reader.take(size) if size else reader.candidates())
        else:
            require(index < len(prior), "Entity Resolution delta has no matching evidence base")
            parts.append(prior[index])
    require(signed(parts[4]) <= signed(parts[5]) and parts[6] != b"\0\0"
            and (status == 3 or signed(parts[5]) <= signed(parts[7])),
            "invalid Entity Resolution retry or Dormant chronology")
    return tuple(parts)


def decode_delta(payload, current):
    require(payload.startswith(DELTA) and len(payload) >= HEADER_END, "invalid Entity Resolution delta")
    memory, field, start, end, revision, updated, status = HEADER.unpack_from(payload, 8)
    require(field in (1, 2) and start < end and status < len(STATUS),
            "invalid Entity Resolution mention or status")
    key = payload[8:8 + 41]
    previous = current.get(key)
    require(revision == (1 if previous is None else previous.revision + 1),
            "Entity Resolution revision sequence differs from accepted state")
    reader = Reader(payload[HEADER_END:])
    if status in (3, 4):
        evidence = unresolved(reader, status, previous)
        reason = evidence[1][0]
    elif status in (1, 2):
        evidence = (reader.take(32), reader.take(1)) if status == 1 else (reader.take(1),)
        reason = evidence[-1][0]
    else:
        evidence = ()
        reason = None
    require(reason is None or 1 <= reason <= 15, "invalid Entity Resolution reason")
    reader.finish()
    comparison = FULL + payload[8:HEADER_END] + b"".join(evidence)
    value = {
        "availability": "retained" if status else "tombstoned",
        "memory_id": memory.hex(), "field": field, "start_byte": start, "end_byte": end,
        "revision": revision, "updated_at_ns": updated, "comparison_sha256": sha256(comparison),
    }
    return key, Decision(revision, status, evidence, sha256(encode(value))), value


def decode_inert(payload):
    require(payload.startswith(LITERAL) and len(payload) >= 52, "truncated inert Entity Resolution")
    size = int.from_bytes(payload[8:12], "little")
    original = payload[12:-32]
    require(8 <= size <= 8 * 1024 * 1024 and size == len(original)
            and original.startswith(b"CVAERS02")
            and hashlib.sha256(payload[:-32]).digest() == payload[-32:],
            "invalid inert Entity Resolution length, codec or checksum")
    return {"availability": "inert", "retained_codec": "CVAERS02",
            "retained_payload_sha256": sha256(original), "retained_payload_byte_length": size}


def expected_records(output, artifacts, owner):
    expected = {}
    for _, row in take_stream(output, artifacts, owner, "records", owner["native"]["record_stream"],
                              schema_id=SCHEMA_V2):
        if row["projection_domain"] == NATIVE_DOMAIN and row["family"] in (REVISION, INERT):
            key = row["family"], row["stable_key_sha256"]
            require(key not in expected, "duplicate native Entity Resolution commitment")
            expected[key] = row
    return expected


def scan(path, owner, output, artifacts, kind):
    before = regular(path)
    require(before.st_size == decimal(owner["source_byte_length"]), "source byte length differs")
    expected = expected_records(output, artifacts, owner)
    clock = LegacyFrameClock()
    counts = Counter()
    current = {}
    retained = []
    with path.open("rb") as source:
        source_hash = open_header(source, kind, owner)
        for ordinal, frame in take_stream(output, artifacts, owner, "frames", owner["frame_stream"],
                                          schema_id=SCHEMA_V2):
            payload = read_frame(source, frame, source_hash, before.st_size)
            global_version = clock.consume(payload)
            require(str(global_version) == frame["latest_global"], "Entity Resolution physical clock differs")
            marker = payload[:8]
            if marker == DELTA:
                key, decision, value = decode_delta(payload, current)
                if decision.status:
                    current[key] = decision
                else:
                    current.pop(key, None)
                family = REVISION
            elif marker == LITERAL:
                value = decode_inert(payload)
                family, key, decision = INERT, None, None
            else:
                require(not frame["carrier"].startswith(("CVAERS", "CVAERL")),
                        "unsupported Entity Resolution carrier")
                continue
            native_key = sha256(counts[family].to_bytes(8, "big"))
            canonical = encode(value)
            record = {
                "projection_domain": NATIVE_DOMAIN, "family": family, "stable_key_sha256": native_key,
                "canonical_value_sha256": sha256(canonical), "canonical_byte_length": str(len(canonical)),
                "origin_coordinate": {}, "availability": value["availability"],
            }
            require(expected.pop((family, native_key), None) == record,
                    f"Entity Resolution differs from its native commitment at frame {ordinal}")
            counts[family] += 1
            retained.append({
                **record, "first_global": str(global_version), "physical_ordinal": str(ordinal),
                "mention_key_sha256": None if key is None else sha256(key),
                "revision": None if decision is None else str(decision.revision),
                "status": "inert" if decision is None else STATUS[decision.status],
                "inherited_fields": (7 + (decision.status == 4) - payload[HEADER_END].bit_count()
                                     if decision is not None and decision.status in (3, 4) else 0),
            })
        require(source.tell() == before.st_size and source_hash.hexdigest() == owner["source_sha256"]
                and snapshot(before) == snapshot(regular(path)), "Entity Resolution source differs from pins")
    require(not expected, "retained native Entity Resolution commitments were omitted")
    require(clock.head + 1 == decimal(owner["native"]["global_stream"]["rows"]),
            "Entity Resolution global frontier count differs from native evidence")
    direct_state = {sha256(key): decision.value_sha256 for key, decision in current.items()}
    return clock.head, retained, direct_state


def current_digest(uuid, active):
    result = hashlib.sha256(CURRENT_DOMAIN + uuid + len(active).to_bytes(8, "little"))
    for key, value in sorted(active.items()):
        result.update(bytes.fromhex(key + value))
    return result.hexdigest()


def selected_interval(record, last):
    return {key: record[key] for key in (
        "mention_key_sha256", "canonical_value_sha256", "revision", "status", "physical_ordinal",
        "first_global",
    )} | {"last_global": str(last)}


def frontier(uuid, head, retained, intervals):
    active = {}
    current = current_digest(uuid, active)
    retained_hash = hashlib.sha256(RETAINED_DOMAIN + uuid)
    ordinal = 0
    for cut in range(head + 1):
        changes = {}
        while ordinal < len(retained) and int(retained[ordinal]["first_global"]) == cut:
            row = retained[ordinal]
            # The coverage endpoint is external to the immutable prefix.
            canonical = encode({key: value for key, value in row.items() if key != "last_global"})
            retained_hash.update(len(canonical).to_bytes(8, "little") + canonical)
            if row["mention_key_sha256"] is not None:
                changes[row["mention_key_sha256"]] = row
            ordinal += 1
        if changes:
            for key, row in sorted(changes.items()):
                previous = active.pop(key, None)
                if previous is not None:
                    intervals.append(selected_interval(previous, cut - 1))
                if row["availability"] != "tombstoned":
                    active[key] = row
            current = current_digest(uuid, {key: row["canonical_value_sha256"] for key, row in active.items()})
        yield {"global_version": str(cut), "retained_rows": str(ordinal),
               "retained_prefix_sha256": retained_hash.hexdigest(), "current_rows": str(len(active)),
               "current_state_sha256": current}
    require(ordinal == len(retained), "Entity Resolution publication lies outside physical history")
    intervals.extend(selected_interval(row, head) for _, row in sorted(active.items()))


def write_rows(path, rows):
    digest = hashlib.sha256()
    last = None
    count = 0
    with path.open("xb") as output:
        for row in rows:
            line = encode(row) + b"\n"
            output.write(line)
            digest.update(line)
            last = row
            count += 1
    return {"file": path.name, "sha256": digest.hexdigest(), "rows": count}, last


def observe(owner, path, output, artifacts, private, kind):
    head, retained, direct_state = scan(path, owner, output, artifacts, kind)
    for row in retained:
        row["last_global"] = str(head)
    copy = owner["copy_id"]
    uuid = bytes.fromhex(owner["owner_uuid"])
    intervals = []
    retained_report, _ = write_rows(private / f"{copy}-resolution-retained.jsonl", retained)
    cut_report, final = write_rows(private / f"{copy}-resolution-cuts.jsonl", frontier(uuid, head, retained, intervals))
    require(int(final["current_rows"]) == len(direct_state)
            and final["current_state_sha256"] == current_digest(uuid, direct_state),
            "Entity Resolution current frontier differs from direct replay")
    interval_report, _ = write_rows(private / f"{copy}-resolution-current-intervals.jsonl", intervals)
    return {
        "copy_id": copy, "source_sha256": owner["source_sha256"],
        "status_counts": dict(Counter(row["status"] for row in retained)),
        "inherited_revision_rows": sum(row["inherited_fields"] > 0 for row in retained), "final_cut": final,
        "retained": retained_report, "cuts": cut_report, "current_intervals": interval_report,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-copies", type=Path, required=True)
    parser.add_argument("--source-coverage", type=Path, required=True)
    parser.add_argument("--private-output", type=Path, required=True,
                        help="fresh directory under a protected evidence root")
    args = parser.parse_args()
    output = args.source_coverage.parent
    coverage, coverage_sha = read_document(args.source_coverage, 2 * 1024 * 1024)
    manifest, manifest_sha = read_document(output / MANIFEST, 2 * 1024 * 1024)
    require(coverage["schema_id"] == manifest["schema_id"] == SCHEMA_V2
            and manifest["source_coverage_sha256"] == coverage_sha, "invalid v2 source evidence provenance")
    owners = {owner["source_sha256"]: owner for owner in coverage["sources"]}
    require(len(coverage["sources"]) == len(PINS) and owners.keys() == set(PINS.values()),
            "Entity Resolution cohort differs from reviewed pins")
    for index, (_, digest) in enumerate(sorted(PINS.items()), 1):
        require(owners[digest]["copy_id"] == f"copy-{index:04d}", "unexpected Entity Resolution copy identifier")
    require(not args.private_output.resolve().is_relative_to(args.source_copies.resolve(strict=True)),
            "Entity Resolution output must be outside source copies")
    regular(args.private_output.parent, directory=True)
    args.private_output.mkdir()
    artifacts = manifest["artifact_sha256"].copy()
    results = [
        observe(owners[digest], args.source_copies / fixture, output, artifacts,
                args.private_output, 1 if fixture.endswith(".rel") else 2)
        for fixture, digest in sorted(PINS.items())
    ]
    print(json.dumps({
        "schema_id": "reliquary-f03-entity-resolution-frontier-verification-v1",
        "status": "PASS", "authentic_admission": False,
        "verifier_sha256": sha256(Path(__file__).read_bytes()),
        "scope": "retained Entity Resolution revisions and resolution-store current state at pinned physical frontiers",
        "source_coverage_sha256": coverage_sha, "source_manifest_sha256": manifest_sha,
        "retained_rows": sum(result["retained"]["rows"] for result in results),
        "global_cuts": sum(result["cuts"]["rows"] for result in results), "owners": results,
    }, indent=2))


if __name__ == "__main__":
    main()
