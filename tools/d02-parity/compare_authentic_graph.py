"""Independently compare current Graph publications and global times to a pinned F03 owner."""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
from struct import unpack
import subprocess
import sys

from archive_binding_evidence import hex_digest
from authentic_witness import index_rows, read_source, require, retained_records, unsigned, witness_batches
from compare_archive_bindings import digest_file
from evidence_io import WitnessInputError, strict_json_document

SCHEMA = "reliquary-d02-authentic-graph-history-comparison-v1"
DOMAINS = ("graph", "memory_graph")
KINDS = {"REL": 1, "PHY": 2}
SCOPES = {"unspecified": 0, "organization": 1, "project": 2, "connection": 3}
HEADER = b"REL\0\r\n\x1a\n\x02\0\x01\0\x28\0\0\0"


def commitment(domain, family, key, value):
    canonical = json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode("utf-8")
    return {
        "projection_domain": domain, "family": family, "stable_key_sha256": hashlib.sha256(key).hexdigest(),
        "canonical_value_sha256": hashlib.sha256(canonical).hexdigest(),
        "canonical_byte_length": str(len(canonical)),
        "origin_coordinate": {"global_version": str(value["global_version"])} if family == "publication" else {},
        "availability": "retained",
    }


class Clock:
    def __init__(self, limit):
        self.times = [None]
        self.reservations = set()
        self.limit = limit

    def allocate(self, start, count, time):
        if count:
            require(start == len(self.times) and len(self.times) + count <= self.limit,
                    "target global allocation is not contiguous and bounded by source")
            self.times.extend([time] * count)

    def read(self, payload):
        marker = payload[:8]
        if marker in (b"RELVERS1", b"RELVERS2"):
            require(len(payload) == (16 if marker == b"RELVERS1" else 24), "invalid current reservation")
            version, = unpack("<Q", payload[8:16])
            time = unpack("<q", payload[16:24])[0] if len(payload) == 24 else None
            self.allocate(version, 1, time)
            self.reservations.add(version)
        elif marker == b"RELPUB01":
            require(len(payload) >= 25 and payload[16] in (0, 1, 2), "invalid current publication")
            version, = unpack("<Q", payload[8:16])
            mode = payload[16]
            start = 25 if mode == 1 else 17
            require(len(payload) >= start + 8, "truncated current publication body")
            if mode == 2:
                require(version in self.reservations, "current publication has no prior reservation")
                self.reservations.remove(version)
            else:
                time = unpack("<q", payload[17:25])[0] if mode == 1 else None
                self.allocate(version, 1, time)
            return version, payload[start:]
        elif marker == b"RELINSC7":
            require(len(payload) >= 84, "truncated grouped Insomnia range")
            self.allocate(*unpack("<QIq", payload[64:84]))
        elif marker == b"RELINSC8":
            require(len(payload) >= 9 and payload[8] in range(1, 7), "invalid grouped Insomnia clock shape")
            if payload[8] != 1:
                require(len(payload) >= (85 if payload[8] >= 4 else 77), "truncated grouped Insomnia range")
                start, count = unpack("<QI", payload[65:77])
                time = unpack("<q", payload[77:85])[0] if payload[8] >= 4 else None
                self.allocate(start, count, time)
        else:
            require(not marker.startswith((b"RELVERS", b"RELPUB", b"RELINSC", b"RELXMEM2")),
                    "unreviewed current clock carrier")
        return None, payload


