#!/usr/bin/env python3
"""Matched accepted-use history profiles from fresh disposable RELs."""
import argparse, hashlib, json, os, pathlib, statistics, subprocess, time, zipfile
from freshness_p2_acceptance import competing_processes, await_quiet, LOAD_OBSERVATIONS
def digest(p): return hashlib.sha256(p.read_bytes()).hexdigest()
def main():
    p=argparse.ArgumentParser()
    p.add_argument("--baseline",type=pathlib.Path,required=True)
    p.add_argument("--candidate",type=pathlib.Path)
    p.add_argument("--out",type=pathlib.Path,required=True)
    p.add_argument("--rounds",type=int,default=3)
    p.add_argument("--resume", action="store_true", help="Reuse only previously completed clean samples in this output directory")
    args=p.parse_args()
    args.out.mkdir(parents=True,exist_ok=True)
    variants={"baseline":args.baseline}
    if args.candidate: variants["candidate"]=args.candidate
    raw=args.out/'raw.json'
    data=json.loads(raw.read_text(encoding='utf-8')) if args.resume and raw.exists() else []
    for iteration in range(args.rounds):
        order=list(variants) if iteration%2==0 else list(reversed(variants))
        for depth in [0,160,1024]:
            for batch in [1,4,16,64]:
                identities=[]
                for label in order:
                    previous=next((r for r in data if (r['variant'],r['depth'],r['batch'],r['round'])==(label,depth,batch,iteration)),None)
                    if previous:
                        identities.append(previous['identity'])
                        continue
                    name=f"{label}-depth{depth}-batch{batch}-round{iteration}"
                    env={**os.environ,"RELIQUARY_FRESHNESS_TRACE":"1","RELIQUARY_FRESHNESS_HISTORY_DEPTH":str(depth),"RELIQUARY_FRESHNESS_HISTORY_BATCH":str(batch)}
                    print(name,flush=True)
                    for attempt in range(1,6):
                        await_quiet()
                        result=subprocess.run([str(variants[label])],env=env,capture_output=True,text=True,encoding="utf-8")
                        paths=[args.out/(name+".stdout.jsonl"),args.out/(name+".stderr.jsonl")]
                        paths[0].write_text(result.stdout,encoding="utf-8")
                        paths[1].write_text(result.stderr,encoding="utf-8")
                        if result.returncode: raise RuntimeError(result.stderr[-4000:])
                        busy=competing_processes()
                        LOAD_OBSERVATIONS.append({"time":time.time(),"competing":busy,"sample":name,"attempt":attempt})
                        if not busy: break
                        for path in paths: path.rename(path.with_name(path.name+f".contaminated-attempt{attempt}"))
                        print("Discard contaminated sample; retry after host quiet: "+name,flush=True)
                    else: raise RuntimeError("Repeated competing work prevented a clean sample")
                    rows=[json.loads(line) for line in result.stdout.splitlines() if line.startswith("{")]
                    traces=[json.loads(line) for line in result.stderr.splitlines() if line.startswith("{")]
                    identity=next(row for row in rows if row["kind"]=="freshness_history_identity")
                    identities.append(identity)
                    # Seed flush emits at most 64 scopes; the final two scopes are use then exact retry.
                    stages={row["stage"]:row for row in traces[-2]["samples"]}
                    retry_stages={row["stage"]:row for row in traces[-1]["samples"]}
                    operation={row["stage"]:row["elapsed_ns"] for row in rows if row["kind"]=="freshness_operation_sample"}
                    data.append({"variant":label,"depth":depth,"batch":batch,"round":iteration,"operation":operation,"stages":stages,"retry_stages":retry_stages,"identity":identity})
                    (args.out/"raw.json").write_text(json.dumps(data,indent=2),encoding="utf-8")
                assert all(identity==identities[0] for identity in identities), "Candidate history/records/event/payload identity mismatch"
    summary=[]
    for label in variants:
        for depth in [0,160,1024]:
            for batch in [1,4,16,64]:
                selected=[row for row in data if (row["variant"],row["depth"],row["batch"])==(label,depth,batch)]
                stage_names=set().union(*(row["stages"] for row in selected))
                stages={stage:statistics.median(row["stages"].get(stage,{}).get("elapsed_ns",0) for row in selected) for stage in sorted(stage_names)}
                summary.append({"variant":label,"depth":depth,"batch":batch,"stages_p50_ns":stages,"operation_p50_ns":{stage:statistics.median(row["operation"][stage] for row in selected) for stage in selected[0]["operation"]},"scan_chunks":selected[0]["stages"].get("freshness.history.scan",{}).get("chunks_visited",0),"scan_bytes":selected[0]["stages"].get("freshness.history.scan",{}).get("payload_bytes",0),"identity":selected[0]["identity"]})
    (args.out/"summary.json").write_text(json.dumps(summary,indent=2),encoding="utf-8")
    manifest={"rustc":subprocess.check_output(["rustc","--version"],text=True).strip(),"lock_sha256":digest(pathlib.Path("Cargo.lock")),"baseline_commit":subprocess.check_output(["git","rev-parse","HEAD"],text=True).strip(),"binary_sha256":{k:digest(v) for k,v in variants.items()},"source_sha256":{str(x):digest(x) for x in [pathlib.Path("src/freshness_storage.rs"),pathlib.Path("src/freshness/trace.rs"),pathlib.Path("src/freshness/freshness_storage_membership.rs"),pathlib.Path("src/freshness/freshness_storage_stream.rs"),pathlib.Path("src/freshness_storage_tests.rs"),pathlib.Path("src/facade/cva_freshness.rs"),pathlib.Path("examples/freshness_operation_baseline.rs"),pathlib.Path("scripts/freshness_history_profile.py")] if x.exists()},"load_observations":LOAD_OBSERVATIONS,"scope":"Release diagnostic, three fixed history cuts; seed construction is outside timed sample; aggregate stages are inclusive."}
    manifest["fixture"]={"published_memories":64,"active_memory_graph_nodes":0,"active_memory_edges":0,"community_snapshot":"absent"}
    manifest["metadata_note"]="Frozen baseline stdout has legacy default-fixture graph fields; history identity and this fixture declaration are authoritative. Candidate corrects sample metadata outside timed operations."
    candidate_manifest=args.candidate.with_suffix(".manifest.json") if args.candidate else None
    if candidate_manifest and candidate_manifest.exists(): manifest["candidate_build_identity"]=json.loads(candidate_manifest.read_text(encoding="utf-8"))
    baseline_manifest=args.baseline.with_suffix(".manifest.json")
    if baseline_manifest.exists(): manifest["baseline_build_identity"]=json.loads(baseline_manifest.read_text(encoding="utf-8"))
    manifest["files"]=[{"name":x.name,"sha256":digest(x),"bytes":x.stat().st_size} for x in sorted(args.out.iterdir()) if x.is_file() and x.name!="MANIFEST.json"]
    (args.out/"MANIFEST.json").write_text(json.dumps(manifest,indent=2),encoding="utf-8")
    archive=pathlib.Path("benchmarks")/("freshness-p4-final-2026-10-04.zip" if args.candidate else "freshness-p4-baseline-2026-10-04.zip")
    with zipfile.ZipFile(archive,"w",zipfile.ZIP_DEFLATED) as z:
        for entry in manifest["files"]+[{"name":"MANIFEST.json"}]: z.write(args.out/entry["name"],entry["name"])
    with zipfile.ZipFile(archive) as z:
        for entry in manifest["files"]:
            assert hashlib.sha256(z.read(entry["name"])).hexdigest()==entry["sha256"]
    print(json.dumps({"archive":str(archive),"sha256":digest(archive)}),flush=True)
if __name__=="__main__": main()
