#!/usr/bin/env python3
"""Bounded current-crate MIR call evidence, joined to compiler item source spans.

Inputs are one private rustdoc JSON inventory and one MIR emission from the same
locked, cfg-pinned build. This does not query Arcana or infer calls by same-name
search. Unknown local targets remain explicit and block acceptance.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import posixpath
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def normalize_source_path(path: str) -> str:
    """Normalize rustc MIR's relative source path before component lookup."""
    return posixpath.normpath(path.replace("\\", "/"))


def without_generics(name: str) -> str:
    """Remove compiler turbofish arguments; retain qualified trait receivers."""
    while "::<" in name:
        start = name.index("::<")
        depth = 1
        end = start + 3
        while end < len(name) and depth:
            if name[end] == "<":
                depth += 1
            elif name[end] == ">":
                depth -= 1
            end += 1
        if depth:
            break
        name = name[:start] + name[end:]
    return name.removeprefix("reliquary_memory::")


def compiler_symbols(doc: dict) -> tuple[dict[str, list[dict]], set[str], set[str]]:
    index = doc["index"]
    paths = doc["paths"]
    symbols: dict[str, list[dict]] = {}
    local_roots = set()
    external_roots = {row["name"] for row in doc.get("external_crates", {}).values()
                      if row.get("name")}

    def canonical(item_id: str | int) -> str | None:
        row = paths.get(str(item_id))
        if not row:
            return None
        path = row["path"]
        if row["crate_id"] == 0 and len(path) > 1:
            local_roots.add(path[1])
        return without_generics("::".join(path))

    def add(key: str | None, row: dict) -> None:
        if not key or row.get("crate_id") != 0 or not row.get("span"):
            return
        span = row["span"]
        symbols.setdefault(key, []).append({
            "path": span["filename"].replace("\\", "/"),
            "line": span["begin"][0], "end_line": span["end"][0],
            "symbol": key,
        })

    for item_id, row in index.items():
        if "function" in row.get("inner", {}):
            add(canonical(item_id), row)
        impl = row.get("inner", {}).get("impl")
        if not impl:
            continue
        receiver = impl["for"].get("resolved_path")
        if not receiver:
            continue
        owner = canonical(receiver["id"])
        if not owner:
            continue
        trait = impl.get("trait")
        for method_id in impl["items"]:
            method = index.get(str(method_id))
            if not method or "function" not in method.get("inner", {}):
                continue
            if trait:
                trait_name = canonical(trait["id"])
                symbol = f"<{owner} as {trait_name}>::{method['name']}"
                add(symbol, method)
            else:
                symbol = f"{owner}::{method['name']}"
                add(symbol, method)
    return symbols, local_roots, external_roots


def compiler_drop_implementations(doc: dict) -> dict[str, list[dict]]:
    """Map exact local nominal receiver paths to local `Drop::drop` spans."""
    index, paths = doc["index"], doc["paths"]
    result: dict[str, list[dict]] = {}
    for impl in index.values():
        inner = impl.get("inner", {}).get("impl")
        if not inner or not inner.get("trait") or impl.get("crate_id") != 0:
            continue
        trait = paths.get(str(inner["trait"].get("id")), {})
        if trait.get("crate_id") == 0 or trait.get("path", [])[-1:] != ["Drop"]:
            continue
        receiver = inner.get("for", {}).get("resolved_path")
        if not receiver:
            continue
        target = paths.get(str(receiver.get("id")), {})
        if target.get("crate_id") != 0:
            continue
        full = target.get("path", [])
        if len(full) < 2:
            continue
        key = without_generics("::".join(full))
        for method_id in inner.get("items", []):
            method = index.get(str(method_id), {})
            span = method.get("span")
            if method.get("name") != "drop" or not span:
                continue
            result.setdefault(key, []).append({
                "path": normalize_source_path(span["filename"]),
                "line": span["begin"][0], "end_line": span["end"][0],
                "symbol": f"<{key} as core::ops::Drop>::drop",
            })
    return result


def exact_drop_implementations(drop_type: str | None,
                               implementations: dict[str, list[dict]]) -> list[dict]:
    """Resolve only a direct nominal `Drop` type, never a guessed field path."""
    if not drop_type:
        return []
    rendered = without_generics(drop_type.strip()).removeprefix("reliquary_memory::")
    matches = []
    for nominal, spans in implementations.items():
        name = nominal.removeprefix("reliquary_memory::")
        if rendered == name or rendered.startswith(name + "<"):
            matches.extend(spans)
    return matches


DROP_OWNING_WRAPPER_PATHS = {
    "core::option::Option", "std::option::Option",
    "core::result::Result", "std::result::Result",
    "alloc::vec::Vec", "std::vec::Vec", "alloc::collections::vec_deque::VecDeque",
    "std::collections::VecDeque", "alloc::collections::linked_list::LinkedList",
    "std::collections::LinkedList", "alloc::collections::binary_heap::BinaryHeap",
    "std::collections::BinaryHeap", "alloc::collections::btree::map::BTreeMap",
    "std::collections::BTreeMap", "alloc::collections::btree::set::BTreeSet",
    "std::collections::BTreeSet", "std::collections::HashMap",
    "std::collections::HashSet", "hashbrown::map::HashMap", "hashbrown::set::HashSet",
    "alloc::boxed::Box", "std::boxed::Box", "core::pin::Pin", "std::pin::Pin",
    "alloc::sync::Arc", "std::sync::Arc", "alloc::rc::Rc", "std::rc::Rc",
    "std::sync::Mutex", "std::sync::RwLock", "std::sync::poison::mutex::Mutex",
    "std::sync::poison::rwlock::RwLock", "core::cell::Cell", "std::cell::Cell",
    "core::cell::RefCell", "std::cell::RefCell", "std::sync::OnceLock",
    "std::sync::LazyLock", "core::ops::Range",
}
DROP_NEVER_WRAPPER_PATHS = {
    "core::marker::PhantomData", "std::marker::PhantomData",
    "core::mem::ManuallyDrop", "core::mem::manually_drop::ManuallyDrop",
    "std::mem::ManuallyDrop", "core::mem::MaybeUninit",
    "core::mem::maybe_uninit::MaybeUninit", "std::mem::MaybeUninit",
}
PRIMITIVE_TYPES = {
    "bool", "char", "str", "u8", "u16", "u32", "u64", "u128", "usize",
    "i8", "i16", "i32", "i64", "i128", "isize", "f32", "f64", "!", "()",
}


def rustdoc_type_args(node: dict) -> list[dict]:
    path = node.get("resolved_path", {}) if isinstance(node, dict) else {}
    args = path.get("args") or {}
    return [arg["type"] for arg in args.get("angle_bracketed", {}).get("args", [])
            if isinstance(arg, dict) and isinstance(arg.get("type"), dict)]


def split_type_arguments(rendered: str) -> tuple[str, list[str]] | None:
    """Split one rendered Rust type at its outer generic argument list."""
    start = rendered.find("<")
    if start < 0 or not rendered.endswith(">"):
        return None
    depth = 0
    parts, item_start = [], start + 1
    for at in range(start, len(rendered)):
        char = rendered[at]
        if char == "<":
            depth += 1
        elif char == ">":
            depth -= 1
            if depth == 0:
                if at != len(rendered) - 1:
                    return None
                final = rendered[item_start:at].strip()
                if final:
                    parts.append(final)
                return rendered[:start].strip(), parts
        elif char == "," and depth == 1:
            parts.append(rendered[item_start:at].strip())
            item_start = at + 1
    return None


def split_top_level(rendered: str) -> list[str]:
    parts, start, angle, paren, bracket, brace = [], 0, 0, 0, 0, 0
    for at, char in enumerate(rendered):
        if char == "<": angle += 1
        elif char == ">": angle = max(0, angle - 1)
        elif char == "(": paren += 1
        elif char == ")": paren = max(0, paren - 1)
        elif char == "[": bracket += 1
        elif char == "]": bracket = max(0, bracket - 1)
        elif char == "{": brace += 1
        elif char == "}": brace = max(0, brace - 1)
        elif char == "," and not (angle or paren or bracket or brace):
            parts.append(rendered[start:at].strip())
            start = at + 1
    if rendered[start:].strip():
        parts.append(rendered[start:].strip())
    return parts


