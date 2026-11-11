#!/usr/bin/env python3
"""Refresh Rust semantic evidence and enforce the local Pitlord policy."""
from __future__ import annotations

import argparse
import json
import os
import signal
import shutil
import subprocess
import sys
import tempfile
import time
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
POLICY = ROOT / "tools" / "pitlord" / "policy.json"


def executable(value: str | None, name: str) -> str:
    candidate = value or shutil.which(name)
    if not candidate:
        raise SystemExit(f"required executable not found: {name}; pass --{name} or add it to PATH")
    path = Path(candidate).expanduser()
    return str(path.resolve()) if path.exists() else candidate


def run(command: list[str], root: Path, environment: dict[str, str], deadline: float, phase_timeout: int, *, capture: bool = False, allowed_codes: tuple[int, ...] = (0,)) -> str:
    timeout = min(phase_timeout, int(deadline - time.monotonic()))
    if timeout <= 0:
        raise SystemExit("overall Pitlord tool run timed out before the next phase")
    print("+ " + subprocess.list2cmdline(command), flush=True)
    options: dict[str, object] = {"cwd": root, "env": environment}
    if capture:
        options.update(stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    if os.name == "nt":
        options["creationflags"] = subprocess.CREATE_NEW_PROCESS_GROUP
    else:
        options["start_new_session"] = True
    process = subprocess.Popen(command, **options)
    try:
        output, errors = process.communicate(timeout=timeout)
    except subprocess.TimeoutExpired as error:
        if os.name == "nt":
            subprocess.run(["taskkill", "/PID", str(process.pid), "/T", "/F"], check=False, capture_output=True)
        else:
            os.killpg(process.pid, signal.SIGKILL)
        process.wait()
        raise SystemExit(f"command timed out after {timeout}s; its process tree was stopped: {command[0]}") from error
    if capture and errors:
        print(errors, file=sys.stderr, end="")
    if process.returncode not in allowed_codes:
        if capture and output:
            print(output, end="")
        raise SystemExit(process.returncode or 1)
    return output or ""


def verify_evidence(analysis: dict, policy: Path) -> None:
    """Reject an empty/partial language graph before a vacuous policy pass."""
    if analysis.get("schema") != "pitlord.area-analysis.v1":
        raise SystemExit("unsupported Pitlord area-analysis schema")
    required = {area["id"] for area in json.loads(policy.read_text(encoding="utf-8"))["areas"]}
    observed = {area["id"]: area for area in analysis["areas"]}
    missing = sorted(area for area in required if observed.get(area, {}).get("kind_counts", {}).get("file", 0) <= 0)
    ownership = analysis["ownership"]
    summary = analysis["summary"]
    if missing or ownership["checked_nodes"] <= 0 or ownership["unowned_nodes"] or ownership["overlapping_nodes"]:
        raise SystemExit(f"incomplete Rust ownership evidence: missing areas={missing}; ownership={ownership}")
    if summary["relationships_selected"] <= 0 or summary["dependency_count"] <= 0:
        raise SystemExit("Rust graph contains no selected import/dependency evidence")
    print(f"Evidence: {len(required)} populated areas; {ownership['checked_nodes']} owned files; "
          f"{summary['relationships_selected']} selected import/dependency edges; {summary['dependency_count']} area dependencies", flush=True)


def negative_control(pitlord: str, arcana: str, root: Path, policy: Path, environment: dict[str, str], deadline: float, timeout: int) -> None:
    """Prove the engine rejects a known real dependency on the same snapshot."""
    scratch = (root / "target" / "standards-policy-controls").resolve()
    if not scratch.is_relative_to(root):
        raise SystemExit("negative-control scratch path escapes repository")
    scratch.mkdir(parents=True, exist_ok=True)
    rule_id = "negative-known-gateway-application-dependency"
    with tempfile.TemporaryDirectory(prefix="pitlord-", dir=scratch) as directory:
        control = Path(directory) / "policy.json"
        control.write_text(json.dumps({
            "version": 1,
            "includes": [str(policy)],
            "rules": [{
                "id": rule_id, "type": "forbid_dependency",
                "from_areas": ["runtime-gateway"], "to_areas": ["application"],
                "relations": ["depends-on"],
                "message": "Negative control: the real permitted Gateway to Application edge must be detected",
            }],
        }, indent=2) + "\n", encoding="utf-8")
        output = run(
            [pitlord, "check", "--repo", str(root), "--policy", str(control),
             "--arcana", arcana, "--format", "json", "--timeout", f"{timeout}s"],
            root, environment, deadline, timeout, capture=True, allowed_codes=(1,),
        )
        report = json.loads(output)
        if not any(item["rule_id"] == rule_id for item in report["diagnostics"]):
            raise SystemExit("Pitlord negative control failed to identify the known dependency")
    print("PASS Pitlord rejected the known Gateway-to-Application dependency negative control", flush=True)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, default=ROOT, help="Reliquary checkout (default: script's repository)")
    parser.add_argument("--pitlord", help="Pitlord executable (default: pitlord from PATH)")
    parser.add_argument("--lexicon", help="Lexicon executable (default: lexicon from PATH)")
    parser.add_argument("--arcana", help="Arcana executable (default: arcana from PATH)")
    parser.add_argument("--adapters", type=Path, help="Lexicon adapter directory (passed to init and scan through LEXICON_ADAPTERS)")
    parser.add_argument("--timeout-seconds", type=int, default=1200, help="maximum duration for each tool command")
    parser.add_argument("--overall-timeout-seconds", type=int, default=1800, help="maximum duration for the complete tool run")
    parser.add_argument("--validate-only", action="store_true", help="validate policy syntax without scanning or graph checks")
    args = parser.parse_args()
    if args.timeout_seconds <= 0 or args.overall_timeout_seconds <= 0:
        raise SystemExit("timeout values must be positive")

    root = args.repo.resolve()
    if not root.is_dir():
        raise SystemExit(f"repository directory does not exist: {root}")
    policy = root / POLICY.relative_to(ROOT)
    if not policy.is_file():
        raise SystemExit(f"Pitlord policy not found: {policy}")

    pitlord = executable(args.pitlord, "pitlord")
    environment = os.environ.copy()
    environment["LEXICON_STATE_DIR"] = str(root / ".lexicon")
    environment.setdefault("CARGO_TARGET_DIR", str(root / "target" / "lexicon-tools"))
    # Lexicon invokes the adapter through cargo run's dev profile. Full-repository
    # semantic propagation needs optimization; this changes the tool build only.
    environment.setdefault("CARGO_PROFILE_DEV_OPT_LEVEL", "3")
    environment.setdefault("CARGO_PROFILE_DEV_DEBUG", "0")
    if args.adapters:
        adapters = args.adapters.resolve()
        if not adapters.is_dir():
            raise SystemExit(f"Lexicon adapter directory does not exist: {adapters}")
        environment["LEXICON_ADAPTERS"] = str(adapters)
    deadline = time.monotonic() + args.overall_timeout_seconds

    run([pitlord, "validate", "--policy", str(policy)], root, environment, deadline, args.timeout_seconds)
    if args.validate_only:
        return 0

    lexicon = executable(args.lexicon, "lexicon")
    arcana = executable(args.arcana, "arcana")
    if (root / ".lexicon" / "config.json").is_file():
        run([lexicon, "scan", "--repo", str(root)], root, environment, deadline, args.timeout_seconds)
    else:
        command = [lexicon, "init", "--repo", str(root), "--languages", "rust"]
        if args.adapters:
            command.extend(["--adapters", str(adapters)])
        run(command, root, environment, deadline, args.timeout_seconds)
    run(
        [arcana, "sync", "--lexicon", str(root / ".lexicon"), "--state", str(root / ".arcana")],
        root,
        environment,
        deadline,
        args.timeout_seconds,
    )
    analysis = run(
        [pitlord, "analyze", "--repo", str(root), "--policy", str(policy),
         "--arcana", arcana, "--format", "json", "--scope", "src,crates,cli/src",
         "--relations", "imports,depends-on", "--ownership-kinds", "file", "--timeout", f"{args.timeout_seconds}s"],
        root, environment, deadline, args.timeout_seconds, capture=True,
    )
    verify_evidence(json.loads(analysis), policy)
    run(
        [pitlord, "check", "--repo", str(root), "--policy", str(policy), "--arcana", arcana, "--timeout", f"{args.timeout_seconds}s"],
        root,
        environment,
        deadline,
        args.timeout_seconds,
    )
    negative_control(pitlord, arcana, root, policy, environment, deadline, args.timeout_seconds)
    return 0


if __name__ == "__main__":
    sys.exit(main())
