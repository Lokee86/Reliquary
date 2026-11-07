"""Static provisional adapter protocol fixture and format checks.

The G5 specification requires runner/adapter boundaries but does not freeze a
wire protocol. This module validates a checked-in handshake fixture only. It
launches no processes, accepts no commands, carries no workload/cohort data,
and does not verify build artifacts or provenance claims.
"""
from __future__ import annotations

import json
from pathlib import Path
from typing import Any, Mapping

REQUEST_SCHEMA_ID = "reliquary-g5-adapter-handshake-request-provisional-v1"
RESPONSE_SCHEMA_ID = "reliquary-g5-adapter-handshake-response-provisional-v1"
MAX_FRAME_BYTES = 16 * 1024
PROVENANCE_VALIDATION_SCOPE = "format_only_not_artifact_verified"
FIXTURE_PATH = Path(__file__).with_name("fixtures") / "synthetic-adapter-handshake-response-v1.json"

_RESPONSE_FIELDS = {
    "schema_id",
    "protocol_status",
    "synthetic_only",
    "measurement_results",
    "request_echo",
    "build_provenance",
}
_PROVENANCE_FIELDS = {
    "adapter_id",
    "adapter_version",
    "synthetic_fixture",
    "build_mode",
    "source_revision",
    "source_sha256",
    "binary_sha256",
    "lockfile_sha256",
    "compiler",
    "optimization",
    "features",
    "target",
}
_COMPILER_FIELDS = {"name", "version"}


class AdapterProtocolError(ValueError):
    """A synthetic fixture or adapter protocol record is invalid."""


def handshake_request() -> dict[str, Any]:
    """Return the fixed hello frame with no paths, workload bytes, or private data."""
    return {
        "schema_id": REQUEST_SCHEMA_ID,
        "request_id": "g5-runner-self-test-v1",
        "operation": "handshake_only",
        "synthetic_only": True,
        "measurement_results": False,
    }


