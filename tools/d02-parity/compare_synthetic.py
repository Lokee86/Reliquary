"""Strict synthetic semantic-row comparator prototype for D02-T01.

Fixture-only foundation. Not the accepted D02 report or current-only reader.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from pathlib import Path
from typing import Any, Iterable

from evidence_io import WitnessInputError, read_jsonl as _read_jsonl
from inventory_v1 import (
    CONTRACT_COMMIT,
    FAMILY_GROUPS,
    INVENTORY_SCHEMA_ID,
    INVENTORY_TRUST_NOTE,
    SEMANTIC_SCHEMA_ID,
    TARGET_COVERAGE_SCHEMA_ID,
    TARGET_TRUST_NOTE,
    inventory_key,
    validate_coverage_bytes,
    validate_supported_semantic_schema,
)
from typed_values import validate_semantic_row_consistency

REPORT_SCHEMA = "reliquary-d02-synthetic-row-diff-v1"
U64_MAX = (1 << 64) - 1
ORDINAL = re.compile(r"^(?:0|[1-9][0-9]*)$")


def _row_key(row: dict[str, Any]) -> tuple[bytes, ...]:
    try:
        owner = row["owner"]
        strings = (
            owner["owner_uuid"],
            owner["owner_kind"],
            owner["scope"],
            row["family"],
            row["stable_key"],
            row["revision_or_event"],
            row["ordinal"],
        )
    except (KeyError, TypeError) as exc:
        raise WitnessInputError(f"semantic row is missing a comparison-key field: {exc}") from exc
    if not all(isinstance(value, str) for value in strings):
        raise WitnessInputError("semantic row comparison-key fields must be strings")
    ordinal_text = strings[-1]
    if not ORDINAL.fullmatch(ordinal_text):
        raise WitnessInputError(f"ordinal is not canonical unsigned decimal: {ordinal_text!r}")
    if len(ordinal_text) > 20:
        raise WitnessInputError(f"ordinal exceeds u64: {ordinal_text[:32]}")
    ordinal = int(ordinal_text)
    if ordinal > U64_MAX:
        raise WitnessInputError(f"ordinal exceeds u64: {ordinal_text}")
    return inventory_key(owner, row["family"], row)


def _validated_rows(raw: bytes, label: str, validator: Any) -> dict[tuple[bytes, ...], dict[str, Any]]:
    rows = _read_jsonl(raw, label)
    keyed: dict[tuple[bytes, ...], dict[str, Any]] = {}
    for number, row in enumerate(rows, 1):
        errors = sorted(
            validator.iter_errors(row),
            key=lambda error: (tuple(str(part) for part in error.absolute_path), error.message),
        )
        if errors:
            error = errors[0]
            pointer = "/".join(str(part) for part in error.absolute_path) or "$"
            raise WitnessInputError(f"{label} line {number}: schema violation at {pointer}: {error.message}")
        family = row["family"]
        if row["group_id"] != FAMILY_GROUPS.get(family):
            raise WitnessInputError(f"{label} line {number}: family/group mapping differs from PG-C01 v1")
        if row["value"].get("schema_id") != family:
            raise WitnessInputError(f"{label} line {number}: value.schema_id does not match family")
        validate_semantic_row_consistency(row, f"{label} line {number}")
        key = _row_key(row)
        if key in keyed:
            raise WitnessInputError(f"{label} line {number}: duplicate semantic row key")
        keyed[key] = row
    return keyed


def _json_pointer(path: Iterable[Any]) -> str:
    parts = [str(part).replace("~", "~0").replace("/", "~1") for part in path]
    return "/" + "/".join(parts) if parts else "/"


def _difference_paths(expected: Any, observed: Any, path: tuple[Any, ...] = ()) -> list[str]:
    if isinstance(expected, dict) and isinstance(observed, dict):
        paths: list[str] = []
        for key in sorted(set(expected) | set(observed), key=lambda item: item.encode("utf-8")):
            if key not in expected or key not in observed:
                paths.append(_json_pointer(path + (key,)))
            else:
                paths.extend(_difference_paths(expected[key], observed[key], path + (key,)))
        return paths
    if isinstance(expected, list) and isinstance(observed, list):
        if len(expected) != len(observed):
            return [_json_pointer(path)]
        paths = []
        for index, (left, right) in enumerate(zip(expected, observed)):
            paths.extend(_difference_paths(left, right, path + (index,)))
        return paths
    if type(expected) is not type(observed) or expected != observed:
        return [_json_pointer(path)]
    return []


def _display_key(key: tuple[bytes, ...]) -> dict[str, str]:
    labels = (
        "owner_uuid", "owner_kind", "scope", "family", "stable_key",
        "revision_or_event", "ordinal",
    )
    values = [part.decode("utf-8") for part in key[:6]] + [
        str(int.from_bytes(key[6], "big")),
    ]
    return dict(zip(labels, values))


def compare_bytes(
    source_bytes: bytes,
    target_bytes: bytes,
    schema: dict[str, Any],
    schema_bytes: bytes,
    inventory_bytes: bytes,
    target_coverage_bytes: bytes,
    source_label: str = "source",
    target_label: str = "target",
) -> dict[str, Any]:
    try:
        from jsonschema import Draft202012Validator
    except ImportError as exc:
        raise WitnessInputError("jsonschema 4.26.0 is required; see requirements-test.txt") from exc
    pinned_schema_hash = validate_supported_semantic_schema(schema, schema_bytes)
    Draft202012Validator.check_schema(schema)
    validator = Draft202012Validator(schema)
    inventory_doc, expected_rows = validate_coverage_bytes(
        inventory_bytes,
        schema,
        INVENTORY_SCHEMA_ID,
        INVENTORY_TRUST_NOTE,
        "expected inventory",
    )
    target_coverage_doc, target_rows = validate_coverage_bytes(
        target_coverage_bytes,
        schema,
        TARGET_COVERAGE_SCHEMA_ID,
        TARGET_TRUST_NOTE,
        "target coverage",
    )
    if inventory_doc["owners"] != target_coverage_doc["owners"] or expected_rows != target_rows:
        raise WitnessInputError("target coverage does not reconcile to the independent expected inventory")
    source = _validated_rows(source_bytes, source_label, validator)
    target = _validated_rows(target_bytes, target_label, validator)

    report: dict[str, Any] = {
        "schema_id": REPORT_SCHEMA,
        "evidence_scope": "synthetic_fixture",
        "status": "INCOMPLETE",
        "source_sha256": hashlib.sha256(source_bytes).hexdigest(),
        "target_sha256": hashlib.sha256(target_bytes).hexdigest(),
        "semantic_schema_id": SEMANTIC_SCHEMA_ID,
        "semantic_schema_sha256": pinned_schema_hash,
        "contract_commit": CONTRACT_COMMIT,
        "inventory_sha256": hashlib.sha256(inventory_bytes).hexdigest(),
        "target_coverage_sha256": hashlib.sha256(target_coverage_bytes).hexdigest(),
        "inventory_authority": "synthetic_only_not_authenticated",
        "source_row_count": str(len(source)),
        "target_row_count": str(len(target)),
        "mismatch_count": "0",
        "diagnostics": [],
    }
    missing = sorted(source.keys() - target.keys())
    extra = sorted(target.keys() - source.keys())
    diagnostics: list[dict[str, Any]] = []
    mismatch_keys = set(missing) | set(extra)
    inventory_missing_source = sorted(expected_rows.keys() - source.keys())
    inventory_extra_source = sorted(source.keys() - expected_rows.keys())
    inventory_missing_target = sorted(expected_rows.keys() - target.keys())
    inventory_extra_target = sorted(target.keys() - expected_rows.keys())
    for key in inventory_missing_source:
        mismatch_keys.add(key)
        diagnostics.append({"kind": "source_omits_independent_inventory_row", "key": _display_key(key)})
    for key in inventory_extra_source:
        mismatch_keys.add(key)
        diagnostics.append({"kind": "source_row_not_in_independent_inventory", "key": _display_key(key)})
    for key in inventory_missing_target:
        mismatch_keys.add(key)
        diagnostics.append({"kind": "target_omits_independent_inventory_row", "key": _display_key(key)})
    for key in inventory_extra_target:
        mismatch_keys.add(key)
        diagnostics.append({"kind": "target_row_not_in_independent_inventory", "key": _display_key(key)})
    if not source and expected_rows:
        report["reason"] = "empty source omits independently inventoried rows"
    elif not source and not expected_rows:
        report["reason"] = "empty source has no independently inventoried rows"
    for key in missing:
        diagnostics.append({"kind": "missing_target_row", "key": _display_key(key)})
    for key in extra:
        diagnostics.append({"kind": "extra_target_row", "key": _display_key(key)})
    for key in sorted(source.keys() & target.keys()):
        paths = _difference_paths(source[key], target[key])
        if paths:
            mismatch_keys.add(key)
            diagnostics.append({
                "kind": "changed_row",
                "key": _display_key(key),
                "different_fields": paths[:20],
                "different_field_count": str(len(paths)),
            })
    mismatch_count = len(mismatch_keys)
    report["mismatch_count"] = str(mismatch_count)
    report["diagnostics"] = diagnostics[:100]
    report["diagnostic_count"] = str(len(diagnostics))
    if not source and not expected_rows and not target:
        report["status"] = "INCOMPLETE"
        report["reason"] = "empty witnesses are not a valid synthetic PASS"
        return report
    report["status"] = "PASS" if mismatch_count == 0 else "FAIL"
    return report


def load_schema(path: Path) -> tuple[dict[str, Any], bytes]:
    raw = path.read_bytes()
    if raw.startswith(b"\xef\xbb\xbf"):
        raise WitnessInputError("schema file must not contain a UTF-8 BOM")
    try:
        schema = json.loads(raw.decode("utf-8", "strict"))
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise WitnessInputError(f"invalid JSON schema file: {exc}") from exc
    if not isinstance(schema, dict):
        raise WitnessInputError("JSON schema root must be an object")
    validate_supported_semantic_schema(schema, raw)
    return schema, raw


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Compare hand-authored synthetic semantic-row witnesses.")
    parser.add_argument("--source", required=True, type=Path)
    parser.add_argument("--target", required=True, type=Path)
    parser.add_argument("--inventory", required=True, type=Path)
    parser.add_argument("--target-coverage", required=True, type=Path)
    parser.add_argument(
        "--schema",
        type=Path,
        default=Path(__file__).parent / "contract-v1" / "schemas" / "semantic-row-v1.schema.json",
    )
    parser.add_argument("--output", type=Path)
    args = parser.parse_args(argv)
    try:
        schema, schema_bytes = load_schema(args.schema)
        report = compare_bytes(
            args.source.read_bytes(),
            args.target.read_bytes(),
            schema,
            schema_bytes,
            args.inventory.read_bytes(),
            args.target_coverage.read_bytes(),
            str(args.source),
            str(args.target),
        )
    except (OSError, WitnessInputError) as exc:
        report = {
            "schema_id": REPORT_SCHEMA,
            "evidence_scope": "synthetic_fixture",
            "status": "INCOMPLETE",
            "reason": f"{type(exc).__name__}: {exc}",
        }
    rendered = json.dumps(report, ensure_ascii=False, sort_keys=True, indent=2) + "\n"
    if args.output:
        try:
            with args.output.open("x", encoding="utf-8", newline="\n") as output:
                output.write(rendered)
        except OSError:
            sys.stderr.write("could not create a new comparator report\n")
            return 2
    else:
        sys.stdout.write(rendered)
    return 0 if report.get("status") == "PASS" else 2


if __name__ == "__main__":
    raise SystemExit(main())
