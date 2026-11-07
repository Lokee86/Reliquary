from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Sequence

from .engine import inspect_state, stage, validate
from .files import D03Error, checked_root
from .rehearsal import rehearse, rollback_prewrite
from .state import _read_selection, _read_state


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Synthetic-only D03 staging/rehearsal model; not a deployment tool."
    )
    commands = parser.add_subparsers(dest="operation", required=True)
    stage_parser = commands.add_parser("stage", help="create a fake six-owner generation")
    stage_parser.add_argument("--root", required=True, type=Path)
    stage_parser.add_argument("--failpoint", choices=("before_sync", "after_sync"))
    validate_parser = commands.add_parser("validate", help="validate synthetic manifest and files")
    validate_parser.add_argument("--root", required=True, type=Path)
    rehearse_parser = commands.add_parser("rehearse", help="simulate routing inside the fake root")
    rehearse_parser.add_argument("--root", required=True, type=Path)
    rehearse_parser.add_argument(
        "--failpoint",
        choices=(
            "crash_before_selection_replace", "crash_after_selection_replace",
            "simulate_selector_replace_unavailable", "health_check",
        ),
    )
    inspect_parser = commands.add_parser("inspect-state", help="inspect the fake rehearsal state")
    inspect_parser.add_argument("--root", required=True, type=Path)
    rollback_parser = commands.add_parser("rollback-prewrite", help="restore fake prior selection before writes")
    rollback_parser.add_argument("--root", required=True, type=Path)
    rollback_parser.add_argument(
        "--failpoint",
        choices=(
            "crash_before_selection_replace", "crash_after_selection_replace",
            "simulate_selector_replace_unavailable",
        ),
    )
    return parser


def _persisted_context(root_path: Path) -> dict | None:
    try:
        root = checked_root(root_path)
        state = _read_state(root)
        actual_selection = _read_selection(root)
    except (D03Error, OSError, ValueError, TypeError, RuntimeError, RecursionError):
        return None
    return {
        "phase": state["phase"],
        "selected_generation_id": state["selected_generation_id"],
        "actual_selection": actual_selection,
        "manifest_sha256": state["manifest_sha256"],
        "pending_transition": state["pending_transition"],
    }


def _failure_record(args: argparse.Namespace, error: Exception) -> dict:
    record = {
        "status": "INCOMPLETE",
        "evidence_scope": "synthetic_fixture",
        "simulation_only": True,
        "error": str(error),
    }
    context = _persisted_context(args.root)
    if context is not None:
        record["persisted_state"] = context
        record["recovery_hint"] = "Run inspect-state separately to reconcile a pending selector intent."
    elif args.operation == "inspect-state":
        record["recovery_hint"] = (
            "Persisted state or selector could not be safely read. This inspection failed closed; "
            "preserve the synthetic root unchanged for diagnosis."
        )
    else:
        record["recovery_hint"] = (
            "Persisted state could not be read safely here. Run inspect-state separately; "
            "if it also reports a state-read error, preserve the synthetic root unchanged."
        )
    return record


def main(argv: Sequence[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    try:
        if args.operation == "stage":
            result = stage(args.root, args.failpoint)
        elif args.operation == "validate":
            result = validate(args.root)
        elif args.operation == "rehearse":
            result = rehearse(args.root, args.failpoint)
        elif args.operation == "inspect-state":
            result = inspect_state(args.root)
        elif args.operation == "rollback-prewrite":
            result = rollback_prewrite(args.root, args.failpoint)
        else:
            raise D03Error("unsupported operation")
    except (D03Error, OSError, ValueError, TypeError, RuntimeError, RecursionError) as error:
        print(
            json.dumps(
                _failure_record(args, error),
                sort_keys=True,
                separators=(",", ":"),
            ),
            file=sys.stderr,
        )
        return 1
    print(json.dumps(result, sort_keys=True, separators=(",", ":")))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
