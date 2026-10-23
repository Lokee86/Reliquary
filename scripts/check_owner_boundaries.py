#!/usr/bin/env python3
"""Fail-closed source import ratchet for the Phase 2 durable owner stores.

This is deliberately a bounded lexical check. It indexes the exact Rust modules
named by the source manifest, strips Rust comments/strings and cfg(test) items,
then checks direct crate imports against the accepted component DAG and exact
producer-owned value exceptions. It is not a compiler-resolved call graph.
"""
from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

from check_multiplexing_phase4_coverage import closing, masked

ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / "tools/pitlord/owner-boundaries.json"
DAG = ROOT / "tools/pitlord/convergence-component-dag.json"

PATH_ATTR = re.compile(r'#\s*\[\s*path\s*=\s*"([^"]+)"\s*\]')
ROOT_MODULE = re.compile(
    r'(?m)(?:#\s*\[\s*path\s*=\s*"([^"]+)"\s*\]\s*)?'
    r'(?:(?:pub(?:\([^)]*\))?|crate)\s+)?mod\s+([A-Za-z_][A-Za-z0-9_]*)\s*([;{])'
)
USE = re.compile(r"(?m)(?:pub\s+(?:\([^)]*\)\s*)?)?use\s+([^;]+);", re.S)
ITEM = re.compile(r"\b(?:struct|enum|trait|type|const|static|fn)\s+([A-Za-z_][A-Za-z0-9_]*)")
TEST_CFG = re.compile(r"#\s*\[\s*cfg\s*\(\s*test\s*\)\s*\]")
IDENT = re.compile(r"[A-Za-z_][A-Za-z0-9_]*")


