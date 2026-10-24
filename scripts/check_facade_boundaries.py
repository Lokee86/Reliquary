#!/usr/bin/env python3
"""Cfg-aware Phase 6 placement ratchet; lexical source proof, not a call graph."""
from __future__ import annotations

import argparse
import json
import re
from pathlib import Path

from check_owner_boundaries import module_tree, resolved_source_imports, strip_inactive_cfg
from check_multiplexing_phase4_coverage import masked

ROOT = Path(__file__).resolve().parents[1]
UPPER = {"public-consumers", "runtime-gateway"}
CONCRETE_READ_CONTRACTS = {"src/perception/entity_audit.rs", "src/external_source/mod.rs"}
VALUE_VARIANTS = {
    "src/container/container.rs": "Phylactery,",
    "src/ego/ego_store.rs": "Phylactery,",
    "src/owner_lifecycle/cva_reconcile_error.rs": "Cva(OwnerError),",
    "src/runtime_inference/insomnia/processor/result.rs": "Phylactery(String),",
}


def violations(path: str, source: str, component: str | None, facts: set[str]) -> list[str]:
    view = masked(strip_inactive_cfg(source, facts))
    errors = []
    if path in CONCRETE_READ_CONTRACTS:
        for hit in re.finditer(r"\bFn(?:Mut|Once)?\b", view):
            errors.append(f"{path}:{view.count(chr(10), 0, hit.start()) + 1}: callback parameter replaces concrete owner read contract")
    # Qualified and aliased imports, macro arguments, and concrete receiver types
    # all retain the forbidden identifier. Error/DTO names are separate tokens.
    if component not in UPPER:
        for hit in re.finditer(r"\b(?:Cva|Phylactery)\b", view):
            # Enum variants describe owner kind/errors, not facade capabilities.
            line = view[view.rfind("\n", 0, hit.start()) + 1:view.find("\n", hit.end())]
            if line.strip() == VALUE_VARIANTS.get(path):
                continue
            if re.search(r"\b(?:Self|[A-Za-z_][A-Za-z0-9_]*(?:Error|Kind|Scope|Target))\s*::\s*$", view[max(0, hit.start()-80):hit.start()]):
                continue
            errors.append(f"{path}:{view.count(chr(10), 0, hit.start()) + 1}: lower facade dependency: {hit.group()}")
    if not path.startswith("src/facade/"):
        if re.search(r"\bimpl\s+ConfiguredRuntime\b", view) and re.search(r"\b(?:Cva|Phylactery)\b", view):
            errors.append(f"{path}: offline public ConfiguredRuntime composition outside facade")
        for hit in re.finditer(r"\bimpl\s+(?:crate\s*::\s*)?(?:Cva|Phylactery)\b|\bimpl_\w+\s*!\s*\(\s*(?:Cva|Phylactery)\b|\bimpl\s+\w+\s+for\s+(?:Cva|Phylactery)\b", view):
            errors.append(f"{path}:{view.count(chr(10), 0, hit.start()) + 1}: public extension outside facade")
    return errors


def self_test() -> None:
    facts = {"windows"}
    for code in ("use crate::Cva;", "use crate::{Cva as Owner, Archive};",
                 "fn f(owner: &mut crate::Phylactery) {}", "impl_owner!(Cva);",
                 "#[cfg(windows)] fn f(owner: &Cva) {}"):
        assert violations("src/memory/example.rs", code, "memory-store", facts), code
    for code in ('// Cva\nconst TEXT: &str = "Phylactery";',
                 "#[cfg(test)] fn f(owner: &Cva) {}",
                 "#[cfg(unix)] fn f(owner: &Cva) {}",
                 "use crate::{CvaError, PhylacteryProfile};"):
        assert not violations("src/memory/example.rs", code, "memory-store", facts), code
    assert violations("src/unknown.rs", "use crate::Cva;", None, facts)
    assert violations("src/retrieval/query.rs", "impl Cva {}", "public-consumers", facts)
    assert violations("src/retrieval/query.rs", "impl_owner!(Cva);", "public-consumers", facts)
    assert violations("src/retrieval/query.rs", "impl Owner for Phylactery {}", "public-consumers", facts)
    assert not violations("src/facade/query.rs", "impl Cva {}", "public-consumers", facts)
    assert violations("src/perception/entity_audit.rs", "fn audit<F: FnMut(MemoryId) -> Vec<EntityId>>(f: F) {}", "perception", facts)
    assert violations("src/external_source/mod.rs", "fn search(read: impl Fn(PackedVectorId) -> PackedVectors) {}", "external-sources", facts)
    assert not violations("src/perception/entity_audit.rs", "fn audit(owner: &EntityAuditContext) {}", "perception", facts)
    assert violations("src/runtime_inference/offline.rs", "impl ConfiguredRuntime { fn run(owner: &mut Cva) {} }", "runtime-gateway", facts)
    print("Facade boundary fixtures passed (18 cases).")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        self_test()
    mapping = json.loads((ROOT / "tools/pitlord/production-components.json").read_text(encoding="utf-8"))
    facts = set(mapping["cfg_profile"]["rustc_cfg_facts"])
    files = {row["path"]: row for row in mapping["files"]}
    errors = []
    paths = sorted(set(module_tree(ROOT, facts).values()))
    for path in paths:
        component = files.get(path, {}).get("component")
        errors.extend(violations(path, (ROOT / path).read_text(encoding="utf-8"), component, facts))
    imports, uncertain = resolved_source_imports(ROOT, facts)
    owner_policy = json.loads((ROOT / "tools/pitlord/owner-boundaries.json").read_text(encoding="utf-8"))
    values = {(row["consumer"], row["owner"], row["symbol"])
              for row in owner_policy["producer_value_contracts"]}
    for row in imports:
        source = files.get(row["source_path"], {}).get("component")
        target = files.get(row["target_path"], {}).get("component")
        if source not in UPPER and target in UPPER and (source, target, row["target_symbol"]) not in values:
            errors.append(f"{row['source_path']}:{row['line']}: lower import of upper composition: {row['import']} -> {row['target_path']}")
    for row in uncertain:
        if files.get(row["source_path"], {}).get("component") not in UPPER:
            # Unknown references to explicitly upper module/type names cannot be
            # silently admitted by an unresolved export or glob expansion.
            if re.search(r"\b(?:Cva|Phylactery|cva_[A-Za-z0-9_]+|phylactery_[A-Za-z0-9_]+)\b", row["import"]):
                errors.append(f"{row['source_path']}:{row['line']}: unresolved possible reverse edge: {row['import']}")
    for error in errors:
        print(error)
    print(f"Facade source guard: {len(paths)} reachable production files, {len(errors)} violations; bounded lexical proof.")
    return int(bool(errors))


if __name__ == "__main__":
    raise SystemExit(main())
