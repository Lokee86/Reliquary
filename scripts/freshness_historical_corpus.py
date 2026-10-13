"""Hash-verified native retained-corpus acceptance gate, using repo-local copies.

Exit 0 requires at least one new native checkpoint with nonempty contributions.
A passing audit test for a rejected input does not satisfy this release gate.
"""
from __future__ import annotations

import argparse
import ctypes
import hashlib
import json
import os
import re
from pathlib import Path
import subprocess
import time

ROOT = Path(__file__).resolve().parents[1]
FROZEN = [
    ("chatgpt-first14d", "49751a62dd8644800d19976a0696a69366dc1e4f8d7c39bb4b69fbd7a8a5d18b"),
    ("chatgpt-first28d", "8f4a2bf871d7f5368edde93954a2260a29c86131b28475b323a7b6c70f8c51e4"),
    ("rhelm-david-r-ellis-pre-relationship", "3efd9c666aa992686abd8aac9b77cfc7dc2a0e15d6fb26ec01f8666db175e476"),
]


def digest(path: Path) -> str:
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def windows_metrics(process: subprocess.Popen) -> dict:
    """Whole audit process counters; these are not temporary allocation peaks."""
    if os.name != "nt":
        return {"cpu_seconds": None, "peak_working_set_bytes": None}
    from ctypes import wintypes

    class Counters(ctypes.Structure):
        _fields_ = [("cb", wintypes.DWORD), ("PageFaultCount", wintypes.DWORD)] + [
            (name, ctypes.c_size_t) for name in (
                "PeakWorkingSetSize", "WorkingSetSize", "QuotaPeakPagedPoolUsage", "QuotaPagedPoolUsage",
                "QuotaPeakNonPagedPoolUsage", "QuotaNonPagedPoolUsage", "PagefileUsage", "PeakPagefileUsage")]

    counters = Counters()
    counters.cb = ctypes.sizeof(counters)
    psapi = ctypes.WinDLL("psapi", use_last_error=True)
    psapi.GetProcessMemoryInfo.argtypes = [wintypes.HANDLE, ctypes.POINTER(Counters), wintypes.DWORD]
    if not psapi.GetProcessMemoryInfo(wintypes.HANDLE(int(process._handle)), ctypes.byref(counters), counters.cb):
        raise ctypes.WinError(ctypes.get_last_error())
    times = [wintypes.FILETIME() for _ in range(4)]
    kernel = ctypes.WinDLL("kernel32", use_last_error=True)
    kernel.GetProcessTimes.argtypes = [wintypes.HANDLE] + [ctypes.POINTER(wintypes.FILETIME)] * 4
    if not kernel.GetProcessTimes(wintypes.HANDLE(int(process._handle)), *(ctypes.byref(t) for t in times)):
        raise ctypes.WinError(ctypes.get_last_error())
    ticks = lambda t: (t.dwHighDateTime << 32) | t.dwLowDateTime
    return {"cpu_seconds": (ticks(times[2]) + ticks(times[3])) / 10_000_000,
            "peak_working_set_bytes": counters.PeakWorkingSetSize,
            "peak_pagefile_bytes": counters.PeakPagefileUsage,
            "scope": "whole audit process including open, inventory, hashing, diagnostics and native operation"}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True, help="compiled library test executable")
    parser.add_argument("--fixtures", type=Path, default=ROOT.parent / "reliquary-fixtures/local/authoritative")
    parser.add_argument("--manifest", type=Path, help="optional JSON list of name/path/sha256 records for newer retained inputs")
    parser.add_argument("--output", type=Path, default=ROOT / "target/historical-corpus/report.json")
    args = parser.parse_args()
    args.output = args.output.resolve()
    if not args.output.is_relative_to(ROOT):
        parser.error("output must stay inside the active repository")
    args.output.parent.mkdir(parents=True, exist_ok=True)
    reports = []
    sources = [(name, (args.fixtures / name / "project.prj.rel").resolve(), expected) for name, expected in FROZEN]
    if args.manifest:
        records = json.loads(args.manifest.read_text(encoding="utf-8"))
        sources = [(r["name"], (args.manifest.resolve().parent / r["path"]).resolve(), r["sha256"]) for r in records]
    if not sources or len({name for name, _, _ in sources}) != len(sources):
        parser.error("nonempty distinct fixture names required")
    for name, source, expected in sources:
        if not re.fullmatch(r"[a-z0-9][a-z0-9-]{0,99}", name) or not re.fullmatch(r"[0-9a-f]{64}", expected):
            parser.error("invalid fixture name or SHA256")
        written = [args.output, args.output.parent / f"{name}.json", args.output.parent / f"{name}.log"]
        if any(path == source for path in written):
            parser.error("report must not overwrite an input")
        if digest(source) != expected:
            raise RuntimeError(f"authoritative digest mismatch: {name}")
        output = args.output.parent / f"{name}.json"
        # A failed attempt must never reuse an earlier acceptance report.
        output.unlink(missing_ok=True)
        env = dict(os.environ, RELIQUARY_HISTORICAL_CORPUS=str(source),
                   RELIQUARY_HISTORICAL_CORPUS_SHA256=expected, RELIQUARY_HISTORICAL_CORPUS_REPORT=str(output))
        started = time.perf_counter()
        with (args.output.parent / f"{name}.log").open("w", encoding="utf-8") as log:
            process = subprocess.Popen([str(args.binary.resolve()), "historical_retained_corpus_native_acceptance",
                                       "--ignored", "--nocapture", "--test-threads=1"],
                                      cwd=ROOT, env=env, stdout=log, stderr=subprocess.STDOUT)
            process.wait()
            metrics = windows_metrics(process)
        if process.returncode or not output.exists():
            raise RuntimeError(f"native audit failed: {name}; inspect its repo-local log")
        row = json.loads(output.read_text(encoding="utf-8"))
        row.update(name=name, elapsed_seconds=time.perf_counter() - started, process_metrics=metrics)
        if digest(source) != expected:
            raise RuntimeError(f"authoritative input changed: {name}")
        row["report_sha256"] = digest(output)
        reports.append(row)
        print(f"{name}: {row['status']} - {row.get('error', 'native checkpoint accepted')}", flush=True)
    complete = any(row["status"] == "accepted" and row.get("diagnostics", {}).get("recipient_contributions", 0) > 0
                   for row in reports)
    result = {"schema": 1, "c10_complete": False, "native_acceptance_satisfied": complete, "binary_sha256": digest(args.binary),
              "harness_sha256": {str(path.relative_to(ROOT)).replace("\\", "/"): digest(path) for path in (
                  Path(__file__).resolve(), ROOT / "src/facade/historical_retained_corpus_tests.rs",
                  ROOT / "src/facade/historical_retained_diagnostics.rs", ROOT / "Cargo.lock")},
              "source_revision": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
              "source_dirty": bool(subprocess.check_output(["git", "status", "--porcelain"], cwd=ROOT, text=True)),
              "reports": reports,
              "limits": "Rejected-input CPU/memory are gate diagnostics, never accepted reconstruction performance. "
                        "Allocator temporary peak, spill high-water and inference costs require an accepted run."}
    args.output.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    print(f"Native acceptance {'satisfied; C10 measurement review required' if complete else 'not satisfied; C10 remains open'}: {args.output}", flush=True)
    return 0 if complete else 2


if __name__ == "__main__":
    raise SystemExit(main())