def compiler_type_node(rendered: str, local_paths: dict[str, str]) -> dict:
    """Parse the bounded compiler type forms needed for concrete field walks."""
    value = rendered.strip()
    if re.match(r"^(?:(?:unsafe\s+)?(?:extern\s+\"[^\"]+\"\s+)?fn)\s*\(", value):
        return {"function_pointer": value}
    if value.startswith("&"):
        remainder = re.sub(r"^&(?:'[^ ]+\s+)?(?:mut\s+)?", "", value)
        return {"borrowed_ref": {"type": compiler_type_node(remainder, local_paths)}}
    if value.startswith(("*const ", "*mut ")):
        return {"raw_pointer": {"type": compiler_type_node(value.split(None, 1)[1], local_paths)}}
    if value.startswith("dyn ") or value.startswith("impl ") or "{closure#" in value:
        return {"unresolved_dynamic": value}
    if value.startswith("(") and value.endswith(")"):
        inner = value[1:-1].strip()
        if not inner:
            return {"primitive": "()"}
        return {"tuple": [compiler_type_node(part, local_paths)
                          for part in split_top_level(inner)]}
    if value.startswith("[") and value.endswith("]"):
        inner = value[1:-1].split(";", 1)[0].strip()
        return {"slice": compiler_type_node(inner, local_paths)}
    parts = split_type_arguments(value)
    if parts:
        base, args = parts
    else:
        base, args = value, []
    base = base.removeprefix("reliquary_memory::").removeprefix("crate::")
    if base in PRIMITIVE_TYPES or re.fullmatch(r"\d+", base):
        return {"primitive": base}
    if base in local_paths:
        return {"resolved_path": {"id": local_paths[base], "path": base,
                                  "args": {"angle_bracketed": {"args": [
                                      ({"lifetime": arg} if arg.startswith("'") else
                                       {"type": compiler_type_node(arg, local_paths)}) for arg in args
                                  ]}} if args else None}}
    if re.fullmatch(r"'[A-Za-z_][\w]*", base):
        return {"lifetime": base}
    if re.fullmatch(r"[A-Z][A-Za-z0-9_]*", base):
        return {"generic": base}
    if "::" not in base and base[:1].isupper():
        return {"unresolved_dynamic": value}
    return {"resolved_path": {"id": None, "path": base,
                              "args": {"angle_bracketed": {"args": [
                                  ({"lifetime": arg} if arg.startswith("'") else
                                   {"type": compiler_type_node(arg, local_paths)}) for arg in args
                              ]}} if args else None}}


def compiler_nominal_inventory(doc: dict):
    """Cache local nominal definitions and their rustdoc field type trees."""
    index, paths = doc["index"], doc["paths"]
    local_paths = {}
    nominal_items = {}
    field_cache = {}
    for item_id, row in index.items():
        if row.get("crate_id") != 0:
            continue
        kind = next((name for name in ("struct", "enum", "union", "type_alias")
                     if name in row.get("inner", {})), None)
        full = paths.get(str(item_id), {}).get("path", [])
        if not kind or len(full) < 2:
            continue
        item_id = str(item_id)
        key = "::".join(full[1:])
        local_paths[key] = item_id
        nominal_items[item_id] = (row, kind)
        data = row["inner"][kind]
        ids = []
        shape = data.get("kind", {})
        if kind == "type_alias":
            field_cache[item_id] = ([data.get("type", {"unresolved_dynamic":
                                      "missing rustdoc type alias target"})],
                                    [param["name"] for param in data.get("generics", {}).get("params", [])
                                     if "type" in param.get("kind", {})])
            continue
        unknown_shape = False
        if kind in ("struct", "union"):
            if isinstance(shape, dict):
                if "plain" in shape:
                    plain = shape["plain"]
                    if isinstance(plain, dict) and isinstance(plain.get("fields", []), list):
                        ids.extend(plain.get("fields", []))
                    else:
                        unknown_shape = True
                if "tuple" in shape:
                    tuple_fields = shape["tuple"]
                    if isinstance(tuple_fields, list):
                        ids.extend(x for x in tuple_fields if x)
                    else:
                        unknown_shape = True
                if not any(key in shape for key in ("plain", "tuple", "unit")):
                    unknown_shape = True
            elif shape != "unit":
                unknown_shape = True
        else:
            for variant_id in data.get("variants", []):
                variant = index.get(str(variant_id), {}).get("inner", {}).get("variant", {})
                variant_kind = variant.get("kind", {})
                if isinstance(variant_kind, dict):
                    if "plain" in variant_kind:
                        plain = variant_kind["plain"]
                        if isinstance(plain, dict) and isinstance(plain.get("fields", []), list):
                            ids.extend(plain.get("fields", []))
                        else:
                            unknown_shape = True
                    if "tuple" in variant_kind:
                        tuple_fields = variant_kind["tuple"]
                        if isinstance(tuple_fields, list):
                            ids.extend(x for x in tuple_fields if x)
                        else:
                            unknown_shape = True
                    if not any(key in variant_kind for key in ("plain", "tuple", "unit")):
                        unknown_shape = True
                elif variant_kind not in ("unit", "plain"):
                    unknown_shape = True
        fields = [index.get(str(field_id), {}).get("inner", {}).get(
            "struct_field", {"unresolved_dynamic": "missing rustdoc field type"})
                  for field_id in ids]
        if unknown_shape:
            fields.append({"unresolved_dynamic": "unsupported rustdoc nominal field shape"})
        params = [param["name"] for param in data.get("generics", {}).get("params", [])
                  if "type" in param.get("kind", {})]
        field_cache[item_id] = ([field for field in fields if field], params)
    return local_paths, nominal_items, field_cache


def nominal_drop_walk(doc: dict, rendered: str,
                      implementations: dict[str, list[dict]],
                      inventory=None) -> tuple[list[dict], list[dict], bool]:
    """Resolve exact local Drop targets through static nominal field layouts.

    Unknown generic/dynamic/associated layouts fail closed. Only standard
    owned wrappers with known drop semantics are traversed through type args.
    """
    index, paths = doc["index"], doc["paths"]
    local_paths, nominal_items, field_cache = inventory or compiler_nominal_inventory(doc)
    external_crate_names = {row.get("name") for row in doc.get("external_crates", {}).values()
                            if isinstance(row, dict) and row.get("name")}

    internal_edges: dict[tuple, dict] = {}
    unresolved = False
    visited = set()
    active_aliases = set()

    def field_nodes(item_id: str) -> tuple[list[dict], list[str]]:
        return field_cache[item_id]

    def walk(node: dict, substitutions: dict[str, dict] | None = None) -> list[dict]:
        nonlocal unresolved
        substitutions = substitutions or {}
        if not isinstance(node, dict):
            unresolved = True
            return []
        if "borrowed_ref" in node or "raw_pointer" in node or "function_pointer" in node:
            return []
        if "generic" in node:
            name = node["generic"]
            if name in substitutions:
                replacement = substitutions[name]
                if replacement == node:
                    unresolved = True
                    return []
                else:
                    return walk(replacement, substitutions)
            else:
                unresolved = True
                return []
        if "unresolved_dynamic" in node or "dyn_trait" in node or "qualified_path" in node:
            unresolved = True
            return []
        if "tuple" in node:
            return [child for value in node["tuple"]
                    for child in walk(value, substitutions)]
        if "slice" in node or "array" in node:
            return walk(node.get("slice", node.get("array", {}).get("type")), substitutions)
        ref = node.get("resolved_path")
        if not ref:
            return []
        item_id = str(ref.get("id")) if ref.get("id") is not None else None
        path = "::".join(paths.get(item_id, {}).get("path", [])) if item_id else ref.get("path", "")
        args = rustdoc_type_args(node)
        if item_id in nominal_items:
            canonical = "::".join(paths.get(item_id, {}).get("path", [])[1:])
            row, kind = nominal_items[item_id]
            if kind == "type_alias":
                marker = (item_id, tuple(json.dumps(arg, sort_keys=True) for arg in args))
                if marker in active_aliases:
                    unresolved = True
                    return []
                params = field_cache[item_id][1]
                if len(args) != len(params):
                    unresolved = True
                    return []
                active_aliases.add(marker)
                nested = dict(substitutions)
                nested.update(zip(params, args))
                alias_fields = field_cache[item_id][0]
                result = [child for alias_field in alias_fields
                          for child in walk(alias_field, nested)]
                active_aliases.remove(marker)
                return result
            marker = (item_id, tuple(json.dumps(arg, sort_keys=True) for arg in args))
            if marker in visited:
                return []
            visited.add(marker)
            methods = implementations.get(canonical, [])
            if len(methods) > 1:
                unresolved = True
            item_row = row
            item_span = item_row.get("span")
            owner_span = methods[0] if len(methods) == 1 else (
                {"path": normalize_source_path(item_span["filename"]),
                 "line": item_span["begin"][0], "end_line": item_span["end"][0],
                 "symbol": f"drop glue for {canonical}"} if item_span else None)
            if owner_span is None:
                unresolved = True
            descriptor = {**owner_span, "type_path": canonical,
                          "explicit_drop": len(methods) == 1} if owner_span else None
            unresolved_before_fields = unresolved
            fields, params = field_nodes(item_id)
            type_args = iter(args)
            nested = dict(substitutions)
            for param in params:
                try:
                    nested[param] = next(type_args)
                except StopIteration:
                    nested.pop(param, None)
            child_nodes = []
            for field in fields:
                child_nodes.extend(walk(field, nested))
            if descriptor:
                for child in child_nodes:
                    key = (descriptor["symbol"], child["symbol"])
                    internal_edges[key] = {
                        "source_definition": descriptor,
                        "target_definition": child,
                        "resolved_by": "exact-rustdoc-nominal-field-drop-graph",
                    }
            local_uncertainty = unresolved and not unresolved_before_fields
            if descriptor and (methods or child_nodes or local_uncertainty):
                return [descriptor]
            return []
        if path in DROP_NEVER_WRAPPER_PATHS:
            return []
        if path in DROP_OWNING_WRAPPER_PATHS:
            return [child for arg in args for child in walk(arg, substitutions)]
        # Unknown external generic types can own or forget their parameters.
        # If any argument could contain local code, keep the drop unresolved.
        root = path.split("::", 1)[0] if path else ""
        if root not in external_crate_names:
            # A missing rustdoc item is not proof of an external type. It may
            # be a local/private nominal omitted from the public rustdoc index.
            unresolved = True
        elif args and any(contains_local_or_generic(arg) for arg in args):
            unresolved = True
        return []

    def contains_local_or_generic(node: dict) -> bool:
        if not isinstance(node, dict):
            return False
        if "generic" in node or "unresolved_dynamic" in node:
            return True
        ref = node.get("resolved_path")
        if ref:
            if ref.get("id") is not None and str(ref["id"]) in nominal_items:
                return True
            return any(contains_local_or_generic(arg) for arg in rustdoc_type_args(node))
        if "borrowed_ref" in node or "raw_pointer" in node or "function_pointer" in node:
            return False
        return any(contains_local_or_generic(child) for child in node.get("tuple", []))

    root = compiler_type_node(rendered, local_paths)
    roots = walk(root)
    return roots, list(internal_edges.values()), unresolved


