#!/usr/bin/env python3
"""Check root entry-point links outside the shared checker's docs_root."""
from __future__ import annotations

import sys
from pathlib import Path

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / ".standards" / "docs_policy"))
from markdown import local_target, markdown_targets  # noqa: E402


def main() -> int:
    errors = []
    for name in ("README.md", "AGENTS.md"):
        source = ROOT / name
        for target in markdown_targets(source.read_text(encoding="utf-8")):
            candidate = local_target(source, target, ROOT)
            if candidate is not None and not candidate.exists():
                errors.append(f"{name}: broken link: {target}")
    for error in errors:
        print(error)
    if not errors:
        print("Root documentation entry-point links passed")
    return int(bool(errors))


if __name__ == "__main__":
    raise SystemExit(main())