def _reject_duplicate_keys(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise AdapterProtocolError(f"duplicate JSON object key: {key}")
        result[key] = value
    return result


def _reject_non_json_constant(value: str) -> None:
    raise AdapterProtocolError(f"non-standard JSON constant is not allowed: {value}")


def _sha256(value: Any) -> bool:
    return (
        isinstance(value, str)
        and len(value) == 64
        and all(char in "0123456789abcdef" for char in value)
    )


def _nonempty_string(value: Any) -> bool:
    return isinstance(value, str) and bool(value.strip())


def _field_shape_errors(record: Mapping[str, Any], expected: set[str], label: str) -> list[str]:
    errors: list[str] = []
    missing = expected - set(record)
    unexpected = set(record) - expected
    if missing:
        errors.append(f"{label} is missing fields: {', '.join(sorted(missing))}")
    if unexpected:
        errors.append(f"{label} has unknown fields: {', '.join(sorted(unexpected))}")
    return errors


def validate_build_provenance_format(
    record: Mapping[str, Any],
    *,
    allow_synthetic_fixture: bool = False,
) -> list[str]:
    """Check record shape and primitive formats only; do not verify any artifact."""
    if not isinstance(record, Mapping):
        return ["build provenance must be an object"]

    errors = _field_shape_errors(record, _PROVENANCE_FIELDS, "build provenance")
    is_fixture = record.get("synthetic_fixture")
    if is_fixture is True:
        if not allow_synthetic_fixture:
            errors.append("synthetic fixture provenance is not release provenance")
        if record.get("build_mode") != "synthetic-fixture":
            errors.append("fixture build_mode must be synthetic-fixture")
        if record.get("source_revision") != "synthetic-fixture":
            errors.append("fixture source_revision must be synthetic-fixture")
        if not _sha256(record.get("source_sha256")):
            errors.append("fixture source_sha256 must be lowercase SHA-256")
        if record.get("binary_sha256") is not None or record.get("lockfile_sha256") is not None:
            errors.append("synthetic fixture must not claim a product binary or locked build")
        if record.get("optimization") != "not-applicable":
            errors.append("fixture optimization must be not-applicable")
        if record.get("target") != "synthetic-test-host":
            errors.append("fixture target must be synthetic-test-host")
    elif is_fixture is False:
        if record.get("build_mode") != "release":
            errors.append("release provenance must identify a release build")
        revision = record.get("source_revision")
        if (
            not isinstance(revision, str)
            or len(revision) not in (40, 64)
            or any(char not in "0123456789abcdef" for char in revision)
        ):
            errors.append("source_revision must be a full lowercase Git object ID")
        for field in ("source_sha256", "binary_sha256", "lockfile_sha256"):
            if not _sha256(record.get(field)):
                errors.append(f"{field} must be lowercase SHA-256")
        if not _nonempty_string(record.get("optimization")):
            errors.append("optimization profile is required")
        if not _nonempty_string(record.get("target")):
            errors.append("target triple is required")
    else:
        errors.append("synthetic_fixture must be explicitly true or false")

    compiler = record.get("compiler")
    if not isinstance(compiler, Mapping):
        errors.append("compiler must be an object")
    else:
        errors.extend(_field_shape_errors(compiler, _COMPILER_FIELDS, "compiler"))
        if not all(_nonempty_string(compiler.get(field)) for field in _COMPILER_FIELDS):
            errors.append("compiler name and version are required")

    if not _nonempty_string(record.get("adapter_id")) or not _nonempty_string(record.get("adapter_version")):
        errors.append("adapter identity and version are required")
    features = record.get("features")
    if not isinstance(features, list) or any(not isinstance(item, str) for item in features):
        errors.append("features must be a string list")
    return errors


def validate_handshake_response(response: Any) -> list[str]:
    """Validate a fixed handshake fixture and reject measurement or artifact claims."""
    if not isinstance(response, Mapping):
        return ["adapter response must be a JSON object"]

    errors = _field_shape_errors(response, _RESPONSE_FIELDS, "handshake response")
    if response.get("schema_id") != RESPONSE_SCHEMA_ID:
        errors.append("unexpected adapter response schema")
    if response.get("protocol_status") != "READY_FOR_PROTOCOL_SELFTEST":
        errors.append("fixture does not carry the protocol-self-test status")
    if response.get("synthetic_only") is not True:
        errors.append("handshake fixture must be explicitly synthetic-only")
    if response.get("measurement_results") is not False:
        errors.append("handshake fixture must not claim measurement results")
    if response.get("request_echo") != handshake_request():
        errors.append("fixture does not echo the fixed handshake request")

    provenance = response.get("build_provenance")
    if not isinstance(provenance, Mapping):
        errors.append("fixture build provenance is required")
    else:
        if provenance.get("synthetic_fixture") is not True:
            errors.append("fixture must identify synthetic-only provenance")
        errors.extend(validate_build_provenance_format(provenance, allow_synthetic_fixture=True))
    return errors


def parse_handshake_response(payload: str | bytes) -> dict[str, Any]:
    """Parse one bounded JSON frame, rejecting duplicate keys and non-standard constants."""
    if isinstance(payload, bytes):
        if len(payload) > MAX_FRAME_BYTES:
            raise AdapterProtocolError("handshake frame exceeds the byte limit")
        try:
            text = payload.decode("utf-8", errors="strict")
        except UnicodeDecodeError as exc:
            raise AdapterProtocolError("handshake frame is not UTF-8") from exc
    elif isinstance(payload, str):
        if len(payload.encode("utf-8")) > MAX_FRAME_BYTES:
            raise AdapterProtocolError("handshake frame exceeds the byte limit")
        text = payload
    else:
        raise AdapterProtocolError("handshake frame must be bytes or text")

    try:
        response = json.loads(
            text,
            object_pairs_hook=_reject_duplicate_keys,
            parse_constant=_reject_non_json_constant,
        )
    except AdapterProtocolError:
        raise
    except json.JSONDecodeError as exc:
        raise AdapterProtocolError("handshake frame is not valid JSON") from exc

    errors = validate_handshake_response(response)
    if errors:
        raise AdapterProtocolError("; ".join(errors))
    return dict(response)


def load_synthetic_handshake_fixture() -> dict[str, Any]:
    """Read the checked-in fixture with a bounded read; no child process is launched."""
    try:
        with FIXTURE_PATH.open("rb") as stream:
            payload = stream.read(MAX_FRAME_BYTES + 1)
    except OSError as exc:
        raise AdapterProtocolError(f"synthetic handshake fixture is unavailable: {exc}") from exc
    return parse_handshake_response(payload)
