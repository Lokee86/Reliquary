#!/usr/bin/env python3
"""Fail closed on changes to Reliquary's selected production package DAG."""
from __future__ import annotations

import json
import re
import sys
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SPEC = ROOT / "architecture/target-crate-dag-2026-11-03.json"
planned = json.loads(SPEC.read_text(encoding="utf-8"))
expected = {item["id"]: set(item["depends_on"]) for item in planned["packages"]}
actual = {}
dev_edges = {}
errors = []
for manifest in [ROOT / "Cargo.toml", *ROOT.glob("crates/reliquary-*/Cargo.toml")]:
    data = tomllib.loads(manifest.read_text(encoding="utf-8"))
    name = data.get("package", {}).get("name")
    if not name:
        continue
    actual[name] = {dep for dep in data.get("dependencies", {}) if dep.startswith("reliquary-")}
    dev_edges[name] = {dep for dep in data.get("dev-dependencies", {}) if dep.startswith("reliquary-")}
for name in sorted(set(actual) ^ set(expected)):
    errors.append(f"package membership mismatch: {name}")
for name in sorted(set(actual) & set(expected)):
    for dep in sorted(actual[name] - expected[name]):
        errors.append(f"unapproved production import: {name} -> {dep}")
    for dep in sorted(expected[name] - actual[name]):
        errors.append(f"missing selected production edge: {name} -> {dep}")
    for dep in sorted(actual[name]):
        if dep not in actual:
            errors.append(f"dangling local dependency: {name} -> {dep}")
visiting = set()
visited = set()
def visit(name: str) -> None:
    if name in visiting:
        errors.append(f"production dependency cycle at {name}")
        return
    if name in visited:
        return
    visiting.add(name)
    for dep in actual.get(name, ()):
        if dep in actual:
            visit(dep)
    visiting.remove(name)
    visited.add(name)
for package in actual:
    visit(package)
# Test-only fixtures may depend on lower libraries. They do not authorize
# importing them from application/Gateway production source.
for package, banned in {
    "reliquary-application": {
        "archive", "graph", "memory_core", "perception_store",
        "vector_store", "freshness_store", "insomnia_store", "container",
    },
    "reliquary-runtime-gateway": {
        "archive", "graph", "memory_core", "perception_store",
        "vector_store", "freshness_store", "insomnia_store", "container",
    },
}.items():
    for source in (ROOT / "crates" / package / "src").rglob("*.rs"):
        if source.name in ("tests.rs", "archive_tests.rs"):
            continue
        content = source.read_text(encoding="utf-8").split("\n#[cfg(test)]", 1)[0]
        for forbidden in sorted(banned):
            if re.search(rf"\breliquary_{forbidden}\s*::", content):
                errors.append(f"direct lower-owner reference in {source.relative_to(ROOT)}: reliquary_{forbidden}")
print(f"Production DAG: {len(actual)} packages, {sum(map(len,actual.values()))} edges; expected {sum(map(len,expected.values()))}.")
print(f"Test-only dependency edges: {sum(map(len,dev_edges.values()))}; not part of production DAG.")
for error in errors:
    print(f"ARCHITECTURE ERROR: {error}", file=sys.stderr)
sys.exit(bool(errors))
