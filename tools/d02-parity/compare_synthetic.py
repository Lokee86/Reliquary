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

REPORT_SCHEMA = "reliquary-d02-synthetic-row-diff-v1"
U64_MAX = (1 << 64) - 1
ORDINAL = re.compile(r"^(?:0|[1-9][0-9]*)$")


class WitnessInputError(ValueError):
    """An input is not a complete, strict witness fixture."""


def _unique_object(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise WitnessInputError(f"duplicate decoded JSON object key: {key!r}")
        result[key] = value
    return result


def _reject_json_number(token: str) -> Any:
    raise WitnessInputError(f"JSON numeric tokens are forbidden in evidence: {token}")


def _reject_surrogates(value: Any, location: str = "$") -> None:
    if isinstance(value, str):
        if any(0xD800 <= ord(char) <= 0xDFFF for char in value):
            raise WitnessInputError(f"unpaired surrogate in evidence string at {location}")
    elif isinstance(value, list):
        for index, item in enumerate(value):
            _reject_surrogates(item, f"{location}[{index}]")
    elif isinstance(value, dict):
        for key, item in value.items():
            _reject_surrogates(key, f"{location}.<key>")
            _reject_surrogates(item, f"{location}.{key}")


def _strict_json_line(raw: bytes, line_number: int) -> dict[str, Any]:
    try:
        text = raw.decode("utf-8", "strict")
        value = json.loads(
            text,
            object_pairs_hook=_unique_object,
            parse_int=_reject_json_number,
            parse_float=_reject_json_number,
            parse_constant=_reject_json_number,
        )
    except (UnicodeDecodeError, json.JSONDecodeError, WitnessInputError) as exc:
        raise WitnessInputError(f"line {line_number}: {exc}") from exc
    if not isinstance(value, dict):
        raise WitnessInputError(f"line {line_number}: each semantic row must be an object")
    _reject_surrogates(value, f"$line[{line_number}]")
    return value


def _read_jsonl(raw: bytes, label: str) -> list[dict[str, Any]]:
    if raw.startswith(b"\xef\xbb\xbf"):
        raise WitnessInputError(f"{label}: UTF-8 BOM is forbidden")
    if not raw:
        return []
    if not raw.endswith(b"\n"):
        raise WitnessInputError(f"{label}: nonempty JSONL must end with LF")
    if b"\r" in raw:
        raise WitnessInputError(f"{label}: CR bytes are forbidden; JSONL uses LF")
    lines = raw[:-1].split(b"\n")
    if any(not line for line in lines):
        raise WitnessInputError(f"{label}: blank JSONL lines are forbidden")
    return [_strict_json_line(line, number) for number, line in enumerate(lines, 1)]


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
    ordinal = int(ordinal_text)
    if ordinal > U64_MAX:
        raise WitnessInputError(f"ordinal exceeds u64: {ordinal_text}")
    return tuple(value.encode("utf-8", "strict") for value in strings[:-1]) + (ordinal.to_bytes(8, "big"),)


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
    labels = ("owner_uuid", "owner_kind", "scope", "family", "stable_key", "revision_or_event", "ordinal")
    values = [part.decode("utf-8") for part in key[:-1]] + [str(int.from_bytes(key[-1], "big"))]
    return dict(zip(labels, values))


def compare_bytes(
    source_bytes: bytes,
    target_bytes: bytes,
    schema: dict[str, Any],
    schema_bytes: bytes,
    source_label: str = "source",
    target_label: str = "target",
) -> dict[str, Any]:
    try:
        from jsonschema import Draft202012Validator
    except ImportError as exc:
        raise WitnessInputError("jsonschema 4.26.0 is required; see requirements-test.txt") from exc
    Draft202012Validator.check_schema(schema)
    validator = Draft202012Validator(schema)
    source = _validated_rows(source_bytes, source_label, validator)
    target = _validated_rows(target_bytes, target_label, validator)

    report: dict[str, Any] = {
        "schema_id": REPORT_SCHEMA,
        "evidence_scope": "synthetic_fixture",
        "status": "INCOMPLETE",
        "source_sha256": hashlib.sha256(source_bytes).hexdigest(),
        "target_sha256": hashlib.sha256(target_bytes).hexdigest(),
        "schema_sha256": hashlib.sha256(schema_bytes).hexdigest(),
        "source_row_count": str(len(source)),
        "target_row_count": str(len(target)),
        "mismatch_count": "0",
        "diagnostics": [],
    }
    if not source:
        report["reason"] = "empty expected witness is not a valid PASS"
        return report

    missing = sorted(source.keys() - target.keys())
    extra = sorted(target.keys() - source.keys())
    diagnostics: list[dict[str, Any]] = []
    mismatch_count = len(missing) + len(extra)
    for key in missing:
        diagnostics.append({"kind": "missing_target_row", "key": _display_key(key)})
    for key in extra:
        diagnostics.append({"kind": "extra_target_row", "key": _display_key(key)})
    for key in sorted(source.keys() & target.keys()):
        paths = _difference_paths(source[key], target[key])
        if paths:
            mismatch_count += 1
            diagnostics.append({
                "kind": "changed_row",
                "key": _display_key(key),
                "different_fields": paths[:20],
                "different_field_count": str(len(paths)),
            })
    report["status"] = "PASS" if mismatch_count == 0 else "FAIL"
    report["mismatch_count"] = str(mismatch_count)
    report["diagnostics"] = diagnostics[:100]
    report["diagnostic_count"] = str(len(diagnostics))
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
    return schema, raw


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Compare hand-authored synthetic semantic-row witnesses.")
    parser.add_argument("--source", required=True, type=Path)
    parser.add_argument("--target", required=True, type=Path)
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
        args.output.write_text(rendered, encoding="utf-8", newline="\n")
    else:
        sys.stdout.write(rendered)
    return 0 if report.get("status") == "PASS" else 2


if __name__ == "__main__":
    raise SystemExit(main())
