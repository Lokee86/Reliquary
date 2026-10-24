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


def sha256_file(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


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
                        hir: Path | None = None) -> dict:
    """Validate source stability and artifact identity before trusting provenance."""
    errors = []
    if not isinstance(receipt, dict):
        return {"status": "invalid", "validated": False,
                "errors": ["provenance receipt is not a JSON object"]}
    if receipt.get("schema") != "phase7-compiler-provenance.v1":
        errors.append("unsupported or missing provenance schema")
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
            normalized = sorted(entries, key=lambda row: row["path"]) if isinstance(entries, list) and valid_entries else []
            fingerprint = source_fingerprint(normalized)
            if (not isinstance(manifest, dict) or not isinstance(entries, list) or not valid_entries
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
    return {"status": "invalid" if errors else "validated",
            "validated": not errors, "errors": errors}


SPAN = re.compile(r"(?:scope \d+ at |\+ span: )(.+?):(\d+):\d+")


def compiler_def_identity(rendered: str, method: str) -> tuple[str, str, str] | None:
    """Extract the exact compiler DefId whose terminal item matches the call."""
    for match in re.finditer(
            r"DefId\((\d+:\d+) ~ ([A-Za-z_]\w*)\[([^\]]+)\]::([^)]*?)\)", rendered):
        path = re.sub(r"::'_#?\d*$", "", match[4])
        if path.split("::")[-1] == method:
            return match[2], path, f"DefId({match[1]} ~ {match[2]}[{match[3]}]::{path})"
    return None


HIR_OWNER = re.compile(r"^(DefId\(\d+:\d+ ~ [^\n]+?\)) => OwnerNodes \{$")
HIR_OWNER_ID = re.compile(r"^(\s*)owner_id: (DefId\(\d+:\d+ ~ [^\n]+?\)),\s*$")
HIR_SPAN = re.compile(r"^(\s*)span: (.+?):(\d+):(\d+): (\d+):(\d+) \(#\d+\),\s*$")


def hir_owner_spans(lines) -> dict[str, list[dict]]:
    """Read compiler HIR OwnerNodes, retaining only their exact item spans.

    The DefId is the compiler's identity; the span comes from that DefId's
    OwnerNodes block. The broadest concrete source span in the owner block is
    the enclosing item span. Ambiguous spans are retained and fail closed.
    """
    blocks: dict[str, dict] = {}
    current = None
    for line in lines:
        header = HIR_OWNER.match(line.rstrip("\r\n"))
        if header:
            current = header[1]
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
    return result


def call_method_name(target: str | None) -> str | None:
    if not target:
        return None
    path = target.rsplit(">::", 1)[-1] if target.startswith("<") else target
    return path.rsplit("::", 1)[-1].split("::<", 1)[0]


def mir_calls(lines):
    """Yield compiler constant-function operands at actual call terminators."""
    pending = None
    for line in lines:
        if " -> [return:" in line or " -> unwind " in line:
            match = SPAN.search(line)
            evidence_hash = hashlib.sha256(line.encode()).hexdigest()
            pending = {"source_path": normalize_source_path(match[1]),
                       "source_line": int(match[2]), "mir_line_sha256": evidence_hash} if match else {
                           "source_path": None, "mir_line_sha256": evidence_hash}
            if line.lstrip().startswith("drop("):
                pending["terminator_kind"] = "drop"
        if pending and "+ const_: Const {" in line:
            match = re.search(r", val: Value\((.*)\) \}\s*$", line)
            if match:
                operand = match[1]
                target = callable_const_value(operand)
                rendered_target = without_generics(target) if target else None
                identity = compiler_def_identity(line, call_method_name(rendered_target)) \
                    if rendered_target else None
                yield {**pending, "target": rendered_target,
                       "definition_crate": identity[0] if identity else None,
                       "compiler_def_path": identity[1] if identity else None,
                       "compiler_def_id": identity[2] if identity else None,
                       "definition_path": identity[1] if identity and identity[0] == "reliquary_memory" else None,
                       **({} if target else {"reason": "unrecognized compiler constant operand"})}
            else:
                yield {**pending, "target": None, "reason": "unrecognized compiler constant operand"}
            pending = None
        elif pending and re.match(r"\s*\}", line):
            reason = ("drop terminator requires destructor analysis" if pending.get("terminator_kind") == "drop"
                      else "indirect or nonconstant call terminator")
            yield {**pending, "target": None, "reason": reason}
            pending = None


def analyze(doc: dict, lines, mapping: dict, dag: dict,
            hir_spans: dict[str, list[dict]] | None = None) -> dict:
    symbols, local_roots, external_roots = compiler_symbols(doc)
    files = {normalize_source_path(row["path"]): row for row in mapping["files"]}
    allowed = {row["id"]: {e["to"] for e in row.get("depends_on", [])}
               for row in dag["components"]}
    cross, unknown = [], []
    total = external = external_unattributed = internal = 0
    hir_spans = hir_spans or {}
    for call in mir_calls(lines):
        total += 1
        target = call["target"]
        source_path = normalize_source_path(call["source_path"]) if call["source_path"] else None
        source = files.get(source_path, {}).get("component")
        is_closure = bool(target and ("{closure" in target or "{{closure" in target))
        matches = symbols.get(target, []) if target and not is_closure else []
        resolution = "exact-rustdoc-symbol" if matches else None
        def_id = call.get("compiler_def_id")
        if not matches and def_id and not is_closure:
            spans = hir_spans.get(def_id, [])
            if len(spans) == 1:
                matches = [spans[0]]
                resolution = "exact-mir-defid-hir-owner-span"
        if not matches:
            receiver_root, trait_root = compiler_target_roots(target)
            external_receiver = receiver_root in external_roots and receiver_root not in local_roots
            direct_external_def = (not (target or "").startswith("<")
                                   and call.get("definition_crate") in external_roots
                                   and "::{impl#" in (call.get("compiler_def_path") or ""))
            if (not is_closure and (external_receiver or direct_external_def)):
                external += 1
                external_unattributed += int(source is None)
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
    return {"schema": "phase7-compiler-calls.v1", "constant_calls": total,
            "external_constant_calls": external,
            "external_calls_without_mapped_caller": external_unattributed,
            "same_component_calls": internal,
            "cross_component_calls": cross, "violations": [r for r in cross if not r["permitted"]],
            "unknown_calls": unknown,
            "unknown_call_reasons": {reason: sum(row["reason"] == reason for row in unknown)
                                     for reason in sorted({row["reason"] for row in unknown})},
            "proof_limit": "One compiler profile; static MIR constant calls only. Indirect/dynamic calls, drop terminators requiring destructor analysis, and unrecognized local targets remain unknown. MIR is an unstable compiler output, pinned by compiler version and input hashes. Exact local DefIds may be joined to compiler HIR OwnerNodes; shortened symbols without an exact identity remain unknown. Same-source provenance is included only when a provenance receipt is supplied; external calls without mapped local caller spans are counted separately."}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--rustdoc", type=Path, required=True)
    parser.add_argument("--mir", type=Path, required=True)
    parser.add_argument("--hir", type=Path,
                        help="Compiler HIR tree from the same crate build; exact DefId to OwnerNodes join")
    parser.add_argument("--provenance", type=Path,
                        help="Optional same-source compiler build provenance receipt")
    parser.add_argument("--output", type=Path, default=ROOT / ".verification-tmp/phase7/compiler-calls.json")
    args = parser.parse_args()
    mapping_path = ROOT / "tools/pitlord/production-components.json"
    dag_path = ROOT / "tools/pitlord/convergence-component-dag.json"
    owner_spans = hir_owner_spans(args.hir.open(encoding="utf-8")) if args.hir else None
    with args.mir.open(encoding="utf-8") as stream:
        report = analyze(json.loads(args.rustdoc.read_text(encoding="utf-8")), stream,
                         json.loads(mapping_path.read_text()), json.loads(dag_path.read_text()),
                         owner_spans)
    input_paths = [args.rustdoc, args.mir, mapping_path, dag_path, Path(__file__)]
    if args.hir:
        input_paths.append(args.hir)
    report["inputs"] = {str(p): hashlib.sha256(p.read_bytes()).hexdigest()
                        for p in input_paths}
    if args.provenance:
        report["inputs"][str(args.provenance)] = sha256_file(args.provenance)
        provenance = json.loads(args.provenance.read_text(encoding="utf-8"))
        report["provenance_validation"] = validate_provenance(provenance, args.rustdoc, args.mir, args.hir)
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