def graph_value(body, owner, local, global_, time):
    require(len(body) >= 36 and body[:8] == b"RELGPB01" and body[8:24] == owner,
            "current Graph publication identity differs")
    version, count = unpack("<QI", body[24:36])
    require(version == local and count > 0 and len(body) == 36 + count * 70,
            "current Graph publication has an invalid local coordinate or extent")
    changes = []
    keys = set()
    for offset in range(36, len(body), 70):
        row = body[offset:offset + 70]
        source_kind, target_kind = row[0], row[33]
        kind, active, origin = unpack("<HBB", row[66:70])
        require(source_kind in (1, 2) and target_kind in (1, 2)
                and kind in (*range(1, 9), 100, 101) and active in (0, 1) and origin in (1, 2, 3),
                "invalid current Graph relation tags")
        key = (source_kind, row[1:33], target_kind, row[34:66], kind)
        require(key not in keys, "duplicate relation in current Graph publication")
        keys.add(key)
        changes.append({
            "source_kind": source_kind, "source": list(row[1:33]), "target_kind": target_kind,
            "target": list(row[34:66]), "kind": kind, "active": bool(active), "origin": origin,
            "global_version": global_, "graph_version": local,
        })
    return {"global_version": global_, "time_ns": time, "changes": changes}


def current_frames(path, owner, candidate_digest):
    identity = bytes.fromhex(owner["owner_uuid"])
    expected_header = HEADER + bytes([KINDS[owner["kind"]], SCOPES[owner["scope"]]]) + bytes(6) + identity
    digest = hashlib.sha256()
    with path.open("rb") as file:
        header = file.read(len(expected_header))
        require(header == expected_header, "candidate current Container identity differs")
        digest.update(header)
        while length := file.read(8):
            extent = int.from_bytes(length, "little")
            require(len(length) == 8 and 8 <= extent <= 128 * 1024 * 1024, "invalid current frame extent")
            payload = file.read(extent)
            require(len(payload) == extent, "truncated current frame")
            digest.update(length)
            digest.update(payload)
            yield payload
    require(digest.hexdigest() == candidate_digest, "candidate current bytes differ")


def target_graph(path, owner, candidate_digest, limit):
    identity = bytes.fromhex(owner["owner_uuid"])
    clock = Clock(limit)
    graph = {domain: [] for domain in DOMAINS}
    timelines = {domain: [] for domain in DOMAINS}
    for payload in current_frames(path, owner, candidate_digest):
        global_, body = clock.read(payload)
        if body.startswith(b"RELGPB"):
            require(global_ is not None, "unwrapped current Graph publication")
            local = len(graph["graph"]) + 1
            value = graph_value(body, identity, local, global_, clock.times[global_])
            key = local.to_bytes(8, "big")
            graph["graph"].append(commitment("graph", "publication", key, value))
            timelines["graph"].append((global_, local))
            memory = [row for row in value["changes"] if row["kind"] <= 8]
            if memory:
                graph["memory_graph"].append(commitment(
                    "memory_graph", "publication", key, {**value, "changes": memory},
                ))
                timelines["memory_graph"].append((global_, local))
        else:
            require(not body.startswith(b"RELGRF"), "unversioned current Graph row outside pinned migration")
    require(len(clock.times) == limit, "candidate current global population differs")
    memory_head = max((local for _, local in timelines["memory_graph"]), default=0)
    for domain in DOMAINS:
        graph[domain].append(commitment(domain, "clock", b"memory_graph", memory_head))
        timelines[domain].sort()
    return graph, timelines, clock.times


def verify_current_open(reader, candidate, candidate_digest, owner, global_head):
    process = subprocess.run(
        [str(reader), "--kind", owner["kind"].lower(), "--candidate", str(candidate)],
        capture_output=True, timeout=300, check=False,
    )
    require(process.returncode == 2 and not process.stderr, "unexpected current reader exit/output")
    report = strict_json_document(process.stdout, "current owner reader")
    require(report.get("same_handle_observation") and report.get("shared_read_only_lease_held_through_finalization")
            and report.get("candidate_unchanged") and report["candidate_sha256_before"] == candidate_digest
            and report["candidate_sha256_after"] == candidate_digest and report["reason"] == ""
            and all(report["owner"].get(key) == value for key, value in owner.items())
            and unsigned(report["target_clocks"]["latest_allocated_global_version"]) == global_head,
            "candidate normal current-only reopen did not confirm identity and global head")