def read_json(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def strip_cfg_test(source: str) -> str:
    """Blank cfg(test) items while preserving offsets and production source."""
    view = masked(source)
    spans: list[tuple[int, int]] = []
    for match in TEST_CFG.finditer(view):
        cursor = match.end()
        while cursor < len(view) and view[cursor].isspace():
            cursor += 1
        brace = view.find("{", cursor)
        semi = view.find(";", cursor)
        if semi >= 0 and (brace < 0 or semi < brace):
            end = semi + 1
        elif brace >= 0:
            end = closing(view, brace) + 1
            if end < len(view) and view[end] == ";":
                end += 1
        else:
            end = match.end()
        spans.append((match.start(), end))
    chars = list(source)
    for start, end in spans:
        chars[start:end] = " " * (end - start)
    return "".join(chars)


def split_use_tree(tree: str, prefix: str = "") -> list[tuple[str, str | None]]:
    """Flatten Rust use trees into (full path, renamed-as) leaves."""
    tree = tree.strip()
    if not tree:
        return []
    depth = 0
    open_brace = -1
    for index, char in enumerate(tree):
        if char == "{":
            if depth == 0:
                open_brace = index
                break
            depth += 1
    if open_brace >= 0 and tree.endswith("}"):
        head = tree[:open_brace].strip()
        if head.endswith("::"):
            head = head[:-2]
        nested_prefix = f"{prefix}{head}::" if head else prefix
        body = tree[open_brace + 1 : -1]
        leaves: list[tuple[str, str | None]] = []
        start = 0
        depth = 0
        for index, char in enumerate(body):
            if char == "{":
                depth += 1
            elif char == "}":
                depth -= 1
            elif char == "," and depth == 0:
                leaves.extend(split_use_tree(body[start:index], nested_prefix))
                start = index + 1
        leaves.extend(split_use_tree(body[start:], nested_prefix))
        return leaves
    leaf, alias = (tree.split(" as ", 1) + [None])[:2] if " as " in tree else (tree, None)
    return [(prefix + leaf.strip(), alias.strip() if alias else None)]


def module_tree(root: Path) -> dict[str, str]:
    """Read crate module declarations to resolve exact module paths to files."""
    modules: dict[str, str] = {}
    visited: set[tuple[Path, str]] = set()

    def walk(file: Path, prefix: str) -> None:
        key = (file.resolve(), prefix)
        if key in visited or not file.is_file():
            return
        visited.add(key)
        source = strip_cfg_test(file.read_text(encoding="utf-8"))
        view = masked(source)
        # Exclude inline modules from this compact module index; the owner file
        # manifest names persistent modules by their actual source path.
        for match in ROOT_MODULE.finditer(view):
            name, ending = match.group(2), match.group(3)
            module = f"{prefix}::{name}" if prefix else name
            if ending != ";":
                continue
            preceding_source = source[:match.start()]
            previous_line = preceding_source.rstrip().splitlines()[-1] if preceding_source.strip() else ""
            attr_match = PATH_ATTR.search(previous_line)
            attr = attr_match.group(1) if attr_match else None
            if attr:
                child = (file.parent / attr).resolve()
            else:
                child = (file.parent / f"{name}.rs").resolve()
                if not child.exists():
                    child = (file.parent / name / "mod.rs").resolve()
            if child.is_file():
                modules[module] = child.relative_to(root).as_posix()
                walk(child, module)

    walk(root / "src/lib.rs", "")
    return modules



def normalize_reference(path: str, namespace: str, modules: dict[str, str]) -> str:
    parts = path.split("::")
    if parts[0] == "crate":
        return "::".join(parts[1:])
    base = namespace.split("::") if namespace else []
    if parts[0] in {"self", "super"}:
        while parts and parts[0] in {"self", "super"}:
            if parts.pop(0) == "super":
                base = base[:-1]
        return "::".join([*base, *parts])
    child = "::".join([*base, parts[0]])
    if child in modules:
        return "::".join([*base, *parts])
    return path


def inline_modules(namespace: str, code: str) -> list[tuple[int, int, str]]:
    result: list[tuple[int, int, str]] = []
    for match in ROOT_MODULE.finditer(code):
        if match.group(3) != "{":
            continue
        start = match.end() - 1
        parent = namespace_at(namespace, result, start)
        name = "::".join(filter(None, [parent, match.group(2)]))
        result.append((start, closing(code, start), name))
    return result


def namespace_at(namespace: str, spans: list[tuple[int, int, str]], offset: int) -> str:
    for start, end, name in reversed(spans):
        if start < offset < end:
            return name
    return namespace


def export_resolver(modules: dict[str, str]):
    """Resolve explicit and wildcard reexports to their defining source module."""
    aliases: dict[str, str] = {}
    stars: dict[str, list[str]] = {}
    members: dict[str, set[str]] = {}
    lexical_modules = dict(modules)
    sources = {}
    for namespace, path in {"": "src/lib.rs", **modules}.items():
        code = masked(strip_cfg_test((ROOT / path).read_text(encoding="utf-8")))
        spans = inline_modules(namespace, code)
        sources[namespace] = (code, spans)
        for _start, _end, child in spans:
            lexical_modules[child] = path
    for namespace, (code, spans) in sources.items():
        for match in ITEM.finditer(code):
            context = namespace_at(namespace, spans, match.start())
            members.setdefault(context, set()).add(match.group(1))
        for match in re.finditer(r"(?m)pub(?:\s*\([^)]*\))?\s+use\s+([^;]+);", code, re.S):
            context = namespace_at(namespace, spans, match.start())
            for path, alias in split_use_tree(match.group(1)):
                target = normalize_reference(path, context, lexical_modules)
                name = alias or target.split("::")[-1]
                if name == "*":
                    stars.setdefault(context, []).append(target[:-3])
                else:
                    key = "::".join(filter(None, [context, name]))
                    aliases[key] = target

    def has_member(namespace: str, name: str, seen: frozenset[str] = frozenset()) -> bool:
        key = "::".join(filter(None, [namespace, name]))
        if key in aliases or key in lexical_modules or name in members.get(namespace, set()):
            return True
        if namespace in seen:
            return False
        return any(has_member(target, name, seen | {namespace}) for target in stars.get(namespace, []))

    def resolve(path: str, seen: frozenset[str] = frozenset()) -> str:
        if path in seen:
            return path
        parts = path.split("::")
        for count in range(len(parts), 0, -1):
            prefix = "::".join(parts[:count])
            # A suffix selects the module namespace, while a bare name may
            # select a reexported value with the same name (imports::prepare).
            # Resolve the value once, without expanding its module prefix.
            if prefix in aliases and not (count < len(parts) and prefix in lexical_modules):
                target = "::".join([aliases[prefix], *parts[count:]])
                if target != path:
                    return resolve(target, seen | {path})
        for count in range(len(parts)):
            namespace = "::".join(parts[:count])
            targets = [target for target in stars.get(namespace, []) if has_member(target, parts[count])]
            if len(targets) == 1:
                return resolve("::".join([targets[0], *parts[count:]]), seen | {path})
        return path

    return resolve, lexical_modules


def root_module_of(path: str) -> str:
    return Path(path).stem


def parse_contract(manifest: dict) -> tuple[dict[str, str], dict[str, set[str]]]:
    """Return exact allowed producer value symbols by consumer and owner."""
    contract: dict[str, set[str]] = {}
    for row in manifest.get("producer_value_contracts", []):
        consumer = row["consumer"]
        owner = row["owner"]
        symbol = row["symbol"]
        contract.setdefault(consumer, set()).add(f"{owner}:{symbol}")
    return {}, contract


def main_check(manifest: dict, dag: dict) -> list[str]:
    errors: list[str] = []
    components = {row["id"] for row in dag.get("components", [])}
    dependencies = {
        row["id"]: {edge["to"] for edge in row.get("depends_on", [])}
        for row in dag.get("components", [])
    }
    owners = manifest.get("owner_files", [])
    if manifest.get("version") != 1 or manifest.get("dag") != "convergence-component-dag.json":
        return ["unsupported owner-boundary manifest version or DAG reference"]
    if not owners:
        return ["owner-boundary manifest has no owner files"]

    module_files = module_tree(ROOT)
    modules_by_file = {path: module for module, path in module_files.items()}
    module_owner: dict[str, str] = {}
    path_owner: dict[str, str] = {}
    for row in owners:
        path = row["path"]
        component = row["component"]
        if component not in components:
            errors.append(f"{path}: unknown target component {component}")
            continue
        if path not in modules_by_file:
            errors.append(f"{path}: source file is not reachable from src/lib.rs module declarations")
            continue
        if row.get("classification") not in {"production", "test"}:
            errors.append(f"{path}: classification must be production or test")
        path_owner[path] = component
        if row.get("classification") == "production":
            module_owner[modules_by_file[path]] = component
    for row in manifest.get("module_components", []):
        module, component = row.get("module"), row.get("component")
        if module not in module_files:
            errors.append(f"module component map references unknown module `{module}`")
        elif component not in components:
            errors.append(f"module component map uses unknown component `{component}`")
        elif module in module_owner and module_owner[module] != component:
            errors.append(f"module `{module}` has conflicting component ownership")
        else:
            module_owner[module] = component

    # A new child of a durable owner must be covered, or explicitly classified
    # as higher-level composition. Parent ownership must not silently hide it.
    explicit_modules = {row["module"] for row in manifest.get("module_components", [])}
    for module, path in module_files.items():
        if path in path_owner or module in explicit_modules:
            continue
        ancestors = module.split("::")[:-1]
        while ancestors:
            ancestor = "::".join(ancestors)
            if ancestor in module_owner and module_files.get(ancestor) in path_owner:
                errors.append(f"{path}: owner child module `{module}` has no explicit source classification")
                break
            ancestors.pop()

    contract_symbols: dict[str, set[str]] = {}
    for row in manifest.get("producer_value_contracts", []):
        consumer, owner, symbol = row.get("consumer"), row.get("owner"), row.get("symbol")
        if consumer not in components or owner not in components or not symbol:
            errors.append(f"invalid producer value contract: {row}")
            continue
        if consumer == owner:
            errors.append(f"producer value contract is not cross-owner: {row}")
        if symbol[0].islower() or symbol.endswith(("Store", "OpenState", "Codec")) or symbol in {"Archive", "Cva", "Phylactery", "EmbeddingEndpoint"}:
            errors.append(f"producer value contract names implementation authority rather than an immutable value: {row}")
        contract_symbols.setdefault(consumer, set()).add(f"{owner}:{symbol}")

    seen_external_contracts: set[tuple[str, str]] = set()
    for row in manifest.get("external_value_contracts", []):
        consumer, path = row.get("consumer"), row.get("path", "")
        key = (consumer, path)
        if consumer not in components or not re.fullmatch(r"[a-zA-Z_]\w*::[A-Z]\w*", path) or path.split("::")[0] in module_files or key in seen_external_contracts:
            errors.append(f"invalid external immutable value contract: {row}")
        seen_external_contracts.add(key)

    resolve_export, lexical_modules = export_resolver(module_files)
    module_index = module_files
    symbol_owners: dict[str, set[str]] = {}
    root_exports: dict[str, str] = {}
    # Public root reexports resolve crate::Value to their actual module owner.
    lib_source = strip_cfg_test((ROOT / "src/lib.rs").read_text(encoding="utf-8"))
    for match in re.finditer(r"(?m)pub(?:\s*\([^)]*\))?\s+use\s+([^;]+);", masked(lib_source), re.S):
        for full_path, _alias in split_use_tree(match.group(1)):
            segments = full_path.split("::")
            if not segments or segments[0] == "crate":
                segments = segments[1:]
            symbol = (_alias or segments[-1]) if segments else ""
            module_path = "::".join(segments[:-1])
            target = module_owner.get(module_path)
            if symbol:
                root_exports[symbol] = full_path
            if target and symbol and symbol != "*":
                symbol_owners.setdefault(symbol, set()).add(target)
    # Names defined in exact owner modules also resolve root-level crate imports.
    for path, component in path_owner.items():
        if component not in module_owner.values():
            continue
        if not path.endswith(".rs") or path_owner.get(path) != component:
            continue
        module = modules_by_file.get(path)
        if not module:
            continue
        source = strip_cfg_test((ROOT / path).read_text(encoding="utf-8"))
        for match in ITEM.finditer(masked(source)):
            symbol_owners.setdefault(match.group(1), set()).add(component)

    root_definitions = {match.group(1) for match in ITEM.finditer(masked(lib_source))}
    forbidden_symbols = set(manifest.get("forbidden_symbols", []))
    # Keep forbidden facade/runtime identity forbidden through root aliases,
    # including aliases whose original module is outside the bounded owner map.
    changed = True
    while changed:
        changed = False
        for match in re.finditer(r"(?m)pub(?:\s*\([^)]*\))?\s+use\s+([^;]+);", masked(lib_source), re.S):
            for full_path, alias in split_use_tree(match.group(1)):
                if alias and full_path.split("::")[-1] in forbidden_symbols and alias not in forbidden_symbols:
                    forbidden_symbols.add(alias)
                    changed = True
    forbidden_modules = set(manifest.get("forbidden_modules", []))
    forbidden_prefixes = tuple(manifest.get("forbidden_module_prefixes", []))
    owner_entries = [row for row in owners if row.get("classification") == "production"]
    for row in owner_entries:
        source_path = row["path"]
        source_component = row["component"]
        source = strip_cfg_test((ROOT / source_path).read_text(encoding="utf-8"))
        code = masked(source)
        # Fully qualified calls/types are dependencies too, even without a use item.
        use_matches = list(USE.finditer(code))
        spans = inline_modules(modules_by_file[source_path], code)
        trees = [(match.group(1).strip(), match.start()) for match in use_matches]
        trees.extend((match.group(0), match.start()) for match in re.finditer(r"\b(?:crate|super|self)\s*::\s*[A-Za-z_]\w*(?:\s*::\s*[A-Za-z_]\w*)*", code)
                     if not any(item.start() <= match.start() < item.end() for item in use_matches))
        for tree, offset in trees:
            tree = re.sub(r"\s*::\s*", "::", tree)
            leaves = split_use_tree(tree)
            if not leaves:
                continue
            for full_path, _alias in leaves:
                # A renamed crate root can hide arbitrary later references from
                # this bounded scanner. Require explicit producer paths instead.
                if full_path == "crate" and _alias:
                    errors.append(f"{source_path}: crate-root alias `{_alias}` is not an explicit owner contract")
                    continue
                segments = [segment for segment in full_path.split("::") if segment]
                if not segments:
                    continue
                context = namespace_at(modules_by_file[source_path], spans, offset)
                normalized = normalize_reference("::".join(segments), context, lexical_modules)
                # Bare third-party imports are outside the crate ownership graph.
                if segments[0] not in {"crate", "super", "self"} and normalized == "::".join(segments) and segments[0] not in module_files:
                    continue
                canonical = resolve_export(normalized)
                segments = ["crate", *canonical.split("::")]
                imported = segments[-1]
                if imported == "*":
                    imported = ""
                if any(segment in forbidden_symbols for segment in segments[1:]):
                    errors.append(f"{source_path}: forbidden owner/service import `{full_path}`")
                    continue
                crate_path = segments[1:]
                target_component = None
                used_module = None
                for count in range(len(crate_path), 0, -1):
                    candidate = "::".join(crate_path[:count])
                    if candidate in module_owner:
                        target_component = module_owner[candidate]
                        used_module = candidate
                        break
                if target_component is None and crate_path:
                    owners_for_symbol = symbol_owners.get(crate_path[0], set())
                    if len(owners_for_symbol) == 1:
                        target_component = next(iter(owners_for_symbol))
                if target_component is None:
                    if any(row.get("consumer") == source_component and (canonical == row.get("path") or canonical.startswith(row.get("path", "") + "::")) for row in manifest.get("external_value_contracts", [])):
                        continue
                    # Module roots known to be facades/runtime/service code are
                    # forbidden even when their exact symbol isn't in the DAG.
                    if crate_path and (
                        crate_path[0] in forbidden_modules
                        or any(crate_path[0].startswith(prefix) for prefix in forbidden_prefixes)
                    ):
                        errors.append(f"{source_path}: forbidden service module import `{full_path}`")
                    elif crate_path and crate_path[0] in root_exports:
                        original = root_exports[crate_path[0]]
                        if not any(row.get("consumer") == source_component and row.get("path") == original for row in manifest.get("external_value_contracts", [])):
                            errors.append(f"{source_path}: unclassified root reexport `{full_path}` from `{original}`")
                    elif crate_path and crate_path[0] in root_definitions:
                        errors.append(f"{source_path}: unclassified crate-root declaration `{full_path}`")
                    elif crate_path and any(module == crate_path[0] or module.startswith(crate_path[0] + "::") for module in module_files):
                        errors.append(f"{source_path}: unclassified source module import `{full_path}`")
                    else:
                        errors.append(f"{source_path}: unresolved crate import `{full_path}` requires explicit ownership")
                    continue
                if target_component == source_component:
                    continue
                if target_component in dependencies.get(source_component, set()):
                    continue
                contract_key = f"{target_component}:{imported}"
                if imported and any(f"{target_component}:{symbol}" in contract_symbols.get(source_component, set()) for symbol in crate_path):
                    continue
                if imported and (imported.endswith("Store") or "Codec" in imported or "Rebuild" in imported):
                    errors.append(
                        f"{source_path}: sibling implementation import `{full_path}` "
                        f"({source_component} -> {target_component})"
                    )
                else:
                    errors.append(
                        f"{source_path}: unallowlisted owner import `{full_path}` "
                        f"({source_component} -> {target_component})"
                    )
    return sorted(set(errors))


def fixture_check() -> None:
    # Isolated minimal model of grouped, aliased, upward and reexport imports.
    def detect(tree: str, modules: dict[str, str], symbols: dict[str, str]) -> set[str]:
        found = set()
        for path, _alias in split_use_tree(tree):
            segments = path.split("::")
            if segments[0] != "crate":
                continue
            for size in range(len(segments) - 1, 0, -1):
                module = "::".join(segments[1 : size + 1])
                if module in modules:
                    found.add(modules[module])
                    break
            else:
                if segments[-1] in symbols:
                    found.add(symbols[segments[-1]])
        return found

    modules = {"memory_store": "memory-store", "retrieval": "retrieval"}
    symbols = {"MemoryAlias": "memory-store"}
    assert "memory-store" in detect("crate::{memory_store::MemoryStore as M}", modules, symbols)
    assert "memory-store" in detect("crate::memory_store::MemoryStore as M", modules, symbols)
    assert "retrieval" in detect("crate::retrieval::Retriever", modules, symbols)
    assert "memory-store" in detect("crate::MemoryAlias", modules, symbols)
    assert not detect('crate::memory_store::MemoryStore // comment', modules, symbols) == set()
    source = 'use crate::Cva; // use crate::MemoryStore;\nlet s = "use crate::Phylactery;";'
    assert "Cva" in masked(source) and "MemoryStore" not in masked(source)



def production_path_fixture_check() -> None:
    """Exercise the actual scanner and policy, including acyclic forbidden edges."""
    import tempfile
    global ROOT
    original_root = ROOT
    temporary_root = original_root / ".verification-tmp"
    temporary_root.mkdir(exist_ok=True)
    if not temporary_root.resolve().is_relative_to(original_root.resolve()):
        raise ValueError("fixture temporary directory must resolve inside the active repository")
    try:
        with tempfile.TemporaryDirectory(prefix="owner-boundary-", dir=temporary_root) as directory:
            ROOT = Path(directory)
            (ROOT / "src").mkdir()
            (ROOT / "src/lib.rs").write_text(
                "mod owner; mod memory; mod service; mod unknown; mod bridge;\n"
                "pub use service::Cva as Facade; pub use memory::MemoryId;\n"
                "pub(crate) use unknown::Cva as Secret; pub use unknown::run as Helper;\n"
                "pub fn root_helper() {} pub use unknown::*;\n",
                encoding="utf-8",
            )
            (ROOT / "src/memory.rs").write_text("pub struct MemoryStore; pub struct MemoryId;", encoding="utf-8")
            (ROOT / "src/service.rs").write_text("pub struct Cva; pub fn service_work() {}", encoding="utf-8")
            (ROOT / "src/unknown.rs").write_text("pub fn run() {}", encoding="utf-8")
            (ROOT / "src/bridge.rs").write_text("pub use crate::service::service_work as Work; pub use crate::memory::MemoryId as Id; pub use crate::service::*; mod inner { pub use crate::service::service_work as Imported; pub use crate::memory::MemoryId as Value; } pub use inner::Imported as Nested; pub use inner::Value as NestedId;", encoding="utf-8")
            # Module/value namespace overlap must terminate and preserve the
            # producer module for calls through either public spelling.
            with (ROOT / "src/lib.rs").open("a", encoding="utf-8") as library:
                library.write("mod prepare; pub use prepare::prepare;\n")
            (ROOT / "src/prepare.rs").write_text("pub fn prepare() {}", encoding="utf-8")
            resolve, _ = export_resolver({"prepare": "src/prepare.rs"})
            assert resolve("prepare") == "prepare::prepare"
            assert resolve("prepare::prepare") == "prepare::prepare"
            library_path = ROOT / "src/lib.rs"
            original_library = library_path.read_text(encoding="utf-8")
            library_path.write_text(original_library.replace(
                "pub use prepare::prepare;", "pub use service::service_work as prepare;"
            ), encoding="utf-8")
            resolve, _ = export_resolver({"prepare": "src/prepare.rs", "service": "src/service.rs"})
            assert resolve("prepare") == "service::service_work"
            assert resolve("prepare::prepare") == "prepare::prepare"
            library_path.write_text(original_library, encoding="utf-8")
            manifest = {
                "version": 1, "dag": "convergence-component-dag.json",
                "owner_files": [{"path": "src/owner.rs", "component": "archive-store", "classification": "production"}],
                "module_components": [
                    {"module": "memory", "component": "memory-store"},
                    {"module": "service", "component": "rel-facade"},
                    {"module": "bridge", "component": "archive-store"},
                ],
                "forbidden_symbols": ["Cva"],
                "producer_value_contracts": [{"consumer": "archive-store", "owner": "memory-store", "symbol": "MemoryId"}],
            }
            dag = {"components": [{"id": name, "depends_on": []} for name in ["archive-store", "memory-store", "rel-facade"]]}
            cases = [
                ("use crate as root; fn invalid() { root::memory::MemoryStore::new(); }", True),
                ("use crate as root; use root::service::Cva;", True),
                ("use crate::memory::MemoryId;", False),
                ("use crate::MemoryId;", False),
                ("use crate::bridge::Id;", False),
                ("use crate::bridge::NestedId;", False),
                ("fn invalid() { crate::bridge::Nested(); }", True),
                ("use crate::bridge::Work;", True),
                ("fn invalid() { crate::bridge::service_work(); }", True),
                ('// use crate::Cva;\nconst S: &str = "crate::service::Cva";', False),
                ("#[cfg(test)] mod tests { use crate::Cva; }", False),
                ("use crate::{memory::MemoryStore as Foreign};", True),
                ("use super::memory::MemoryStore;", True),
                ("fn invalid() { crate::memory::MemoryStore::new(); }", True),
                ("use crate::Facade;", True),
                ("fn invalid() { crate::Helper(); }", True),
                ("fn invalid() { crate::root_helper(); }", True),
                ("fn invalid() { crate::run(); }", True),
                ("use crate::Secret;", True),
                ("fn invalid() { crate::Cva::create(); }", True),
                ("fn invalid() { crate::Secret::create(); }", True),
                ("use crate::service::Cva;", True),
                ("fn invalid() { crate::unknown::run(); }", True),
            ]
            for source, rejected in cases:
                (ROOT / "src/owner.rs").write_text(source, encoding="utf-8")
                errors = main_check(manifest, dag)
                assert bool(errors) == rejected, (source, errors)
            manifest["producer_value_contracts"].append({"consumer": "archive-store", "owner": "memory-store", "symbol": "MemoryStore"})
            (ROOT / "src/owner.rs").write_text("use crate::memory::MemoryStore;", encoding="utf-8")
            assert any("implementation authority" in error for error in main_check(manifest, dag))
            manifest["producer_value_contracts"].pop()
            # Every migrated owner uses the same default-deny evaluation;
            # none of these forbidden edges needs a cycle to be rejected.
            for consumer in ["graph-store", "perception-store", "freshness-store", "vector-store"]:
                dag["components"].append({"id": consumer, "depends_on": []})
                manifest["owner_files"][0]["component"] = consumer
                (ROOT / "src/owner.rs").write_text("use crate::memory::MemoryStore;", encoding="utf-8")
                assert main_check(manifest, dag), consumer
            # History may read lower owner facts, but must not gain physical,
            # facade, Import coordination or Maintenance service authority.
            with (ROOT / "src/lib.rs").open("a", encoding="utf-8") as library:
                library.write("mod physical; mod import_coordinator; mod maintenance;\n")
            for module, component, definition in [
                ("physical", "container", "pub struct Container;"),
                ("import_coordinator", "import", "pub fn complete_epoch() {}"),
                ("maintenance", "maintenance", "pub fn authorize() {}"),
            ]:
                (ROOT / f"src/{module}.rs").write_text(definition, encoding="utf-8")
                manifest["module_components"].append({"module": module, "component": component})
                dag["components"].append({"id": component, "depends_on": []})
            dag["components"].append({"id": "history", "depends_on": [{"to": "memory-store"}]})
            manifest["owner_files"][0]["component"] = "history"
            for source, rejected in [
                ("use crate::physical::Container as Reader;", True),
                ("use crate::import_coordinator::complete_epoch as Inspect;", True),
                ("fn invalid() { crate::maintenance::authorize(); }", True),
                ("use crate::service::Cva;", True),
                ("use crate::memory::MemoryStore;", False),
            ]:
                (ROOT / "src/owner.rs").write_text(source, encoding="utf-8")
                errors = main_check(manifest, dag)
                assert bool(errors) == rejected, ("history", source, errors)
            # Import consumes scoped lifecycle operations and History facts,
            # never facade, Container, or generic Maintenance policy. Exercise
            # the forbidden dependencies through renamed imports as well.
            dag["components"] = [row if row["id"] != "import" else {"id": "import", "depends_on": []} for row in dag["components"]]
            manifest["owner_files"][0]["component"] = "import"
            for source in [
                "use crate::physical::Container as ImportReader;",
                "use crate::service::Cva as NativeOwner;",
                "fn invalid() { crate::maintenance::authorize(); }",
            ]:
                (ROOT / "src/owner.rs").write_text(source, encoding="utf-8")
                assert main_check(manifest, dag), ("import", source)
            manifest["owner_files"][0]["component"] = "archive-store"
            (ROOT / "src/child.rs").write_text("pub struct Hidden;", encoding="utf-8")
            (ROOT / "src/owner.rs").write_text('#[path = "child.rs"] mod child;', encoding="utf-8")
            assert any("no explicit source classification" in error for error in main_check(manifest, dag))
    finally:
        ROOT = original_root


def main() -> int:
    parser = argparse.ArgumentParser(description="Check Phase 2 owner source boundaries.")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        fixture_check()
        production_path_fixture_check()
        print("Owner boundary checker fixtures passed")
        return 0
    errors = main_check(read_json(MANIFEST), read_json(DAG))
    if errors:
        print("\n".join(errors), file=sys.stderr)
        print(f"Owner boundary checker: {len(errors)} source finding(s)", file=sys.stderr)
        return 1
    print("Owner boundary checker: production owner imports passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
