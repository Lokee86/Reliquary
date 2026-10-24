#!/usr/bin/env python3
"""Prepare a narrow Phase 6 source-map delta. Writes only with --write.

This updates source ownership rows for changed, added, deleted, and relocated
Rust files. It does not alter the target DAG, deferred-edge register, or
historical rows other than appending before-images for paths removed at B6.
It does not claim compiler or symbol inventory for newly added files.
"""
from __future__ import annotations

import argparse
import copy
import hashlib
import json
import subprocess
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_MAP = ROOT / "tools/pitlord/production-components.json"
DEFAULT_EVIDENCE = ROOT / "benchmarks/phase6-source-evidence.json"
DEFAULT_BASE = "17514f007e48e81d5ea1ec7f41088564de9e7411"


def git(*args: str) -> str:
    return subprocess.run(
        ["git", *args], cwd=ROOT, check=True, capture_output=True, text=True
    ).stdout


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def source_changes(base: str) -> tuple[set[str], set[str], set[str]]:
    changed: set[str] = set()
    deleted: set[str] = set()
    for line in git("diff", "--name-status", "--find-renames", base, "--", "src").splitlines():
        fields = line.split("\t")
        status = fields[0]
        if status.startswith("R"):
            deleted.add(fields[1])
            changed.add(fields[2])
        elif status == "D":
            deleted.add(fields[1])
        elif status in {"A", "M", "T"}:
            changed.add(fields[1])
    for path in git("ls-files", "--others", "--exclude-standard", "--", "src").splitlines():
        if path.endswith(".rs"):
            changed.add(path.replace("\\", "/"))
    return changed, deleted, set()


def role(path: str, previous: str | None) -> tuple[str | None, str | None]:
    if path in {"src/external_source/mod.rs", "src/external_source/processing.rs", "src/external_source/provenance.rs"}:
        return "external-sources", "External Sources provider, search, preparation and evidence policy; public owner extensions were physically extracted to facade."
    if path.startswith("src/runtime_inference/external_owner_access") or path == "src/runtime_inference/agentic_memory_grading.rs":
        return "runtime-gateway", "Concrete mounted owner access over existing owner stores and immutable identity/activity facts; no public product dependency."
    if path.startswith("src/retrieval/external_vector"):
        return "vectors", "Vectors-owned packed-row validation/scoring over the packed-vector owner's concrete read view."
    if path.startswith("src/runtime_inference/perception_publication"):
        return "runtime-gateway", "Concrete native Perception publication over existing owner stores; shared by mounted execution and public adapters without product handles or provider callbacks."
    if path.startswith("src/facade/"):
        return "public-consumers", "Phase 6 public extension or composition source; placed above owner/domain implementation."
    if path == "src/archive/archive_search.rs":
        return "archive-store", "Archive owns search result/value DTOs and search policy; public Cva adapter resides in facade."
    if path == "src/retrieval/vector_generation_store.rs":
        return "vector-store", "VectorGenerationStore is a durable owner store; the baseline owner-boundaries manifest explicitly assigns this path to vector-store. Retrieval consumes it through store APIs."
    if path.startswith("src/retrieval/"):
        return "retrieval", "Retrieval policy/implementation remains Retrieval-owned; public Cva/Phylactery adapters reside in facade."
    if path == "src/runtime_inference/runtime_vector_step.rs":
        return "vectors", "Owner-qualified vector batch and prepare/validate/commit mechanics; façade owner adapters live under src/facade."
    if path in {"src/runtime_inference/configured_runtime_insomnia_model.rs", "src/runtime_inference/configured_runtime_vectors_model.rs"}:
        return "runtime-gateway", "ConfiguredRuntime-owned immutable offline options/reports; no public product handles or provider execution."
    if path == "src/perception/entity_resolution_owner.rs":
        return "perception", "Cross-owner Entity/Memory admission remains visibly Perception service policy; it is not relabeled owner-lifecycle because its current imports expose those dependencies."
    if path.startswith("src/perception/"):
        name = Path(path).name
        if name.endswith(("_model.rs", "_error.rs")) or name in {"entity_store.rs", "entity_codec.rs"}:
            return "perception-store", "Perception-owned durable model, error, codec, or store validation."
        return "perception", "Perception-owned candidate, audit, reconciliation, or resolution calculation; public owner adapters reside in facade."
    return previous, None


