#!/usr/bin/env python3
"""Synthetic-only G5 runner entry point; never executes a benchmark in this slice."""
from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]
if str(REPO_ROOT) not in sys.path:
    sys.path.insert(0, str(REPO_ROOT))

from benchmarks.g5.runner import (  # noqa: E402
    DEFAULT_POLICY,
    DEFAULT_SELFTESTS,
    DEFAULT_WORKLOADS,
    ContractError,
    preflight_result,
    read_json,
    run_synthetic_selftests,
    sha256_file,
)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Run frozen G5 synthetic self-tests or fail-closed benchmark preflight.")
    commands = parser.add_subparsers(dest="command", required=True)

    self_test = commands.add_parser("self-test", help="validate frozen manifests and synthetic statistics vectors")
    self_test.add_argument("--policy", type=Path, default=DEFAULT_POLICY)
    self_test.add_argument("--workloads", type=Path, default=DEFAULT_WORKLOADS)
    self_test.add_argument("--statistics-selftests", type=Path, default=DEFAULT_SELFTESTS)

    benchmark = commands.add_parser("benchmark", help="G5 interface; currently gated before any adapter is executed")
    benchmark.add_argument("--workloads", type=Path, required=True)
    benchmark.add_argument("--old", required=True, help="baseline binary path; not opened in the synthetic-only slice")
    benchmark.add_argument("--new", required=True, help="candidate binary path; not opened in the synthetic-only slice")
    benchmark.add_argument("--cohort", required=True, help="approved cohort manifest; not opened in the synthetic-only slice")
    benchmark.add_argument("--output", required=True, help="output directory; not created in the synthetic-only slice")
    benchmark.add_argument("--policy", type=Path, default=DEFAULT_POLICY)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    try:
        if args.command == "self-test":
            result = run_synthetic_selftests(args.policy, args.workloads, args.statistics_selftests)
            print(json.dumps(result, sort_keys=True))
            return 0

        policy = read_json(args.policy)
        workloads = read_json(args.workloads)
        result = preflight_result(policy, workloads)
        result["policy_sha256"] = sha256_file(args.policy)
        result["workload_manifest_sha256"] = sha256_file(args.workloads)
        result["output_created"] = False
        print(json.dumps(result, sort_keys=True))
        return 2
    except (ContractError, OSError, json.JSONDecodeError) as exc:
        print(json.dumps({"schema_id": "reliquary-g5-runner-error-v1", "error": str(exc)}, sort_keys=True), file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