def compiler_target_root(target: str | None) -> str | None:
    """Return the crate root of a rendered call path, when its form is clear."""
    return compiler_target_roots(target)[1]


def compiler_target_roots(target: str | None) -> tuple[str | None, str | None]:
    """Return receiver and trait roots for trait calls, or one path root twice."""
    if not target:
        return None, None
    path = target.strip()
    if path.startswith("<"):
        if " as " not in path or ">::" not in path:
            return None, None
        receiver, trait = path[1:].split(" as ", 1)
        trait = trait.split(">::", 1)[0]
        root_pattern = r"([A-Za-z_]\w*)::"
        receiver_match = re.match(root_pattern, receiver.strip().lstrip("&*"))
        trait_match = re.match(root_pattern, trait.strip().lstrip("&*"))
        return (receiver_match[1] if receiver_match else None,
                trait_match[1] if trait_match else None)
    else:
        path = path.lstrip("&*")
    root = re.match(r"([A-Za-z_]\w*)::", path)
    value = root[1] if root else None
    return value, value


def primitive_foreign_receiver(target: str | None) -> bool:
    """Recognize only primitive/reference/slice/array receivers for orphan proof.

    This deliberately excludes named standard types and arbitrary generics.
    Exact external THIR identities are still required before this fact is used.
    """
    if not target or not target.startswith("<") or " as " not in target:
        return False
    receiver = target[1:].split(" as ", 1)[0].strip()
    receiver = re.sub(r"'\{erased\}\s*", "", receiver)
    receiver = re.sub(r"'[_A-Za-z][\w]*\s*", "", receiver)
    receiver = receiver.removeprefix("&").removeprefix("mut ").strip()
    primitives = PRIMITIVE_TYPES - {"str", "()", "!"}

    def builtin(value: str) -> bool:
        value = value.strip()
        if value in PRIMITIVE_TYPES:
            return True
        if value.startswith("&"):
            value = re.sub(r"^&(?:'[_A-Za-z][\w]*\s*)?", "", value).strip()
            return builtin(value)
        if value.startswith("[") and value.endswith("]"):
            inner = value[1:-1].strip()
            element, separator, count = inner.partition(";")
            return builtin(element) and (not separator or bool(re.fullmatch(r"\d+", count.strip())))
        return value in primitives

    return builtin(receiver)


def concrete_external_trait_receiver(target: str | None, local_paths: dict[str, str],
                                     external_roots: set[str]) -> bool:
    """Prove an external trait receiver has no local/generic type component.

    A path root such as ``std`` is not enough: ``Box<Local>`` and references
    to local types can be local for orphan-rule purposes. This deliberately
    rejects references, slices, tuples, unknown aliases, and every unresolved
    or local generic argument. Non-fundamental external nominal wrappers such
    as ``Vec<Local>`` are also kept unresolved here for bounded proof.
    """
    if not target or not target.startswith("<") or " as " not in target:
        return False
    receiver = target[1:].split(" as ", 1)[0].strip()
    if receiver.startswith(("&", "*", "[", "(")):
        return False
    node = compiler_type_node(receiver, local_paths)
    if "resolved_path" not in node:
        return False

    local_item_ids = set(local_paths.values())

    def is_concrete_foreign(value: dict) -> bool:
        if "primitive" in value:
            return True
        ref = value.get("resolved_path")
        if not ref or "generic" in value or "unresolved_dynamic" in value:
            return False
        if ref.get("id") is not None and str(ref["id"]) in local_item_ids:
            return False
        path = ref.get("path", "")
        root = path.split("::", 1)[0]
        if root not in external_roots:
            return False
        for arg in rustdoc_type_args(value):
            if "lifetime" in arg:
                continue
            if not is_concrete_foreign(arg):
                return False
        return True

    return is_concrete_foreign(node)


def external_trait_arguments_are_closed(target: str | None, local_paths: dict[str, str],
                                        external_roots: set[str]) -> bool:
    """Reject trait arguments that could authorize a local implementation.

    Foreign traits may be implemented locally for a foreign/builtin receiver
    when a local type appears in the trait's arguments (for example
    ``impl ForeignTrait<Local> for u8``). Lifetime-only arguments are inert;
    type arguments must parse as closed builtin or external types.
    """
    if not target or not target.startswith("<") or " as " not in target:
        return False
    trait = target[1:].split(" as ", 1)[1].split(">::", 1)[0].strip()
    parsed = split_type_arguments(trait)
    if parsed is None:
        return "<" not in trait and ">" not in trait
    _, arguments = parsed
    local_item_ids = set(local_paths.values())

    def closed_type(node: dict) -> bool:
        if "primitive" in node or "lifetime" in node:
            return True
        if "generic" in node or "unresolved_dynamic" in node:
            return False
        if "tuple" in node:
            return all(closed_type(item) for item in node["tuple"])
        # References, pointers, slices, function types, and associated types
        # can carry local/generic types in forms this proof does not model.
        ref = node.get("resolved_path")
        if not ref:
            return False
        if ref.get("id") is not None and str(ref["id"]) in local_item_ids:
            return False
        path = ref.get("path", "")
        if path.split("::", 1)[0] not in external_roots:
            return False
        return all(closed_type(arg) for arg in rustdoc_type_args(node))

    for argument in arguments:
        if re.fullmatch(r"'(?:\{erased\}|[A-Za-z_]\w*)", argument.strip()):
            continue
        if not closed_type(compiler_type_node(argument, local_paths)):
            return False
    return True


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def resolved_path(path: str | Path) -> str:
    return str(Path(path).resolve()).casefold()


def callable_const_value(value: str) -> str | None:
    """Extract a complete compiler-rendered callable symbol from ConstValue."""
    if not value.startswith("ConstValue("):
        return value if value and not value.startswith("ConstValue") else None
    # The final brace group in a function ConstValue is its DefPath. Parse it
    # backwards so nested erased-region placeholders such as `'{erased}` stay
    # inside the symbol instead of being mistaken for the symbol itself.
    end = len(value.rstrip()) - 1
    while end >= 0 and value[end] in ") }\t":
        if value[end] == "}":
            break
        end -= 1
    if end < 0 or value[end] != "}":
        return None
    depth = 0
    for start in range(end, -1, -1):
        if value[start] == "}":
            depth += 1
        elif value[start] == "{":
            depth -= 1
            if depth == 0:
                symbol = value[start + 1:end].strip()
                return symbol or None
    return None


def source_fingerprint(files: list[dict]) -> str:
    canonical = json.dumps(files, sort_keys=True, separators=(",", ":")).encode()
    return hashlib.sha256(canonical).hexdigest()


