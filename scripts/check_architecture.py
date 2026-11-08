#!/usr/bin/env python3
"""Check Reliquary's maintained architecture boundaries.

Ordinary checks are deterministic source guards. The optional graph check uses
Lexicon/Arcana/Pitlord directly; this repository does not implement its own
compiler call-graph analyzer or reconcile historical finding fingerprints.
"""
from __future__ import annotations

import argparse
import os
from pathlib import Path
import shutil
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
WORKSPACE = ROOT.parent
POLICY = ROOT / "tools" / "pitlord" / "policy.json"


def tool(name: str, environment: str, candidates: list[Path]) -> str:
    supplied = os.environ.get(environment)
    if supplied:
        path = Path(supplied).expanduser()
        if not path.is_file():
            raise RuntimeError(f"{environment} does not name an executable: {path}")
        return str(path.resolve())
    for candidate in candidates:
        if candidate.is_file():
            return str(candidate.resolve())
    result = shutil.which(name)
    if not result:
        raise RuntimeError(f"{name} is unavailable; set {environment} or install it")
    return result


def run(*args: str, timeout: int = 180) -> None:
    print("+", " ".join(args), flush=True)
    subprocess.run(args, cwd=ROOT, check=True, timeout=timeout)


def source_checks() -> None:
    scripts = [
        ("check_component_dag.py", "--self-test"),
        ("check_phase7_component_map.py",),
        ("check_phase7_source_cuts.py",),
        ("check_physical_boundary.py",),
        ("check_owner_boundaries.py",),
        ("check_runtime_boundaries.py",),
        ("check_facade_boundaries.py",),
        ("check_history_boundaries.py",),
        ("check_external_source_ownership.py",),
    ]
    for script, *args in scripts:
        run(sys.executable, str(ROOT / "scripts" / script), *args)


def graph_check(refresh: bool) -> None:
    exe = ".exe" if os.name == "nt" else ""
    if refresh:
        lexicon = tool("lexicon", "LEXICON", [
            WORKSPACE / "lexicon-arcana" / "build" / "bin" / f"lexicon{exe}",
        ])
        adapters = os.environ.get(
            "LEXICON_ADAPTERS",
            str(WORKSPACE / "lexicon-arcana" / "lexicon" / "adapters"),
        )
        if not (ROOT / ".lexicon" / "config.json").exists():
            run(lexicon, "init", "--repo", str(ROOT),
                "--adapters", adapters, "--languages", "rust", timeout=1200)
        run(lexicon, "rebuild", "--repo", str(ROOT), "--languages", "rust", timeout=1200)
        arcana = tool("arcana", "ARCANA", [
            WORKSPACE / "lexicon-arcana" / "build" / "bin" / f"arcana{exe}",
        ])
        run(arcana, "sync", "--lexicon", str(ROOT / ".lexicon"),
            "--state", str(ROOT / ".arcana"), timeout=600)
    arcana = tool("arcana", "ARCANA", [
        WORKSPACE / "lexicon-arcana" / "build" / "bin" / f"arcana{exe}",
    ])
    pitlord = tool("pitlord", "PITLORD", [
        WORKSPACE / "pitlord" / "bin" / f"pitlord{exe}",
    ])
    if not (ROOT / ".arcana" / "CURRENT").exists():
        raise RuntimeError("no Arcana snapshot; rerun with --graph --refresh")
    run(pitlord, "validate", "--policy", str(POLICY))
    run(pitlord, "check", "--repo", str(ROOT), "--policy", str(POLICY),
        "--arcana", arcana, "--timeout", "2m")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--graph", action="store_true",
                        help="also run Pitlord against the existing Arcana graph")
    parser.add_argument("--refresh", action="store_true",
                        help="rebuild the graph with Lexicon and Arcana before Pitlord")
    args = parser.parse_args()
    if args.refresh and not args.graph:
        parser.error("--refresh requires --graph")
    source_checks()
    if args.graph:
        graph_check(args.refresh)
    else:
        print("Source boundaries passed; full graph is a separate optional check.")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (RuntimeError, subprocess.CalledProcessError, subprocess.TimeoutExpired) as exc:
        print(f"architecture check failed: {exc}", file=sys.stderr)
        raise SystemExit(1)
