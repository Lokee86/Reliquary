#!/usr/bin/env python3
"""Narrow read-only guard for production historical proof modules.

The component DAG check remains authoritative for imports. This additional
guard rejects explicit persistence/publication API calls from production
History code; it is intentionally not a general Rust effect analyzer.
"""
from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

import check_owner_boundaries as owners
from check_multiplexing_phase4_coverage import masked

ROOT = Path(__file__).resolve().parents[1]
HISTORY = ROOT / "src/history"

# Exact write/side-effect verbs used by owner and publication APIs. Keep this
# list narrow: local proof builders (for example record_fragment) are allowed.
WRITE_CALL = re.compile(
    r"\b(?:append|append_[A-Za-z0-9_]*|publish[A-Za-z0-9_]*|sync|"
    r"repair(?:_[A-Za-z0-9_]*)?|prune(?:_[A-Za-z0-9_]*)?|reclaim(?:_[A-Za-z0-9_]*)?|"
    r"commit[A-Za-z0-9_]*|allocate_version|record_accepted_memory_use|"
    r"put_[A-Za-z0-9_]*|write_all|write_chunk|write_frame|write_at|delete_[A-Za-z0-9_]*)\s*\("
)
FILE_WRITE = re.compile(
    r"\b(?:std::)?fs::(?:write|remove_file|rename|copy|create_dir[A-Za-z0-9_]*)\s*\("
    r"|\b(?:File|OpenOptions)::(?:create|options|new)\s*\("
)
CALLBACK_DECL = re.compile(r"\b(?:EmbeddingEndpoint|Fn|FnMut|FnOnce)\b")


def production_files() -> list[Path]:
    # Resolve from the crate module tree so detached/unreachable files cannot
    # silently enter the protected set and inline tests remain cfg(test)-only.
    modules = owners.module_tree(ROOT)
    files = {
        (ROOT / path).resolve()
        for path in modules.values()
        if path.startswith("src/history/")
    }
    return sorted(path for path in files if path.is_file())


def violations(path: Path, source: str) -> list[str]:
    code = masked(owners.strip_cfg_test(source))
    findings = []
    for match in WRITE_CALL.finditer(code):
        findings.append(
            f"{path.relative_to(ROOT).as_posix()}:{match.start()}: forbidden owner write call `{match.group(0).strip()}`"
        )
    for match in FILE_WRITE.finditer(code):
        findings.append(
            f"{path.relative_to(ROOT).as_posix()}:{match.start()}: file mutation `{match.group(0).strip()}` in production History"
        )
    for match in CALLBACK_DECL.finditer(code):
        findings.append(
            f"{path.relative_to(ROOT).as_posix()}:{match.start()}: provider/callback capability `{match.group(0)}` in production History"
        )
    return findings


def actual_check() -> list[str]:
    findings: list[str] = []
    for path in production_files():
        findings.extend(violations(path, path.read_text(encoding="utf-8")))
    # Reuse the existing exact module resolver, alias handling, cfg(test)
    # stripping and accepted owner DAG. This keeps direct and renamed imports
    # under the same default-deny rules as the production owner boundary.
    import json

    manifest = json.loads((ROOT / "tools/pitlord/owner-boundaries.json").read_text(encoding="utf-8"))
    dag = json.loads((ROOT / "tools/pitlord/convergence-component-dag.json").read_text(encoding="utf-8"))
    findings.extend(owners.main_check(manifest, dag))
    return findings


def self_test() -> list[str]:
    fixtures = {
        "write method": "fn bad(store: &mut Store) { store.append(chunk); }",
        "qualified writer": "fn bad(c: &mut Container) { Container::sync(c); }",
        "provider callback": "fn bad<E: EmbeddingEndpoint>(endpoint: E) {}",
        "shared callback trait": "fn bad(f: impl Fn()) {}",
        "file write": 'fn bad() { std::fs::write(path, bytes); }',
        "file creation": 'fn bad() { File::create(path); }',
        "pruned evidence getters": 'fn ok(s: &Store) { s.pruned_body_markers(); s.pruned_content_markers(); }',
        "prune writer": 'fn bad(s: &mut Store) { s.prune_tombstoned_history(p); }',
        "local read builder": 'fn ok(v: &mut Vec<u8>) { v.push(1); v.extend([2]); }',
        "callback trait": "fn bad(f: impl FnMut()) {}",
        "comment/string masking": '// store.append(x);\nlet s = "container.sync()"; fn ok() {}',
    }
    errors = []
    for name, fixture in fixtures.items():
        found = violations(ROOT / "src/history/fixture.rs", fixture)
        if name in {"comment/string masking", "local read builder", "pruned evidence getters"}:
            if found:
                errors.append(f"self-test {name}: false positive {found}")
        elif not found:
            errors.append(f"self-test {name}: expected rejection")
    return errors


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    errors = self_test() if args.self_test else actual_check()
    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print("history boundary self-test passed" if args.self_test else "history read-only boundary passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
