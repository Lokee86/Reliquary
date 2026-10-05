"""Collect P5 breadth; no inference or authoritative fixture mutation."""
import argparse,hashlib,json,pathlib,statistics,subprocess,tempfile,zipfile
ROOT=pathlib.Path(__file__).resolve().parents[1]
FIXTURES=[("14d","chatgpt-first14d","49751a62dd8644800d19976a0696a69366dc1e4f8d7c39bb4b69fbd7a8a5d18b"),("28d","chatgpt-first28d","8f4a2bf871d7f5368edde93954a2260a29c86131b28475b323a7b6c70f8c51e4"),("ellis","rhelm-david-r-ellis-pre-relationship","3efd9c666aa992686abd8aac9b77cfc7dc2a0e15d6fb26ec01f8666db175e476")]
def sha(p):return hashlib.sha256(pathlib.Path(p).read_bytes()).hexdigest()
def main():
 ap=argparse.ArgumentParser();ap.add_argument("--binary",required=True);a=ap.parse_args()
 out=pathlib.Path(tempfile.mkdtemp(prefix="freshness-p5-"));summary=[]
 manifest={"binary_sha256":sha(a.binary),"source_commit":subprocess.check_output(["git","rev-parse","HEAD"],cwd=ROOT,text=True).strip(),"files":{}}
 jobs=[("synthetic",[])]+[(label,["--fixture",str(ROOT.parent/"reliquary-fixtures"/"local"/"authoritative"/name/"project.prj.rel"),"--sha256",digest]) for label,name,digest in FIXTURES]
 for label,args in jobs:
  run=subprocess.run([a.binary,*args],cwd=ROOT,capture_output=True,text=True,timeout=180)
  if run.returncode:raise RuntimeError(f"{label}: {run.stderr}")
  data=json.loads(run.stdout)
  if label!="synthetic":assert data["source_hash_unchanged"]
  (out/f"{label}.json").write_text(run.stdout,encoding="utf-8")
  grouped={}
  for row in data["results"]:
   assert row["origin_reconstruction_equal"]
   assert sum(row["effect_hop_histogram"].values())==row["recipient_count"]
   key=(row["principal"],row["root_count"],row["layout"],row["membership_mode"])
   grouped.setdefault(key,[]).append(row)
  for key,rows in grouped.items():
   f=[r["touched_fraction"] for r in rows]
   summary.append({"fixture":label,"nodes":data["graph"]["node_count"],"snapshot":data["graph"]["snapshot_validity"],"principal":key[0],"roots":key[1],"layout":key[2],"membership":key[3],"samples":len(rows),"reach_min":min(f),"reach_median":statistics.median(f),"reach_max":max(f),"nearly_full_rate":sum(r["nearly_full_web"] for r in rows)/len(rows),"edges_per_recipient_median":statistics.median(r["edges_per_recipient"] for r in rows)})
  print(label,len(data["results"]),"events",flush=True)
 (out/"summary.json").write_text(json.dumps(summary,indent=2),encoding="utf-8")
 for source in [ROOT/"examples"/"freshness_breadth.rs",pathlib.Path(__file__)]: (out/source.name).write_bytes(source.read_bytes())
 for p in sorted(out.iterdir()):manifest["files"][p.name]={"sha256":sha(p),"bytes":p.stat().st_size}
 (out/"MANIFEST.json").write_text(json.dumps(manifest,indent=2),encoding="utf-8")
 archive=ROOT/"benchmarks"/"freshness-p5-breadth-2026-10-04.zip"
 with zipfile.ZipFile(archive,"w",zipfile.ZIP_DEFLATED) as z:
  for p in sorted(out.iterdir()):z.write(p,p.name)
 with zipfile.ZipFile(archive) as z:
  for name,meta in manifest["files"].items():assert hashlib.sha256(z.read(name)).hexdigest()==meta["sha256"]
 print(json.dumps({"archive":str(archive),"sha256":sha(archive),"summary":summary},indent=2),flush=True)
if __name__=="__main__":main()
