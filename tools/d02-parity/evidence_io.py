"""Strict JSON input helpers for offline D02 evidence tools."""
from __future__ import annotations

import json
from typing import Any


class WitnessInputError(ValueError):
    """An input is not a complete, strict witness fixture."""


def unique_object(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise WitnessInputError(f"duplicate decoded JSON object key: {key!r}")
        result[key] = value
    return result


def reject_json_number(token: str) -> Any:
    raise WitnessInputError(f"JSON numeric tokens are forbidden in evidence: {token}")


def reject_surrogates(value: Any, location: str = "$") -> None:
    if isinstance(value, str):
        if any(0xD800 <= ord(char) <= 0xDFFF for char in value):
            raise WitnessInputError(f"unpaired surrogate in evidence string at {location}")
    elif isinstance(value, list):
        for index, item in enumerate(value):
            reject_surrogates(item, f"{location}[{index}]")
    elif isinstance(value, dict):
        for key, item in value.items():
            reject_surrogates(key, f"{location}.<key>")
            reject_surrogates(item, f"{location}.{key}")


def strict_json_document(raw: bytes, label: str) -> dict[str, Any]:
    if raw.startswith(b"\xef\xbb\xbf"):
        raise WitnessInputError(f"{label}: UTF-8 BOM is forbidden")
    try:
        text = raw.decode("utf-8", "strict")
        value = json.loads(
            text,
            object_pairs_hook=unique_object,
            parse_int=reject_json_number,
            parse_float=reject_json_number,
            parse_constant=reject_json_number,
        )
    except (UnicodeDecodeError, json.JSONDecodeError, WitnessInputError) as exc:
        raise WitnessInputError(f"{label}: {exc}") from exc
    if not isinstance(value, dict):
        raise WitnessInputError(f"{label}: root must be an object")
    reject_surrogates(value, f"${label}")
    return value


def strict_json_line(raw: bytes, line_number: int) -> dict[str, Any]:
    try:
        text = raw.decode("utf-8", "strict")
        value = json.loads(
            text,
            object_pairs_hook=unique_object,
            parse_int=reject_json_number,
            parse_float=reject_json_number,
            parse_constant=reject_json_number,
        )
    except (UnicodeDecodeError, json.JSONDecodeError, WitnessInputError) as exc:
        raise WitnessInputError(f"line {line_number}: {exc}") from exc
    if not isinstance(value, dict):
        raise WitnessInputError(f"line {line_number}: each semantic row must be an object")
    reject_surrogates(value, f"$line[{line_number}]")
    return value


def read_jsonl(raw: bytes, label: str) -> list[dict[str, Any]]:
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
    return [strict_json_line(line, number) for number, line in enumerate(lines, 1)]
