#!/usr/bin/env python3
"""Compare cfg-aware production source imports at the Phase 6 baseline and current tree.

This uses the existing owner-boundary import resolver and one identical component
map at both source checkpoints. It reports source-import deltas, not compiler
resolved calls. Moving a public extension changes its source location and owner
classification; it is not reported as an eliminated logical dependency unless
the exact import edge is absent and source review confirms no replacement.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
import check_owner_boundaries as owner_resolver  # noqa: E402
import check_production_component_graph as component_graph  # noqa: E402


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def file_map(mapping: dict) -> tuple[dict[str, dict], list[str]]:
    rows = {row["path"].replace("\\", "/"): row for row in mapping.get("files", [])}
    for row in mapping.get("historical_files", mapping.get("historical_only_files", [])):
        path = row["path"].replace("\\", "/")
        path = path if path.startswith("src/") else f"src/{path}"
        rows.setdefault(path, {**row, "path": path})
    dag = json.loads((ROOT / "tools/pitlord/convergence-component-dag.json").read_text(encoding="utf-8"))
    return rows, [row["id"] for row in dag["components"]]


def import_rows(source_root: Path, files: dict[str, dict], facts: set[str], components: list[str]) -> tuple[list[dict], list[dict]]:
    policy = json.loads((ROOT / "tools/pitlord/owner-boundaries.json").read_text(encoding="utf-8"))
    values = {(row["consumer"], row["owner"], row["symbol"])
              for row in policy["producer_value_contracts"]}
    edges, uncertain = component_graph.source_import_edges(source_root, files, facts, components, values)
    for edge in edges:
        edge["relation"] = "imports"
        edge["source"]["line"] = edge["source"].get("line")
    # Keep resolver uncertainty that touches a mapped production module, while
    # labeling it instead of inferring a component edge.
    bounded = []
    for row in uncertain:
        if files.get(row["source_path"], {}).get("classification") == "production":
            bounded.append(row)
    return edges, bounded


def edge_key(edge: dict) -> tuple[str, str, str, str, str]:
    return (
        edge["source"]["path"].replace("\\", "/"),
        edge["target"]["path"].replace("\\", "/"),
        edge["target"].get("name", ""),
        edge["source_component"],
        edge["target_component"],
    )


def moved_extension_rows(baseline: Path, current: Path) -> list[dict]:
    """Pair exact public method declarations from known source families to facade files."""
    pairs = {
        "src/archive/archive_search.rs": "src/facade/cva_archive_search.rs",
        "src/retrieval/conversation_search.rs": "src/facade/cva_retrieval_conversation_search.rs",
        "src/retrieval/lexical_search.rs": "src/facade/cva_retrieval_lexical_search.rs",
        "src/retrieval/memory_search.rs": "src/facade/cva_retrieval_memory_search.rs",
        "src/retrieval/semantic_search.rs": "src/facade/cva_retrieval_semantic_search.rs",
        "src/retrieval/search.rs": "src/facade/cva_retrieval_search.rs",
        "src/retrieval/memory_candidate_pool.rs": "src/facade/cva_memory_candidate_pool.rs",
        "src/perception/entity_audit.rs": "src/facade/cva_entity_audit.rs",
        "src/perception/entity_candidates.rs": "src/facade/cva_entity_candidates.rs",
        "src/perception/entity_reconciliation.rs": "src/facade/cva_entity_reconciliation.rs",
        "src/perception/entity_resolution_batch.rs": "src/facade/cva_entity_resolution_batch.rs",
        "src/perception/entity_resolution_processor.rs": "src/facade/cva_entity_resolution_processor.rs",
    }
    import re
    declaration = re.compile(r"\bpub(?:\([^)]*\))?\s+fn\s+([A-Za-z_][A-Za-z0-9_]*)\s*\(")
    result = []
    for old, new in pairs.items():
        old_path, new_path = baseline / old, current / new
        if not old_path.is_file():
            continue
        old_text = old_path.read_text(encoding="utf-8")
        old_names = sorted(set(declaration.findall(old_text)))
        new_text = new_path.read_text(encoding="utf-8") if new_path.is_file() else ""
        new_names = sorted(set(declaration.findall(new_text)))
        for name in old_names:
            result.append({
                "baseline_source": old,
                "current_source": new,
                "method": name,
                "declaration_present_in_target": name in new_names,
                "interpretation": "method location reclassified to public façade; declaration persistence alone is not an edge-elimination proof",
            })
    return result


def perception_closure(current_edges: list[dict], closure: dict, pinned_imports: dict) -> dict:
    src_paths = set(closure["source_paths"])
    target_prefixes = tuple(closure["target_path_prefixes"])
    # Freeze the original 82-row ownership register from its source-import
    # receipt. The caller's final map may have reclassified the old source paths
    # as Perception implementation, which must not erase the original register.
    rows = [
        edge for edge in pinned_imports.get("edges", [])
        if edge["source"]["path"] in src_paths
        and edge["target"]["path"].startswith(target_prefixes)
        and edge["source_component"] == closure["source_component"]
        and edge["target_component"] == closure["target_component"]
    ]
    matched = []
    for edge in rows:
        logical = [candidate for candidate in current_edges
                   if candidate["target"]["path"] == edge["target"]["path"]
                   and candidate["target"].get("name") == edge["target"].get("name")
                   and candidate["source_component"] == closure["source_component"]
                   and candidate["target_component"] == closure["target_component"]]
        matched.append({
            "baseline_pinned_witness": edge,
            "baseline_same_map_component_view": "classified independently by the provided map; see source-import delta rows",
            "current_same_path_import_witnesses": logical,
            "status": "retained-logical-target-import" if logical else "no_same_path_import_witness; symbol/call equivalence unproven",
            "actual_dependency_elimination_claim": False,
        })
    return {
        "baseline_register_row_count": closure["row_count"],
        "pinned_baseline_witness_count": len(rows),
        "rows": matched,
        "caveat": "Pinned baseline witnesses remain authoritative even when the final same-map classification changes. Current target-symbol imports are witnesses only; missing matches do not prove call-edge elimination.",
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--mapping", type=Path, default=ROOT / ".verification-tmp/phase6-baseline-components.json")
    parser.add_argument("--output", type=Path, default=ROOT / "benchmarks/phase6-source-evidence.json")
    parser.add_argument("--baseline-src", type=Path, default=ROOT / ".verification-tmp/phase6-before-src")
    args = parser.parse_args()
    mapping = json.loads(args.mapping.read_text(encoding="utf-8"))
    files, components = file_map(mapping)
    facts = set(mapping["cfg_profile"]["rustc_cfg_facts"])
    before_root = args.baseline_src.resolve()
    if (before_root / "src").is_dir():
        before_root = before_root / "src"
    if not (before_root / "lib.rs").is_file():
        raise SystemExit(f"baseline source tree missing lib.rs: {before_root}")

    baseline_edges, baseline_uncertain = import_rows(before_root.parent, files, facts, components)
    current_edges, current_uncertain = import_rows(ROOT, files, facts, components)
    before_by_key = {edge_key(row): row for row in baseline_edges}
    after_by_key = {edge_key(row): row for row in current_edges}
    closure = json.loads((ROOT / "tools/pitlord/convergence-phase6-perception-facade-import-closure.json").read_text(encoding="utf-8"))
    pinned_imports = json.loads((ROOT / "benchmarks/phase6-before-imports.json").read_text(encoding="utf-8"))
    dag = json.loads((ROOT / "tools/pitlord/convergence-component-dag.json").read_text(encoding="utf-8"))
    allowed = {row["id"]: {edge["to"] for edge in row.get("depends_on", [])} for row in dag["components"]}
    removed = [before_by_key[key] for key in sorted(before_by_key.keys() - after_by_key.keys())]
    added = [after_by_key[key] for key in sorted(after_by_key.keys() - before_by_key.keys())]
    for row in current_edges:
        source, target = row["source_component"], row["target_component"]
        row["disposition"] = ("accepted-immutable-value" if row["category"] == "approved-immutable-value"
                              else "accepted-target-dependency" if target in allowed.get(source, set())
                              else "strict-import-requires-source-review")
    strict_groups = component_graph.sccs(components, current_edges, {"strict-import"})
    output = {
        "schema": "dependency-convergence-phase6-source-evidence.v1",
        "baseline_commit": "17514f007e48e81d5ea1ec7f41088564de9e7411",
        "baseline_source_root": str(before_root),
        "mapping_path": str(args.mapping),
        "mapping_sha256": sha256(args.mapping),
        "cfg_profile": mapping["cfg_profile"],
        "scope": "cfg-aware source imports between mapped production files, resolved by scripts/check_owner_boundaries.py; not compiler call graph",
        "counts": {
            "baseline_production_import_edges": len(baseline_edges),
            "current_production_import_edges": len(current_edges),
            "exact_source_import_edges_removed": len(removed),
            "exact_source_import_edges_added": len(added),
            "baseline_uncertain_imports": len(baseline_uncertain),
            "current_uncertain_imports": len(current_uncertain),
        },
        "actual_dependency_elimination": {
            "proven_count": 0,
            "reason": "Source import disappearance or module relocation is insufficient to prove semantic/call edge elimination; API and call-site review is required.",
        },
        "exact_removed_import_rows": removed,
        "exact_added_import_rows": added,
        "current_import_rows": current_edges,
        "remaining_strict_component_sccs": strict_groups,
        "remaining_cycle_witnesses": [component_graph.cycle_witness(group, current_edges, {"strict-import"}) for group in strict_groups],
        "uncertain": {"baseline": baseline_uncertain, "current": current_uncertain},
        "ownership_reclassification_and_relocation": moved_extension_rows(before_root.parent, ROOT),
        "perception_82_edge_closure": perception_closure(current_edges, closure, pinned_imports),
        "limitations": [
            "Path/symbol import resolution is lexical and cfg-aware, not compiler-resolved call analysis.",
            "Identical 34-component mapping is used for both checkpoints; this isolates source changes from later map reassignment.",
            "Rows that disappear because a public extension moved are reported as source import delta only; no dependency elimination is inferred.",
            "The exact 82-row historical Perception denominator is retained in output, including witness count mismatches.",
        ],
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(output, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"output": str(args.output), **output["counts"], "perception_baseline_witnesses": output["perception_82_edge_closure"]["pinned_baseline_witness_count"]}, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
