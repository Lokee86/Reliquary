#!/usr/bin/env python3
"""Freeze existing P0 outputs in one checksum-verified, reproducible repository archive."""
import argparse, hashlib, json, pathlib, zipfile
p=argparse.ArgumentParser()
p.add_argument("--evidence-dir",default=r"C:\Users\archa\AppData\Local\Temp\reliquary-freshness-perf-baseline\freshness-evidence")
p.add_argument("--output",default="benchmarks/freshness-p0-2026-10-03.zip")
args=p.parse_args()
root=pathlib.Path(args.evidence_dir)
names=[
 "p0-propagation-synthetic.json","p0-propagation-14d.json","p0-propagation-28d.json",
 "p0-propagation-ellis.json","p0-operation.stdout.jsonl","p0-operation.stderr.jsonl",
 "p0-operation-aggregate.json","p0-operation-summary.json","p0-comparator-selftest.json",
 "p0-measurement-manifest.json"]
for i in range(2,6):
 names.extend([f"p0-operation-run{i}.stdout.jsonl",f"p0-operation-run{i}.stderr.jsonl"])
entries=[]
for name in names:
 source=root/name
 assert source.is_file(),f"missing {source}"
 data=source.read_bytes()
 entries.append({"name":name,"sha256":hashlib.sha256(data).hexdigest(),"bytes":len(data)})
meta={
 "purpose":"P0 optimized propagation and deterministic local operation baseline; not live-host production telemetry",
 "core_head":"20e1e9ab0513b47b1e6726e45f340d3db11c4bc9",
 "toolchain":"rustc 1.97.1 (8bab26f4f 2026-07-14)",
 "cargo_lock_sha256":"5362fb09abc0995f99bb33462eede0666f9721f734f8f29cef54ab944170dd85",
 "propagation_profile":"release; freshness-performance-trace",
 "fixture_runs":5,
 "files":entries
}
out=pathlib.Path(args.output);out.parent.mkdir(parents=True,exist_ok=True)
with zipfile.ZipFile(out,"w",compression=zipfile.ZIP_DEFLATED,compresslevel=9) as z:
 for entry in entries:z.write(root/entry["name"],entry["name"])
 z.writestr("MANIFEST.json",json.dumps(meta,indent=2)+"\n")
with zipfile.ZipFile(out) as z:
 manifest=json.loads(z.read("MANIFEST.json"))
 for entry in manifest["files"]:
  assert hashlib.sha256(z.read(entry["name"])).hexdigest()==entry["sha256"],entry["name"]
print(json.dumps({"archive":str(out),"compressed_bytes":out.stat().st_size,"file_count":len(entries),"sha256":hashlib.sha256(out.read_bytes()).hexdigest(),"validation":"all archive checksums passed"}))
