#!/usr/bin/env python3
"""Compare two baseline JSON outputs only after matching workload and correctness evidence."""
import argparse
import json
from pathlib import Path
import sys


def load(path):
    with Path(path).open(encoding="utf-8") as handle:
        return json.load(handle)


def fail(message):
    raise SystemExit(f"incomparable measurements: {message}")


def result_map(doc, label):
    rows = doc.get("results", [])
    if not isinstance(rows, list) or not rows:
        fail(f"{label} has no measured results")
    result = {}
    for row in rows:
        key = (row.get("scenario_id"), row.get("requested_workers"))
        if not isinstance(key[0], str) or not isinstance(key[1], int) or key in result:
            fail(f"{label} has invalid or duplicate scenario/requested-worker key {key!r}")
        result[key] = row
    return result


def positive_int(value):
    return isinstance(value, int) and not isinstance(value, bool) and value > 0


def check_result(row, label, key):
    repetitions = row.get("repetitions")
    event_count = row.get("event_count")
    if not positive_int(repetitions) or not positive_int(event_count):
        fail(f"{label} {key}: repetitions/event_count must be positive integers")
    raw = row.get("operation_latency_ns", {}).get("raw")
    if not isinstance(raw, list) or len(raw) != repetitions or not all(positive_int(x) for x in raw):
        fail(f"{label} {key}: raw operation latencies are missing, invalid, or wrong length")
    percentiles = row.get("operation_latency_ns", {})
    if not all(positive_int(percentiles.get(name)) for name in ("p50", "p95", "p99")):
        fail(f"{label} {key}: p50/p95/p99 latencies must be positive")
    samples = row.get("samples")
    if not isinstance(samples, list) or len(samples) != repetitions:
        fail(f"{label} {key}: sample count differs from repetitions")
    ref = row.get("correctness_reference_events")
    if not isinstance(ref, list) or len(ref) != event_count:
        fail(f"{label} {key}: correctness reference event count is invalid")
    expected = {event.get("event_identity"): event.get("candidate_map_sha256") for event in ref}
    if len(expected) != len(ref) or any(not isinstance(i, str) or not isinstance(d, str) for i, d in expected.items()):
        fail(f"{label} {key}: event identities/digests are missing or duplicated")

    for sample_index, sample in enumerate(samples):
        if sample.get("operation_ns") != raw[sample_index]:
            fail(f"{label} {key}: raw latency and sample latency disagree")
        events = sample.get("events")
        if not isinstance(events, list) or len(events) != event_count:
            fail(f"{label} {key}: sample {sample_index} event count is invalid")
        observed = {}
        for event in events:
            identity, digest = event.get("event_identity"), event.get("candidate_map_sha256")
            if identity in observed or identity not in expected or not isinstance(digest, str):
                fail(f"{label} {key}: sample {sample_index} has invalid event identity/digest")
            observed[identity] = digest
        if observed != expected:
            fail(f"{label} {key}: candidate digest changed across repetitions")
        if not positive_int(sample.get("operation_ns")):
            fail(f"{label} {key}: sample {sample_index} operation latency must be positive")


def build_qualification(before, after):
    fields = ("profile", "rustc", "cargo_lock_sha256", "features")
    b = before.get("build_metadata")
    c = after.get("build_metadata")
    missing = []
    matched = {}
    for field in fields:
        bv = b.get(field) if isinstance(b, dict) else None
        cv = c.get(field) if isinstance(c, dict) else None
        if bv is None or cv is None:
            matched[field] = "unavailable"
            missing.append(field)
        else:
            if bv != cv:
                fail(f"build metadata {field} differs")
            matched[field] = True
    optimized = matched["profile"] is True and str(b["profile"]).lower() in ("release", "bench", "optimized")
    return {
        "matched_fields": matched,
        "optimized_build_identity_verified": optimized and not missing,
        "qualification": "matched optimized build identity" if optimized and not missing else "unavailable; do not claim matched optimized qualification",
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("baseline", help="baseline JSON")
    parser.add_argument("candidate", help="candidate JSON")
    args = parser.parse_args()
    before, after = load(args.baseline), load(args.candidate)

    for key in ("fixture", "fixture_sha256"):
        if before.get(key) != after.get(key):
            fail(f"{key} differs")
    for key in ("node_count", "undirected_active_edges", "community_membership_count", "snapshot_validity",
                "projection_sha256", "graph_version", "community_generation", "community_graph_version"):
        if before.get("graph", {}).get(key) != after.get("graph", {}).get(key):
            fail(f"graph.{key} differs")
    if before.get("unavailable_scenarios") != after.get("unavailable_scenarios"):
        fail("unavailable scenario sets differ")

    bmap, cmap = result_map(before, "baseline"), result_map(after, "candidate")
    if bmap.keys() != cmap.keys():
        fail("scenario/requested-worker matrix differs")
    comparisons = []
    for key in sorted(bmap):
        b, c = bmap[key], cmap[key]
        check_result(b, "baseline", key)
        check_result(c, "candidate", key)
        for field in ("family", "topology", "event_count", "repetitions"):
            if b.get(field) != c.get(field):
                fail(f"{key}: {field} differs")
        if b.get("correctness_reference_events") != c.get("correctness_reference_events"):
            fail(f"{key}: event roots, exclusions, groups, counts, or candidate digests differ")

        bp, cp = b["operation_latency_ns"], c["operation_latency_ns"]
        b50, c50 = bp["p50"], cp["p50"]
        comparisons.append({
            "scenario_id": key[0], "requested_workers": key[1],
            "baseline_effective_workers": b.get("effective_workers"),
            "candidate_effective_workers": c.get("effective_workers"),
            "baseline_p50_ns": b50, "candidate_p50_ns": c50,
            "speedup_baseline_over_candidate": b50 / c50,
            "baseline_p95_ns": bp.get("p95"), "candidate_p95_ns": cp.get("p95"),
            "candidate_digest_equal": True,
        })
    json.dump({
        "baseline": args.baseline,
        "candidate": args.candidate,
        "build_qualification": build_qualification(before, after),
        "comparisons": comparisons,
    }, sys.stdout, indent=2)
    print()


if __name__ == "__main__":
    main()
