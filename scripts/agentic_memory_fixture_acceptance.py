#!/usr/bin/env python3
"""Run ignored native acceptance tests only on verified, worktree-local REL copies."""
from __future__ import annotations
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import uuid

ROOT = Path(__file__).resolve().parents[1]
CORPORA = [
    ("first14d", "chatgpt-first14d", "800d98cc09081901ffdc8e37b6c4dab60579e6356ac710a810bd37508343c851", "aa92f99110e55efc0b498b9603a5ffe75b67eb04f0d5ba327cff998c3bc4e130"),
    ("first28d", "chatgpt-first28d", "3816e5a83c73a0c30465536402bab07a938c97cfaabf2b0be1a672ba2cc02c2b", "b1f37b9a1dd108259754d5eb702e6782eb711f343f7a22b2c1bac7452161a58e"),
    ("ellis", "rhelm-david-r-ellis-pre-relationship", "4ab338734939e6bece41622b927648366ed7db33f91b0ec2eb49f390948d49f8", "110301f1f12873b8ac1a2eb5000a004fdcd0287d7e141622e6cd18e7b2afe937"),
]

def digest(path: Path) -> str:
    value = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            value.update(chunk)
    return value.hexdigest()

def source_hashes(root: Path) -> dict:
    return {str(Path(directory) / name): digest(root / directory / name)
            for _, directory, _, _ in CORPORA
            for name in ("project.prj.rel", "user.phy", "AUTHORITATIVE.txt", "SHA256SUMS.txt")}

def local(path: Path) -> Path:
    path = path.resolve()
    if not path.is_relative_to(ROOT):
        raise ValueError(f"output must remain inside active worktree: {path}")
    return path

def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--phase", choices=["inventory", "functional", "fidelity"], required=True)
    parser.add_argument("--fixture-root", type=Path, default=ROOT.parent / "reliquary-fixtures/local/authoritative")
    parser.add_argument("--out", type=Path)
    args = parser.parse_args()
    binary = local(args.binary)
    out = local(args.out or ROOT / f"benchmarks/agentic-memory-fixture-{args.phase}-2026-10-16.json")
    scratch = local(ROOT / "bin/agentic-fixture-runs" / str(uuid.uuid4()))
    before = source_hashes(args.fixture_root)
    for _, directory, rel_hash, phy_hash in CORPORA:
        for name, expected in [("project.prj.rel", rel_hash), ("user.phy", phy_hash)]:
            if before[str(Path(directory) / name)] != expected:
                raise ValueError(f"authoritative frozen input hash mismatch: {directory}/{name}")
    scratch.mkdir(parents=True)
    source_files = sorted((ROOT / "src/runtime_inference").glob("agentic_memory_frozen_fixture_*.rs")) + [Path(__file__).resolve()]
    report = {"version": 1, "phase": args.phase, "fixture_root": str(args.fixture_root.resolve()),
              "source_before": before, "binary_sha256": digest(binary),
              "harness_sources": {str(path.relative_to(ROOT)): digest(path) for path in source_files},
              "head": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
              "results": [], "failure": None,
              "scope": "REL-only native acceptance on disposable copies; deterministic oracle and stored-vector replay; F3 is a separate real-host gate"}
    try:
        for label, directory, rel_hash, _ in CORPORA:
            case = scratch / label
            case.mkdir()
            copied = case / "owner.rel"
            shutil.copyfile(args.fixture_root / directory / "project.prj.rel", copied)
            assert digest(copied) == rel_hash
            result_path = case / "result.json"
            env = os.environ.copy()
            env.update(RELIQUARY_AGENTIC_FIXTURE=str(copied), RELIQUARY_AGENTIC_PHASE=args.phase,
                       RELIQUARY_AGENTIC_RESULT=str(result_path), TEMP=str(ROOT / "target/test-tmp"), TMP=str(ROOT / "target/test-tmp"))
            command = [str(binary), "agentic_frozen_fixture_acceptance", "--ignored", "--nocapture", "--test-threads=1"]
            completed = subprocess.run(command, cwd=ROOT, env=env, capture_output=True, text=True, encoding="utf-8", errors="replace")
            (case / "stdout.log").write_text(completed.stdout, encoding="utf-8")
            (case / "stderr.log").write_text(completed.stderr, encoding="utf-8")
            if completed.returncode or not result_path.is_file() or "1 passed" not in completed.stdout:
                raise RuntimeError(f"{label}: native fixture gate failed ({completed.returncode}); {case}\n{completed.stdout[-1800:]}\n{completed.stderr[-1800:]}")
            result = json.loads(result_path.read_text(encoding="utf-8"))
            report["results"].append({"corpus": label, "command": command, "copy": str(copied), "result": result})
            print(f"{args.phase}: {label}: {result.get('status', 'executed')}", flush=True)
    except Exception as error:
        report["failure"] = str(error)
    finally:
        report["corpus_limitations"] = sum(row["result"].get("status") == "corpus_limitation" for row in report["results"])
        report["source_after"] = source_hashes(args.fixture_root)
        report["source_unchanged"] = report["source_after"] == before
        if not report["source_unchanged"]:
            report["failure"] = "authoritative source bytes changed during execution"
        report["acceptance_status"] = ("failed" if report["failure"] else
                                       "blocked_by_corpus_format" if report["corpus_limitations"] else "executed")
        out.parent.mkdir(parents=True, exist_ok=True)
        out.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    if report["failure"]:
        print(report["failure"])
        return 1
    print(f"source hashes unchanged; evidence: {out}")
    return 0

if __name__ == "__main__":
    raise SystemExit(main())
