#!/usr/bin/env python3
"""P6-H reproducible production-runtime runner; never a P6-R release verdict."""
from __future__ import annotations
import argparse, datetime, hashlib, json, os, platform, re, statistics, subprocess, sys, time
from pathlib import Path
try:
    import psutil
except ImportError:
    psutil = None

ROOT = Path(__file__).resolve().parents[1]
PREFIX = "runtime_host::gateway::tests::p6_harness"
CRASHES = ["admission", "finalize-intent", "archive-before-receipt", "checkpoint",
           "checkpoint-source-ahead", "host-after-acceptance", "host-cancel-before-replay"]
MANIFEST = {
    "schema": 1, "common_base": "999fbf7d85c9f892a1bf979293588033bcc1484b",
    "accepted_p34": "e1c8f88", "profile": "test/debug", "build_jobs": 1,
    "harness_threads": 1, "existing_gateway_threads": 4,
    "workloads": [
        {"id":"H1-H5", "selector":PREFIX, "repeated":True},
        {"id":"existing-real-gateway-matrix", "selector":"runtime_host::gateway", "repeated":False},
    ],
    "crash_boundaries":CRASHES,
    "gaps": [
        "P6-R requires P5-D deletion and upgraded Warlock; neither is invoked by this harness.",
        "Native endpoint abort and real Warlock adapter conformance are not proved by controlled hosts.",
        "Owner probe holds are measured; all production coordinator/owner hold intervals are not instrumented.",
        "Exact REL/PHY live worker counts and notification epoch deltas are observed; deltas count requested notifications, not executed or timeout wakes.",
        "Sampled RSS is not an allocator-exact peak; OS file cache is not flushed.",
    ],
}

def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def diagnostic_observer_sha(path):
    source = path.read_text(encoding="utf-8")
    offset = source.rfind("#[cfg(test)]\nimpl ")
    if offset < 0:
        raise ValueError("missing read-only diagnostic observer: " + str(path))
    return hashlib.sha256(source[offset:].encode("utf-8")).hexdigest()

def git(repo, *args):
    p = subprocess.run(["git", *args], cwd=repo, capture_output=True, text=True, check=True)
    return p.stdout.strip()

def describe(repo):
    hardware = {"processor":platform.processor(), "machine":platform.machine()}
    if os.name == "nt":
        import winreg
        with winreg.OpenKey(winreg.HKEY_LOCAL_MACHINE, r"HARDWARE\DESCRIPTION\System\CentralProcessor\0") as key:
            hardware["processor"] = winreg.QueryValueEx(key, "ProcessorNameString")[0].strip()
    if psutil:
        hardware.update(logical_cpus=psutil.cpu_count(), physical_cpus=psutil.cpu_count(logical=False),
                        ram_bytes=psutil.virtual_memory().total)
    files = sorted((repo/"src/runtime_inference").glob("runtime_host_gateway_p6_*.rs"))
    observers = [repo/"src/runtime_inference"/name for name in (
        "runtime_host_gateway_subscriptions.rs", "runtime_host_gateway_stream_registry.rs",
        "runtime_host_gateway_events.rs", "runtime_host_phy_execution.rs",
        "runtime_host_gateway_conversation_continuation.rs")]

    return {"revision":git(repo,"rev-parse","HEAD"), "git_status":git(repo,"status","--porcelain"),
            "platform":platform.platform(), "python":sys.version.split()[0], "hardware":hardware,
            "rustc":subprocess.check_output(["rustc","--version"],text=True).strip(),
            "cargo":subprocess.check_output(["cargo","--version"],text=True).strip(),
            "build_environment":{key:os.environ.get(key) for key in ("RUSTFLAGS","CARGO_ENCODED_RUSTFLAGS","CARGO_BUILD_TARGET","CARGO_PROFILE_TEST_OPT_LEVEL","CARGO_PROFILE_TEST_DEBUG","CARGO_PROFILE_DEV_DEBUG","CARGO_INCREMENTAL","RUSTUP_TOOLCHAIN")},
            "fixture_sources":{p.name:sha(p) for p in files},
            "diagnostic_sources":{p.name:sha(p) for p in observers},
            "diagnostic_observers":{p.name:diagnostic_observer_sha(p) for p in observers},
            "runner_sha256":sha(Path(__file__)),
            "manifest":MANIFEST, "sampling_ms":20, "psutil_available":psutil is not None}

