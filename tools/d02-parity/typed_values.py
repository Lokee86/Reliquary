"""Independent typed-number validation for pinned PG-C01 semantic rows."""
from __future__ import annotations

import re
from typing import Any

from evidence_io import WitnessInputError

U64_MAX = (1 << 64) - 1
I64_MIN = -(1 << 63)
I64_MAX = (1 << 63) - 1
UDEC = re.compile(r"^(?:0|[1-9][0-9]*)$")
IDEC = re.compile(r"^(?:0|[1-9][0-9]*|-[1-9][0-9]*)$")
HEX = re.compile(r"(?:[0-9a-f]{2})+")
SHA256 = re.compile(r"[0-9a-f]{64}")
DOMAIN_IDS = frozenset({
    "fixture.activity",
    "fixture.archive",
    "fixture.global",
    "fixture.memory",
})


def _unsigned(value: Any, label: str) -> int:
    if not isinstance(value, str) or not UDEC.fullmatch(value):
        raise WitnessInputError(f"{label} must be canonical unsigned decimal")
    if len(value) > 20:
        raise WitnessInputError(f"{label} exceeds u64")
    number = int(value)
    if number > U64_MAX:
        raise WitnessInputError(f"{label} exceeds u64")
    return number


def _signed(value: Any, label: str) -> int:
    if not isinstance(value, str) or not IDEC.fullmatch(value):
        raise WitnessInputError(f"{label} must be canonical signed decimal")
    if len(value) > 20:
        raise WitnessInputError(f"{label} exceeds i64")
    number = int(value)
    if number < I64_MIN or number > I64_MAX:
        raise WitnessInputError(f"{label} exceeds i64")
    return number


def validate_coordinate_ranges(coordinate: Any, label: str) -> None:
    if not isinstance(coordinate, dict):
        raise WitnessInputError(f"{label} must be an object")
    try:
        _unsigned(coordinate["global_version"], f"{label}.global_version")
        domain_versions = coordinate["domain_versions"]
        if not isinstance(domain_versions, list):
            raise WitnessInputError(f"{label}.domain_versions must be an array")
        previous_domain_id: bytes | None = None
        for index, entry in enumerate(domain_versions):
            if not isinstance(entry, dict):
                raise WitnessInputError(f"{label}.domain_versions[{index}] must be an object")
            domain_id = entry.get("domain_id")
            if not isinstance(domain_id, str):
                raise WitnessInputError(f"{label}.domain_versions[{index}].domain_id must be text")
            if domain_id not in DOMAIN_IDS:
                raise WitnessInputError(f"{label}.domain_versions[{index}] names an unknown contract domain")
            try:
                encoded_domain_id = domain_id.encode("utf-8", "strict")
            except UnicodeEncodeError as exc:
                raise WitnessInputError(f"{label}.domain_versions[{index}].domain_id is not valid UTF-8") from exc
            if previous_domain_id is not None and encoded_domain_id <= previous_domain_id:
                raise WitnessInputError(
                    f"{label}.domain_versions must be unique and UTF-8-byte ordered"
                )
            previous_domain_id = encoded_domain_id
            _unsigned(entry["version"], f"{label}.domain_versions[{index}].version")
        for field, signed in (
            ("transaction_time", True),
            ("rel_activity_position", False),
            ("accepted_event_boundary", False),
        ):
            optional = coordinate[field]
            if not isinstance(optional, dict):
                raise WitnessInputError(f"{label}.{field} must be a tagged optional")
            if optional.get("state") == "present":
                parser = _signed if signed else _unsigned
                parser(optional.get("value"), f"{label}.{field}.value")
    except KeyError as exc:
        raise WitnessInputError(f"{label} is missing numeric coordinate field {exc}") from exc


def _validate_integer(value: dict[str, Any], label: str) -> None:
    signed = value.get("signed")
    if signed is True:
        _signed(value.get("decimal"), label)
    elif signed is False:
        _unsigned(value.get("decimal"), label)
    else:
        raise WitnessInputError(f"{label} integer signed tag must be boolean")


