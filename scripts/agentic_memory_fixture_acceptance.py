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

def source_hashes(root: Path, corpora=CORPORA) -> dict:
    return {str(Path(directory) / name): digest(root / directory / name)
            for _, directory, _, _ in corpora
            for name in ("project.prj.rel", "user.phy", "AUTHORITATIVE.txt", "SHA256SUMS.txt")}

def local(path: Path) -> Path:
    path = path.resolve()
    if not path.is_relative_to(ROOT):
        raise ValueError(f"output must remain inside active worktree: {path}")
    return path

def converted_corpora(receipt_path: Path) -> tuple[list[tuple[str, str, str, str]], dict]:
    receipt_path = local(receipt_path)
    receipt = json.loads(receipt_path.read_text(encoding="utf-8"))
    if not isinstance(receipt, dict):
        raise ValueError("conversion receipt must be a JSON object")
    if receipt.get("format") != "CVAFRS07":
        raise ValueError("conversion receipt must select CVAFRS07")
    semantic_oracle = receipt.get("semantic_oracle")
    if not isinstance(semantic_oracle, dict) or semantic_oracle.get("passed") is not True:
        raise ValueError("conversion receipt lacks a passing frozen semantic oracle")

    promotion_path = ROOT / "benchmarks/canonical-history-promotion-2026-10-21.json"
    promotion = json.loads(promotion_path.read_text(encoding="utf-8"))
    expected = {
        f"{directory}/{name}"
        for _, directory, _, _ in CORPORA
        for name in ("project.prj.rel", "user.phy")
    }

    def unique_fixtures(document: dict, label: str) -> dict:
        if not isinstance(document, dict):
            raise ValueError(f"{label} receipt must be a JSON object")
        rows = document.get("fixtures")
        if not isinstance(rows, list):
            raise ValueError(f"{label} receipt must contain a fixture list")
        indexed = {}
        for row in rows:
            if not isinstance(row, dict):
                raise ValueError(f"{label} receipt has a malformed fixture row")
            key = row.get("fixture")
            if not isinstance(key, str) or key in indexed:
                raise ValueError(f"{label} receipt has a missing or duplicate fixture")
            indexed[key] = row
        if set(indexed) != expected:
            raise ValueError(f"{label} receipt must identify the six promoted REL/PHY fixtures")
        return indexed

    converted = unique_fixtures(receipt, "conversion")
    promoted = unique_fixtures(promotion, "promotion")
    selected = []
    for label, directory, _, _ in CORPORA:
        hashes = []
        for name in ("project.prj.rel", "user.phy"):
            key = f"{directory}/{name}"
            row = converted[key]
            original = row.get("original_sha256")
            current = row.get("converted_sha256")
            if original != promoted[key].get("canonical_sha256"):
                raise ValueError(f"conversion before-image differs from immutable promotion for {key}")
            if not isinstance(current, str) or len(current) != 64 or any(c not in "0123456789abcdef" for c in current):
                raise ValueError(f"conversion receipt has an invalid converted SHA256 for {key}")
            hashes.append(current)
        selected.append((label, directory, hashes[0], hashes[1]))
    return selected, {
        "path": str(receipt_path),
        "sha256": digest(receipt_path),
        "format": receipt["format"],
        "semantic_oracle_passed": True,
        "scope": "current-format corpus inputs proven by the immutable Oct 21 promotion before-images and passing frozen semantic oracle",
    }

def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--phase", choices=["inventory", "functional", "fidelity"], required=True)
    parser.add_argument("--fixture-root", type=Path, default=ROOT.parent / "reliquary-fixtures/local/authoritative")
    parser.add_argument("--conversion-receipt", type=Path,
                        help="opt in to promoted CVAFRS07 inputs bound to the Oct 21 immutable before-images")
    parser.add_argument("--out", type=Path)
    args = parser.parse_args()
    binary = local(args.binary)
    conversion = None
    corpora = CORPORA
    if args.conversion_receipt is not None:
        corpora, conversion = converted_corpora(args.conversion_receipt)
    default_report_date = "2026-10-23" if conversion else "2026-10-16"
    out = local(args.out or ROOT / f"benchmarks/agentic-memory-fixture-{args.phase}-{default_report_date}.json")
    scratch = local(ROOT / "bin/agentic-fixture-runs" / str(uuid.uuid4()))
    before = source_hashes(args.fixture_root, corpora)
    expected_sources = {}
    for _, directory, rel_hash, phy_hash in corpora:
        for name, expected in [("project.prj.rel", rel_hash), ("user.phy", phy_hash)]:
            relative = str(Path(directory) / name)
            expected_sources[relative] = expected
            if before[relative] != expected:
                raise ValueError(f"authoritative frozen input hash mismatch: {directory}/{name}")
    scratch.mkdir(parents=True)
    source_files = sorted((ROOT / "src/runtime_inference").glob("agentic_memory_frozen_fixture_*.rs")) + [Path(__file__).resolve()]
    report = {"version": 1, "phase": args.phase, "fixture_root": str(args.fixture_root.resolve()),
              "source_before": before, "binary_sha256": digest(binary),
              "harness_sources": {str(path.relative_to(ROOT)): digest(path) for path in source_files},
              "head": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
              "results": [], "failure": None,
              "scope": "REL-only native acceptance on disposable copies; deterministic oracle and stored-vector replay; F3 is a separate real-host gate"}
    if conversion:
        report["conversion_receipt"] = conversion
        report["expected_source_hashes"] = expected_sources
        report["corpus_input_scope"] = conversion["scope"]
    try:
        for label, directory, rel_hash, _ in corpora:
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
            if conversion and result.get("status") == "corpus_limitation":
                raise RuntimeError(f"{label}: CVAFRS07 conversion input was blocked by corpus format")
    except Exception as error:
        report["failure"] = str(error)
    finally:
        report["corpus_limitations"] = sum(row["result"].get("status") == "corpus_limitation" for row in report["results"])
        report["source_after"] = source_hashes(args.fixture_root, corpora)
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
