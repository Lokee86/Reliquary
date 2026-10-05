#!/usr/bin/env python3
"""Collect interleaved P1/P2 release evidence; compare exact event maps before timing claims."""
import argparse, csv, hashlib, io, json, os, pathlib, statistics, subprocess, sys, time, zipfile

FIXTURES = [
 ("synthetic", None, None),
 ("first14d", "chatgpt-first14d", "49751a62dd8644800d19976a0696a69366dc1e4f8d7c39bb4b69fbd7a8a5d18b"),
 ("first28d", "chatgpt-first28d", "8f4a2bf871d7f5368edde93954a2260a29c86131b28475b323a7b6c70f8c51e4"),
 ("ellis", "rhelm-david-r-ellis-pre-relationship", "3efd9c666aa992686abd8aac9b77cfc7dc2a0e15d6fb26ec01f8666db175e476"),
]
def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()
def save(path, data):
    path.write_text(json.dumps(data, indent=2), encoding="utf-8")
QUIET_REQUIRED = False
LOAD_OBSERVATIONS = []
def competing_processes():
    if os.name != "nt":
        raise RuntimeError("Quiet-host process check currently supports Windows only")
    raw = subprocess.check_output(["tasklist", "/FO", "CSV", "/NH"], text=True, errors="replace")
    names = {"cargo.exe", "rustc.exe", "lexicon.exe", "arcana.exe", "link.exe"}
    return [{"name":row[0], "pid":row[1]} for row in csv.reader(io.StringIO(raw)) if len(row)>1 and row[0].lower() in names]
def await_quiet():
    deadline = time.monotonic() + 900
    while True:
        busy = competing_processes()
        LOAD_OBSERVATIONS.append({"time":time.time(), "competing":busy})
        if not busy:
            return
        if time.monotonic() > deadline:
            raise RuntimeError("Host remained busy; no quiet benchmark claim is valid")
        print("Waiting for competing compiler/indexer: " + json.dumps(busy), flush=True)
        time.sleep(10)
def run(command, out, err, env=None):
    for attempt in range(1, 6):
        if QUIET_REQUIRED:
            await_quiet()
        with out.open("wb") as stdout, err.open("wb") as stderr:
            result = subprocess.run(command, stdout=stdout, stderr=stderr, env=env, check=False)
        if result.returncode:
            raise RuntimeError(f"{command[0]} exited {result.returncode}: {err.read_text(errors='replace')[-4000:]}")
        if QUIET_REQUIRED:
            busy = competing_processes()
            LOAD_OBSERVATIONS.append({"time":time.time(), "competing":busy, "sample":out.name, "attempt":attempt})
            if busy:
                for path in (out, err):
                    path.rename(path.with_name(path.name + f".contaminated-attempt{attempt}"))
                print("Discard contaminated sample; retry after host quiet: " + out.name, flush=True)
                continue
        return
    raise RuntimeError("Repeated competing work prevented a quiet sample")