def reachable_production_paths() -> set[str]:
    # Reuse the repository's cfg-aware module resolver; importing it does not
    # execute the checker or mutate repository state.
    sys_path = str(ROOT / "scripts")
    import sys

    if sys_path not in sys.path:
        sys.path.insert(0, sys_path)
    from check_owner_boundaries import module_tree  # type: ignore[import-not-found]

    mapping = json.loads(DEFAULT_MAP.read_text(encoding="utf-8"))
    facts = set(mapping["cfg_profile"]["rustc_cfg_facts"])
    return {path.replace("\\", "/") for path in module_tree(ROOT, facts).values()}


def extension_imports(source: str) -> list[str]:
    import re

    return [
        match.group(0).strip()
        for match in re.finditer(
            r"(?ms)(?:pub\s+)?use\s+[^;]*\b(?:Cva|Phylactery)\b[^;]*;", source
        )
    ]


def apply_delta(mapping: dict[str, Any], changed: set[str], deleted: set[str], base: str) -> dict[str, Any]:
    # These sibling service/value files had inherited aggregate public roles.
    # Review their real responsibilities after extension extraction even when
    # their own text did not change. This is role correction, not edge removal.
    changed.update({"src/external_source/mod.rs", "src/external_source/processing.rs", "src/external_source/provenance.rs"})
    result = copy.deepcopy(mapping)
    before_rows = {row["path"]: row for row in result["files"]}
    historical_paths = {row["path"] for row in result.get("historical_files", [])}
    existing_components = {entry["id"]: entry for entry in result["components"]}
    live = {path.replace("\\", "/") for path in reachable_production_paths()}
    relocation = {}
    for row in json.loads(DEFAULT_EVIDENCE.read_text(encoding="utf-8")).get(
        "ownership_reclassification_and_relocation", []
    ):
        old, new = row.get("baseline_source"), row.get("current_source")
        if old and new and old != new:
            relocation[old] = new
            changed.add(new)
            if old not in live:
                deleted.add(old)

    # Keep test/uncertain rows and unrelated production rows untouched.
    removals = set(deleted) | {old for old in relocation if old not in live}
    for old in sorted(removals):
        row = before_rows.get(old)
        if row is None:
            continue
        result["files"].remove(row)
        component = row.get("component")
        if component in existing_components:
            entry = existing_components[component]
            paths = entry.get("current_production_source_files", [])
            entry["current_production_source_files"] = [path for path in paths if path != old]
        if old not in historical_paths:
            image = copy.deepcopy(row)
            image["phase6_before_image"] = {
                "base_checkpoint": base,
                "reason": "source path deleted or relocated during Phase 6; retained for same-map before/after evidence",
            }
            result.setdefault("historical_files", []).append(image)

    rows = {row["path"]: row for row in result["files"]}
    for path in sorted(changed):
        normalized = path.replace("\\", "/")
        file_path = ROOT / normalized
        if normalized not in live or not file_path.is_file() or not normalized.startswith("src/"):
            continue
        old_row = rows.get(normalized) or before_rows.get(normalized)
        old_component = old_row.get("component") if old_row else None
        component, rationale = role(normalized, old_component)
        if component is None:
            continue
        if old_row and normalized in rows:
            row = rows[normalized]
            prior_component = row.get("component")
            row["component"] = component
            row["source_sha256"] = sha256(file_path)
            row["evidence"] = rationale or row.get("evidence", "")
            row["certainty"] = "reviewed-explicit"
            # The old source spans/compiler counts no longer describe changed text.
            row["compiler_inventory_match"] = False
            row["compiler_item_count"] = None
            row["symbol_spans_sha256"] = None
            if prior_component != component:
                if prior_component in existing_components:
                    prior_paths = existing_components[prior_component].get(
                        "current_production_source_files", []
                    )
                    existing_components[prior_component]["current_production_source_files"] = [
                        entry for entry in prior_paths if entry != normalized
                    ]
                if component in existing_components:
                    current_paths = existing_components[component].setdefault(
                        "current_production_source_files", []
                    )
                    if normalized not in current_paths:
                        current_paths.append(normalized)
        else:
            row = {
                "path": normalized,
                "component": component,
                "classification": "production",
                "reachability": "reachable-profile-module",
                "evidence": rationale or "Phase 6 source role retained from existing component ownership.",
                "certainty": "reviewed-explicit",
                "source_sha256": sha256(file_path),
                "compiler_inventory_match": False,
                "compiler_item_count": None,
                "symbol_spans_sha256": None,
            }
            result["files"].append(row)
            if component in existing_components:
                entry = existing_components[component]
                paths = entry.setdefault("current_production_source_files", [])
                if normalized not in paths:
                    paths.append(normalized)
                    paths.sort()
        rows[normalized] = row
        if component != old_component:
            for entry in result["components"]:
                paths = entry.get("current_production_source_files", [])
                entry["current_production_source_files"] = [p for p in paths if p != normalized]
            if component in existing_components:
                existing_components[component].setdefault("current_production_source_files", []).append(normalized)

    # Refresh only extension records on the exact changed/relocated paths.
    extension_paths = changed | set(relocation.values()) | set(relocation)
    result["facade_extensions"] = [
        row for row in result.get("facade_extensions", []) if row.get("path") not in extension_paths
    ]
    import re

    for path in sorted(extension_paths & live):
        file_path = ROOT / path
        if not file_path.is_file() or not path.startswith("src/facade/"):
            continue
        source = file_path.read_text(encoding="utf-8")
        if re.search(r"\bimpl\s+(?:crate\s*::\s*)?(?:Cva|Phylactery)\b", source):
            result["facade_extensions"].append({
                "path": path,
                "component": "public-consumers",
                "role": "Cva/Phylactery extension implementation",
                "disposition": "phase-6-facade-placement",
                "source_imports": extension_imports(source),
                "evidence": "direct impl Cva/Phylactery declaration in facade source; lower implementation remains owner-qualified",
            })

    for entry in result["components"]:
        entry["current_production_source_files"] = sorted(set(entry.get("current_production_source_files", [])))
    # The public bindings now own their lower inference cores. Keep the type
    # surface inventory aligned with the actual root reexports, rather than
    # retaining the former Perception definitions as public API owners.
    for row in result.get("root_reexports", []):
        module = {
            "EntityResolver": "cva_entity_resolver",
            "EntityResolutionEngine": "cva_entity_resolution_engine",
        }.get(row.get("exported_name"))
        if module:
            row.update(path=f"{module}::{row['exported_name']}",
                       defining_module=module,
                       defining_source=f"src/facade/{module}.rs",
                       defining_component="public-consumers")
    result["phase6_map_delta"] = {
        "base_checkpoint": base,
        "updated_paths": sorted(changed & live),
        "deleted_or_relocated_paths": sorted(removals),
        "added_reachable_paths": sorted((live & changed) - set(before_rows)),
        "compiler_inventory_refreshed": False,
        "note": "Source hashes and explicit ownership rows only; compiler and symbol inventories require their separate acceptance refresh.",
    }
    return result


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--base", default=DEFAULT_BASE)
    parser.add_argument("--map", type=Path, default=DEFAULT_MAP)
    parser.add_argument("--write", action="store_true", help="write the reviewed delta to --map")
    args = parser.parse_args()
    mapping = json.loads(args.map.read_text(encoding="utf-8"))
    changed, deleted, _ = source_changes(args.base)
    updated = apply_delta(mapping, changed, deleted, args.base)
    text = json.dumps(updated, indent=2, ensure_ascii=False) + "\n"
    if args.write:
        args.map.write_text(text, encoding="utf-8")
        print(f"Updated {args.map.relative_to(ROOT)}")
    else:
        print(json.dumps(updated.get("phase6_map_delta", {}), indent=2))
        print("Dry run only; pass --write after review to update the source map.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
