#!/usr/bin/env python3
"""Aggregate complete P0 full-operation *fixture* runs without confusing nested scopes."""
import collections, hashlib, json, math, pathlib, statistics
ROOT = pathlib.Path(r"C:\Users\archa\AppData\Local\Temp\reliquary-freshness-perf-baseline\freshness-evidence")
def percentile(a,p):
    s=sorted(a)
    return s[max(0,math.ceil(len(s)*p/100)-1)]
o=collections.defaultdict(list); t=collections.defaultdict(list); files=[]
for n in range(1,6):
    suffix="" if n==1 else f"-run{n}"
    out=ROOT/f"p0-operation{suffix}.stdout.jsonl"
    err=ROOT/f"p0-operation{suffix}.stderr.jsonl"
    assert out.is_file() and err.is_file(),f"missing run {n}"
    files.extend([{"name":x.name,"sha256":hashlib.sha256(x.read_bytes()).hexdigest()} for x in [out,err]])
    for line in out.read_text(encoding="utf-8-sig").splitlines():
        if not line.strip():continue
        row=json.loads(line)
        if row.get("kind")=="freshness_operation_sample":
            o[row["stage"]].append(row["elapsed_ns"]/1e6)
    for line in err.read_text(encoding="utf-8-sig").splitlines():
        if not line.strip():continue
        row=json.loads(line)
        if row.get("kind")!="freshness_trace":continue
        for stage in row["samples"]:
            t[row["operation"]+"/"+stage["stage"]].append(stage["elapsed_ns"]/1e6)
assert len(o)>15 and min(map(len,o.values()))==5, {k:len(v) for k,v in o.items()}
def summarize(x):return {k:{"n":len(v),"p50_ms":round(statistics.median(v),4),"p95_ms":round(percentile(v,95),4),"p99_ms":round(percentile(v,99),4)} for k,v in sorted(x.items())}
result={"run_count":5,"note":"Five newly created RELs; p95/p99 equal maximum and are directional only","outer_operations":summarize(o),"stage_timings":summarize(t),"raw_file_hashes":files}
(ROOT/"p0-operation-aggregate.json").write_text(json.dumps(result,indent=2),encoding="utf-8")
print(json.dumps({"run_count":5,"selected_operations":{k:v for k,v in result["outer_operations"].items() if "root_prepare" in k or "16_memories_workers_" in k or "64_memories_workers_" in k},"selected_stage_medians_ms":sorted(((k,v["p50_ms"]) for k,v in result["stage_timings"].items()),key=lambda x:x[1],reverse=True)[:14]},indent=2))
