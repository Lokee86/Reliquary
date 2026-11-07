"""Synthetic-only owner/family/cut inventory validation for D02 tests.

The fixture inventory is a test oracle, not an authenticated real-source manifest.
Real D02 authority must be bound to the accepted preconversion F03 witness.
"""
from __future__ import annotations

import hashlib
import json
import re
from typing import Any

from evidence_io import WitnessInputError, strict_json_document
from typed_values import validate_coordinate_ranges

CONTRACT_COMMIT = "0f824a09fe01f5ff327ab56f3296f36d8f4d7c87"
SEMANTIC_SCHEMA_ID = "https://reliquary.invalid/corpus-evidence/semantic-row-v1"
SEMANTIC_SCHEMA_SHA256 = "77499989921a42ff529fb768c1d3215aeb6ed73bbfd0564b2f057bbfdc8c1165"
INVENTORY_SCHEMA_ID = "reliquary-d02-synthetic-inventory-v1"
TARGET_COVERAGE_SCHEMA_ID = "reliquary-d02-synthetic-target-coverage-v1"
INVENTORY_TRUST_NOTE = "hand-authored synthetic test inventory; not authenticated real-source authority"
TARGET_TRUST_NOTE = "synthetic target observation; not authenticated"

# Confirmed against PG-C01's 0f824a09 record-schemas-v1.json and
# validate_semantic_row: each family ID is its record schema ID and maps to a group.
FAMILY_GROUPS = {
    "fixture.archive_turn_revision.v1": "archive_echo",
    "fixture.memory_revision.v1": "memory",
    "fixture.vector_probe.v1": "vectors",
    "fixture.freshness_event.v1": "freshness",
    "fixture.ego_anchor.v1": "ego",
}
HEX64 = re.compile(r"^[0-9a-f]{64}$")
UUID32 = re.compile(r"^[0-9a-f]{32}$")
UDEC = re.compile(r"^(?:0|[1-9][0-9]*)$")
U64_MAX = (1 << 64) - 1
CUT_DOMAIN = b"reliquary:corpus-evidence:cut:v1\0"


def canonical_json_bytes(value: Any) -> bytes:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode("utf-8", "strict")


def cut_id_for(coordinate: dict[str, Any]) -> str:
    encoded = canonical_json_bytes(coordinate)
    return hashlib.sha256(CUT_DOMAIN + len(encoded).to_bytes(8, "big") + encoded).hexdigest()


def _u64_text(value: Any, label: str) -> int:
    if not isinstance(value, str) or not UDEC.fullmatch(value):
        raise WitnessInputError(f"{label} must be canonical unsigned decimal")
    if len(value) > 20:
        raise WitnessInputError(f"{label} exceeds u64")
    number = int(value)
    if number > U64_MAX:
        raise WitnessInputError(f"{label} exceeds u64")
    return number


def validate_supported_semantic_schema(schema: dict[str, Any], schema_bytes: bytes) -> str:
    if schema.get("$id") != SEMANTIC_SCHEMA_ID:
        raise WitnessInputError("unsupported semantic schema $id")
    actual_hash = hashlib.sha256(schema_bytes).hexdigest()
    if actual_hash != SEMANTIC_SCHEMA_SHA256:
        raise WitnessInputError("unsupported semantic schema bytes/hash")
    try:
        parsed = json.loads(schema_bytes.decode("utf-8", "strict"))
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise WitnessInputError(f"invalid pinned semantic schema bytes: {exc}") from exc
    if parsed != schema:
        raise WitnessInputError("schema object differs from the pinned schema bytes")
    return actual_hash


def inventory_key(
    owner: dict[str, str], family: str, row: dict[str, Any]
) -> tuple[bytes, ...]:
    strings = (
        owner["owner_uuid"], owner["owner_kind"], owner["scope"], family,
        row["stable_key"], row["revision_or_event"], row["ordinal"],
    )
    ordinal = _u64_text(strings[-1], "inventory row ordinal")
    return tuple(value.encode("utf-8", "strict") for value in strings[:-1]) + (
        ordinal.to_bytes(8, "big"),
    )


def _exact_keys(value: Any, expected: set[str], label: str) -> dict[str, Any]:
    if not isinstance(value, dict) or set(value) != expected:
        raise WitnessInputError(f"{label} must have exactly {sorted(expected)}")
    return value