def compare(reader, candidate, candidate_digest, coverage_path, coverage_digest, manifest_digest, source_digest):
    source, owner, artifacts = read_source(coverage_path, coverage_digest, manifest_digest, source_digest)
    hex_digest(candidate_digest, 64, "candidate SHA-256")
    require(candidate_digest != source_digest and digest_file(candidate) == candidate_digest,
            "candidate SHA-256 differs or identifies the legacy source")
    reader_digest = digest_file(reader)
    global_metadata = source["native"]["global_stream"]
    count = unsigned(global_metadata["rows"])
    verify_current_open(reader, candidate, candidate_digest, owner, count - 1)
    graph, timelines, times = target_graph(candidate, owner, candidate_digest, count)
    expected = {domain: [] for domain in DOMAINS}
    for row in retained_records(coverage_path.parent, source, artifacts):
        if row["projection_domain"] in expected:
            expected[row["projection_domain"]].append(row)
    for domain in DOMAINS:
        new = index_rows(graph[domain], domain, {"publication", "clock"})
        old = index_rows(expected[domain], domain, {"baseline_memory", "publication", "clock"})
        require(old == new, f"retained current {domain} publication, relation order, or clock differs")
    latest = dict.fromkeys(DOMAINS, 0)
    positions = dict.fromkeys(DOMAINS, 0)
    for index, row in enumerate(witness_batches(coverage_path.parent, source, artifacts, "global", global_metadata)):
        time = times[index]
        require(unsigned(row["global_version"]) == index and row["transaction_time_ns"] == (
            None if time is None else str(time)), f"current global clock differs at {index}")
        for domain in DOMAINS:
            timeline = timelines[domain]
            while positions[domain] < len(timeline) and timeline[positions[domain]][0] <= index:
                local = timeline[positions[domain]][1]
                latest[domain] = max(latest[domain], local)
                positions[domain] += 1
            require(unsigned(row["domain_versions"][domain]) == latest[domain],
                    f"current {domain} frontier differs at global {index}")
    require(digest_file(candidate) == candidate_digest and digest_file(reader) == reader_digest,
            "candidate or current reader changed during verification")
    return {
        "schema_id": SCHEMA, "status": "PASS", "cohort_parity_status": "INCOMPLETE",
        "scope": "one_owner_retained_graph_memory_graph_and_global_transaction_clocks",
        "source_file_sha256": source_digest, "candidate_sha256": candidate_digest, "reader_sha256": reader_digest,
        "source_coverage_sha256": coverage_digest, "source_manifest_sha256": manifest_digest,
        "owner": owner, "global_cuts": str(count), "local_heads": {domain: str(latest[domain]) for domain in DOMAINS},
        "matched_rows_by_domain": {domain: str(len(graph[domain])) for domain in DOMAINS},
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--reader", type=Path, required=True)
    parser.add_argument("--candidate", type=Path, required=True)
    parser.add_argument("--candidate-sha256", required=True)
    parser.add_argument("--source-coverage", type=Path, required=True)
    parser.add_argument("--source-coverage-sha256", required=True)
    parser.add_argument("--source-manifest-sha256", required=True)
    parser.add_argument("--source-file-sha256", required=True)
    args = parser.parse_args()
    try:
        result = compare(
            args.reader.resolve(strict=True), args.candidate.resolve(strict=True), args.candidate_sha256,
            args.source_coverage.resolve(strict=True), args.source_coverage_sha256,
            args.source_manifest_sha256, args.source_file_sha256,
        )
    except (OSError, ValueError, KeyError, TypeError, subprocess.SubprocessError) as error:
        result = {"schema_id": SCHEMA, "status": "FAIL", "reason": str(error)}
    sys.stdout.write(json.dumps(result, ensure_ascii=False, sort_keys=True) + "\n")
    return 0 if result["status"] == "PASS" else 2


if __name__ == "__main__":
    raise SystemExit(main())
