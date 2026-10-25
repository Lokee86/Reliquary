#!/usr/bin/env python3
"""Capture a bounded Phase 7 source-import baseline without refreshing Arcana.

This measurement resolves direct imports from the live production module tree
using the accepted 34-component map and the existing cfg-aware lexical resolver.
It deliberately does not infer symbol calls, suppress old debt, or claim a full
compiler-resolved dependency graph.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
import sys
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
from check_owner_boundaries import module_tree, resolved_source_imports  # noqa: E402
from check_production_component_graph import component_manifest, cycle_witness, sccs  # noqa: E402

MAPPING = ROOT / "tools/pitlord/production-components.json"
DAG = ROOT / "tools/pitlord/convergence-component-dag.json"
OWNER_POLICY = ROOT / "tools/pitlord/owner-boundaries.json"
OUTPUT = ROOT / ".verification-tmp/phase7/current-imports.json"
PHASE5 = ROOT / "benchmarks/dependency-convergence-phase5-source-2026-10-23.json"
PHASE5_ACCEPTANCE = ROOT / "benchmarks/dependency-convergence-phase5-acceptance-2026-10-23.json"
PHASE6 = ROOT / "benchmarks/phase6-final-verification.json"
PHASE6_SOURCE = ROOT / "benchmarks/phase6-source-evidence.json"
MAP_CHECKER = ROOT / "scripts/check_phase7_component_map.py"
INITIAL_B7_MAP_SHA256 = "d8a7065f1572a945287832c642509a2deb0ad5aea05ec6cc36e4a5fa4b2289e1"
INITIAL_B7_MAP_HOLE = "src/runtime_inference/insomnia/native_processor.rs"


def digest(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def hash_or_missing(path: Path) -> dict:
    if not path.is_file():
        return {"path": path.relative_to(ROOT).as_posix(), "available": False, "sha256": None}
    return {"path": path.relative_to(ROOT).as_posix(), "available": True, "sha256": digest(path)}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=OUTPUT)
    args = parser.parse_args()
    output = args.output if args.output.is_absolute() else ROOT / args.output
    if output.resolve() == (ROOT / "benchmarks/phase7-baseline.json").resolve() and output.exists():
        raise ValueError("Phase 7 B7 before-image is immutable; choose a separate current-source receipt")

    for path in (MAPPING, DAG, OWNER_POLICY):
        if not path.is_file():
            raise FileNotFoundError(f"required pinned input is missing: {path}")

    source_paths_before = sorted((ROOT / "src").rglob("*.rs"))
    source_hashes_before = {
        path.relative_to(ROOT).as_posix(): digest(path) for path in source_paths_before
    }
    policy_hashes_before = {
        path.relative_to(ROOT).as_posix(): digest(path) for path in (MAPPING, DAG, OWNER_POLICY)
    }

    mapping, dag = json.loads(MAPPING.read_text(encoding="utf-8")), json.loads(DAG.read_text(encoding="utf-8"))
    owner_policy = json.loads(OWNER_POLICY.read_text(encoding="utf-8"))
    file_map, _baseline_map, components, mapping_conflicts = component_manifest(MAPPING)
    # The accepted component map carries the pinned Windows rustc cfg profile.
    # Do not depend on ignored/generated .verification-tmp state.
    profile = mapping.get("cfg_profile", {})
    rustc_cfg = profile.get("rustc_cfg_facts")
    if not isinstance(rustc_cfg, list):
        raise ValueError("production component map lacks cfg_profile.rustc_cfg_facts")
    component_ids = set(components)
    allowed = {row["id"]: {edge["to"] for edge in row.get("depends_on", [])} for row in dag["components"]}
    value_contracts = {
        (row["consumer"], row["owner"], row["symbol"])
        for row in owner_policy.get("producer_value_contracts", [])
    }
    features = profile.get("features", [])
    facts = set(rustc_cfg) | {f'feature="{feature}"' for feature in features}

    modules = module_tree(ROOT, cfg_facts=facts)
    reachable = {path.replace("\\", "/") for path in modules.values()}
    rust_sources = {p.relative_to(ROOT).as_posix() for p in (ROOT / "src").rglob("*.rs")}
    missing_map = sorted(path for path in reachable if not (
        path in file_map and file_map[path].get("classification") == "production"
        and file_map[path].get("component") in component_ids
    ))
    ambiguous_map = sorted(
        {path for path in reachable if path in file_map and (
            file_map[path].get("classification") != "production"
            or file_map[path].get("component") not in component_ids
        )}
    )
    stale_map = sorted(path for path in file_map if path not in rust_sources)
    unmapped_reachable = sorted(path for path in reachable if path not in file_map)

    imports, uncertain = resolved_source_imports(ROOT, facts)
    edges = []
    for row in imports:
        source = file_map.get(row["source_path"], {})
        target = file_map.get(row["target_path"], {})
        if source.get("classification") != "production" or target.get("classification") != "production":
            continue
        source_component, target_component = source.get("component"), target.get("component")
        if not source_component or not target_component or source_component == target_component:
            continue
        target_symbol = row["target_symbol"]
        immutable = (source_component, target_component, target_symbol) in value_contracts
        permitted_pair = target_component in allowed.get(source_component, set())
        edges.append({
            "source_component": source_component,
            "target_component": target_component,
            "relation": "source-import",
            "category": "approved-immutable-value" if immutable else "strict-import",
            "pair_permitted_by_target_dag": permitted_pair,
            "source": {"path": row["source_path"], "name": row["import"], "line": row["line"]},
            "target": {"path": row["target_path"], "name": target_symbol},
            "canonical": row["canonical"],
            "alias": row.get("alias"),
            "disposition": "approved-immutable-value" if immutable else (
                "permitted-target-pair" if permitted_pair else "UNAPPROVED-PAIR"),
        })
    edges.sort(key=lambda e: (e["source_component"], e["target_component"], e["source"]["path"], e["source"]["line"], e["target"]["name"]))

    unapproved = [e for e in edges if e["disposition"] == "UNAPPROVED-PAIR"]
    pair_index: dict[tuple[str, str], list[dict]] = defaultdict(list)
    for edge in unapproved:
        pair_index[(edge["source_component"], edge["target_component"])].append(edge)
    unapproved_pairs = [
        {"source_component": a, "target_component": b, "edge_count": len(rows), "edges": rows}
        for (a, b), rows in sorted(pair_index.items())
    ]

    strict_edges = [e for e in edges if e["category"] == "strict-import"]
    strict_sccs = sccs(components, strict_edges, {"strict-import"})
    unapproved_sccs = sccs(components, unapproved, {"strict-import"})
    strict_scc_details = []
    for group in strict_sccs:
        members = set(group)
        internal = [e for e in strict_edges if e["source_component"] in members and e["target_component"] in members]
        strict_scc_details.append({
            "components": group,
            "internal_strict_edge_count": len(internal),
            "internal_unapproved_edge_count": sum(e["disposition"] == "UNAPPROVED-PAIR" for e in internal),
            "cycle_witness": cycle_witness(group, strict_edges, {"strict-import"}),
            "internal_edges": internal,
        })
    unresolved_material = bool(uncertain or missing_map or ambiguous_map)

    receipts = {
        "phase5_source": hash_or_missing(PHASE5),
        "phase5_acceptance": hash_or_missing(PHASE5_ACCEPTANCE),
        "phase6_final": hash_or_missing(PHASE6),
        "phase6_source": hash_or_missing(PHASE6_SOURCE),
    }
    rust_files = sorted((ROOT / "src").rglob("*.rs"))
    source_manifest = {
        p.relative_to(ROOT).as_posix(): digest(p)
        for p in rust_files
    }
    source_manifest_sha = hashlib.sha256(json.dumps(source_manifest, sort_keys=True, separators=(",", ":")).encode()).hexdigest()
    head = subprocess.run(["git", "rev-parse", "HEAD"], cwd=ROOT, capture_output=True, text=True, check=True).stdout.strip()
    rustc_version = subprocess.run(["rustc", "--version", "--verbose"], cwd=ROOT, capture_output=True, text=True, check=True).stdout.strip()
    a5_accepted = False  # Phase 5 receipt explicitly leaves A5 limited.
    result = {
        "schema": "dependency-convergence-phase7-baseline.v1",
        "status": "B7 PARTIAL / OPEN — bounded direct source-import inventory; material call/cfg/macro uncertainty remains",
        "captured_at": "2026-10-24",
        "method": "live cfg-aware module_tree plus resolved_source_imports; no Lexicon scan, Arcana export, stale analyzer debt disposition, or historical edge reuse",
        "scope": {
            "production_components": len(components),
            "reachable_module_files": len(reachable),
            "reachable_mapped_production_files": len(reachable) - len(missing_map),
            "reachable_missing_or_nonproduction_map_paths": missing_map,
            "reachable_ambiguous_map_paths": ambiguous_map,
            "unmapped_reachable_paths": unmapped_reachable,
            "stale_manifest_source_paths": stale_map,
            "historical_component_conflicts": mapping_conflicts,
            "mapping_reconciliation": {
                "pre_reconciliation_map_sha256": INITIAL_B7_MAP_SHA256,
                "pre_reconciliation_missing_reachable_path": INITIAL_B7_MAP_HOLE,
                "resolution": "The initial baseline observed this reachable source path without a component row. The current map adds an explicit insomnia assignment, grounded in owner-boundaries.json module insomnia::native_processor and a source hash; no import from this path was assigned by inference.",
                "current_assignment": file_map.get(INITIAL_B7_MAP_HOLE),
            },
            "source_file_count": len(rust_sources),
            "source_manifest_sha256": source_manifest_sha,
            "working_tree_stability": {
                "source_manifest_sha256_before": hashlib.sha256(json.dumps(source_hashes_before, sort_keys=True, separators=(",", ":")).encode()).hexdigest(),
                "source_manifest_sha256_after": source_manifest_sha,
                "production_component_map_sha256_before": policy_hashes_before[MAPPING.relative_to(ROOT).as_posix()],
                "production_component_map_sha256_after": digest(MAPPING),
                "target_dag_sha256_before": policy_hashes_before[DAG.relative_to(ROOT).as_posix()],
                "target_dag_sha256_after": digest(DAG),
                "owner_boundary_policy_sha256_before": policy_hashes_before[OWNER_POLICY.relative_to(ROOT).as_posix()],
                "owner_boundary_policy_sha256_after": digest(OWNER_POLICY),
            },
        },
        "source_imports": edges,
        "unapproved_pairs": unapproved_pairs,
        "strict_import_sccs": strict_sccs,
        "strict_import_scc_details": strict_scc_details,
        "unapproved_strict_import_sccs": unapproved_sccs,
        "uncertainties": {
            "count": len(uncertain),
            "rows": uncertain,
            "coverage_note": "Resolver-reported wildcard/unresolved imports are retained verbatim. Macro expansion, cfg_attr/platform variants beyond the pinned profile, compiler name resolution beyond this lexical resolver, and symbol-level calls are not covered.",
        },
        "proof_limits": [
            "Direct import edges are source-resolved under the single pinned Windows x86_64 MSVC profile; they are not a complete execution/capability graph.",
            f"Current source-resolved strict imports contain {len(strict_sccs)} component SCC(s), of which {len(unapproved_sccs)} are unapproved. This lexical result does not prove implementation-call direction.",
            "Macro-expanded dependencies, cfg_attr and other platform profiles are not exhaustively enumerated.",
            "Historical Phase 0/1–3 edge rows were not reused: their ownership map/profile is not established as same-map comparable to this live A6 map. Phase 6 source evidence is pinned for provenance, not substituted for current imports.",
            "The 34-component target DAG is a policy input only; DAG validation does not demonstrate source conformance.",
        ],
        "prerequisites": {
            "phase5_A5_accepted": a5_accepted,
            "phase5_A5_disposition": "Phase 5 source/behavior is integrated; P5.7.1 full graph proof remains limited and must be discharged at A7. It does not block source cuts.",
            "phase5_source_checkpoint_available": PHASE5.is_file(),
            "phase6_A6_bounded_accepted": json.loads(PHASE6.read_text(encoding="utf-8")).get("status", "") if PHASE6.is_file() else False,
            "overall": "Integrated Phase 5 source/behavior and accepted Phase 6 A6 authorize execution; inherited A5 graph proof remains an A7 closure obligation under the amended specification.",
        },
        "inputs": {
            "git_head": head,
            "rustc_version_verbose": rustc_version,
            "cargo_manifests": {name: hash_or_missing(ROOT / name) for name in ("Cargo.toml", "Cargo.lock", "cli/Cargo.toml", "cli/Cargo.lock")},
            "production_component_map": {**hash_or_missing(MAPPING), "source_checkpoint": mapping.get("source_checkpoint")},
            "target_dag": {**hash_or_missing(DAG), "component_count": len(dag.get("components", [])), "allowed_edge_count": sum(len(r.get("depends_on", [])) for r in dag.get("components", []))},
            "owner_boundary_value_contracts": hash_or_missing(OWNER_POLICY),
            "cfg_profile": {"source": "tools/pitlord/production-components.json cfg_profile", "target": profile.get("target"), "features": features, "rustc_cfg": rustc_cfg, "profile_source_checkpoint": mapping.get("source_checkpoint"), "module_tree_sha256": hashlib.sha256(json.dumps(modules, sort_keys=True, separators=(",", ":")).encode()).hexdigest()},
            "measurement_tool": hash_or_missing(Path(__file__).resolve()),
            "resolver_tool": hash_or_missing(ROOT / "scripts/check_owner_boundaries.py"),
            "graph_tool": hash_or_missing(ROOT / "scripts/check_production_component_graph.py"),
            "component_map_checker": hash_or_missing(MAP_CHECKER),
            "receipts": receipts,
        },
        "counts": {
            "source_import_edges": len(edges),
            "unapproved_edge_rows": len(unapproved),
            "unapproved_component_pairs": len(unapproved_pairs),
            "strict_import_sccs": len(strict_sccs),
            "unapproved_strict_import_sccs": len(unapproved_sccs),
            "uncertain_import_rows": len(uncertain),
        },
        "material_unknowns_remain": True,
    }
    stability = result["scope"]["working_tree_stability"]
    stability["stable"] = (
        source_hashes_before == source_manifest
        and all(policy_hashes_before[path.relative_to(ROOT).as_posix()] == digest(path)
                for path in (MAPPING, DAG, OWNER_POLICY))
    )
    if not stability["stable"]:
        raise RuntimeError("source or architecture policy changed during the bounded measurement")
    output = args.output if args.output.is_absolute() else ROOT / args.output
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"status": result["status"], "output": str(output), "counts": result["counts"], "missing_map": len(missing_map), "uncertain": len(uncertain)}, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
