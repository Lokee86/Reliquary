#!/usr/bin/env python3
from __future__ import annotations

import argparse
import os
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
WORKSPACE = ROOT.parent
POLICY = ROOT / "tools" / "pitlord" / "policy.json"


def executable(name: str) -> str:
    return name + ".exe" if os.name == "nt" else name


def resolve(environment: str, name: str, candidates: list[Path]) -> Path:
    configured = os.environ.get(environment)
    if configured:
        path = Path(configured).expanduser().resolve()
        if path.is_file():
            return path
        raise RuntimeError(f"{environment} does not name a file: {path}")
    for candidate in candidates:
        if candidate.is_file():
            return candidate.resolve()
    discovered = shutil.which(name)
    if discovered:
        return Path(discovered).resolve()
    raise RuntimeError(
        f"{name} is unavailable; set {environment}, install it on PATH, or build the sibling tool"
    )


def analysis_roots() -> list[Path]:
    configured = os.environ.get("LEXICON_ARCANA_ROOT")
    roots: list[Path] = []
    if configured:
        roots.append(Path(configured).expanduser().resolve())
    roots.append(WORKSPACE / "lexicon-arcana")
    return roots


def lexicon_candidates() -> list[Path]:
    candidates: list[Path] = []
    for root in analysis_roots():
        candidates.append(root / "build" / "bin" / executable("lexicon"))
    return candidates


def arcana_candidates() -> list[Path]:
    candidates: list[Path] = []
    for root in analysis_roots():
        candidates.extend(
            [
                root / "build" / "bin" / executable("arcana"),
                root / "arcana" / "target" / "release" / executable("arcana"),
                root / "arcana" / "target-v3" / "release" / executable("arcana"),
            ]
        )
    return candidates


def adapter_root() -> Path:
    configured = os.environ.get("LEXICON_ADAPTERS")
    if configured:
        path = Path(configured).expanduser().resolve()
        if path.is_dir():
            return path
        raise RuntimeError(f"LEXICON_ADAPTERS does not name a directory: {path}")
    for root in analysis_roots():
        path = root / "lexicon" / "adapters"
        if path.is_dir():
            return path.resolve()
    raise RuntimeError(
        "Lexicon adapters are unavailable; set LEXICON_ADAPTERS or provide the sibling lexicon-arcana checkout"
    )


def run(command: list[str]) -> None:
    print("+", subprocess.list2cmdline(command), flush=True)
    subprocess.run(command, cwd=ROOT, check=True)


def refresh_evidence(lexicon: Path, arcana: Path) -> None:
    run(
        [
            str(lexicon),
            "init",
            "--repo",
            str(ROOT),
            "--adapters",
            str(adapter_root()),
        ]
    )
    run(
        [
            str(arcana),
            "sync",
            "--lexicon",
            str(ROOT / ".lexicon"),
            "--state",
            str(ROOT / ".arcana"),
        ]
    )


def main() -> int:
    parser = argparse.ArgumentParser(description="Validate Reliquary architecture policy.")
    parser.add_argument(
        "--refresh",
        action="store_true",
        help="rebuild Lexicon/Arcana evidence before checking policy",
    )
    args = parser.parse_args()

    run([sys.executable, "scripts/check_multiplexing_phase4_coverage.py", "--self-test"])

    pitlord = resolve(
        "PITLORD",
        "pitlord",
        [WORKSPACE / "pitlord" / "bin" / executable("pitlord")],
    )
    arcana = resolve("ARCANA", "arcana", arcana_candidates())

    if args.refresh:
        lexicon = resolve("LEXICON", "lexicon", lexicon_candidates())
        refresh_evidence(lexicon, arcana)

    if not (ROOT / ".arcana" / "CURRENT").is_file():
        raise RuntimeError(
            "Reliquary has no current Arcana snapshot; run this command again with --refresh"
        )

    run([str(pitlord), "validate", "--policy", str(POLICY)])
    run(
        [
            str(pitlord),
            "check",
            "--repo",
            str(ROOT),
            "--policy",
            str(POLICY),
            "--arcana",
            str(arcana),
            "--timeout",
            "2m",
        ]
    )
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (RuntimeError, subprocess.CalledProcessError) as error:
        print(f"architecture check failed: {error}", file=sys.stderr)
        raise SystemExit(1)
