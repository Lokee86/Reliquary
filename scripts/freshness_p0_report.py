#!/usr/bin/env python3
"""Summarize immutable P0 JSON evidence; keep nested timings separate."""
import json
import statistics
from collections import defaultdict
from pathlib import Path
import sys

def percentile(values, fraction):
    v = sorted(values)
    if not v:
        return None
    import math
    return v[max(0, math.ceil(len(v) * fraction) - 1)]

def summarize_operation(out_path, err_path):
    operations = []
    stages = defaultdict(list)
    counts = defaultdict(list)
    for line in Path(out_path).read_text(encoding="utf-8-sig").splitlines():
        if not line.strip():
            continue
        row = json.loads(line)
        if row.get("kind") == "freshness_operation_sample":
            operations.append(row)
    for line in Path(err_path).read_text(encoding="utf-8-sig").splitlines():
        if not line.strip():
            continue
        row = json.loads(line)
        if row.get("kind") != "freshness_trace":
            continue
        for sample in row["samples"]:
            key = row["operation"] + " / " + sample["stage"]
            stages[key].append(sample["elapsed_ns"] / 1e6)
            for name in ("chunks_visited", "graph_mutations_decoded", "payload_bytes"):
                if sample.get(name) is not None:
                    counts[(key, name)].append(sample[name])
    return {
        "operations": operations,
        "stages": {k: {"samples": len(v), "median_ms": statistics.median(v), "p95_ms": percentile(v, .95)} for k,v in sorted(stages.items())},
        "counts": {k + " / " + name: {"samples":len(v),"median":statistics.median(v)} for (k,name),v in sorted(counts.items())}
    }
def summarize_prop(path):
    d=json.loads(Path(path).read_text(encoding="utf-8-sig"))
    return {"fixture":d["fixture"],"graph":d["graph"],
        "scenarios":[{"id":x["scenario_id"],"workers":x["requested_workers"],
            "p50_ms": x["operation_latency_ns"]["p50"]/1e6,
            "p95_ms": x["operation_latency_ns"]["p95"]/1e6,
            "events":x["event_count"],
            "event_first":x["correctness_reference_events"][0] if x["correctness_reference_events"] else {}
          } for x in d["results"]],
        "original_source_unchanged": d.get("source_hash_unchanged"),
        "fixture_sha256":d.get("fixture_sha256")}
def main():
    mode=sys.argv[1]
    if mode=="operation":
        v=summarize_operation(sys.argv[2],sys.argv[3])
    elif mode=="propagation":
        v=summarize_prop(sys.argv[2])
    else:
        raise SystemExit("operation OUT STDERR | propagation JSON")
    print(json.dumps(v,indent=2))
if __name__=="__main__":
    main()