def kill(proc):
    if psutil:
        try:
            for child in psutil.Process(proc.pid).children(recursive=True): child.kill()
        except psutil.Error:
            pass
    proc.kill()
    proc.wait()

def summary(values):
    if not values: return None
    v=sorted(values)
    return {"samples":len(v),"min":v[0],"median":statistics.median(v),
            "p95":v[min(len(v)-1,int((len(v)-1)*.95))],"max":v[-1],
            "population_stdev":statistics.pstdev(v)}

def run(repo, output, name, cmd, env, timeout, expected_abort=False, require_tests=False):
    output.mkdir(parents=True,exist_ok=True)
    stdout=output/(name+".stdout.log");stderr=output/(name+".stderr.log")
    start=time.perf_counter();sampled=[];steady=[];phase=False;position=0;timed_out=False
    background = []
    if psutil:
        for p in psutil.process_iter(["name","pid"]):
            if p.info["name"] in ("cargo.exe","rustc.exe","cargo","rustc"):
                background.append(p.info)
    with stdout.open("wb") as out, stderr.open("wb") as err:
        proc=subprocess.Popen(cmd,cwd=repo,env=env,stdout=out,stderr=err)
        while proc.poll() is None:
            if time.perf_counter()-start>timeout:
                timed_out=True;kill(proc);break
            try:
                with stdout.open("rb") as log:
                    log.seek(position);chunk=log.read();position=log.tell()
                if b'"workload":"steady_begin"' in chunk or b'"workload": "steady_begin"' in chunk: phase=True
                if b'"workload":"steady_end"' in chunk or b'"workload": "steady_end"' in chunk: phase=False
                if psutil:
                    p=psutil.Process(proc.pid);processes=[p]+p.children(recursive=True)
                    rss=sum(q.memory_info().rss for q in processes if q.is_running())
                    threads=sum(q.num_threads() for q in processes if q.is_running())
                    sampled.append((rss,threads))
                    if phase:steady.append(rss)
            except (OSError, psutil.Error if psutil else OSError):
                pass
            time.sleep(.02)
        code=proc.wait()
    text=stdout.read_text(encoding="utf-8",errors="replace")
    metrics=[]
    for line in text.splitlines():
        if "P6_METRIC " in line:
            payload=line.split("P6_METRIC ",1)[1]
            try: metrics.append(json.JSONDecoder().raw_decode(payload)[0])
            except ValueError: pass
    counts=[tuple(map(int,m)) for m in re.findall(r"test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored",text)]
    tests=sum(c[0] for c in counts)
    abort_proved=(code!=0 and "P6_CRASH_READY " in text and not timed_out)
    passed=(abort_proved if expected_abort else code==0 and not timed_out)
    if require_tests and tests==0:passed=False
    result={"name":name,"command":cmd,"exit_code":code,"timed_out":timed_out,
            "expectation":"abrupt process termination after durable marker" if expected_abort else "exit zero",
            "passed":passed,"seconds":time.perf_counter()-start,"test_summaries":counts,"background_build_processes_at_start":background,
            "sampled_peak_rss_bytes":max((r for r,t in sampled),default=None),
            "sampled_peak_threads":max((t for r,t in sampled),default=None),
            "steady_rss_bytes":summary(steady),"metrics":metrics,
            "stdout":str(stdout),"stderr":str(stderr)}
    print(json.dumps({k:result[k] for k in ("name","passed","exit_code","seconds")}),flush=True)
    return result

def rollup(results):
    groups={}
    for result in results:
        if not result["name"].startswith("harness-"): continue
        for metric in result["metrics"]:
            workload=metric["workload"];values=metric["values"]
            scope=workload+(":"+str(values["instances"]) if workload=="fanout" else "")
            for key,value in values.items():
                if key.endswith("_ns") and isinstance(value,(int,float)):
                    groups.setdefault(scope+"/"+key,[]).append(value)
                elif key.endswith("_ns") and isinstance(value,list):
                    groups.setdefault(scope+"/"+key,[]).extend(value)
    return {name:summary(v) for name,v in sorted(groups.items())}