def validate_provenance(receipt: dict, rustdoc: Path, mir: Path,
                        hir: Path | None = None, thir: Path | None = None,
                        built_mir_manifest: Path | None = None) -> dict:
    """Validate source stability and artifact identity before trusting provenance."""
    errors = []
    if not isinstance(receipt, dict):
        return {"status": "invalid", "validated": False,
                "errors": ["provenance receipt is not a JSON object"]}
    if receipt.get("schema") != "phase7-compiler-provenance.v1":
        errors.append("unsupported or missing provenance schema")
    # Source and artifact hashes alone do not bind the compiler result to the
    # component policy used to interpret it. Pin those policy inputs as well.
    policy_hashes = receipt.get("policy_hashes", {})
    required_policy = ("Cargo.toml", "Cargo.lock",
                       "tools/pitlord/production-components.json",
                       "tools/pitlord/convergence-component-dag.json")
    if not isinstance(policy_hashes, dict):
        errors.append("policy hashes are not a JSON object")
        policy_hashes = {}
    for relative in required_policy:
        expected_hash = policy_hashes.get(relative)
        policy_path = ROOT / relative
        if (not isinstance(expected_hash, str) or not policy_path.is_file()
                or expected_hash != sha256_file(policy_path)):
            errors.append(f"current policy input differs from compiler provenance: {relative}")
    if receipt.get("policy_hashes_after") != policy_hashes:
        errors.append("compiler policy inputs changed during emission")
    tools = receipt.get("tools", {})
    if not isinstance(tools, dict):
        errors.append("compiler evidence tool records are not a JSON object")
        tools = {}
    emitter = tools.get("emitter", {})
    emitter_hash = emitter.get("sha256_before")
    if (emitter.get("path") != "scripts/emit_phase7_compiler_evidence.py"
            or not isinstance(emitter_hash, str)
            or not re.fullmatch(r"[0-9a-f]{64}", emitter_hash)
            or emitter_hash != emitter.get("sha256_after")):
        errors.append("emitter tool identity is missing or changed during capture")
    if (receipt.get("build_profile") != "dev"
            or not isinstance(receipt.get("features"), list)
            or not isinstance(receipt.get("cfg"), list)
            or not isinstance(receipt.get("environment"), dict)
            or not isinstance(receipt.get("rustc_version"), str)
            or not receipt.get("rustc_version")):
        errors.append("compiler profile, cfg, environment or rustc version is missing")
    commands = receipt.get("commands", [])
    expected_commands = {"rustdoc", "mir", "hir", "thir"}
    recorded = {}
    if not isinstance(commands, list):
        errors.append("compiler command records are not a list")
    else:
        recorded = {row.get("name"): row for row in commands if isinstance(row, dict)}
        if set(recorded) != expected_commands:
            errors.append("compiler command records do not cover rustdoc, MIR, HIR and THIR")
        for name in expected_commands & set(recorded):
            row = recorded[name]
            if row.get("exit") != 0:
                errors.append(f"{name} compiler command did not succeed")
            log_path = row.get("log")
            log_hash = row.get("log_sha256")
            log_file = ROOT / log_path if isinstance(log_path, str) else None
            if (not log_file or not log_file.is_file() or not isinstance(log_hash, str)
                    or log_hash != sha256_file(log_file)):
                errors.append(f"{name} compiler log is missing or its hash does not match")
        receipt_artifacts = receipt.get("artifacts", {})
        if isinstance(receipt_artifacts, dict) and receipt_artifacts.get("built_mir_manifest"):
            mir_command = recorded.get("mir", {}).get("command", [])
            if not isinstance(mir_command, list) or "-Zdump-mir=built" not in mir_command \
                    or not any(isinstance(arg, str) and arg.startswith("-Zdump-mir-dir=")
                               for arg in mir_command):
                errors.append("MIR command does not record the bounded built MIR dump pass")
    source = receipt.get("source_snapshot", {})
    if not isinstance(source, dict):
        source = {}
        errors.append("source snapshot is not a JSON object")
    before, after = source.get("before_sha256"), source.get("after_sha256")
    manifest_path = source.get("manifest_path")
    if not before or before != after or not isinstance(source.get("file_count"), int) \
            or isinstance(source.get("file_count"), bool) or source.get("file_count", 0) <= 0 \
            or not manifest_path:
        errors.append("source snapshot is missing, empty, or changed during emission")
    else:
        manifest_file = ROOT / manifest_path
        if not manifest_file.is_file() or sha256_file(manifest_file) != source.get("manifest_sha256"):
            errors.append("source manifest is missing or its hash does not match the receipt")
        else:
            manifest = json.loads(manifest_file.read_text(encoding="utf-8"))
            entries = manifest.get("files", []) if isinstance(manifest, dict) else []
            valid_entries = all(isinstance(row, dict) and isinstance(row.get("path"), str)
                                and isinstance(row.get("sha256"), str) for row in entries)
            # The emitter fingerprints its filesystem-sorted list. Preserve the
            # receipt's exact ordering here; sorting POSIX strings differently
            # on Windows changes the serialized source fingerprint.
            normalized = entries if isinstance(entries, list) and valid_entries else []
            fingerprint = source_fingerprint(normalized)
            if (not isinstance(manifest, dict) or not isinstance(entries, list) or not valid_entries
                    or len({row["path"] for row in normalized}) != len(normalized)
                    or manifest.get("schema") != "phase7-compiler-source-manifest.v1"
                    or manifest.get("fingerprint_sha256") != fingerprint
                    or fingerprint != before or len(entries) != source["file_count"]):
                errors.append("source manifest does not match the before/after source fingerprint")
            else:
                expected = {row["path"]: row["sha256"] for row in normalized}
                current = {p.relative_to(ROOT).as_posix(): sha256_file(p)
                           for p in (ROOT / "src").rglob("*.rs")}
                if expected != current:
                    errors.append("current Rust source files differ from the emitted source manifest")
    artifacts = receipt.get("artifacts", {})
    if not isinstance(artifacts, dict):
        artifacts = {}
        errors.append("artifact records are not a JSON object")
    for name, path in (("rustdoc", rustdoc), ("mir", mir)):
        artifact = artifacts.get(name, {})
        if not artifact.get("path") or resolved_path(artifact["path"]) != resolved_path(path):
            errors.append(f"{name} artifact path does not match the supplied input")
        if not artifact.get("sha256") or artifact["sha256"] != sha256_file(path):
            errors.append(f"{name} artifact hash does not match the supplied input")
    if hir:
        artifact = artifacts.get("hir", {})
        if not artifact.get("path") or resolved_path(artifact["path"]) != resolved_path(hir):
            errors.append("hir artifact path does not match the supplied input")
        if not artifact.get("sha256") or artifact["sha256"] != sha256_file(hir):
            errors.append("hir artifact hash does not match the supplied input")
    if thir:
        artifact = artifacts.get("thir", {})
        if not artifact.get("path") or resolved_path(artifact["path"]) != resolved_path(thir):
            errors.append("thir artifact path does not match the supplied input")
        if not artifact.get("sha256") or artifact["sha256"] != sha256_file(thir):
            errors.append("thir artifact hash does not match the supplied input")
    built_artifact = artifacts.get("built_mir_manifest")
    if built_artifact:
        if not isinstance(built_artifact, dict):
            errors.append("built MIR manifest artifact record is malformed")
        elif built_mir_manifest is None:
            errors.append("built MIR manifest is recorded but was not supplied")
        else:
            try:
                supplied_manifest_hash = sha256_file(built_mir_manifest)
            except OSError:
                supplied_manifest_hash = None
            if (not built_artifact.get("path")
                    or resolved_path(built_artifact["path"]) != resolved_path(built_mir_manifest)):
                errors.append("built MIR manifest path does not match the supplied input")
            if (not built_artifact.get("sha256")
                    or built_artifact["sha256"] != supplied_manifest_hash):
                errors.append("built MIR manifest hash does not match the supplied input")
            _, _, manifest_errors, _ = load_built_mir_manifest(built_mir_manifest)
            errors.extend(manifest_errors)
            try:
                manifest = json.loads(built_mir_manifest.read_text(encoding="utf-8"))
                recorded = {row.get("name"): row for row in receipt.get("commands", [])
                            if isinstance(row, dict)}
                mir_command = recorded.get("mir", {}).get("command", [])
                dump_argument = next((arg for arg in mir_command if isinstance(arg, str)
                                      and arg.startswith("-Zdump-mir-dir=")), None)
                expected_dir = Path(manifest["directory"])
                if not expected_dir.is_absolute():
                    expected_dir = ROOT / expected_dir
                if (dump_argument is None or resolved_path(dump_argument.split("=", 1)[1])
                        != resolved_path(expected_dir)):
                    errors.append("MIR command dump directory disagrees with the built MIR manifest")
            except (OSError, ValueError, KeyError, TypeError):
                errors.append("cannot verify the MIR command built dump directory")
    elif built_mir_manifest is not None:
        errors.append("built MIR manifest was supplied but is absent from provenance")
    return {"status": "invalid" if errors else "validated",
            "validated": not errors, "errors": errors}


SPAN = re.compile(r"(?:scope \d+ at |\+ span: )(.+?):(\d+):(\d+)(?::\s*(\d+):(\d+))?")