def aggregate(docs):
    first = docs[0]
    rows = {(r["scenario_id"],r["requested_workers"]):r for r in first["results"]}
    for doc in docs[1:]:
        other = {(r["scenario_id"],r["requested_workers"]):r for r in doc["results"]}
        assert rows.keys() == other.keys(), "matrix changed across rounds"
        assert doc["graph"] == first["graph"], "projection changed across rounds"
        for key, row in rows.items():
            incoming = other[key]
            assert row["correctness_reference_events"] == incoming["correctness_reference_events"], "effects changed across rounds"
            row["samples"].extend(incoming["samples"])
            row["operation_latency_ns"]["raw"].extend(incoming["operation_latency_ns"]["raw"])
    for row in rows.values():
        raw = row["operation_latency_ns"]["raw"]
        row["repetitions"] = len(raw)
        ordered = sorted(raw)
        for p in [50,95,99]:
            row["operation_latency_ns"][f"p{p}"] = ordered[max(0,(len(raw)*p+99)//100-1)]
        row["events_per_second_at_p50"] = row["event_count"]*1e9/row["operation_latency_ns"]["p50"]
    return first
def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument("--baseline",type=pathlib.Path,required=True)
    p.add_argument("--candidate",type=pathlib.Path,required=True)
    p.add_argument("--baseline-operation",type=pathlib.Path,required=True)
    p.add_argument("--candidate-operation",type=pathlib.Path,required=True)
    p.add_argument("--out",type=pathlib.Path,required=True)
    p.add_argument("--archive",type=pathlib.Path,help="Separate evidence archive; required for A/A controls")
    p.add_argument("--control",choices=["p1-aa","p2-aa"],help="Verify identical binaries on both sides")
    p.add_argument("--require-quiet",action="store_true",help="Wait for compiler/indexer processes before samples; fail if observed afterwards")
    p.add_argument("--load-conditions",default="No concurrent measurement jobs intentionally scheduled; host isolation not established.")
    p.add_argument("--rounds",type=int,default=5)
    p.add_argument("--repetitions",type=int,default=11)
    p.add_argument("--fixture-root",type=pathlib.Path,default=pathlib.Path(r"C:\!bin\workspace\reliquary-fixtures\local\authoritative"))
    args=p.parse_args()
    global QUIET_REQUIRED
    QUIET_REQUIRED = args.require_quiet
    if args.control:
        if not args.archive:
            p.error("A/A controls require --archive to preserve existing acceptance evidence")
        if digest(args.baseline) != digest(args.candidate) or digest(args.baseline_operation) != digest(args.candidate_operation):
            p.error("A/A controls require identical propagation and operation binaries")
    if not 3 <= args.rounds <= 15 or not 5 <= args.repetitions <= 100:
        p.error("rounds must be 3..15 and repetitions 5..100")
    root=pathlib.Path(__file__).resolve().parents[1]
    args.out.mkdir(parents=True,exist_ok=True)
    paths={"baseline":args.baseline,"candidate":args.candidate}
    operation_paths={"baseline":args.baseline_operation,"candidate":args.candidate_operation}
    metadata={"profile":"release","rustc":subprocess.check_output(["rustc","--version"],text=True).strip(),
              "cargo_lock_sha256":digest(root/"Cargo.lock"),"features":["freshness-performance-trace"]}
    manifest={"scope":"Interleaved local fixture/topology benchmark, not production telemetry",
              "build_metadata":metadata,"rounds":args.rounds,"inner_repetitions":args.repetitions,
              "binary_sha256":{k:digest(v) for k,v in {**paths,**{k+"_operation":v for k,v in operation_paths.items()}}.items()},
              "source_sha256":{str(x.relative_to(root)):digest(x) for x in [root/"src/freshness/propagation.rs",root/"src/runtime_inference/freshness_access.rs"]},
              "baseline_source_sha256": ({str(x.relative_to(root)):digest(x) for x in [root/"src/freshness/propagation.rs",root/"src/freshness/policy.rs"]} if args.control == "p2-aa" else {name:digest(args.baseline.parent/name) for name in ["p1-propagation.rs","p1-policy.rs"]}),
              "baseline_commit":subprocess.check_output(["git","rev-parse","HEAD"],cwd=root,text=True).strip(),
              "candidate_source_state":"uncommitted P2 checkpoint" if args.control != "p1-aa" else "accepted P1 frozen source; current worktree source is provenance only",
              "logical_cpu_count":os.cpu_count(),
              "control":args.control,
              "load_conditions":args.load_conditions,
              "started_at_unix":time.time(),"order":[]}
    documents={}
    operation_outputs={k:[] for k in paths}
    for n in range(args.rounds):
        order=["baseline","candidate"] if n%2==0 else ["candidate","baseline"]
        for label,fixture,sha in FIXTURES:
            for variant in order:
                name=f"{variant}-{label}-round{n+1}"
                command=[str(paths[variant]),"--repetitions",str(args.repetitions)]
                if fixture:
                    command += ["--fixture",str(args.fixture_root/fixture/"project.prj.rel"),"--sha256",sha]
                out=args.out/(name+".json"); err=args.out/(name+".stderr")
                print(name,flush=True)
                run(command,out,err)
                doc=json.loads(out.read_text(encoding="utf-8-sig"));doc["build_metadata"]=metadata
                documents.setdefault((variant,label),[]).append(doc)
                manifest["order"].append(name)
        for variant in order:
            name=f"{variant}-operation-round{n+1}"
            out=args.out/(name+".stdout.jsonl");err=args.out/(name+".stderr.jsonl")
            print(name,flush=True)
            run([str(operation_paths[variant])],out,err,{**os.environ,"RELIQUARY_FRESHNESS_TRACE":"1"})
            rows=[json.loads(line) for line in out.read_text(encoding="utf-8-sig").splitlines() if line.strip()]
            operation_outputs[variant].append({r["stage"]:r["elapsed_ns"] for r in rows if r.get("kind")=="freshness_operation_sample"})
            manifest["order"].append(name)
    summaries=[]
    for label,_,_ in FIXTURES:
        files={}
        for variant in paths:
            files[variant]=args.out/(variant+"-"+label+"-aggregate.json")
            save(files[variant],aggregate(documents[(variant,label)]))
        compared=json.loads(subprocess.check_output([sys.executable,str(root/"scripts/freshness_performance_compare.py"),str(files["baseline"]),str(files["candidate"])],text=True))
        save(args.out/("comparison-"+label+".json"),compared)
        summaries.extend(compared["comparisons"])
    operation_summary={}
    for variant,runs in operation_outputs.items():
        assert all(run.keys()==runs[0].keys() for run in runs), "operation matrix changed"
        operation_summary[variant]={}
        for stage in runs[0]:
            values=[r[stage] for r in runs];ordered=sorted(values)
            operation_summary[variant][stage]={"raw_ns":values,"p50_ns":statistics.median(values),"p95_ns":ordered[(len(values)*95+99)//100-1]}
    save(args.out/"operation-comparison.json",operation_summary)
    save(args.out/"summary.json",{"comparisons":len(summaries),"p95_regressions_over_10_percent":[r for r in summaries if r["candidate_p95_ns"]>r["baseline_p95_ns"]*1.1],
        "minimum_median_speedup":min(r["speedup_baseline_over_candidate"] for r in summaries),"operation_scope":"Each receipt shape times three distinct receipts, not per-receipt latency; fresh disposable REL per process."})
    for name, expected in manifest["source_sha256"].items():
        if digest(root/name) != expected:
            raise RuntimeError("Production source changed during measurement: " + name)
    manifest["finished_at_unix"]=time.time()
    manifest["load_observations"]=LOAD_OBSERVATIONS
    manifest["quiet_check_scope"]="Windows compiler/linker/indexer process snapshots before and after each measurement; unrelated OS load and transient between-snapshot activity are not excluded" if args.require_quiet else "not checked"
    manifest["files"]=[{"name":x.name,"bytes":x.stat().st_size,"sha256":digest(x)} for x in sorted(args.out.iterdir()) if x.is_file() and x.name!="MANIFEST.json"]
    save(args.out/"MANIFEST.json",manifest)
    archive=args.archive or root/"benchmarks/freshness-p2-release-2026-10-03.zip"
    if args.control and archive.exists():
        raise RuntimeError("Refusing to overwrite an existing control archive")
    with zipfile.ZipFile(archive,"w",zipfile.ZIP_DEFLATED) as z:
        for item in manifest["files"]+[{"name":"MANIFEST.json"}]:
            z.write(args.out/item["name"],item["name"])
    with zipfile.ZipFile(archive) as z:
        for item in manifest["files"]:
            data=z.read(item["name"]);assert len(data)==item["bytes"] and hashlib.sha256(data).hexdigest()==item["sha256"]
    print(json.dumps({"archive":str(archive),"sha256":digest(archive)},indent=2),flush=True)
if __name__=="__main__":
    main()
