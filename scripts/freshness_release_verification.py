#!/usr/bin/env python3
"""Final P0/final release comparison with frozen build identities and exact effects."""
import argparse,ctypes,hashlib,json,os,pathlib,statistics,subprocess,sys,time,zipfile
import freshness_p2_acceptance as guard
from freshness_performance_compare import main as unused_comparator
def digest(p):return hashlib.sha256(pathlib.Path(p).read_bytes()).hexdigest()
def save(p,value):p.write_text(json.dumps(value,indent=2),encoding="utf-8")
def percentile(values,p):
 s=sorted(values);return s[max(0,(len(s)*p+99)//100-1)]
def peak_working_set(process):
 if os.name!="nt":return None
 class Counters(ctypes.Structure):
  _fields_=[("cb",ctypes.c_ulong),("PageFaultCount",ctypes.c_ulong)]+[(n,ctypes.c_size_t) for n in ["PeakWorkingSetSize","WorkingSetSize","QuotaPeakPagedPoolUsage","QuotaPagedPoolUsage","QuotaPeakNonPagedPoolUsage","QuotaNonPagedPoolUsage","PagefileUsage","PeakPagefileUsage"]]
 counters=Counters();counters.cb=ctypes.sizeof(counters)
 fn=ctypes.windll.psapi.GetProcessMemoryInfo
 fn.argtypes=[ctypes.c_void_p,ctypes.c_void_p,ctypes.c_ulong];fn.restype=ctypes.c_int
 if fn(ctypes.c_void_p(int(process._handle)),ctypes.byref(counters),counters.cb):return counters.PeakWorkingSetSize
 return None
def run(command,out,err,env=None):
 for attempt in range(1,6):
  guard.await_quiet()
  with out.open("wb") as stdout,err.open("wb") as stderr:
   child=subprocess.Popen(command,stdout=stdout,stderr=stderr,env=env)
   exitcode=child.wait();peak=peak_working_set(child)
  if exitcode:raise RuntimeError(err.read_text(errors="replace")[-6000:])
  busy=guard.competing_processes()
  guard.LOAD_OBSERVATIONS.append({"time":time.time(),"competing":busy,"sample":out.name,"attempt":attempt})
  if not busy:return peak
  for path in (out,err):path.rename(path.with_name(path.name+f".contaminated{attempt}"))
 raise RuntimeError("Repeated competing compilation/indexing; no valid sample")
def main():
 ap=argparse.ArgumentParser(description=__doc__);ap.add_argument("--out",type=pathlib.Path,required=True)
 ap.add_argument("--rounds",type=int,default=3);ap.add_argument("--repetitions",type=int,default=11);ap.add_argument("--operation-rounds",type=int,default=9)
 ap.add_argument("--history-rounds",type=int,default=3)
 a=ap.parse_args();a.out.mkdir(parents=True,exist_ok=True)
 root=pathlib.Path(__file__).resolve().parents[1];build=a.out.parent
 manifest=json.loads((build/"BUILD-SOURCES.json").read_text())
 meta={"profile":"release","rustc":manifest["rustc"],"cargo_lock_sha256":manifest["candidate_lock_sha256"],"features":manifest["features"]}
 paths={v:build/(v+"-propagation.exe") for v in ["baseline","candidate"]}
 ops={v:build/(v+"-operation.exe") for v in paths};docs={};rss=[];operations=[]
 manifest["binary_sha256"]={v+"-"+family:digest(build/(v+"-"+family+".exe")) for v in paths for family in ["propagation","operation"]}
 for family in ["propagation","operation"]:
  if manifest["binary_sha256"]["baseline-"+family]==manifest["binary_sha256"]["candidate-"+family]:
   raise RuntimeError("Baseline and candidate binaries are identical; rebuild provenance is invalid")
 manifest["started_unix"]=time.time();manifest["order"]=[]
 for round in range(a.rounds):
  order=list(paths) if round%2==0 else list(reversed(paths))
  for label,fixture,sha in guard.FIXTURES:
   for variant in order:
    name=f"propagation-{label}-{variant}-round{round}";print(name,flush=True)
    cmd=[str(paths[variant]),"--repetitions",str(a.repetitions)]
    if fixture:cmd+=["--fixture",str(root.parent/"reliquary-fixtures"/"local"/"authoritative"/fixture/"project.prj.rel"),"--sha256",sha]
    out=a.out/(name+".json");err=a.out/(name+".stderr")
    peak=run(cmd,out,err)
    doc=json.loads(out.read_text(encoding="utf-8-sig"));doc["build_metadata"]=meta
    if fixture:assert doc["source_hash_unchanged"]
    docs.setdefault((variant,label),[]).append(doc);rss.append({"sample":name,"peak_process_working_set_bytes":peak})
    manifest["order"].append(name)
 comparisons=[]
 for label,_,_ in guard.FIXTURES:
  files={}
  for variant in paths:
   files[variant]=a.out/(f"aggregate-{label}-{variant}.json")
   save(files[variant],guard.aggregate(docs[(variant,label)]))
  compared=json.loads(subprocess.check_output([sys.executable,str(root/"scripts/freshness_performance_compare.py"),str(files["baseline"]),str(files["candidate"])],text=True))
  assert compared["build_qualification"]["optimized_build_identity_verified"]
  save(a.out/(f"comparison-{label}.json"),compared)
  comparisons += [dict(row,fixture=label) for row in compared["comparisons"]]
 for round in range(a.operation_rounds):
  order=list(ops) if round%2==0 else list(reversed(ops));identities=[]
  for variant in order:
   name=f"operation-{variant}-round{round}";print(name,flush=True)
   out=a.out/(name+".stdout.jsonl");err=a.out/(name+".stderr.jsonl")
   env={**os.environ,"RELIQUARY_FRESHNESS_TRACE":"1"}
   env.pop("RELIQUARY_FRESHNESS_HISTORY_DEPTH",None);env.pop("RELIQUARY_FRESHNESS_HISTORY_BATCH",None)
   peak=run([str(ops[variant])],out,err,env)
   rows=[json.loads(line) for line in out.read_text(encoding="utf-8-sig").splitlines() if line.startswith("{")]
   identity=next(r for r in rows if r["kind"]=="freshness_release_identity");identities.append(identity)
   sample=[r for r in rows if r["kind"]=="freshness_operation_sample"]
   operations.append({"variant":variant,"round":round,"samples":sample,"identity":identity})
   rss.append({"sample":name,"peak_process_working_set_bytes":peak});manifest["order"].append(name)
  assert identities[0]==identities[1],"Current/historical records differ across variants"
 save(a.out/"operation-raw.json",operations)
 operation_summary=[]
 stages=[r["stage"] for r in operations[0]["samples"]]
 for stage in stages:
  row={"stage":stage}
  for variant in ops:
   chosen=[s for s in operations if s["variant"]==variant]
   values=[next(r["elapsed_ns"] for r in s["samples"] if r["stage"]==stage) for s in chosen]
   row[variant]={"raw_ns":values,"p50_ns":statistics.median(values),"p95_ns":percentile(values,95),"p99_ns":percentile(values,99)}
  row["candidate_over_baseline_p50"]=row["candidate"]["p50_ns"]/row["baseline"]["p50_ns"]
  row["candidate_over_baseline_p95"]=row["candidate"]["p95_ns"]/row["baseline"]["p95_ns"]
  operation_summary.append(row)
 save(a.out/"operation-summary.json",operation_summary)
 summary={"comparisons":comparisons,"propagation_regressions_over_10_percent_p95":[r for r in comparisons if r["candidate_p95_ns"]>r["baseline_p95_ns"]*1.1],
 "operation_regressions_over_10_percent_p95":[r for r in operation_summary if r["candidate_over_baseline_p95"]>1.1],"operation_rounds":a.operation_rounds,"propagation_samples_per_shape":a.rounds*a.repetitions}
 history=[]
 for iteration in range(a.history_rounds):
  order=list(ops) if iteration%2==0 else list(reversed(ops))
  for depth in [160,1024]:
   for batch in [1,64]:
    identities=[]
    for variant in order:
     name=f"history-{variant}-depth{depth}-batch{batch}-round{iteration}";print(name,flush=True)
     out=a.out/(name+".stdout.jsonl");err=a.out/(name+".stderr.jsonl")
     env={**os.environ,"RELIQUARY_FRESHNESS_TRACE":"1","RELIQUARY_FRESHNESS_HISTORY_DEPTH":str(depth),"RELIQUARY_FRESHNESS_HISTORY_BATCH":str(batch)}
     peak=run([str(ops[variant])],out,err,env)
     rows=[json.loads(line) for line in out.read_text(encoding="utf-8-sig").splitlines() if line.startswith("{")]
     identity=next(r for r in rows if r["kind"]=="freshness_history_identity");identities.append(identity)
     sample=[r for r in rows if r["kind"]=="freshness_operation_sample"]
     history.append({"variant":variant,"depth":depth,"batch":batch,"round":iteration,"samples":sample,"identity":identity})
     rss.append({"sample":name,"peak_process_working_set_bytes":peak});manifest["order"].append(name)
     save(a.out/"history-raw.json",history)
    assert identities[0]==identities[1],"History payload/event/current/historical identity differs across variants"
 history_summary=[]
 for depth in [160,1024]:
  for batch in [1,64]:
   for stage in ["history_accepted_use","history_exact_retry"]:
    row={"depth":depth,"batch":batch,"stage":stage}
    for variant in ops:
     chosen=[s for s in history if (s["variant"],s["depth"],s["batch"])==(variant,depth,batch)]
     values=[next(r["elapsed_ns"] for r in s["samples"] if r["stage"]==stage) for s in chosen]
     row[variant]={"raw_ns":values,"p50_ns":statistics.median(values),"p95_ns":percentile(values,95),"p99_ns":percentile(values,99)}
    row["candidate_over_baseline_p95"]=row["candidate"]["p95_ns"]/row["baseline"]["p95_ns"]
    history_summary.append(row)
 save(a.out/"history-summary.json",history_summary)
 save(a.out/"summary.json",summary);save(a.out/"peak-working-set.json",rss)
 summary["history_regressions_over_10_percent_p95"]=[r for r in history_summary if r["candidate_over_baseline_p95"]>1.1]
 save(a.out/"summary.json",summary)
 manifest["load_observations"]=guard.LOAD_OBSERVATIONS;manifest["finished_unix"]=time.time()
 manifest["scope"]="Interleaved topology and complete disposable local operation. p95/p99 are sample quantiles, not production tails. Process peak working set is not allocator peak heap."
 save(a.out/"MEASUREMENT-MANIFEST.json",manifest)
 print(json.dumps({"comparison_count":len(comparisons),"propagation_flags":len(summary["propagation_regressions_over_10_percent_p95"]),"operation_flags":len(summary["operation_regressions_over_10_percent_p95"])}),flush=True)
if __name__=="__main__":main()
