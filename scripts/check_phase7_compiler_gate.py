#!/usr/bin/env python3
"""Require same-source, policy-bound compiler evidence for Phase 7 acceptance."""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
import check_phase7_compiler_calls as compiler_checker


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def evidence_path(value: str, root: Path = ROOT) -> Path:
    path = Path(value)
    if not path.is_absolute():
        path = root / Path(value.replace("\\", "/"))
    return path.resolve()


def validate_report(report_path: Path, root: Path = ROOT) -> dict:
    report_path = report_path.resolve()
    errors: list[str] = []
    try:
        report = json.loads(report_path.read_text(encoding="utf-8"))
    except (OSError, ValueError) as error:
        return {"status": "invalid", "validated": False,
                "errors": [f"cannot read compiler call report: {error}"]}
    if not isinstance(report, dict) or report.get("schema") != "phase7-compiler-calls.v1":
        return {"status": "invalid", "validated": False,
                "errors": ["unsupported or missing compiler call report schema"]}

    inputs = report.get("inputs")
    if not isinstance(inputs, dict):
        inputs = {}
        errors.append("compiler report inputs are missing")
    resolved_inputs: dict[Path, str] = {}
    for raw_path, expected in inputs.items():
        if not isinstance(raw_path, str) or not isinstance(expected, str):
            errors.append("compiler report input entry is malformed")
            continue
        path = evidence_path(raw_path, root)
        try:
            valid_hash = path.is_file() and sha256(path) == expected
        except OSError:
            valid_hash = False
        if not valid_hash:
            errors.append(f"compiler report input is missing or changed: {raw_path}")
        resolved_inputs[path] = expected

    checker_path = (root / "scripts/check_phase7_compiler_calls.py").resolve()
    map_path = (root / "tools/pitlord/production-components.json").resolve()
    dag_path = (root / "tools/pitlord/convergence-component-dag.json").resolve()
    for label, path in (("compiler checker", checker_path),
                        ("production component map", map_path),
                        ("component DAG", dag_path)):
        try:
            current_hash = sha256(path)
        except OSError:
            current_hash = None
        if current_hash is None or resolved_inputs.get(path) != current_hash:
            errors.append(f"compiler report does not pin the current {label}")

    provenance = report.get("build_provenance")
    artifacts = provenance.get("artifacts") if isinstance(provenance, dict) else None
    artifact_paths: dict[str, Path] = {}
    if not isinstance(artifacts, dict):
        errors.append("compiler build provenance or artifact identities are missing")
    else:
        for name in ("rustdoc", "mir", "hir", "thir"):
            row = artifacts.get(name)
            if not isinstance(row, dict) or not isinstance(row.get("path"), str):
                errors.append(f"compiler provenance is missing the {name} artifact")
                continue
            path = evidence_path(row["path"], root)
            artifact_paths[name] = path
            try:
                artifact_hash = sha256(path)
            except OSError:
                artifact_hash = None
            if artifact_hash is None or resolved_inputs.get(path) != artifact_hash:
                errors.append(f"compiler report inputs omit or disagree with the {name} artifact")
        built_row = artifacts.get("built_mir_manifest")
        if built_row is not None:
            if not isinstance(built_row, dict) or not isinstance(built_row.get("path"), str):
                errors.append("compiler provenance has a malformed built MIR manifest artifact")
            else:
                path = evidence_path(built_row["path"], root)
                artifact_paths["built_mir_manifest"] = path
                try:
                    artifact_hash = sha256(path)
                except OSError:
                    artifact_hash = None
                if artifact_hash is None or resolved_inputs.get(path) != artifact_hash:
                    errors.append("compiler report inputs omit or disagree with the built MIR manifest")
                else:
                    try:
                        built_manifest = json.loads(path.read_text(encoding="utf-8"))
                        entries = built_manifest.get("files", [])
                        if not isinstance(entries, list):
                            raise ValueError("built MIR file inventory is malformed")
                        for entry in entries:
                            if not isinstance(entry, dict) or not isinstance(entry.get("path"), str) \
                                    or not isinstance(entry.get("sha256"), str):
                                raise ValueError("built MIR file inventory entry is malformed")
                            dump_path = evidence_path(entry["path"], root)
                            if resolved_inputs.get(dump_path) != entry["sha256"]:
                                errors.append(f"compiler report inputs omit or disagree with built MIR dump: {entry['path']}")
                    except (OSError, ValueError, TypeError) as error:
                        errors.append(f"cannot revalidate built MIR manifest inputs: {error}")
    if len(artifact_paths) == 4:
        receipt_path = next((path for path in resolved_inputs
                             if path.name.lower() == "provenance.json"), None)
        if receipt_path is None:
            errors.append("compiler report does not pin its provenance receipt")
        else:
            try:
                receipt = json.loads(receipt_path.read_text(encoding="utf-8"))
                if report.get("build_provenance") != receipt:
                    errors.append("inline compiler provenance differs from the pinned receipt")
                old_root = compiler_checker.ROOT
                compiler_checker.ROOT = root
                try:
                    validation = compiler_checker.validate_provenance(
                    receipt, artifact_paths["rustdoc"], artifact_paths["mir"],
                        artifact_paths["hir"], artifact_paths["thir"],
                        artifact_paths.get("built_mir_manifest"))
                finally:
                    compiler_checker.ROOT = old_root
                if not validation.get("validated"):
                    errors.extend(validation.get("errors", ["compiler provenance failed validation"]))
                recorded = report.get("provenance_validation", {})
                if not isinstance(recorded, dict) or not recorded.get("validated"):
                    errors.append("compiler call report records invalid provenance")
            except (OSError, ValueError, KeyError, TypeError) as error:
                errors.append(f"cannot revalidate compiler provenance: {error}")

    violations = report.get("violations")
    unknown = report.get("unknown_calls")
    if not isinstance(violations, list):
        errors.append("compiler report violations are missing or malformed")
        violations = []
    if not isinstance(unknown, list):
        errors.append("compiler report unknown calls are missing or malformed")
        unknown = []
    cross = report.get("cross_component_calls")
    if not isinstance(cross, list):
        errors.append("compiler report cross-component calls are missing or malformed")
        cross = []
    try:
        dag = json.loads(dag_path.read_text(encoding="utf-8"))
        allowed = {row["id"]: {edge["to"] for edge in row.get("depends_on", [])}
                   for row in dag["components"]}
        derived_violations = []
        for row in cross:
            if not isinstance(row, dict):
                errors.append("compiler report cross-component call entry is malformed")
                continue
            source = row.get("source_component")
            target = row.get("target_component")
            expected_permitted = isinstance(source, str) and target in allowed.get(source, set())
            if row.get("permitted") is not expected_permitted:
                errors.append("cross-component call permission disagrees with the current DAG")
            if not expected_permitted:
                derived_violations.append(row)
        if violations != derived_violations:
            errors.append("reported violation list differs from the DAG-derived cross-component calls")
    except (OSError, ValueError, KeyError, TypeError):
        errors.append("cannot derive compiler call violations from the current component DAG")
    summary = {
        "status": "pass" if not errors and not violations and not unknown else "fail",
        "validated": not errors,
        "report": str(report_path),
        "violations": len(violations),
        "unknown_calls": len(unknown),
        "errors": errors,
    }
    if violations:
        summary["errors"].append(f"{len(violations)} unpermitted compiler call instances remain")
    if unknown:
        summary["errors"].append(f"{len(unknown)} compiler call/destructor instances remain unresolved")
    if violations or unknown:
        summary["status"] = "fail"
    return summary


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args()
    result = validate_report(args.report)
    print(json.dumps(result, sort_keys=True))
    return int(result["status"] != "pass")


if __name__ == "__main__":
    raise SystemExit(main())
