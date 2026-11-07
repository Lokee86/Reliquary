from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Sequence

from .engine import inspect_state, stage, validate
from .files import D03Error
from .rehearsal import rehearse, rollback_prewrite


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
        choices=("before_selection_replace", "after_selection_replace", "health_check"),
    )
    inspect_parser = commands.add_parser("inspect-state", help="inspect the fake rehearsal state")
    inspect_parser.add_argument("--root", required=True, type=Path)
    rollback_parser = commands.add_parser("rollback-prewrite", help="restore fake prior selection before writes")
    rollback_parser.add_argument("--root", required=True, type=Path)
    return parser


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
            result = rollback_prewrite(args.root)
        else:
            raise D03Error("unsupported operation")
    except D03Error as error:
        print(
            json.dumps(
                {"status": "INCOMPLETE", "evidence_scope": "synthetic_fixture", "simulation_only": True, "error": str(error)},
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