def compare(report, baseline):
    if report["environment"]["hardware"]!=baseline["environment"]["hardware"]:
        raise ValueError("comparison requires identical recorded hardware")
    if report["environment"].get("fixture_sources")!=baseline["environment"].get("fixture_sources"):
        raise ValueError("comparison requires identical fixture sources")
    if report["environment"].get("diagnostic_observers")!=baseline["environment"].get("diagnostic_observers"):
        raise ValueError("comparison requires identical diagnostic observers")
    if report["workload"]!=baseline["workload"]:
        raise ValueError("comparison requires identical seed/instances/repetitions/features")
    for key in ("platform","rustc","cargo","build_environment","runner_sha256"):
        if report["environment"].get(key)!=baseline["environment"].get(key):
            raise ValueError("comparison requires identical "+key)
    before=baseline["rollup"];after=report["rollup"];rows=[]
    for metric in sorted(before.keys() & after.keys()):
        b=before[metric]["median"];a=after[metric]["median"]
        rows.append({"metric":metric,"before_median":b,"after_median":a,"ratio":a/b if b else None})
    return {"baseline_revision":baseline["environment"]["revision"],
            "candidate_revision":report["environment"]["revision"],"rows":rows,"thresholds":"none invented", "caveat":"observational comparison; OS cache and other workloads are not isolated"}