def _valid_row_identity(row: Any, label: str) -> dict[str, Any]:
    fields = _exact_keys(row, {"stable_key", "revision_or_event", "ordinal", "cut_ids"}, label)
    if not isinstance(fields["stable_key"], str) or not fields["stable_key"]:
        raise WitnessInputError(f"{label}.stable_key must be nonempty text")
    if not isinstance(fields["revision_or_event"], str) or not fields["revision_or_event"]:
        raise WitnessInputError(f"{label}.revision_or_event must be nonempty text")
    ordinal = fields["ordinal"]
    _u64_text(ordinal, f"{label}.ordinal")
    if not isinstance(fields["cut_ids"], list) or not fields["cut_ids"]:
        raise WitnessInputError(f"{label}.cut_ids must be a nonempty array")
    if any(not isinstance(cut_id, str) or not HEX64.fullmatch(cut_id) for cut_id in fields["cut_ids"]):
        raise WitnessInputError(f"{label}.cut_ids contains an invalid digest")
    if fields["cut_ids"] != sorted(set(fields["cut_ids"])):
        raise WitnessInputError(f"{label}.cut_ids must be unique and byte ordered")
    return fields


def _validate_coordinate(coordinate: Any, schema: dict[str, Any], label: str) -> dict[str, Any]:
    if not isinstance(coordinate, dict):
        raise WitnessInputError(f"{label} must be an object")
    try:
        from jsonschema import Draft202012Validator
    except ImportError as exc:
        raise WitnessInputError("jsonschema 4.26.0 is required; see requirements-test.txt") from exc
    coordinate_schema = {
        "$schema": schema["$schema"],
        "$defs": schema["$defs"],
        "$ref": "#/$defs/coordinate",
    }
    Draft202012Validator.check_schema(coordinate_schema)
    errors = list(Draft202012Validator(coordinate_schema).iter_errors(coordinate))
    if errors:
        raise WitnessInputError(f"{label} violates the frozen coordinate schema: {errors[0].message}")
    validate_coordinate_ranges(coordinate, label)
    domains = [entry["domain_id"] for entry in coordinate["domain_versions"]]
    if domains != sorted(set(domains), key=lambda item: item.encode("utf-8")):
        raise WitnessInputError(f"{label}.domain_versions must be unique and UTF-8-byte ordered")
    return coordinate


def _validate_owner(owner: Any, label: str) -> dict[str, str]:
    value = _exact_keys(owner, {"owner_uuid", "owner_kind", "scope"}, label)
    if not isinstance(value["owner_uuid"], str) or not UUID32.fullmatch(value["owner_uuid"]):
        raise WitnessInputError(f"{label}.owner_uuid must be 16-byte lowercase hex")
    if not isinstance(value["owner_kind"], str) or value["owner_kind"] not in {"REL", "PHY"}:
        raise WitnessInputError(f"{label}.owner_kind is unsupported")
    # PG-C01 v1 deliberately leaves scope as a string. Membership in the pinned
    # synthetic owner inventory, not an invented global pairing rule, controls scope.
    if not isinstance(value["scope"], str):
        raise WitnessInputError(f"{label}.scope must be text")
    return value


