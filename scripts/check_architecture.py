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
try:
    workspace_data = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))
except (OSError, tomllib.TOMLDecodeError) as error:
    workspace_data = {}
    errors.append(f"cannot read workspace Cargo manifest: {error}")
workspace_dependencies = workspace_data.get("workspace", {}).get("dependencies", {})
manifests = [ROOT / "Cargo.toml", *ROOT.glob("crates/reliquary-*/Cargo.toml")]
manifest_data = {}
package_manifests = {}
for manifest in manifests:
    try:
        data = tomllib.loads(manifest.read_text(encoding="utf-8"))
    except (OSError, tomllib.TOMLDecodeError) as error:
        errors.append(f"cannot read Cargo manifest {manifest.relative_to(ROOT)}: {error}")
        continue
    name = data.get("package", {}).get("name")
    if name:
        manifest_data[manifest] = data
        if name in package_manifests:
            errors.append(f"duplicate internal package manifest: {name}")
        package_manifests[name] = manifest.resolve()


def local_package_name(dependency: str, declaration: object, base: Path, owner: str) -> str | None:
    """Resolve Cargo aliases to the package the dependency actually names."""
    if isinstance(declaration, str):
        package = dependency
        path = None
    elif isinstance(declaration, dict):
        package = declaration.get("package", dependency)
        path = declaration.get("path")
    else:
        errors.append(f"unsupported dependency declaration: {dependency}")
        return None
    if path is not None:
        target_manifest = (base / path / "Cargo.toml").resolve()
        try:
            target = tomllib.loads(target_manifest.read_text(encoding="utf-8"))
        except (OSError, tomllib.TOMLDecodeError) as error:
            errors.append(f"cannot resolve local dependency {dependency} at {target_manifest}: {error}")
            return package if isinstance(package, str) and package.startswith("reliquary-") else None
        path_package = target.get("package", {}).get("name")
        explicit_package = isinstance(declaration, dict) and "package" in declaration
        if explicit_package and path_package != package:
            errors.append(
                f"dependency package/path mismatch: {owner} {dependency} declares {package} "
                f"but target manifest names {path_package}"
            )
        if not explicit_package and path_package:
            package = path_package
        if package in package_manifests and target_manifest != package_manifests[package]:
            errors.append(
                f"noncanonical internal package path: {owner} -> {package}; "
                f"expected {package_manifests[package].relative_to(ROOT)}, got {target_manifest}"
            )
    elif package in package_manifests:
        errors.append(f"internal package dependency must use its canonical local path: {owner} -> {package}")
    return package if isinstance(package, str) and package.startswith("reliquary-") else None


def declarations(manifest_data: dict, manifest: Path, section: str) -> set[str]:
    """Collect normal or dev dependencies, including target-conditioned tables."""
    package_root = manifest.parent
    owner = manifest_data.get("package", {}).get("name", str(manifest.relative_to(ROOT)))

    def resolve(table: dict, base: Path) -> set[str]:
        found = set()
        for dependency, raw in table.items():
            declaration = raw
            dependency_base = base
            if isinstance(raw, dict) and raw.get("workspace") is True:
                inherited = workspace_dependencies.get(dependency)
                if not isinstance(inherited, (dict, str)):
                    errors.append(f"unresolved workspace dependency: {manifest.relative_to(ROOT)} {dependency}")
                    continue
                overrides = {key: value for key, value in raw.items() if key != "workspace"}
                if isinstance(inherited, dict):
                    declaration = {**inherited, **overrides}
                else:
                    declaration = inherited
                dependency_base = ROOT
            package = local_package_name(dependency, declaration, dependency_base, owner)
            if package:
                found.add(package)
        return found

    found = resolve(manifest_data.get(section, {}), package_root)
    for target in manifest_data.get("target", {}).values():
        if isinstance(target, dict):
            found.update(resolve(target.get(section, {}), package_root))
    return found


for manifest, data in manifest_data.items():
    name = data["package"]["name"]
    actual[name] = declarations(data, manifest, "dependencies")
    dev_edges[name] = declarations(data, manifest, "dev-dependencies")
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
