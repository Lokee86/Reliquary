#!/usr/bin/env python3
"""Bounded effect guard for the canonical read-only rewrite planner.

The owner checker separately enforces dependencies. These exact planner files
must not acquire a physical writer, facade, provider or publication operation.
Private pure lookup closures within the planner are deliberately allowed.
"""
from __future__ import annotations

import argparse
import re
from pathlib import Path

from check_history_boundaries import FILE_WRITE, WRITE_CALL
from check_multiplexing_phase4_coverage import masked
from check_owner_boundaries import strip_cfg_test

ROOT = Path(__file__).resolve().parents[1]
FILES = (
    "src/maintenance/rewrite/plan.rs",
    "src/maintenance/rewrite/plan_source.rs",
    "src/maintenance/rewrite/closure.rs",
    "src/maintenance/rewrite/archive_selection.rs",
)
CAPABILITY = re.compile(
    r"\b(?:Cva|Phylactery|ReliquaryOwner|PhylacteryOwner|ImportSession|EmbeddingEndpoint)\b"
    r"|&\s*mut\s+(?:[A-Za-z_]\w*::)*Container\b"
)


def findings(source: str) -> list[str]:
    code = masked(strip_cfg_test(source))
    return [match.group(0) for pattern in (WRITE_CALL, FILE_WRITE, CAPABILITY)
            for match in pattern.finditer(code)]


def self_test() -> None:
    for source in (
        "fn bad(c: &mut Container) {}",
        "fn bad(s: &Store) { s.sync(); }",
        "fn bad(s: &mut Store) { s.append(record); }",
        "fn bad() { std::fs::write(path, bytes); }",
        "use crate::Cva as Source;",
        "fn bad(source: ReliquaryOwner) {}",
        "fn bad(endpoint: &impl EmbeddingEndpoint) {}",
    ):
        assert findings(source), source
    for source in (
        "fn read(c: &Container) { c.read(object); }",
        "fn pure(lookup: impl FnMut(u64) -> u64) {}",
        '// c.append(x);\nlet s = "c.sync()";',
        "fn read(s: &Store) { s.pruned_content_markers(); }",
    ):
        assert not findings(source), source


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        self_test()
        print("Rewrite read-only planner fixtures passed")
        return 0
    errors = [f"{name}: forbidden planner capability/effect `{item}`"
              for name in FILES
              for item in findings((ROOT / name).read_text(encoding="utf-8"))]
    if errors:
        print("\n".join(errors))
        return 1
    print("Rewrite read-only planner boundary passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