def _validate_float_bits(value: dict[str, Any], label: str) -> None:
    scalar = value.get("scalar")
    endian = value.get("endianness")
    hex_value = value.get("hex")
    if scalar not in {"f32", "f64"} or endian not in {"little", "big"}:
        raise WitnessInputError(f"{label} has invalid IEEE scalar/endian tag")
    expected_bytes = 4 if scalar == "f32" else 8
    if not isinstance(hex_value, str) or not HEX.fullmatch(hex_value):
        raise WitnessInputError(f"{label}.hex must be lowercase even-length hex")
    raw = bytes.fromhex(hex_value)
    if len(raw) != expected_bytes:
        raise WitnessInputError(f"{label} bit width does not match {scalar}")
    fraction_width, exponent_width = (23, 8) if scalar == "f32" else (52, 11)
    bits = int.from_bytes(raw, endian)
    exponent = (bits >> fraction_width) & ((1 << exponent_width) - 1)
    if exponent == (1 << exponent_width) - 1:
        raise WitnessInputError(f"{label} encodes non-finite IEEE bits")


def _validate_bytes(value: dict[str, Any], label: str) -> None:
    hex_value = value.get("hex")
    if not isinstance(hex_value, str) or not HEX.fullmatch(hex_value):
        raise WitnessInputError(f"{label}.hex must be lowercase even-length nonempty hex")
    field_name = label.rsplit(".", 1)[-1]
    if field_name in {"source_body_sha256", "proof_sha256"} and not SHA256.fullmatch(hex_value):
        raise WitnessInputError(f"{label}.hex must be exactly 64 lowercase hex characters")


def _walk_typed_values(value: Any, label: str) -> None:
    if isinstance(value, dict):
        tag = value.get("kind")
        if tag == "integer":
            _validate_integer(value, label)
        elif tag == "float_bits":
            _validate_float_bits(value, label)
        elif tag == "bytes":
            _validate_bytes(value, label)
        for key, child in value.items():
            _walk_typed_values(child, f"{label}.{key}")
    elif isinstance(value, list):
        for index, child in enumerate(value):
            _walk_typed_values(child, f"{label}[{index}]")


def validate_semantic_row_numbers(row: dict[str, Any], label: str) -> None:
    try:
        _unsigned(row["ordinal"], f"{label}.ordinal")
        validate_coordinate_ranges(row["coordinate"], f"{label}.coordinate")
        _walk_typed_values(row["value"], f"{label}.value")
        _walk_typed_values(row["source_binding"], f"{label}.source_binding")
    except KeyError as exc:
        raise WitnessInputError(f"{label} is missing typed numeric data {exc}") from exc


def validate_semantic_row_consistency(row: dict[str, Any], label: str) -> None:
    validate_semantic_row_numbers(row, label)
    binding = row.get("source_binding")
    if isinstance(binding, dict) and binding.get("state") == "present":
        provenance = binding.get("value")
        if isinstance(provenance, dict) and provenance.get("schema_id") == "fixture.logical_provenance.v1":
            try:
                fields = provenance["fields"]
                start = _unsigned(
                    fields["source_body_span_start"]["decimal"],
                    f"{label}.source_binding.source_body_span_start",
                )
                end = _unsigned(
                    fields["source_body_span_end"]["decimal"],
                    f"{label}.source_binding.source_body_span_end",
                )
            except KeyError as exc:
                raise WitnessInputError(f"{label} provenance span is incomplete: {exc}") from exc
            if end < start:
                raise WitnessInputError(f"{label} source-body span must be a valid half-open byte range")

    availability = row.get("availability")
    if availability == "present":
        return
    value = row.get("value")
    reason = value.get("reason_code") if isinstance(value, dict) else None
    if not isinstance(value, dict) or not isinstance(value.get("evidence_sha256"), str) or not SHA256.fullmatch(value["evidence_sha256"]):
        raise WitnessInputError(f"{label} unavailable evidence digest must be exactly 64 lowercase hex characters")
    if reason == "intra_settlement_unavailable":
        if availability != "unavailable" or row.get("group_id") != "freshness":
            raise WitnessInputError(
                f"{label} intra-settlement unavailability is a Freshness-only unavailable state"
            )
    elif reason != availability:
        raise WitnessInputError(f"{label} unavailable reason code must match row availability")
