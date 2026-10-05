#!/usr/bin/env python3
"""Summarize identical-binary A/A noise without treating it as scheduler behavior."""
import argparse
import json
import math
import pathlib
import statistics


def load(path):
    return json.loads(path.read_text(encoding="utf-8-sig"))


def symmetric_delta(a, b):
    return max(a / b, b / a) - 1


def quantile(values, p):
    ordered = sorted(values)
    return ordered[max(0, math.ceil(len(ordered) * p) - 1)]


def summarize(root):
    manifest = load(root / "MANIFEST.json")
    hashes = manifest["binary_sha256"]
    assert manifest["control"] in ("p1-aa", "p2-aa")
    assert hashes["baseline"] == hashes["candidate"]
    assert hashes["baseline_operation"] == hashes["candidate_operation"]
    rows = []
    for fixture in ("synthetic", "first14d", "first28d", "ellis"):
        base = load(root / f"baseline-{fixture}-aggregate.json")
        before = {(r["scenario_id"], r["requested_workers"]): r for r in base["results"]}
        compared = load(root / f"comparison-{fixture}.json")["comparisons"]
        per_round = []
        for n in range(1, manifest["rounds"] + 1):
            b = load(root / f"baseline-{fixture}-round{n}.json")
            c = load(root / f"candidate-{fixture}-round{n}.json")
            bm = {(r["scenario_id"], r["requested_workers"]): r for r in b["results"]}
            cm = {(r["scenario_id"], r["requested_workers"]): r for r in c["results"]}
            assert bm.keys() == cm.keys()
            per_round.append((bm, cm))
        for row in compared:
            key = row["scenario_id"], row["requested_workers"]
            original = before[key]
            assert row["candidate_digest_equal"]
            repeated = []
            for bm, cm in per_round:
                bp = bm[key]["operation_latency_ns"]
                cp = cm[key]["operation_latency_ns"]
                repeated.append({"p50_ratio": cp["p50"] / bp["p50"],
                                 "p95_ratio": cp["p95"] / bp["p95"]})
            rows.append({**row, "fixture": fixture,
                         "family": original["family"],
                         "event_count": original["event_count"],
                         "single_event": original["event_count"] == 1,
                         "symmetric_p50_delta": symmetric_delta(row["baseline_p50_ns"], row["candidate_p50_ns"]),
                         "symmetric_p95_delta": symmetric_delta(row["baseline_p95_ns"], row["candidate_p95_ns"]),
                         "rounds_p95_candidate_over_10pct": sum(r["p95_ratio"] > 1.1 for r in repeated),
                         "round_ratios": repeated})
    groups = {}
    for label, selected in [
        ("all", rows),
        ("single_event", [r for r in rows if r["single_event"]]),
        ("dream", [r for r in rows if r["family"].lower().startswith("dream") or "dream" in r["scenario_id"].lower()]),
        ("batches", [r for r in rows if not r["single_event"]]),
    ]:
        if not selected:
            continue
        groups[label] = {
            "rows": len(selected),
            "median_symmetric_p50_delta": statistics.median(r["symmetric_p50_delta"] for r in selected),
            "p95_across_rows_symmetric_p50_delta": quantile([r["symmetric_p50_delta"] for r in selected], .95),
            "maximum_symmetric_p50_delta": max(r["symmetric_p50_delta"] for r in selected),
            "median_symmetric_p95_delta": statistics.median(r["symmetric_p95_delta"] for r in selected),
            "p95_across_rows_symmetric_p95_delta": quantile([r["symmetric_p95_delta"] for r in selected], .95),
            "maximum_symmetric_p95_delta": max(r["symmetric_p95_delta"] for r in selected),
            "candidate_p95_over_10pct": sum(r["candidate_p95_ns"] > r["baseline_p95_ns"] * 1.1 for r in selected),
            "candidate_p95_over_10pct_and_p50_over_5pct": sum(r["candidate_p95_ns"] > r["baseline_p95_ns"] * 1.1 and r["candidate_p50_ns"] > r["baseline_p50_ns"] * 1.05 for r in selected),
            "rows_with_repeated_round_p95_over_10pct": sum(r["rounds_p95_candidate_over_10pct"] >= 2 for r in selected),
        }
    return {"control": manifest["control"], "rounds": manifest["rounds"],
            "repetitions": manifest["inner_repetitions"], "groups": groups,
            "scope": "Observed sample envelope, not a statistical confidence bound or production percentile. Symmetric deltas include both faster and slower labels.",
            "rows": rows}


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("root", type=pathlib.Path)
    p.add_argument("--out", type=pathlib.Path, required=True)
    args = p.parse_args()
    result = summarize(args.root)
    args.out.write_text(json.dumps(result, indent=2), encoding="utf-8")
    print(json.dumps({k: v for k, v in result.items() if k != "rows"}, indent=2))


if __name__ == "__main__":
    main()