def validate_coverage_bytes(
    raw: bytes,
    schema: dict[str, Any],
    expected_schema_id: str,
    trust_note: str,
    label: str,
) -> tuple[dict[str, Any], dict[tuple[bytes, ...], dict[str, Any]]]:
    document = strict_json_document(raw, label)
    required = {
        "schema_id", "evidence_scope", "semantic_schema_id", "semantic_schema_sha256",
        "trust_note", "owners",
    }
    _exact_keys(document, required, label)
    if (
        not isinstance(document["schema_id"], str)
        or document["schema_id"] != expected_schema_id
        or document["evidence_scope"] != "synthetic_fixture_only"
    ):
        raise WitnessInputError(f"{label} has an unsupported schema or scope")
    if (
        document["semantic_schema_id"] != SEMANTIC_SCHEMA_ID
        or document["semantic_schema_sha256"] != SEMANTIC_SCHEMA_SHA256
    ):
        raise WitnessInputError(f"{label} semantic schema pin mismatch")
    if document["trust_note"] != trust_note:
        raise WitnessInputError(f"{label} trust note mismatch")
    if not isinstance(document["owners"], list) or not document["owners"]:
        raise WitnessInputError(f"{label}.owners must be nonempty")

    owner_keys: list[bytes] = []
    expected_rows: dict[tuple[bytes, ...], dict[str, Any]] = {}
    for owner_index, owner_entry in enumerate(document["owners"]):
        owner_entry = _exact_keys(owner_entry, {"owner", "cuts", "families"}, f"{label}.owners[{owner_index}]")
        owner = _validate_owner(owner_entry["owner"], f"{label}.owners[{owner_index}].owner")
        owner_key_bytes = canonical_json_bytes(owner)
        owner_keys.append(owner_key_bytes)
        cuts = owner_entry["cuts"]
        if not isinstance(cuts, list) or not cuts:
            raise WitnessInputError(f"{label} owner cuts must be nonempty")
        cut_ids: list[str] = []
        for cut_index, cut in enumerate(cuts):
            cut = _exact_keys(cut, {"cut_id", "coordinate"}, f"{label} cut[{cut_index}]")
            if not isinstance(cut["cut_id"], str) or not HEX64.fullmatch(cut["cut_id"]):
                raise WitnessInputError(f"{label} cut ID is invalid")
            coordinate = _validate_coordinate(cut["coordinate"], schema, f"{label} cut coordinate")
            if cut["cut_id"] != cut_id_for(coordinate):
                raise WitnessInputError(f"{label} cut ID does not bind its coordinate")
            cut_ids.append(cut["cut_id"])
        if cut_ids != sorted(set(cut_ids)):
            raise WitnessInputError(f"{label} cuts must be unique and cut-ID ordered")

        families = owner_entry["families"]
        if not isinstance(families, list) or [entry.get("family") for entry in families if isinstance(entry, dict)] != sorted(FAMILY_GROUPS):
            raise WitnessInputError(f"{label} must enumerate the exact sorted v1 family catalog")
        for family_index, family_entry in enumerate(families):
            family_entry = _exact_keys(
                family_entry,
                {"family", "group_id", "presence", "completeness", "row_count", "cuts", "rows"},
                f"{label} family[{family_index}]",
            )
            family = family_entry["family"]
            if (
                not isinstance(family, str)
                or family not in FAMILY_GROUPS
                or family_entry["group_id"] != FAMILY_GROUPS[family]
            ):
                raise WitnessInputError(f"{label} family/group is not in the frozen PG-C01 registry")
            if (
                not isinstance(family_entry["presence"], str)
                or family_entry["presence"] not in {"supported", "absent"}
                or family_entry["completeness"] != "exhaustive"
            ):
                raise WitnessInputError(f"{label} inventory must be exhaustive and supported/absent")
            rows = family_entry["rows"]
            if not isinstance(rows, list):
                raise WitnessInputError(f"{label} family rows must be an array")
            identities = [_valid_row_identity(row, f"{label} family row") for row in rows]
            identity_keys = [
                (
                    row["stable_key"].encode("utf-8"),
                    row["revision_or_event"].encode("utf-8"),
                    _u64_text(row["ordinal"], f"{label} family row ordinal"),
                )
                for row in identities
            ]
            if identity_keys != sorted(identity_keys) or len(identity_keys) != len(set(identity_keys)):
                raise WitnessInputError(f"{label} family row identities must be unique and ordered")
            row_count = family_entry["row_count"]
            if _u64_text(row_count, f"{label} family row_count") != len(rows):
                raise WitnessInputError(f"{label} family row count does not match its inventory")
            if family_entry["presence"] == "absent" and rows:
                raise WitnessInputError(f"{label} absent family cannot list rows")

            per_cut = family_entry["cuts"]
            if not isinstance(per_cut, list) or [entry.get("cut_id") for entry in per_cut if isinstance(entry, dict)] != cut_ids:
                raise WitnessInputError(f"{label} family must enumerate every owner cut in order")
            for cut_index, cut_row in enumerate(per_cut):
                cut_row = _exact_keys(
                    cut_row,
                    {"cut_id", "presence", "completeness", "row_count", "reason"},
                    f"{label} family cut[{cut_index}]",
                )
                actual_count = sum(cut_row["cut_id"] in row["cut_ids"] for row in identities)
                count = cut_row["row_count"]
                if _u64_text(count, f"{label} family cut row_count") != actual_count:
                    raise WitnessInputError(f"{label} per-cut row count does not match row memberships")
                if cut_row["completeness"] != "exhaustive":
                    raise WitnessInputError(f"{label} contains non-exhaustive cut coverage")
                if cut_row["presence"] == "supported":
                    if cut_row["reason"] is not None:
                        raise WitnessInputError(f"{label} supported cut reason must be null")
                elif cut_row["presence"] == "absent":
                    if actual_count != 0 or not isinstance(cut_row["reason"], str) or not cut_row["reason"]:
                        raise WitnessInputError(f"{label} absent cut must be empty and have a reason")
                else:
                    raise WitnessInputError(f"{label} cut presence must be supported or absent")
            if family_entry["presence"] == "absent" and any(cut["presence"] != "absent" for cut in per_cut):
                raise WitnessInputError(f"{label} absent family has a supported cut")

            for row in identities:
                if any(cut_id not in cut_ids for cut_id in row["cut_ids"]):
                    raise WitnessInputError(f"{label} row references an undeclared cut")
                key = inventory_key(owner, family, row)
                if key in expected_rows:
                    raise WitnessInputError(f"{label} has a duplicate owner/family/row identity")
                # Membership is explicit inventory metadata. Semantic-row
                # coordinate remains publication/origin provenance, not as-of
                # membership, so one immutable row may appear at several cuts.
                expected_rows[key] = row

    if owner_keys != sorted(set(owner_keys)):
        raise WitnessInputError(f"{label} owners must be unique and canonical-byte ordered")
    return document, expected_rows