def compiler_def_identity(rendered: str, method: str) -> tuple[str, str, str] | None:
    """Extract a compiler-associated DefId hint matching the displayed method.

    MIR function signatures may contain DefIds for named bound regions whose
    enclosing item path ends in the method name. This metadata is useful for
    diagnostics, but is never authoritative evidence of the Const callee.
    """
    for match in re.finditer(
            r"DefId\((\d+:\d+) ~ ([A-Za-z_]\w*)\[([^\]]+)\]::([^)]*?)\)", rendered):
        path = re.sub(r"::'_#?\d*$", "", match[4])
        if path.split("::")[-1] == method:
            return match[2], path, f"DefId({match[1]} ~ {match[2]}[{match[3]}]::{path})"
    return None


HIR_OWNER = re.compile(r"^(DefId\(\d+:\d+ ~ [^\n]+?\)) => OwnerNodes \{$")
HIR_OWNER_ID = re.compile(r"^(\s*)owner_id: (DefId\(\d+:\d+ ~ [^\n]+?\)),\s*$")
HIR_SPAN = re.compile(r"^(\s*)span: (.+?):(\d+):(\d+): (\d+):(\d+) \(#\d+\),\s*$")
HIR_CLOSURE = re.compile(r"^(\s*)Closure \{$")
HIR_CLOSURE_DEF = re.compile(r"^\s*def_id: (DefId\(\d+:\d+ ~ [^\n]+?\)),\s*$")
HIR_CLOSURE_SPAN = re.compile(
    r"^\s*fn_decl_span: (.+?):(\d+):(\d+): (\d+):(\d+) \(#\d+\),\s*$")


def hir_owner_spans(lines, only_def_ids: set[str] | None = None) -> dict[str, list[dict]]:
    """Read compiler HIR OwnerNodes, retaining only their exact item spans.

    The DefId is the compiler's identity; the span comes from that DefId's
    OwnerNodes block. The broadest concrete source span in the owner block is
    the enclosing item span. Ambiguous spans are retained and fail closed.
    """
    blocks: dict[str, dict] = {}
    closures: dict[str, list[dict]] = {}
    closure_stack: list[dict] = []
    current = None
    for line in lines:
        line_text = line.rstrip("\r\n")
        closure_header = HIR_CLOSURE.match(line_text)
        if closure_header:
            closure_stack.append({"indent": len(closure_header[1]), "def_id": None,
                                  "spans": []})
        if closure_stack:
            closure = closure_stack[-1]
            indent = len(line_text) - len(line_text.lstrip())
            if indent == closure["indent"] + 4:
                definition = HIR_CLOSURE_DEF.match(line_text)
                if definition:
                    closure["def_id"] = definition[1]
                span = HIR_CLOSURE_SPAN.match(line_text)
                if span:
                    path, start_line, start_column, end_line, end_column = span.groups()
                    closure["spans"].append({"path": normalize_source_path(path),
                                             "line": int(start_line),
                                             "end_line": int(end_line),
                                             "column": int(start_column),
                                             "end_column": int(end_column)})
            if indent == closure["indent"] and line_text.strip() in ("}", "},"):
                closed = closure_stack.pop()
                if (closed["def_id"] and closed["spans"]
                        and (only_def_ids is None or closed["def_id"] in only_def_ids)):
                    closures.setdefault(closed["def_id"], []).extend(closed["spans"])
        header = HIR_OWNER.match(line_text)
        if header:
            current = header[1] if only_def_ids is None or header[1] in only_def_ids else None
            if current is not None:
                blocks[current] = {"owner_indent": None, "spans": []}
            continue
        if current is None:
            continue
        stripped = line.rstrip("\r\n")
        owner_id = HIR_OWNER_ID.match(stripped)
        if owner_id and owner_id[2] == current and blocks[current]["owner_indent"] is None:
            blocks[current]["owner_indent"] = len(owner_id[1])
        match = HIR_SPAN.match(stripped)
        if match and blocks[current]["owner_indent"] is not None \
                and len(match[1]) == blocks[current]["owner_indent"]:
            _, path, start_line, start_col, end_line, end_col = match.groups()
            start = (int(start_line), int(start_col))
            end = (int(end_line), int(end_col))
            extent = (end[0] - start[0]) * 1_000_000 + end[1] - start[1]
            blocks[current]["spans"].append({"path": normalize_source_path(path),
                                             "line": start[0], "end_line": end[0],
                                             "extent": extent})
    result = {}
    for def_id, block in blocks.items():
        spans = block["spans"]
        if not spans:
            continue
        concrete = [span for span in spans if not span["path"].startswith("<")]
        if not concrete:
            continue
        widest = max(span["extent"] for span in concrete)
        winners = [{k: v for k, v in span.items() if k != "extent"}
                   for span in concrete if span["extent"] == widest]
        result[def_id] = winners
    # Nested closure identities are compiler-generated but fully concrete in
    # HIR. Use only DefId plus its own fn_decl_span; preserve duplicate spans
    # so ambiguity continues to fail closed in the join.
    for def_id, spans in closures.items():
        result.setdefault(def_id, []).extend(spans)
    return result


THIR_OWNER = re.compile(r"^(DefId\(\d+:\d+ ~ [^\n]+?\)):$")
THIR_DEF = re.compile(r"^\s*ty: FnDef\((DefId\(\d+:\d+ ~ [^\n]+?\)), \[.*\]\),?$")
THIR_SPAN = re.compile(r"^\s*fn_span: (.+?):(\d+):(\d+):\s*(\d+):(\d+) \(#\d+\),?$")
THIR_EXPR_SPAN = re.compile(r"^\s*span: (.+?):(\d+):(\d+):\s*(\d+):(\d+) \(#\d+\),?$")


def caller_owner_identity(def_id: str | None) -> str | None:
    """Return the exact compiler path portion of a local THIR caller DefId."""
    if not def_id:
        return None
    match = re.search(r"~ ([A-Za-z_]\w*)\[[^\]]+\]::(.+?)\)?$", def_id)
    return f"{match[1]}::{match[2]}" if match else None


def thir_call_sites(lines, only_spans: set[tuple] | None = None) -> list[dict]:
    """Extract compiler-resolved THIR FnDef calls with exact callee spans.

    FnDef DefIds come from the typechecked THIR Call node. The enclosing owner
    DefId identifies the caller; the callee path is never inferred from names.
    Function-pointer and closure calls without FnDef remain unresolved.
    """
    calls = []
    owner_id = None
    expr_spans: dict[int, dict] = {}
    active = []
    for raw in lines:
        line = raw.rstrip("\r\n")
        header = THIR_OWNER.match(line)
        if header:
            owner_id = header[1]
            expr_spans = {}
            active = []
            continue
        indent = len(line) - len(line.lstrip())
        stripped = line.strip()
        for call in active:
            if indent != call["indent"] + 4:
                continue
            if call["target_def_id"] is None:
                target = THIR_DEF.match(line)
                if target:
                    call["target_def_id"] = target[1]
            if call["callee_span"] is None:
                span = THIR_SPAN.match(line)
                if span:
                    path, sl, sc, el, ec = span.groups()
                    call["callee_span"] = {
                        "path": normalize_source_path(path), "line": int(sl),
                        "column": int(sc), "end_line": int(el), "end_column": int(ec),
                    }
        if stripped.startswith("}") and active and indent == active[-1]["indent"]:
            call = active.pop()
            if owner_id and call["target_def_id"] and call["callee_span"]:
                row = {"caller_def_id": owner_id,
                       "caller_owner_identity": caller_owner_identity(owner_id),
                       "target_def_id": call["target_def_id"],
                       "callee_span": call["callee_span"],
                       "expr_span": call.get("expr_span")}
                span_keys = {tuple(row["callee_span"].get(key) for key in
                                   ("path", "line", "column", "end_line", "end_column"))}
                if row["expr_span"]:
                    span_keys.add(tuple(row["expr_span"].get(key) for key in
                                        ("path", "line", "column", "end_line", "end_column")))
                if only_spans is None or span_keys & only_spans:
                    calls.append(row)
        if stripped == "Expr {":
            continue
        span = THIR_EXPR_SPAN.match(line)
        if span:
            path, sl, sc, el, ec = span.groups()
            # THIR emits the Expr span four columns deeper than its Expr header.
            expr_spans[indent - 4] = {
                "path": normalize_source_path(path), "line": int(sl),
                "column": int(sc), "end_line": int(el), "end_column": int(ec),
            }
        if stripped == "Call {":
            active.append({"indent": indent, "target_def_id": None, "callee_span": None,
                           "expr_span": expr_spans.get(indent - 8)})
    return calls


BUILT_MIR_FILENAME = re.compile(
    r"^(?P<crate>[A-Za-z_]\w*)(?:\.(?P<modules>.*?))?[-.]\{impl#(?P<ordinal>\d+)\}-(?P<method>[A-Za-z_]\w*)\.\d+-\d+-\d+\.built\.after\.mir$")
BUILT_MIR_IMPL_SHAPE = re.compile(
    r"^[A-Za-z_]\w*(?:\..*)?[-.]\{impl#\d+\}-.+\.built\.after\.mir$")