def validate_h5(metrics, instances):
    by_workload = {}
    for metric in metrics:
        by_workload.setdefault(metric["workload"], []).append(metric["values"])
    for name in ("owner_workers", "coordinator_lock_probe", "owner_lock_probe", "checkpoints"):
        if name not in by_workload:
            raise ValueError("missing H5 measurement: " + name)
    workers = by_workload["owner_workers"][0]
    if workers["rel_workers"] != [3]*4 or workers["phy_workers"] != [3]*2:
        raise ValueError("unexpected live owner worker counts")
    if len(workers["rel_notification_epoch_deltas"]) != 4 or len(workers["phy_notification_epoch_deltas"]) != 2:
        raise ValueError("incomplete owner notification diagnostics")
    if any(n < 10 for n in workers["rel_notification_epoch_deltas"] + workers["phy_notification_epoch_deltas"]):
        raise ValueError("route notification signals are incomplete")
    for name in ("owner_lock_probe", "coordinator_lock_probe"):
        probe = by_workload[name][0]
        if len(probe["wait_ns"]) != 100 or len(probe["probe_hold_ns"]) != 100:
            raise ValueError("incomplete lock probe samples")
    fanout = by_workload.get("fanout", [])
    if sorted(v["instances"] for v in fanout) != sorted(instances):
        raise ValueError("incomplete fanout topology measurements")
    if any(len(v["queue_samples"]) != 24 for v in fanout):
        raise ValueError("incomplete recipient occupancy measurements")
    checkpoint = by_workload["checkpoints"][0]
    if checkpoint["queue_before_drain"]["streams"]["retained_text_bytes"] != checkpoint["utf8_bytes"]:
        raise ValueError("retained stream byte diagnostic disagrees with workload")
    if checkpoint["queue_after_drain"]["streams"]["retained_text_bytes"] != 0:
        raise ValueError("drained stream retained bytes")
    return True

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo",type=Path,default=ROOT)
    parser.add_argument("--output",type=Path)
    parser.add_argument("--target-dir",type=Path)
    parser.add_argument("--seed",type=int,default=38)
    parser.add_argument("--repeat",type=int,default=3)
    parser.add_argument("--instances",default="1,8,32")
    parser.add_argument("--features",default="")
    parser.add_argument("--timeout",type=int,default=1200)
    parser.add_argument("--baseline-report",type=Path)
    parser.add_argument("--list",action="store_true")
    args=parser.parse_args()
    if args.list:print(json.dumps(MANIFEST,indent=2));return 0
    if not 1<=args.repeat<=20:parser.error("repeat must be 1..20")
    if args.timeout<1:parser.error("timeout must be positive")
    try:
        sizes=[int(x) for x in args.instances.split(",")]
        if any(n<1 or n>256 for n in sizes):raise ValueError()
    except ValueError:parser.error("instances must be comma-separated integers 1..256")
    repo=args.repo.resolve();target=(args.target_dir or repo/"target").resolve()
    out=(args.output or repo/"target/p6-results"/datetime.datetime.now(datetime.timezone.utc).strftime("%Y%m%dT%H%M%SZ")).resolve()
    if out.exists():parser.error("output already exists; use a new evidence directory")
    out.mkdir(parents=True)
    env=os.environ.copy();env["CARGO_TARGET_DIR"]=str(target)
    env["RELIQUARY_P6_SEED"]=str(args.seed);env["RELIQUARY_P6_INSTANCES"]=args.instances
    fixtures=out/"fixtures";fixtures.mkdir();env["RELIQUARY_P6_FIXTURE_ROOT"]=str(fixtures)
    report={"schema":1,"kind":"P6-H","release_accepted":False,"environment":describe(repo),
            "workload":{"seed":args.seed,"instances":args.instances,"repeat":args.repeat,"features":args.features},
            "cache_policy":"fresh physical fixture files per process; OS cache not flushed; first repetition is not claimed cold",
            "target_dir":str(target),"results":[],"rollup":{},"passed":False}
    def persist():
        report["rollup"]=rollup(report["results"])
        (out/"report.json").write_text(json.dumps(report,indent=2),encoding="utf-8")
    def execute(name,cmd,**options):
        r=run(repo,out,name,cmd,env,args.timeout,**options);report["results"].append(r);persist()
        return r["passed"]
    cmd=["cargo","test","-j1","--locked","--lib","--no-run","--message-format=json"]
    if args.features:cmd+=["--features",args.features]
    if not execute("build",cmd):return 1
    binary=None
    for line in (out/"build.stdout.log").read_text(encoding="utf-8").splitlines():
        try:artifact=json.loads(line)
        except ValueError:continue
        if artifact.get("reason")=="compiler-artifact" and artifact.get("profile",{}).get("test") and artifact.get("executable"):
            binary=artifact["executable"]
    if not binary:raise RuntimeError("Cargo did not identify a real library test binary")
    report["binary_sha256"]=sha(Path(binary));persist()
    for repeat in range(args.repeat):
        if not execute(f"harness-{repeat}",[binary,PREFIX,"--nocapture","--test-threads=1"],require_tests=True):return 1
        try:
            validate_h5(report["results"][-1]["metrics"], sizes)
            if not report["results"][-1]["steady_rss_bytes"]:
                raise ValueError("missing steady-state memory samples")
            if report["results"][-1]["sampled_peak_threads"] is None:
                raise ValueError("missing process/thread samples")
        except (ValueError, KeyError) as error:
            report["diagnostic_error"] = str(error); persist(); return 1

    if not execute("existing-gateway",[binary,"runtime_host::gateway","--test-threads=4"],require_tests=True):return 1
    for boundary in CRASHES:
        directory=fixtures/("crash-"+boundary)
        env["RELIQUARY_P6_CRASH_DIR"]=str(directory);env["RELIQUARY_P6_CRASH_MODE"]=boundary
        if not execute("crash-"+boundary,[binary,PREFIX+"::crash::p6_crash_child","--ignored","--exact","--nocapture","--test-threads=1"],expected_abort=True):return 1
        if not directory.joinpath("identity.json").is_file():raise RuntimeError("missing durable identity marker")
        if not execute("recover-"+boundary,[binary,PREFIX+"::crash::p6_crash_recover","--ignored","--exact","--nocapture","--test-threads=1"],require_tests=True):return 1
    report["passed"]=True;persist()
    if args.baseline_report:
        baseline=json.loads(args.baseline_report.read_text(encoding="utf-8"))
        if not baseline["passed"]:raise ValueError("baseline did not pass")
        report["comparison"]=compare(report,baseline);persist()
    print("P6-H evidence: "+str(out/"report.json"))
    return 0

if __name__=="__main__":
    raise SystemExit(main())
