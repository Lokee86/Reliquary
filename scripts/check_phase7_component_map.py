#!/usr/bin/env python3
"""Check that cfg-reachable production Rust modules have exact component owners.

This is a source-map ratchet, not a dependency or call-graph proof. It uses the
recorded cfg profile and existing module resolver; it does not invoke Cargo or
Arcana.
"""
from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_MAP = ROOT / "tools/pitlord/production-components.json"
DEFAULT_DAG = ROOT / "tools/pitlord/convergence-component-dag.json"
sys.path.insert(0, str(ROOT / "scripts"))
import check_owner_boundaries as owner_resolver  # noqa: E402


def validate_map(reachable_paths: set[str], rows: list[dict[str, Any]], component_ids: set[str]) -> list[dict[str, str]]:
    """Return deterministic mapping errors for reachable production paths."""
    normalized = [dict(row, path=row.get("path", "").replace("\\", "/")) for row in rows]
    by_path: dict[str, list[dict[str, Any]]] = {}
    for row in normalized:
        by_path.setdefault(row["path"], []).append(row)

    errors: list[dict[str, str]] = []
    for path, path_rows in sorted(by_path.items()):
        if path.startswith("src/") and len(path_rows) > 1:
            errors.append({"path": path, "problem": "duplicate-map-entry"})

    for row in normalized:
        component = row.get("component")
        if isinstance(component, str) and component and component not in component_ids:
            errors.append({"path": row["path"], "problem": "unknown-component"})

    for path in sorted(path.replace("\\", "/") for path in reachable_paths):
        matches = by_path.get(path, [])
        if not matches:
            errors.append({"path": path, "problem": "reachable-production-file-unmapped"})
            continue
        row = matches[0]
        if row.get("classification") != "production":
            errors.append({"path": path, "problem": "reachable-file-not-classified-production"})
        component = row.get("component")
        if not isinstance(component, str) or not component:
            errors.append({"path": path, "problem": "reachable-production-file-unassigned"})
    return errors


def check(map_path: Path, dag_path: Path, source_root: Path = ROOT) -> dict[str, Any]:
    mapping = json.loads(map_path.read_text(encoding="utf-8"))
    dag = json.loads(dag_path.read_text(encoding="utf-8"))
    profile = mapping.get("cfg_profile", {})
    cfg_facts = profile.get("rustc_cfg_facts")
    if not isinstance(cfg_facts, list) or not cfg_facts or not all(isinstance(fact, str) for fact in cfg_facts):
        raise ValueError("component map requires a nonempty recorded rustc cfg profile")
    features = profile.get("features", [])
    if not isinstance(features, list) or not all(isinstance(feature, str) for feature in features):
        raise ValueError("component map features must be a list of names")
    facts = set(cfg_facts) | {f'feature="{feature}"' for feature in features}
    reachable = {
        path.replace("\\", "/")
        for path in owner_resolver.module_tree(source_root.resolve(), facts).values()
    }
    ids = {row["id"] for row in dag.get("components", [])}
    errors = validate_map(reachable, mapping.get("files", []), ids)
    return {
        "schema": "phase7-component-map-check.v1",
        "source_root": str(source_root.resolve()),
        "cfg_facts_count": len(facts),
        "reachable_production_module_count": len(reachable),
        "component_count": len(ids),
        "errors": errors,
        "status": "pass" if not errors else "fail",
        "proof_limit": "Validates cfg-reachable file ownership only; does not prove source dependencies, symbol calls, macro expansion, or other cfg profiles.",
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--map", type=Path, default=DEFAULT_MAP)
    parser.add_argument("--dag", type=Path, default=DEFAULT_DAG)
    parser.add_argument("--json", action="store_true", help="print machine-readable JSON")
    args = parser.parse_args()
    result = check(args.map, args.dag)
    if args.json:
        print(json.dumps(result, indent=2, sort_keys=True))
    else:
        print(f"Phase 7 component map: {result['status']} ({result['reachable_production_module_count']} reachable modules, {len(result['errors'])} errors)")
        for error in result["errors"]:
            print(f"{error['problem']}: {error['path']}")
        print(result["proof_limit"])
    return 0 if not result["errors"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