BUILT_MIR_COMMENT = re.compile(
    r"^// MIR for `(?:[A-Za-z_]\w*(?:::[A-Za-z_]\w*)*::)?<impl at (.+):(\d+):(\d+): (\d+):(\d+)>::([A-Za-z_]\w*)` after built$")
BUILT_MIR_FN = re.compile(
    r"^fn (?:.*::)?<impl at (.+):(\d+):(\d+): (\d+):(\d+)>::([A-Za-z_]\w*)\(")
MAIN_MIR_FN = re.compile(
    r"^fn (?:.*::)?<impl at (.+):(\d+):(\d+): (\d+):(\d+)>::([A-Za-z_]\w*)\(")


def mir_header_key(source_path: str, start_line: str, start_column: str,
                   end_line: str, end_column: str, method: str) -> tuple:
    path = source_path.replace("\\", "/")
    try:
        candidate = Path(path)
        if candidate.is_absolute():
            path = candidate.resolve().relative_to(ROOT.resolve()).as_posix()
    except (OSError, ValueError):
        pass
    return (normalize_source_path(path), int(start_line), int(start_column),
            int(end_line), int(end_column), method)


def parse_built_mir_file(path: Path) -> tuple[str, tuple, str]:
    """Validate a built.after MIR filename and its complete body header."""
    filename = BUILT_MIR_FILENAME.fullmatch(path.name)
    if not filename:
        raise ValueError(f"unrecognized built MIR filename: {path.name}")
    identity_prefix = filename["crate"]
    if filename["modules"]:
        identity_prefix += "::" + filename["modules"].replace(".", "::")
    identity = (f"{identity_prefix}::{{impl#{filename['ordinal']}}}::"
                f"{filename['method']}")
    with path.open(encoding="utf-8", errors="replace") as stream:
        first = stream.readline().rstrip("\r\n")
        function_headers = [line.rstrip("\r\n") for line in stream
                           if line.startswith("fn ")]
    comment = BUILT_MIR_COMMENT.fullmatch(first)
    body = BUILT_MIR_FN.match(function_headers[0]) if len(function_headers) == 1 else None
    if not comment or not body:
        raise ValueError(f"built MIR dump lacks one exact full body header: {path.name}")
    comment_key = mir_header_key(*comment.groups())
    body_key = mir_header_key(*body.groups())
    prefix = first.split("`", 1)[1].split("<impl at", 1)[0]
    expected_prefix = (filename["modules"].replace(".", "::") + "::"
                       if filename["modules"] else "")
    if (comment_key != body_key or comment[6] != filename["method"]
            or prefix != expected_prefix):
        raise ValueError(f"built MIR filename and full body header disagree: {path.name}")
    return identity, body_key, first + "\n" + function_headers[0]


def load_built_mir_manifest(manifest_path: Path, root: Path | None = None
                            ) -> tuple[dict[tuple, dict], list[Path], list[str], list[dict]]:
    """Verify the complete sorted dump inventory and derive exact body identities."""
    errors: list[str] = []
    try:
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    except (OSError, ValueError) as error:
        return {}, [], [f"cannot read built MIR manifest: {error}"], []
    if not isinstance(manifest, dict) or manifest.get("schema") != "phase7-built-mir-manifest.v1":
        return {}, [], ["unsupported or missing built MIR manifest schema"], []
    raw_dir = manifest.get("directory")
    rows = manifest.get("files")
    if not isinstance(raw_dir, str) or not isinstance(rows, list) or not rows:
        return {}, [], ["built MIR manifest directory or file inventory is missing"], []
    root = (root or ROOT).resolve()
    directory = Path(raw_dir)
    if not directory.is_absolute():
        directory = root / Path(raw_dir.replace("\\", "/"))
    directory = directory.resolve()
    try:
        directory.relative_to(root)
    except ValueError:
        return {}, [], ["built MIR dump directory escapes the repository"], []
    paths = []
    normalized_names = []
    for row in rows:
        if not isinstance(row, dict) or not isinstance(row.get("path"), str) \
                or not isinstance(row.get("sha256"), str):
            errors.append("built MIR manifest entry is malformed")
            continue
        relative = Path(row["path"].replace("\\", "/"))
        if relative.is_absolute() or ".." in relative.parts:
            errors.append("built MIR manifest contains an unsafe dump path")
            continue
        path = (root / relative).resolve()
        try:
            path.relative_to(directory)
        except ValueError:
            errors.append("built MIR dump path escapes its declared directory")
            continue
        normalized_names.append(relative.as_posix())
        paths.append((path, row["sha256"]))
    if normalized_names != sorted(normalized_names) or len(set(normalized_names)) != len(normalized_names):
        errors.append("built MIR dump paths are not unique and sorted")
    listed = {path for path, _ in paths}
    actual = set(directory.rglob("*.built.after.mir"))
    if listed != actual:
        errors.append("built MIR manifest file set differs from the dump directory")
    index: dict[tuple, dict] = {}
    seen_identities: set[str] = set()
    file_paths = []
    unindexed = []
    for path, expected_hash in paths:
        file_paths.append(path)
        try:
            actual_hash = sha256_file(path)
        except OSError as error:
            errors.append(str(error))
            continue
        if actual_hash != expected_hash:
            errors.append(f"built MIR dump hash does not match manifest: {path}")
            continue
        if not BUILT_MIR_FILENAME.fullmatch(path.name):
            if BUILT_MIR_IMPL_SHAPE.fullmatch(path.name):
                errors.append(f"malformed supported built MIR implementation filename: {path.name}")
            else:
                unindexed.append({"path": path.relative_to(root).as_posix(),
                                  "sha256": actual_hash,
                                  "reason": "unsupported built MIR body filename; identity left unindexed"})
            continue
        try:
            identity, header_key, full_header = parse_built_mir_file(path)
        except ValueError as error:
            errors.append(str(error))
            continue
        if identity in seen_identities:
            errors.append(f"duplicate built MIR body identity: {identity}")
        if header_key in index:
            errors.append(f"ambiguous built MIR full body header: {header_key}")
        seen_identities.add(identity)
        body_header = full_header.split("\n", 1)[1]
        body_match = BUILT_MIR_FN.match(body_header)
        if not body_match:
            errors.append(f"built MIR function header is malformed: {path.name}")
            continue
        index[header_key] = {"identity": identity, "header": full_header,
                             "signature": body_header[body_match.end():],
                             "path": path.relative_to(root).as_posix(),
                             "sha256": actual_hash}
    return (index if not errors else {}), file_paths, errors, unindexed


def thir_calls_for_mir_call(call: dict, thir_by_span: dict[tuple, list[dict]]) -> list[dict]:
    """Join one MIR call to THIR using its exact body owner when available."""
    span_key = (call.get("source_path"), call.get("source_line"), call.get("source_column"),
                call.get("source_end_line"), call.get("source_end_column"))
    rows = thir_by_span.get(span_key, [])
    identity = call.get("caller_owner_identity")
    if identity:
        rows = [row for row in rows if row.get("caller_owner_identity") == identity]
    return list({row["target_def_id"]: row for row in rows}.values())


def call_method_name(target: str | None) -> str | None:
    if not target:
        return None
    path = target.rsplit(">::", 1)[-1] if target.startswith("<") else target
    return path.rsplit("::", 1)[-1].split("::<", 1)[0]


def mir_calls(lines, built_mir_index: dict[tuple, dict] | None = None):
    """Yield compiler constant-function operands at actual call terminators."""
    pending = None
    local_types: dict[str, str] = {}
    caller_body = None
    for line in lines:
        if line.startswith("fn "):
            local_types = {}
            caller_body = None
            header = MAIN_MIR_FN.match(line.rstrip("\r\n"))
            if header and built_mir_index:
                candidate = built_mir_index.get(mir_header_key(*header.groups()))
                if candidate and line.rstrip("\r\n")[header.end():] == candidate["signature"]:
                    caller_body = {**candidate,
                                   "main_header": line.rstrip("\r\n")}
            # Arguments are declared in the signature, not among MIR's `let`
            # locals. Retain their exact rendered types for destructor analysis.
            argument_start = re.search(r"\((?=_\d+:)", line)
            if argument_start:
                depth = 1
                end = argument_start.end()
                while end < len(line) and depth:
                    if line[end] == "(":
                        depth += 1
                    elif line[end] == ")":
                        depth -= 1
                    end += 1
                if depth == 0:
                    for argument in split_top_level(line[argument_start.end():end - 1]):
                        declaration = re.fullmatch(r"(_\d+):\s*(.+)", argument)
                        if declaration:
                            local_types[declaration[1]] = declaration[2].strip()
        local_decl = re.match(r"\s*let(?:\s+mut)?\s+(_\d+):\s*(.*?)\s*;\s*//", line)
        if local_decl:
            local_types[local_decl[1]] = local_decl[2].strip()
        if " -> [return:" in line or " -> unwind " in line:
            match = SPAN.search(line)
            evidence_hash = hashlib.sha256(line.encode()).hexdigest()
            pending = {"source_path": normalize_source_path(match[1]),
                       "source_line": int(match[2]), "source_column": int(match[3]),
                       "source_end_line": int(match[4]) if match[4] else int(match[2]),
                       "source_end_column": int(match[5]) if match[5] else int(match[3]),
                       "mir_line_sha256": evidence_hash} if match else {
                           "source_path": None, "mir_line_sha256": evidence_hash}
            if line.lstrip().startswith("drop("):
                pending["terminator_kind"] = "drop"
                place = re.search(r"\bdrop\((.*?)\)\s*->", line)
                if place:
                    operand = place[1].strip()
                    typed_place = re.match(r"\(?\s*.*?:\s*(.+?)\s*\)?$", operand)
                    local_place = re.fullmatch(r"(_\d+)", operand)
                    if typed_place:
                        pending["drop_type"] = typed_place[1].strip()
                    elif local_place:
                        pending["drop_type"] = local_types.get(local_place[1])
                    pending["drop_operand"] = operand
        if pending and "+ const_: Const {" in line:
            match = re.search(r", val: Value\((.*)\) \}\s*$", line)
            if match:
                operand = match[1]
                target = callable_const_value(operand)
                rendered_target = without_generics(target) if target else None
                identity = compiler_def_identity(line, call_method_name(rendered_target)) \
                    if rendered_target else None
                yield {**pending, "target": rendered_target,
                       **({"caller_owner_identity": caller_body["identity"],
                          "caller_body_header": caller_body["header"],
                          "caller_mir_fn_header": caller_body["main_header"],
                          "caller_body_artifact": caller_body["path"],
                          "caller_body_artifact_sha256": caller_body["sha256"]}
                         if caller_body else {}),
                       "definition_crate": identity[0] if identity else None,
                       "compiler_def_path": identity[1] if identity else None,
                       "compiler_def_id": identity[2] if identity else None,
                       "compiler_def_id_role": "hint_only_possible_bound_region",
                       "definition_path": identity[1] if identity and identity[0] == "reliquary_memory" else None,
                       **({} if target else {"reason": "unrecognized compiler constant operand"})}
            else:
                yield {**pending, "target": None,
                       **({"caller_owner_identity": caller_body["identity"],
                          "caller_body_header": caller_body["header"],
                          "caller_mir_fn_header": caller_body["main_header"],
                          "caller_body_artifact": caller_body["path"],
                          "caller_body_artifact_sha256": caller_body["sha256"]}
                         if caller_body else {}),
                       "reason": "unrecognized compiler constant operand"}
            pending = None
        elif pending and re.match(r"\s*\}", line):
            reason = ("drop terminator requires destructor analysis" if pending.get("terminator_kind") == "drop"
                      else "indirect or nonconstant call terminator")
            yield {**pending, "target": None,
                   **({"caller_owner_identity": caller_body["identity"],
                      "caller_body_header": caller_body["header"],
                      "caller_mir_fn_header": caller_body["main_header"],
                      "caller_body_artifact": caller_body["path"],
                      "caller_body_artifact_sha256": caller_body["sha256"]}
                     if caller_body else {}), "reason": reason}
            pending = None


def analyze(doc: dict, lines, mapping: dict, dag: dict,
            hir_spans: dict[str, list[dict]] | None = None,
            thir_calls: list[dict] | None = None,
            built_mir_index: dict[tuple, dict] | None = None) -> dict:
    symbols, local_roots, external_roots = compiler_symbols(doc)
    drop_implementations = compiler_drop_implementations(doc)
    nominal_inventory = compiler_nominal_inventory(doc)
    local_type_paths = nominal_inventory[0]
    files = {normalize_source_path(row["path"]): row for row in mapping["files"]}
    allowed = {row["id"]: {e["to"] for e in row.get("depends_on", [])}
               for row in dag["components"]}
    cross, unknown = [], []
    external_resolution_witnesses = []
    total = constants = external = external_unattributed = internal = 0
    raw_drop_terminators = fully_resolved_drop_terminators = no_local_drop_path_terminators = 0
    direct_drop_candidates = 0
    unresolved_drop_types: dict[str, int] = {}
    drop_witnesses = []
    explicit_drop_calls = []
    passive_field_layout_witnesses = []
    drop_layout_cache = {}
    drop_layout_cache_hits = 0
    hir_spans = hir_spans or {}
    thir_by_span: dict[tuple, list[dict]] = {}
    for row in thir_calls or []:
        for span_name in ("callee_span", "expr_span"):
            span = row.get(span_name)
            if not span:
                continue
            key = (span["path"], span["line"], span["column"], span["end_line"], span["end_column"])
            thir_by_span.setdefault(key, []).append(row)
    for call in mir_calls(lines, built_mir_index):
        is_drop = call.get("terminator_kind") == "drop"
        total += 1
        constants += int(not is_drop and call.get("target") is not None)
        source_path = normalize_source_path(call["source_path"]) if call["source_path"] else None
        source = files.get(source_path, {}).get("component")
        if is_drop:
            raw_drop_terminators += 1
            drop_type = call.get("drop_type")
            direct = exact_drop_implementations(drop_type, drop_implementations)
            direct_drop_candidates += int(len(direct) == 1)
            if drop_type in drop_layout_cache:
                roots, field_edges, unresolved_layout = drop_layout_cache[drop_type]
                drop_layout_cache_hits += 1
            else:
                roots, field_edges, unresolved_layout = nominal_drop_walk(
                    doc, drop_type or "", drop_implementations, nominal_inventory)
                drop_layout_cache[drop_type] = (roots, field_edges, unresolved_layout)
            unresolved_layout = unresolved_layout or not drop_type or not source
            edges = [(None, root) for root in roots]
            edges.extend((edge["source_definition"], edge["target_definition"])
                         for edge in field_edges)
            for owner, target_definition in edges:
                source_component = (source if owner is None else files.get(
                    normalize_source_path(owner["path"]), {}).get("component"))
                dependency = files.get(normalize_source_path(target_definition["path"]), {}).get("component")
                if dependency is None or source_component is None:
                    unresolved_layout = True
                    continue
                witness = {**call, "target": target_definition["symbol"],
                           "source_component": source_component, "target_component": dependency,
                           "target_definitions": [target_definition],
                           "resolved_by": "exact-rustdoc-nominal-field-drop-graph",
                           "explicit_drop_capability": target_definition.get("explicit_drop", False),
                           "permitted": dependency in allowed.get(source_component, set())}
                drop_witnesses.append(witness)
                if not target_definition.get("explicit_drop", False):
                    passive_field_layout_witnesses.append(witness)
                else:
                    explicit_drop_calls.append(witness)
                    if source_component == dependency:
                        internal += 1
                    else:
                        cross.append(witness)
            if unresolved_layout:
                raw_type = drop_type or "<unavailable MIR place type>"
                unresolved_drop_types[raw_type] = unresolved_drop_types.get(raw_type, 0) + 1
                unknown.append({**call, "source_component": source,
                                "reason": "drop terminator has unresolved generic or dynamic field glue"})
            else:
                fully_resolved_drop_terminators += 1
                no_local_drop_path_terminators += int(not roots)
            continue
        target = call["target"]
        is_closure = bool(target and ("{closure" in target or "{{closure" in target))
        matches = symbols.get(target, []) if target and not is_closure else []
        resolution = "exact-rustdoc-symbol" if matches else None
        # MIR-associated DefIds are diagnostic metadata only; the IDs printed
        # in function-signature regions can identify bound regions rather than
        # the callable item. They are never joined to HIR or used for target
        # classification.
        # Use the exact body owner when the built-MIR dump index is available;
        # otherwise retain the existing fail-closed span-only resolution.
        resolved_thir = thir_calls_for_mir_call(call, thir_by_span)
        thir_definition_crate = None
        if len(resolved_thir) == 1:
            call["thir_caller_def_id"] = resolved_thir[0].get("caller_def_id")
            call["thir_caller_owner_identity"] = resolved_thir[0].get("caller_owner_identity")
            thir_def = resolved_thir[0]["target_def_id"]
            crate_match = re.search(r"~ ([A-Za-z_]\w*)\[", thir_def)
            thir_definition_crate = crate_match[1] if crate_match else None
            thir_local = thir_def.startswith("DefId(0:") or thir_definition_crate in local_roots
            external_trait_receiver = concrete_external_trait_receiver(
                target, local_type_paths, external_roots)
            external_trait_args_closed = external_trait_arguments_are_closed(
                target, local_type_paths, external_roots)
            primitive_orphan_proof = (
                thir_definition_crate in external_roots
                and thir_definition_crate not in local_roots
                and external_trait_args_closed
                and primitive_foreign_receiver(target))
            thir_external_trait_uncertain = ((target or "").startswith("<")
                                              and (not external_trait_receiver
                                                   or not external_trait_args_closed)
                                              and not thir_local
                                              and not primitive_orphan_proof)
            if thir_external_trait_uncertain:
                # A trait declaration's external DefId does not prove that a
                # local receiver selected the external implementation.
                thir_definition_crate = None
                resolved_thir = []
            elif thir_local:
                thir_matches = hir_spans.get(thir_def, [])
                matches = thir_matches if len(thir_matches) == 1 else []
                resolution = "exact-thir-defid-hir-owner-span" if matches else None
            else:
                # A resolved THIR external target overrides any shortened-name
                # local guess; classify it below by its exact external crate.
                matches = []
                resolution = None
        elif len(resolved_thir) > 1:
            # Several compiler targets share this exact source span (for example
            # macro or generic expansions); source names cannot disambiguate them.
            matches = []
            resolution = None
        if not matches:
            receiver_root, trait_root = compiler_target_roots(target)
            external_receiver = (
                concrete_external_trait_receiver(target, local_type_paths, external_roots)
                if (target or "").startswith("<") else
                receiver_root in external_roots and receiver_root not in local_roots)
            exact_thir_external_def = (thir_definition_crate in external_roots
                                       and thir_definition_crate not in local_roots)
            # A trait DefId alone does not prove the selected implementation is
            # external when MIR abbreviates a local receiver's type.
            # Only THIR's exact FnDef callee identity is authoritative for this
            # orphan proof. DefIds printed inside MIR function-signature region
            # types may identify bound regions rather than the Const callee.
            primitive_orphan_proof = (exact_thir_external_def
                                      and external_trait_arguments_are_closed(
                                          target, local_type_paths, external_roots)
                                      and primitive_foreign_receiver(target))
            direct_external_path = (not (target or "").startswith("<")
                                    and not is_closure and external_receiver)
            if exact_thir_external_def or primitive_orphan_proof or direct_external_path:
                external += 1
                external_unattributed += int(source is None)
                if primitive_orphan_proof:
                    external_resolution_witnesses.append({**call,
                        "thir_target_def_id": (resolved_thir[0]["target_def_id"]
                                               if len(resolved_thir) == 1 else None),
                        "source_component": source,
                        "resolved_by": "exact-external-thir-defid-builtin-receiver-orphan-rule",
                        "proof_reason": "exact external FnDef DefId from the THIR call and a concrete Rust primitive/reference/slice/array receiver; the current crate cannot implement an external trait for that builtin receiver"})
            else:
                reason = ("unresolved compiler closure target" if is_closure
                          else call.get("reason", "unresolved local compiler target"))
                unknown.append({**call, "source_component": source,
                                "reason": reason})
            continue
        matches = {json.dumps(row, sort_keys=True): row for row in matches}.values()
        targets = {files.get(normalize_source_path(row["path"]), {}).get("component")
                   for row in matches}
        if len(targets) != 1 or None in targets or not source:
            unknown.append({**call, "reason": "ambiguous or unmapped compiler target",
                            "target_definitions": list(matches)})
            continue
        dependency = next(iter(targets))
        if source == dependency:
            internal += 1
            continue
        cross.append({**call, "source_component": source, "target_component": dependency,
                      "target_definitions": list(matches),
                      "resolved_by": resolution,
                      "permitted": dependency in allowed.get(source, set())})
    return {"schema": "phase7-compiler-calls.v1", "constant_calls": constants,
            "mir_call_and_drop_instances": total,
            "external_constant_calls": external,
            "external_resolution_witnesses": external_resolution_witnesses,
            "external_calls_without_mapped_caller": external_unattributed,
            "same_component_calls": internal,
            "cross_component_calls": cross, "violations": [r for r in cross if not r["permitted"]],
            "unknown_calls": unknown,
            "unknown_call_reasons": {reason: sum(row["reason"] == reason for row in unknown)
                                     for reason in sorted({row["reason"] for row in unknown})},
            "drop_analysis": {
                "raw_mir_drop_terminators": raw_drop_terminators,
                "direct_local_drop_implementation_candidates": direct_drop_candidates,
                "fully_resolved_drop_terminators": fully_resolved_drop_terminators,
                "no_local_drop_path_terminators": no_local_drop_path_terminators,
                "unresolved_or_structural_drop_terminators": raw_drop_terminators - fully_resolved_drop_terminators,
                "unresolved_rendered_types": dict(sorted(unresolved_drop_types.items())),
                "direct_drop_witnesses": drop_witnesses,
                "explicit_local_drop_calls": explicit_drop_calls,
                "passive_field_layout_witnesses": passive_field_layout_witnesses,
                "distinct_rendered_types": len(drop_layout_cache),
                "layout_cache_hits": drop_layout_cache_hits,
                "layout_analysis_misses": len(drop_layout_cache) - int(None in drop_layout_cache),
            },
            "proof_limit": "One compiler profile; static MIR constant calls only. Indirect/dynamic calls, drop terminators requiring destructor analysis, and unrecognized local targets remain unknown. Compiler-generated nominal field layout edges without an explicit local Drop::drop implementation remain passive value-layout witnesses; only exact local Drop implementations create executable destructor-call edges. MIR is an unstable compiler output, pinned by compiler version and input hashes. DefIds extracted from MIR function-signature text are diagnostic hints only and may identify bound regions; local implementation joins require exact THIR FnDef identities plus HIR owner spans or exact rustdoc symbols. When supplied, the optional built.after MIR dump manifest joins a MIR body header to one exact THIR caller owner through the compiler impl ordinal in the dump filename; absent or ambiguous identities retain existing unresolved behavior. External trait calls require exact THIR FnDef evidence, a concrete receiver without local/generic type components, and closed trait type arguments (lifetime-only arguments are inert); an external path root alone is insufficient. A closure-containing external call resolves only its callee; closure bodies are separate MIR items and remain subject to their own call analysis. Local closure DefIds require an exact HIR owner-span join. Same-source provenance is included only when a provenance receipt is supplied; external calls without mapped local caller spans are counted separately."}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--rustdoc", type=Path, required=True)
    parser.add_argument("--mir", type=Path, required=True)
    parser.add_argument("--hir", type=Path,
                        help="Compiler HIR tree from the same crate build; exact DefId to OwnerNodes join")
    parser.add_argument("--thir", type=Path,
                        help="Compiler THIR tree from the same crate build; exact typechecked FnDef calls")
    parser.add_argument("--built-mir-manifest", type=Path,
                        help="Optional hash-pinned inventory of one built MIR dump pass")
    parser.add_argument("--provenance", type=Path,
                        help="Optional same-source compiler build provenance receipt")
    parser.add_argument("--output", type=Path, default=ROOT / ".verification-tmp/phase7/compiler-calls.json")
    args = parser.parse_args()
    mapping_path = ROOT / "tools/pitlord/production-components.json"
    dag_path = ROOT / "tools/pitlord/convergence-component-dag.json"
    owner_spans = hir_owner_spans(args.hir.open(encoding="utf-8")) if args.hir else None
    resolved_thir_calls = thir_call_sites(args.thir.open(encoding="utf-8")) if args.thir else None
    built_mir_index = None
    built_mir_files: list[Path] = []
    built_mir_unindexed = []
    if args.built_mir_manifest:
        built_mir_index, built_mir_files, _, built_mir_unindexed = load_built_mir_manifest(
            args.built_mir_manifest)
    with args.mir.open(encoding="utf-8") as stream:
        report = analyze(json.loads(args.rustdoc.read_text(encoding="utf-8")), stream,
                         json.loads(mapping_path.read_text()), json.loads(dag_path.read_text()),
                         owner_spans, resolved_thir_calls, built_mir_index)
    report["built_mir_unindexed_bodies"] = built_mir_unindexed
    input_paths = [args.rustdoc, args.mir, mapping_path, dag_path, Path(__file__)]
    if args.hir:
        input_paths.append(args.hir)
    if args.thir:
        input_paths.append(args.thir)
    if args.built_mir_manifest:
        input_paths.append(args.built_mir_manifest)
        input_paths.extend(built_mir_files)
    report["inputs"] = {str(p): sha256_file(p)
                        for p in input_paths}
    if args.provenance:
        report["inputs"][str(args.provenance)] = sha256_file(args.provenance)
        provenance = json.loads(args.provenance.read_text(encoding="utf-8"))
        report["provenance_validation"] = validate_provenance(
            provenance, args.rustdoc, args.mir, args.hir, args.thir,
            args.built_mir_manifest)
        report["build_provenance"] = provenance
    else:
        report["provenance_validation"] = {"status": "not_supplied", "validated": False,
                                           "errors": []}
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"constant_calls": report["constant_calls"],
                      "cross_component_calls": len(report["cross_component_calls"]),
                      "violations": len(report["violations"]),
                      "unknown_calls": len(report["unknown_calls"]), "output": str(args.output)}))
    return int(bool(report["violations"] or report["unknown_calls"]
                    or not report["provenance_validation"]["validated"]))


if __name__ == "__main__":
    raise SystemExit(main())
