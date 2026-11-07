"""Deterministic, fail-closed primitives for the provisional PG-C01 bundle."""
from __future__ import annotations
import hashlib
import json
import re
from pathlib import Path
from typing import Any, Iterable, Mapping, Sequence

CONTRACT_ID = "reliquary-corpus-evidence-v1"
GROUP_IDS = (
    "container_lineage", "archive_echo", "memory", "entity_relationship",
    "graph_community", "ego", "freshness", "insomnia_dream_import",
    "vectors", "external_project_operational", "maintenance",
)
FAMILY_DOMAIN = b"reliquary:corpus-evidence:v1\x00"
OWNER_DOMAIN = b"reliquary:corpus-evidence:owner:v1\x00"
CORPUS_DOMAIN = b"reliquary:corpus-evidence:corpus:v1\x00"
CUSTODY_DOMAIN = b"reliquary:corpus-evidence:custody:v1\x00"
CUT_DOMAIN = b"reliquary:corpus-evidence:cut:v1\x00"
BUNDLE_DOMAIN = b"reliquary:corpus-evidence:contract-bundle:v1\x00"
MAX_U64 = (1 << 64) - 1
AVAILABILITY = {"present", "missing", "tombstoned", "pruned", "unavailable", "unauthorized", "corrupt"}
PRESENCE = {"supported", "absent", "unsupported", "unavailable", "corrupt", "unaudited", "unclassified"}
COMPLETENESS = {"exhaustive", "sampled", "incomplete"}
_HEX = re.compile(r"(?:[0-9a-f]{2})+\Z")
_SHA256 = re.compile(r"[0-9a-f]{64}\Z")
_UUID = re.compile(r"[0-9a-f]{32}\Z")
_UDEC = re.compile(r"(?:0|[1-9][0-9]*)\Z")
_IDEC = re.compile(r"(?:0|[1-9][0-9]*|-[1-9][0-9]*)\Z")


class ContractError(ValueError):
    """Evidence is malformed, incomplete, or not in its reviewed registry."""


def _number_forbidden(token: str) -> None:
    raise ContractError(f"JSON number token {token!r} is forbidden; use typed strings")


def _pairs_unique(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise ContractError(f"duplicate JSON key after escape decoding: {key!r}")
        result[key] = value
    return result


def _constant_forbidden(token: str) -> None:
    raise ContractError(f"non-finite JSON constant {token!r} is forbidden")


def _check_unicode(value: Any) -> None:
    if isinstance(value, str):
        try:
            value.encode("utf-8", "strict")
        except UnicodeEncodeError as exc:
            raise ContractError("unpaired surrogate is forbidden") from exc
    elif isinstance(value, list):
        for child in value:
            _check_unicode(child)
    elif isinstance(value, dict):
        for key, child in value.items():
            _check_unicode(key)
            _check_unicode(child)


def strict_json_loads(data: bytes | str) -> Any:
    if isinstance(data, bytes):
        try:
            text = data.decode("utf-8", "strict")
        except UnicodeDecodeError as exc:
            raise ContractError("evidence JSON is not valid UTF-8") from exc
    else:
        text = data
    if text.startswith("\ufeff"):
        raise ContractError("UTF-8 BOM is forbidden")
    try:
        parsed = json.loads(
            text, object_pairs_hook=_pairs_unique, parse_int=_number_forbidden,
            parse_float=_number_forbidden, parse_constant=_constant_forbidden,
        )
    except ContractError:
        raise
    except (json.JSONDecodeError, UnicodeError) as exc:
        raise ContractError(f"invalid evidence JSON: {exc}") from exc
    _check_unicode(parsed)
    return parsed


def canonical_json_bytes(value: Any) -> bytes:
    """RFC 8785/JCS string/object handling, restricted to number-free JSON."""
    def emit(item: Any) -> str:
        if item is None:
            return "null"
        if item is True:
            return "true"
        if item is False:
            return "false"
        if isinstance(item, str):
            _check_unicode(item)
            return json.dumps(item, ensure_ascii=False, separators=(",", ":"), allow_nan=False)
        if isinstance(item, (int, float)):
            raise ContractError("JSON numeric values are forbidden in evidence")
        if isinstance(item, list):
            return "[" + ",".join(emit(child) for child in item) + "]"
        if isinstance(item, dict):
            if any(not isinstance(key, str) for key in item):
                raise ContractError("JSON object keys must be strings")
            keys = sorted(item, key=lambda key: key.encode("utf-16-be", "strict"))
            return "{" + ",".join(emit(key) + ":" + emit(item[key]) for key in keys) + "}"
        raise ContractError(f"unsupported JSON type: {type(item).__name__}")
    _check_unicode(value)
    return emit(value).encode("utf-8", "strict")


def u64be(number: int) -> bytes:
    if isinstance(number, bool) or not isinstance(number, int) or number < 0 or number > MAX_U64:
        raise ContractError("value does not fit unsigned 64-bit integer")
    return number.to_bytes(8, "big")


def lp(data: bytes) -> bytes:
    return u64be(len(data)) + data


def decimal_u64(value: Any, label: str) -> int:
    if not isinstance(value, str) or not _UDEC.fullmatch(value):
        raise ContractError(f"{label} must be canonical unsigned decimal")
    number = int(value)
    if number > MAX_U64:
        raise ContractError(f"{label} exceeds u64")
    return number


def decimal_i64(value: Any, label: str) -> int:
    if not isinstance(value, str) or not _IDEC.fullmatch(value):
        raise ContractError(f"{label} must be canonical signed decimal")
    number = int(value)
    if number < -(1 << 63) or number > (1 << 63) - 1:
        raise ContractError(f"{label} exceeds i64")
    return number


def _exact(value: Any, fields: set[str], label: str) -> None:
    if not isinstance(value, dict) or set(value) != fields:
        actual = set(value) if isinstance(value, dict) else set()
        raise ContractError(f"{label} fields differ: missing={sorted(fields-actual)}, extra={sorted(actual-fields)}")


def _hex(value: Any, label: str) -> bytes:
    if not isinstance(value, str) or not _HEX.fullmatch(value):
        raise ContractError(f"{label} must be nonempty lowercase even-length hex")
    return bytes.fromhex(value)


def _hash(value: Any, label: str) -> bytes:
    if not isinstance(value, str) or not _SHA256.fullmatch(value):
        raise ContractError(f"{label} must be 64 lowercase hex characters")
    return bytes.fromhex(value)


def _uuid(value: Any, label: str) -> bytes:
    if not isinstance(value, str) or not _UUID.fullmatch(value):
        raise ContractError(f"{label} must be a 16-byte lowercase hex UUID")
    return bytes.fromhex(value)


def load_registries(root: Path) -> tuple[dict[str, Any], dict[str, Any], dict[str, Any]]:
    folder = root / "registry"
    groups = strict_json_loads((folder / "semantic-groups-v1.json").read_bytes())
    records = strict_json_loads((folder / "record-schemas-v1.json").read_bytes())
    carriers = strict_json_loads((folder / "physical-carriers-v1.json").read_bytes())
    if tuple(row.get("id") for row in groups.get("groups", [])) != GROUP_IDS:
        raise ContractError("semantic group registry must contain the frozen ordered 11-code catalog")
    if groups.get("contract_id") != CONTRACT_ID or records.get("contract_id") != CONTRACT_ID or carriers.get("contract_id") != CONTRACT_ID:
        raise ContractError("registry contract ID mismatch")
    validate_registry_bundle(groups, records, carriers)
    return groups, records, carriers


def owner_key(owner: Mapping[str, Any]) -> bytes:
    _exact(dict(owner), {"owner_uuid", "owner_kind", "scope"}, "owner key")
    _uuid(owner["owner_uuid"], "owner_uuid")
    if owner["owner_kind"] not in {"REL", "PHY"} or not isinstance(owner["scope"], str):
        raise ContractError("invalid owner kind or scope")
    _check_unicode(owner["scope"])
    return canonical_json_bytes(dict(owner))


def _schema(records: Mapping[str, Any], schema_id: str, expected_group: str | None, authentic: bool) -> Mapping[str, Any]:
    matches = [item for item in records.get("records", []) if item.get("schema_id") == schema_id]
    if len(matches) != 1:
        raise ContractError(f"unknown or duplicate value schema {schema_id!r}")
    entry = matches[0]
    if expected_group is not None and entry.get("group_id") != expected_group:
        raise ContractError(f"{schema_id} does not belong to group {expected_group}")
    if authentic and entry.get("authentic_admission") is not True:
        raise ContractError(f"{schema_id} is synthetic-only")
    return entry
def _validate_float(value: Any, label: str) -> None:
    _exact(value, {"kind", "scalar", "endianness", "hex"}, label)
    if value["kind"] != "float_bits" or value["scalar"] not in {"f32", "f64"} or value["endianness"] not in {"little", "big"}:
        raise ContractError(f"{label} has invalid scalar/endian tag")
    raw = _hex(value["hex"], label + ".hex")
    if len(raw) != (4 if value["scalar"] == "f32" else 8):
        raise ContractError(f"{label} bit width does not match scalar tag")
    fraction_width, exponent_width = (23, 8) if value["scalar"] == "f32" else (52, 11)
    bits = int.from_bytes(raw, value["endianness"])
    exponent = (bits >> fraction_width) & ((1 << exponent_width) - 1)
    if exponent == (1 << exponent_width) - 1:
        raise ContractError(f"{label} encodes non-finite IEEE bits")


def _field(value: Any, field_type: str, label: str) -> None:
    if field_type == "text":
        _exact(value, {"kind", "value"}, label)
        if value["kind"] != "text" or not isinstance(value["value"], str):
            raise ContractError(f"{label} must be tagged text")
        _check_unicode(value["value"])
    elif field_type in {"hex", "sha256"}:
        _exact(value, {"kind", "hex"}, label)
        if value["kind"] != "bytes":
            raise ContractError(f"{label} must be tagged bytes")
        (_hash if field_type == "sha256" else _hex)(value["hex"], label + ".hex")
    elif field_type in {"u64", "i64"}:
        _exact(value, {"kind", "signed", "decimal"}, label)
        signed = field_type == "i64"
        if value["kind"] != "integer" or value["signed"] is not signed:
            raise ContractError(f"{label} integer tag differs from schema")
        (decimal_i64 if signed else decimal_u64)(value["decimal"], label + ".decimal")
    elif field_type == "float_bits":
        _validate_float(value, label)
    elif field_type == "optional_text":
        if not isinstance(value, dict) or value.get("kind") != "optional":
            raise ContractError(f"{label} must be tagged optional")
        if value.get("state") == "absent":
            _exact(value, {"kind", "state"}, label)
        elif value.get("state") == "present":
            _exact(value, {"kind", "state", "value"}, label)
            _field(value["value"], "text", label + ".value")
        else:
            raise ContractError(f"{label} optional state must be present or absent")
    elif field_type == "hex_array":
        _exact(value, {"kind", "ordering", "items"}, label)
        if value["kind"] != "array" or value["ordering"] != "ordered" or not isinstance(value["items"], list):
            raise ContractError(f"{label} must be an ordered tagged array")
        for index, child in enumerate(value["items"]):
            _field(child, "hex", f"{label}.items[{index}]")
    else:
        raise ContractError(f"unknown record field type {field_type!r}")


def validate_record(value: Any, records: Mapping[str, Any], group_id: str | None = None, authentic: bool = False) -> None:
    _exact(value, {"kind", "schema_id", "fields"}, "record value")
    if value["kind"] != "record" or not isinstance(value["schema_id"], str) or not isinstance(value["fields"], dict):
        raise ContractError("record kind, schema_id, or fields invalid")
    schema = _schema(records, value["schema_id"], group_id, authentic)
    declared = schema["fields"]
    names = [item["name"] for item in declared]
    if len(names) != len(set(names)) or set(value["fields"]) != set(names):
        raise ContractError("record field names do not exactly match its closed schema")
    for item in declared:
        _field(value["fields"][item["name"]], item["type"], f"{value['schema_id']}.{item['name']}")


def _optional_integer(value: Any, signed: bool, label: str) -> None:
    if not isinstance(value, dict):
        raise ContractError(f"{label} must be explicitly present or absent")
    if value.get("state") == "absent":
        _exact(value, {"state"}, label)
    elif value.get("state") == "present":
        _exact(value, {"state", "value"}, label)
        (decimal_i64 if signed else decimal_u64)(value["value"], label + ".value")
    else:
        raise ContractError(f"{label} state must be present or absent")


def validate_coordinate(value: Any, domain_ids: set[str]) -> None:
    _exact(value, {"global_version", "domain_versions", "transaction_time", "rel_activity_position", "accepted_event_boundary"}, "coordinate")
    decimal_u64(value["global_version"], "coordinate.global_version")
    if not isinstance(value["domain_versions"], list):
        raise ContractError("coordinate domain_versions must be an array")
    previous = None
    for index, entry in enumerate(value["domain_versions"]):
        _exact(entry, {"domain_id", "version"}, f"domain_versions[{index}]")
        domain = entry["domain_id"]
        if not isinstance(domain, str) or domain not in domain_ids:
            raise ContractError(f"unknown coordinate domain {domain!r}")
        encoded = domain.encode("utf-8")
        if previous is not None and encoded <= previous:
            raise ContractError("domain versions must be unique and UTF-8-byte ordered")
        previous = encoded
        decimal_u64(entry["version"], f"domain_versions[{index}].version")
    _optional_integer(value["transaction_time"], True, "transaction_time")
    _optional_integer(value["rel_activity_position"], False, "rel_activity_position")
    _optional_integer(value["accepted_event_boundary"], False, "accepted_event_boundary")


def cut_id_for(coordinate: Mapping[str, Any]) -> str:
    return hashlib.sha256(CUT_DOMAIN + lp(canonical_json_bytes(dict(coordinate)))).hexdigest()


def validate_cut(value: Any, domain_ids: set[str]) -> None:
    _exact(value, {"cut_id", "coordinate", "availability", "completeness", "reason"}, "cut")
    validate_coordinate(value["coordinate"], domain_ids)
    if value["cut_id"] != cut_id_for(value["coordinate"]):
        raise ContractError("cut_id is not the hash of its typed coordinate")
    if value["availability"] not in AVAILABILITY or value["completeness"] not in COMPLETENESS:
        raise ContractError("unknown cut availability/completeness")
    if value["availability"] == "present":
        if value["reason"] is not None:
            raise ContractError("present cut reason must be null")
    elif not isinstance(value["reason"], str) or not value["reason"]:
        raise ContractError("unavailable cut requires a reason")


def validate_owner_descriptor(value: Any, records: Mapping[str, Any], domain_ids: set[str], authentic: bool = False) -> None:
    _exact(value, {"owner_uuid", "owner_kind", "scope", "source_file_sha256", "history_generation", "lineage", "version_heads", "supported_cuts"}, "owner descriptor")
    owner_key({key: value[key] for key in ("owner_uuid", "owner_kind", "scope")})
    _hash(value["source_file_sha256"], "source_file_sha256")
    decimal_u64(value["history_generation"], "history_generation")
    lineage = value["lineage"]
    if not isinstance(lineage, dict) or lineage.get("kind") != "optional":
        raise ContractError("lineage must be a tagged optional record")
    if lineage.get("state") == "absent":
        _exact(lineage, {"kind", "state"}, "lineage")
    elif lineage.get("state") == "present":
        _exact(lineage, {"kind", "state", "value"}, "lineage")
        validate_record(lineage["value"], records, "container_lineage", authentic)
        lineage_schema = _schema(records, lineage["value"]["schema_id"], "container_lineage", authentic)
        if lineage_schema.get("semantic_family") is not False:
            raise ContractError("owner lineage must use a non-family lineage record schema")
    else:
        raise ContractError("invalid lineage option")
    if not isinstance(value["version_heads"], list):
        raise ContractError("version_heads must be an array")
    previous = None
    for index, entry in enumerate(value["version_heads"]):
        _exact(entry, {"domain_id", "version"}, f"version_heads[{index}]")
        domain = entry["domain_id"]
        if domain not in domain_ids:
            raise ContractError(f"unknown version-head domain {domain!r}")
        encoded = domain.encode("utf-8")
        if previous is not None and encoded <= previous:
            raise ContractError("version heads must be unique and UTF-8-byte ordered")
        previous = encoded
        decimal_u64(entry["version"], f"version_heads[{index}].version")
    if not isinstance(value["supported_cuts"], list):
        raise ContractError("supported_cuts must be an array")
    previous = None
    for cut in value["supported_cuts"]:
        validate_cut(cut, domain_ids)
        encoded = bytes.fromhex(cut["cut_id"])
        if previous is not None and encoded <= previous:
            raise ContractError("supported cuts must be unique and ordered by cut-id bytes")
        previous = encoded


def validate_semantic_row(value: Any, groups: Mapping[str, Any], records: Mapping[str, Any], domain_ids: set[str], authentic: bool = False) -> None:
    _exact(value, {"family", "group_id", "stable_key", "revision_or_event", "ordinal", "owner", "coordinate", "availability", "value", "source_binding"}, "semantic row")
    if value["group_id"] not in {entry["id"] for entry in groups["groups"]}:
        raise ContractError("unknown semantic group")
    if not isinstance(value["family"], str):
        raise ContractError("family must be a versioned logical family ID")
    schema = _schema(records, value["family"], value["group_id"], authentic)
    if schema.get("semantic_family") is not True:
        raise ContractError(f"{value['family']} is not a semantic family")
    for name in ("stable_key", "revision_or_event"):
        if not isinstance(value[name], str):
            raise ContractError(f"{name} must be text")
        _check_unicode(value[name])
    decimal_u64(value["ordinal"], "ordinal")
    owner_key(value["owner"])
    validate_coordinate(value["coordinate"], domain_ids)
    if value["availability"] not in AVAILABILITY:
        raise ContractError("unknown row availability")
    if value["availability"] == "present":
        validate_record(value["value"], records, value["group_id"], authentic)
        if value["value"]["schema_id"] != value["family"]:
            raise ContractError("row family ID must equal its typed record schema ID")
    else:
        _exact(value["value"], {"kind", "schema_id", "reason_code", "evidence_sha256"}, "unavailable value")
        if value["value"]["kind"] != "unavailable" or value["value"]["schema_id"] != value["family"]:
            raise ContractError("unavailable value must name its family schema")
        reason = value["value"]["reason_code"]
        if reason not in {"missing", "tombstoned", "pruned", "unavailable", "unauthorized", "corrupt", "intra_settlement_unavailable"}:
            raise ContractError("unknown unavailable reason code")
        if reason == "intra_settlement_unavailable":
            if value["availability"] != "unavailable" or value["group_id"] != "freshness":
                raise ContractError("intra-settlement unavailability is a typed Freshness-only state")
        elif reason != value["availability"]:
            raise ContractError("unavailable reason code must match row availability")
        _hash(value["value"]["evidence_sha256"], "unavailable evidence_sha256")
    binding = value["source_binding"]
    if not isinstance(binding, dict) or binding.get("kind") != "optional":
        raise ContractError("source_binding must be an explicit optional")
    if binding.get("state") == "absent":
        _exact(binding, {"kind", "state"}, "source_binding")
    elif binding.get("state") == "present":
        _exact(binding, {"kind", "state", "value"}, "source_binding")
        validate_record(binding["value"], records, "provenance", authentic)
        provenance_schema = _schema(records, binding["value"]["schema_id"], "provenance", authentic)
        if provenance_schema.get("semantic_family") is not False:
            raise ContractError("source_binding must use a non-family provenance record schema")
        fields = binding["value"]["fields"]
        start = decimal_u64(fields["source_body_span_start"]["decimal"], "source body span start")
        end = decimal_u64(fields["source_body_span_end"]["decimal"], "source body span end")
        if end < start:
            raise ContractError("source-body span must be a valid half-open byte range")
    else:
        raise ContractError("invalid source_binding option")
    if schema.get("requires_source_binding") is True and binding.get("state") != "present":
        raise ContractError(f"{value['family']} requires logical source provenance")
def row_sort_key(row: Mapping[str, Any]) -> tuple[bytes, bytes, bytes, bytes]:
    return (
        row["family"].encode("utf-8", "strict"),
        row["stable_key"].encode("utf-8", "strict"),
        row["revision_or_event"].encode("utf-8", "strict"),
        u64be(decimal_u64(row["ordinal"], "ordinal")),
    )


def canonical_rows(rows: Iterable[Mapping[str, Any]], groups: Mapping[str, Any], records: Mapping[str, Any], domain_ids: set[str], authentic: bool = False) -> list[bytes]:
    materialized = list(rows)
    for row in materialized:
        validate_semantic_row(row, groups, records, domain_ids, authentic)
    ordered = sorted(materialized, key=row_sort_key)
    keys = [row_sort_key(row) for row in ordered]
    if len(keys) != len(set(keys)):
        raise ContractError("duplicate semantic row key")
    return [canonical_json_bytes(dict(row)) for row in ordered]


def canonical_jsonl(rows: Sequence[bytes]) -> bytes:
    if any(b"\n" in row or b"\r" in row for row in rows):
        raise ContractError("canonical row bytes cannot contain line delimiters")
    return b"".join(row + b"\n" for row in rows)


def parse_jsonl(data: bytes) -> list[Any]:
    if data.startswith(b"\xef\xbb\xbf"):
        raise ContractError("JSONL BOM is forbidden")
    if not data:
        return []
    if not data.endswith(b"\n"):
        raise ContractError("JSONL must end with LF")
    if b"\r" in data:
        raise ContractError("JSONL must use LF, not CR/CRLF")
    lines = data[:-1].split(b"\n")
    if any(not line for line in lines):
        raise ContractError("empty JSONL record is forbidden")
    return [strict_json_loads(line) for line in lines]


def validate_family_coverage(value: Any, family_registry: Mapping[str, Any], cut_ids: set[str]) -> None:
    _exact(value, {"family", "group_id", "presence", "completeness", "row_count", "cuts", "row_cut_memberships"}, "family coverage")
    records = {item["schema_id"]: item for item in family_registry["records"] if item.get("semantic_family") is True}
    if value["family"] not in records or value["group_id"] != records[value["family"]]["group_id"]:
        raise ContractError("coverage names unknown family or wrong semantic group")
    if value["presence"] not in PRESENCE or value["completeness"] not in COMPLETENESS:
        raise ContractError("unknown family presence/completeness")
    decimal_u64(value["row_count"], "family row_count")
    if not isinstance(value["cuts"], list):
        raise ContractError("family cuts must be an array")
    actual: list[str] = []
    for index, entry in enumerate(value["cuts"]):
        _exact(entry, {"cut_id", "presence", "completeness", "row_count", "reason"}, f"family cut coverage {index}")
        if entry["cut_id"] not in cut_ids:
            raise ContractError("family cut references an unlisted owner cut")
        if entry["presence"] not in PRESENCE or entry["completeness"] not in COMPLETENESS:
            raise ContractError("unknown family-cut presence/completeness")
        decimal_u64(entry["row_count"], f"family cut {index} row_count")
        if entry["presence"] == "supported":
            if entry["reason"] is not None:
                raise ContractError("supported family cut reason must be null")
        else:
            if not isinstance(entry["reason"], str) or not entry["reason"]:
                raise ContractError("non-supported family cut requires a reason")
            if entry["row_count"] != "0":
                raise ContractError("non-supported family cut cannot serialize semantic rows")
        actual.append(entry["cut_id"])
    ordered = sorted(set(actual), key=lambda value: bytes.fromhex(value))
    if actual != ordered:
        raise ContractError("family cut catalog must be unique and ordered by cut ID")
    if not isinstance(value["row_cut_memberships"], list):
        raise ContractError("row_cut_memberships must be an array")
    membership_keys: list[tuple[bytes, bytes, bytes, bytes]] = []
    membership_counts: dict[str, int] = {}
    membership_rows: set[tuple[bytes, bytes, bytes]] = set()
    for index, membership in enumerate(value["row_cut_memberships"]):
        _exact(membership, {"stable_key", "revision_or_event", "ordinal", "cut_id"}, f"row-cut membership {index}")
        if not isinstance(membership["stable_key"], str) or not isinstance(membership["revision_or_event"], str):
            raise ContractError("row-cut membership row key must be text")
        _check_unicode(membership["stable_key"])
        _check_unicode(membership["revision_or_event"])
        ordinal = u64be(decimal_u64(membership["ordinal"], f"row-cut membership {index} ordinal"))
        cut_id = membership["cut_id"]
        _hash(cut_id, f"row-cut membership {index} cut ID")
        if cut_id not in cut_ids or cut_id not in actual:
            raise ContractError("row-cut membership references an unlisted owner/family cut")
        row_key = (membership["stable_key"].encode("utf-8"), membership["revision_or_event"].encode("utf-8"), ordinal)
        membership_rows.add(row_key)
        membership_keys.append((*row_key, bytes.fromhex(cut_id)))
        membership_counts[cut_id] = membership_counts.get(cut_id, 0) + 1
    if membership_keys != sorted(set(membership_keys)):
        raise ContractError("row-cut memberships must be unique and ordered by row key then raw cut-ID bytes")
    for entry in value["cuts"]:
        if decimal_u64(entry["row_count"], "family cut row_count") != membership_counts.get(entry["cut_id"], 0):
            raise ContractError("family cut row_count differs from explicit row-cut membership table")
    if value["completeness"] == "exhaustive" and actual != sorted(cut_ids, key=lambda value: bytes.fromhex(value)):
        raise ContractError("exhaustive family coverage must name every owner cut")
    if value["completeness"] == "exhaustive" and any(entry["completeness"] != "exhaustive" for entry in value["cuts"]):
        raise ContractError("exhaustive family coverage requires exhaustive completeness at every cut")
    if value["presence"] != "supported" and value["row_count"] != "0":
        raise ContractError("non-supported family cannot serialize semantic rows")
    if value["presence"] != "supported" and value["row_cut_memberships"]:
        raise ContractError("non-supported family cannot claim row-to-cut membership")
    if value["presence"] == "supported" and value["completeness"] == "exhaustive":
        if any(item["presence"] not in {"supported", "absent"} for item in value["cuts"]):
            raise ContractError("exhaustive supported family cannot hide an unsupported cut")
        if value["cuts"] and not any(item["presence"] == "supported" for item in value["cuts"]):
            raise ContractError("family absent at every cut must use absent presence")
    if value["presence"] == "absent" and value["completeness"] == "exhaustive":
        if any(item["presence"] != "absent" for item in value["cuts"]):
            raise ContractError("exhaustive absent family must be absent at every owner cut")
    if value["presence"] not in {"supported", "absent"} and value["completeness"] == "exhaustive":
        raise ContractError("unsupported family state cannot claim exhaustive semantic rows")


def validate_group_coverage(value: Any, cut_ids: set[str]) -> None:
    _exact(value, {"group_id", "presence", "completeness", "family_count", "reason"}, "group coverage")
    if value["group_id"] not in GROUP_IDS or value["presence"] not in PRESENCE or value["completeness"] not in COMPLETENESS:
        raise ContractError("unknown semantic group coverage")
    decimal_u64(value["family_count"], "group family_count")
    if value["presence"] == "supported":
        if value["reason"] is not None:
            raise ContractError("supported group reason must be null")
    elif not isinstance(value["reason"], str) or not value["reason"]:
        raise ContractError("non-supported group needs explicit reason")
    if value["presence"] in {"unsupported", "unavailable", "corrupt", "unaudited", "unclassified"} and value["completeness"] == "exhaustive":
        raise ContractError("unreviewed group state cannot claim exhaustive family coverage")


def _family_preimage(owner: Mapping[str, Any], coverage: Mapping[str, Any], rows: Sequence[bytes]) -> bytes:
    return (
        FAMILY_DOMAIN
        + lp(owner_key(owner))
        + lp(coverage["family"].encode("utf-8"))
        + lp(canonical_json_bytes(dict(coverage)))
        + u64be(len(rows))
        + b"".join(lp(row) for row in rows)
    )


def family_digest(
    owner: Mapping[str, Any],
    coverage: Mapping[str, Any],
    rows: Sequence[Mapping[str, Any]],
    groups: Mapping[str, Any],
    records: Mapping[str, Any],
    domain_ids: set[str],
    owner_cut_ids: set[str],
) -> tuple[bytes, str]:
    validate_family_coverage(coverage, records, owner_cut_ids)
    materialized = list(rows)
    if decimal_u64(coverage.get("row_count"), "family row_count") != len(materialized):
        raise ContractError("family coverage row_count differs from digest row count")
    expected_owner = owner_key(owner)
    row_keys: set[tuple[bytes, bytes, bytes]] = set()
    for row in materialized:
        validate_semantic_row(row, groups, records, domain_ids)
        if row["family"] != coverage["family"] or owner_key(row["owner"]) != expected_owner:
            raise ContractError("family digest row is outside the declared family or owner")
        row_keys.add((row["stable_key"].encode("utf-8"), row["revision_or_event"].encode("utf-8"), u64be(decimal_u64(row["ordinal"], "ordinal"))))
    membership_rows = {
        (entry["stable_key"].encode("utf-8"), entry["revision_or_event"].encode("utf-8"), u64be(decimal_u64(entry["ordinal"], "membership ordinal")))
        for entry in coverage["row_cut_memberships"]
    }
    if row_keys != membership_rows:
        raise ContractError("row-cut membership table must reference exactly the family's semantic row keys")
    encoded = canonical_rows(materialized, groups, records, domain_ids)
    preimage = _family_preimage(owner, coverage, encoded)
    return preimage, hashlib.sha256(preimage).hexdigest()


def _semantic_owner_bytes(descriptor: Mapping[str, Any]) -> bytes:
    # Source file hashes, physical offsets and relocation data are custody only.
    semantic = {key: value for key, value in descriptor.items() if key != "source_file_sha256"}
    return canonical_json_bytes(semantic)


def registry_identity(groups: Mapping[str, Any], records: Mapping[str, Any], carriers: Mapping[str, Any]) -> bytes:
    # Bind reviewed catalog content, not caller-selected IDs alone. Canonical
    # registry hashes remain stable across JSON whitespace changes.
    validate_registry_bundle(groups, records, carriers)
    return canonical_json_bytes({
        "contract_id": CONTRACT_ID,
        "semantic_groups_sha256": hashlib.sha256(canonical_json_bytes(dict(groups))).hexdigest(),
        "record_schemas_sha256": hashlib.sha256(canonical_json_bytes(dict(records))).hexdigest(),
        "physical_carriers_sha256": hashlib.sha256(canonical_json_bytes(dict(carriers))).hexdigest(),
    })


def owner_digest(
    descriptor: Mapping[str, Any],
    group_coverage: Sequence[Mapping[str, Any]],
    family_coverage: Sequence[Mapping[str, Any]],
    family_rows: Mapping[str, Sequence[Mapping[str, Any]]],
    groups: Mapping[str, Any],
    records: Mapping[str, Any],
    domain_ids: set[str],
    carriers: Mapping[str, Any],
) -> tuple[bytes, str]:
    validate_owner_descriptor(descriptor, records, domain_ids)
    identity = registry_identity(groups, records, carriers)
    cuts = {entry["cut_id"] for entry in descriptor["supported_cuts"]}
    if {entry["group_id"] for entry in group_coverage} != set(GROUP_IDS) or len(group_coverage) != len(GROUP_IDS):
        raise ContractError("owner must bind exactly one coverage row for each semantic group")
    semantic_families = {entry["schema_id"]: entry for entry in records["records"] if entry.get("semantic_family") is True}
    if {entry["family"] for entry in family_coverage} != set(semantic_families) or len(family_coverage) != len(semantic_families):
        raise ContractError("owner must bind one coverage row for every registered logical family")
    if set(family_rows) != set(semantic_families):
        raise ContractError("owner digest rows differ from the reviewed family catalog")
    expected_group_counts = {group: sum(1 for schema in semantic_families.values() if schema["group_id"] == group) for group in GROUP_IDS}
    family_by_id = {row["family"]: row for row in family_coverage}
    for group in group_coverage:
        validate_group_coverage(group, cuts)
        if decimal_u64(group["family_count"], "group family_count") != expected_group_counts[group["group_id"]]:
            raise ContractError("group coverage family_count differs from versioned family catalog")
        children = [family_by_id[family_id] for family_id, schema in semantic_families.items() if schema["group_id"] == group["group_id"]]
        if not children and group["presence"] == "supported":
            raise ContractError("group with no reviewed family schemas cannot claim supported")
        if not children and group["presence"] == "absent":
            if (records["status"] != "synthetic-fixture-only" or records["authentic_admission"] is not False
                    or not group["reason"].startswith("outside synthetic population fixture.")):
                raise ContractError("group with no family schemas may be absent only in an explicitly named synthetic population")
        if group["presence"] == "supported" and not any(child["presence"] == "supported" for child in children):
            raise ContractError("supported group must have at least one supported registered family")
        if group["presence"] == "absent" and any(child["presence"] != "absent" for child in children):
            raise ContractError("absent group conflicts with registered family coverage")
        if group["completeness"] == "exhaustive" and any(child["completeness"] != "exhaustive" for child in children):
            raise ContractError("exhaustive group coverage requires exhaustive registered family coverage")
    groups_sorted = sorted(group_coverage, key=lambda row: row["group_id"].encode("utf-8"))
    for row in groups_sorted:
        validate_group_coverage(row, cuts)
    families_sorted = sorted(family_coverage, key=lambda row: row["family"].encode("utf-8"))
    family_hashes: dict[str, str] = {}
    for row in families_sorted:
        family_id = row["family"]
        _, family_hashes[family_id] = family_digest(
            {key: descriptor[key] for key in ("owner_uuid", "owner_kind", "scope")},
            row, family_rows[family_id], groups, records, domain_ids, cuts,
        )
    semantic_owner_key = owner_key({key: descriptor[key] for key in ("owner_uuid", "owner_kind", "scope")})
    parts = [OWNER_DOMAIN, lp(identity), lp(semantic_owner_key), lp(_semantic_owner_bytes(descriptor)), u64be(len(groups_sorted))]
    for group in groups_sorted:
        parts.append(lp(canonical_json_bytes(dict(group))))
    parts.append(u64be(len(families_sorted)))
    for family in families_sorted:
        family_id = family["family"]
        digest = family_hashes.get(family_id)
        if digest is None:
            raise ContractError(f"missing family digest for {family_id}")
        parts.extend((lp(family_id.encode("utf-8")), lp(canonical_json_bytes(dict(family))), u64be(len(family["cuts"]))))
        for cut in family["cuts"]:
            parts.append(lp(canonical_json_bytes(dict(cut))))
        parts.append(_hash(digest, f"family digest {family_id}"))
    preimage = b"".join(parts)
    return preimage, hashlib.sha256(preimage).hexdigest()


def _catalog_owner_key(entry: Mapping[str, Any]) -> bytes:
    _exact(entry, {"owner_uuid", "owner_kind", "scope", "source_file_sha256"}, "expected owner catalog entry")
    key = owner_key({name: entry[name] for name in ("owner_uuid", "owner_kind", "scope")})
    _hash(entry["source_file_sha256"], "expected owner source_file_sha256")
    return key


def _compose_verified_corpus_digest(
    owners: Sequence[tuple[Mapping[str, Any], str]],
    expected_owner_catalog: Sequence[Mapping[str, Any]],
    groups: Mapping[str, Any],
    records: Mapping[str, Any],
    domain_ids: set[str],
    carriers: Mapping[str, Any],
) -> tuple[bytes, str]:
    catalog = sorted(expected_owner_catalog, key=_catalog_owner_key)
    catalog_keys = [_catalog_owner_key(entry) for entry in catalog]
    if len(catalog_keys) != len(set(catalog_keys)):
        raise ContractError("expected owner catalog contains duplicate owner keys")
    ordered = sorted(owners, key=lambda item: owner_key({key: item[0][key] for key in ("owner_uuid", "owner_kind", "scope")}))
    keys = [owner_key({key: descriptor[key] for key in ("owner_uuid", "owner_kind", "scope")}) for descriptor, _ in ordered]
    if len(keys) != len(set(keys)):
        raise ContractError("duplicate semantic owner key")
    if keys != catalog_keys:
        raise ContractError("evidence owner set differs from explicit expected owner catalog")
    for (descriptor, _), catalog_entry in zip(ordered, catalog):
        if descriptor["source_file_sha256"] != catalog_entry["source_file_sha256"]:
            raise ContractError("owner source hash differs from expected owner catalog")
    identity = registry_identity(groups, records, carriers)
    parts = [CORPUS_DOMAIN, lp(identity), u64be(len(catalog))]
    parts.extend(lp(key) for key in catalog_keys)
    parts.append(u64be(len(ordered)))
    for descriptor, digest in ordered:
        validate_owner_descriptor(descriptor, records, domain_ids)
        parts.extend((lp(owner_key({key: descriptor[key] for key in ("owner_uuid", "owner_kind", "scope")})),
                      lp(_semantic_owner_bytes(descriptor)), _hash(digest, "verified owner digest")))
    preimage = b"".join(parts)
    return preimage, hashlib.sha256(preimage).hexdigest()


def corpus_digest(
    owners: Sequence[Mapping[str, Any]],
    expected_owner_catalog: Sequence[Mapping[str, Any]],
    groups: Mapping[str, Any],
    records: Mapping[str, Any],
    domain_ids: set[str],
    carriers: Mapping[str, Any],
) -> tuple[bytes, str]:
    # Full evidence is recomputed. An optional claim is checked but never trusted.
    verified: list[tuple[Mapping[str, Any], str]] = []
    allowed = {"descriptor", "group_coverage", "family_coverage", "family_rows", "claimed_owner_digest"}
    required = {"descriptor", "group_coverage", "family_coverage", "family_rows"}
    for index, evidence in enumerate(owners):
        if not isinstance(evidence, dict) or not required.issubset(evidence) or not set(evidence).issubset(allowed):
            raise ContractError(f"corpus owner evidence {index} fields differ")
        descriptor = evidence["descriptor"]
        _, computed = owner_digest(descriptor, evidence["group_coverage"], evidence["family_coverage"],
                                   evidence["family_rows"], groups, records, domain_ids, carriers)
        claimed = evidence.get("claimed_owner_digest")
        if claimed is not None:
            claimed_bytes = _hash(claimed, f"corpus owner evidence {index} claimed owner digest")
            if claimed_bytes != bytes.fromhex(computed):
                raise ContractError(f"corpus owner evidence {index} claimed owner digest does not match recomputed evidence")
        verified.append((descriptor, computed))
    return _compose_verified_corpus_digest(verified, expected_owner_catalog, groups, records, domain_ids, carriers)

def custody_digest(custody: Mapping[str, Any], records: Mapping[str, Any]) -> tuple[bytes, str]:
    _exact(custody, {"schema_id", "contract_id", "owner_files", "relocations"}, "physical custody")
    if custody["schema_id"] != "reliquary-corpus-physical-custody-v1" or custody["contract_id"] != CONTRACT_ID:
        raise ContractError("wrong physical custody schema or contract ID")
    if not isinstance(custody["owner_files"], list) or not isinstance(custody["relocations"], list):
        raise ContractError("owner_files and relocations must be arrays")
    owner_files = []
    owner_keys = []
    for index, item in enumerate(custody["owner_files"]):
        _exact(item, {"owner_uuid", "owner_kind", "scope", "source_file_sha256", "source_byte_length", "target_file_sha256", "target_byte_length"}, f"custody owner file {index}")
        key = owner_key({name: item[name] for name in ("owner_uuid", "owner_kind", "scope")})
        _hash(item["source_file_sha256"], f"custody owner file {index} source hash")
        decimal_u64(item["source_byte_length"], f"custody owner file {index} source byte length")
        _hash(item["target_file_sha256"], f"custody owner file {index} target hash")
        decimal_u64(item["target_byte_length"], f"custody owner file {index} target byte length")
        owner_keys.append(key)
        owner_files.append(item)
    if owner_keys != sorted(set(owner_keys)):
        raise ContractError("custody owner files must be unique and ordered by canonical full owner key")
    owner_key_set = set(owner_keys)
    if records.get("schema_id") != "reliquary-corpus-record-schemas-v1" or records.get("contract_id") != CONTRACT_ID:
        raise ContractError("custody requires the versioned contract record registry")
    family_ids = {record["schema_id"] for record in records.get("records", []) if record.get("semantic_family") is True}
    for index, row in enumerate(custody["relocations"]):
        _exact(row, {"owner_uuid", "owner_kind", "scope", "family", "source_object_sha256", "source_offset", "source_length", "target_object_sha256", "target_offset", "target_length", "payload_identity", "reference_kind"}, f"relocation {index}")
        full_owner_key = owner_key({name: row[name] for name in ("owner_uuid", "owner_kind", "scope")})
        if full_owner_key not in owner_key_set:
            raise ContractError("relocation has no matching full owner custody binding")
        if row["family"] not in family_ids:
            raise ContractError("relocation names unknown logical family schema")
        for field in ("source_object_sha256", "target_object_sha256"):
            _hash(row[field], f"relocation {index} {field}")
        for field in ("source_offset", "source_length", "target_offset", "target_length"):
            decimal_u64(row[field], f"relocation {index} {field}")
        _hex(row["payload_identity"], f"relocation {index} payload identity")
        if not isinstance(row["reference_kind"], str) or not row["reference_kind"]:
            raise ContractError("relocation reference_kind must be nonempty")
        _check_unicode(row["reference_kind"])
    def relocation_key(row: Mapping[str, Any]) -> tuple[bytes, ...]:
        return (
            owner_key({name: row[name] for name in ("owner_uuid", "owner_kind", "scope")}),
            row["family"].encode("utf-8"),
            _hash(row["source_object_sha256"], "source object hash"),
            u64be(decimal_u64(row["source_offset"], "source offset")),
            u64be(decimal_u64(row["source_length"], "source length")),
            _hash(row["target_object_sha256"], "target object hash"),
            u64be(decimal_u64(row["target_offset"], "target offset")),
            u64be(decimal_u64(row["target_length"], "target length")),
            _hex(row["payload_identity"], "payload identity"),
            row["reference_kind"].encode("utf-8"),
        )
    ordered = sorted(custody["relocations"], key=relocation_key)
    keys = [relocation_key(row) for row in ordered]
    if len(keys) != len(set(keys)):
        raise ContractError("duplicate physical relocation row")
    normalized = dict(custody)
    normalized["owner_files"] = owner_files
    normalized["relocations"] = ordered
    body = canonical_json_bytes(normalized)
    preimage = CUSTODY_DOMAIN + lp(body)
    return preimage, hashlib.sha256(preimage).hexdigest()

def bundle_digest(root: Path) -> tuple[bytes, str]:
    selected = []
    for top in ("registry", "schemas", "fixtures"):
        for path in (root / top).rglob("*"):
            if path.is_file() and path.name != "bundle.sha256" and not path.name.startswith("report-example-"):
                selected.append((path.relative_to(root).as_posix(), path.read_bytes()))
    selected.sort(key=lambda row: row[0].encode("utf-8"))
    parts = [BUNDLE_DOMAIN, u64be(len(selected))]
    for name, body in selected:
        parts.extend((lp(name.encode("utf-8")), lp(body)))
    preimage = b"".join(parts)
    return preimage, hashlib.sha256(preimage).hexdigest()


def validate_authentic_admission(groups: Mapping[str, Any], records: Mapping[str, Any], carriers: Mapping[str, Any]) -> None:
    if any(source.get("authentic_admission") is not True for source in (groups, records, carriers)):
        raise ContractError("provisional/synthetic registry cannot admit authentic sources")
    if carriers.get("status") != "reviewed" or not carriers.get("entries"):
        raise ContractError("physical carrier map missing or unreviewed")
    if any(not item.get("reviewed") for item in carriers["entries"]):
        raise ContractError("unreviewed/unknown physical carrier fails closed")
    if any(item.get("authentic_admission") is not True for item in records.get("records", [])):
        raise ContractError("synthetic record schema cannot admit authentic sources")


def validate_registry_bundle(groups: Mapping[str, Any], records: Mapping[str, Any], carriers: Mapping[str, Any]) -> None:
    _exact(groups, {"schema_id","contract_id","status","authentic_admission","carrier_registry_ref","record_schema_registry_ref","groups"}, "group registry")
    _exact(records, {"schema_id","contract_id","status","authentic_admission","version_policy","domain_ids","records"}, "record registry")
    _exact(carriers, {"schema_id","contract_id","status","authentic_admission","inventory_state","coverage","entries","unclassified_carriers"}, "carrier registry")
    expected_ids = ("reliquary-corpus-semantic-groups-v1","reliquary-corpus-record-schemas-v1","reliquary-corpus-physical-carriers-v1")
    if (groups["schema_id"],records["schema_id"],carriers["schema_id"]) != expected_ids:
        raise ContractError("wrong registry schema ID")
    if groups["status"] != "provisional-review-required" or records["status"] != "synthetic-fixture-only" or carriers["status"] != "unclassified-no-authenticated-inventory":
        raise ContractError("unexpected provisional registry state")
    if any(item["authentic_admission"] is not False for item in (groups,records,carriers)):
        raise ContractError("provisional registries must fail authentic admission")
    if len(groups["groups"]) != len(GROUP_IDS):
        raise ContractError("semantic group catalog has duplicates or omissions")
    for group in groups["groups"]:
        _exact(group, {"id","contract_label","retained_value_obligations"}, "semantic group")
        if not group["contract_label"] or not group["retained_value_obligations"]:
            raise ContractError("semantic group needs label and retained-value obligations")
    domain_ids = records["domain_ids"]
    if not domain_ids or domain_ids != sorted(set(domain_ids), key=lambda item: item.encode("utf-8")):
        raise ContractError("domain ID catalog must be nonempty, unique and UTF-8-byte ordered")
    seen = set()
    allowed_types = {"text","hex","sha256","u64","i64","float_bits","optional_text","hex_array"}
    for record in records["records"]:
        _exact(record, {"schema_id","group_id","semantic_family","requires_source_binding","authentic_admission","fields"}, "record schema")
        if record["schema_id"] in seen or record["group_id"] not in set(GROUP_IDS) | {"provenance"}:
            raise ContractError("duplicate schema ID or invalid record group")
        seen.add(record["schema_id"])
        if record["authentic_admission"] is not False or not record["schema_id"].startswith("fixture."):
            raise ContractError("provisional record schema must be fixture-only")
        if not isinstance(record["semantic_family"], bool) or not isinstance(record["requires_source_binding"], bool):
            raise ContractError("record flags must be booleans")
        names = [field["name"] for field in record["fields"]]
        if len(names) != len(set(names)) or not names or any(field["type"] not in allowed_types for field in record["fields"]):
            raise ContractError("record schema fields must be unique and use closed types")
    if carriers["inventory_state"] != "not_started" or carriers["coverage"] != "incomplete" or carriers["entries"] or carriers["unclassified_carriers"]:
        raise ContractError("physical carrier inventory must remain explicitly incomplete and empty")
    if records["status"] != "synthetic-fixture-only":
        raise ContractError("record registry status must remain synthetic-only")


def registry_hashes(root: Path) -> dict[str, str]:
    folder = root / "registry"
    names = {
        "semantic_groups": "semantic-groups-v1.json",
        "record_schemas": "record-schemas-v1.json",
        "physical_carriers": "physical-carriers-v1.json",
    }
    result = {key: hashlib.sha256((folder / name).read_bytes()).hexdigest() for key, name in names.items()}
    result["contract_bundle"] = bundle_digest(root)[1]
    return result


def _report_completeness(owners: Sequence[Mapping[str, Any]]) -> str:
    rank = {"exhaustive": 0, "sampled": 1, "incomplete": 2}
    result = 0
    for evidence in owners:
        descriptor = evidence["descriptor"]
        for cut in descriptor["supported_cuts"]:
            result = max(result, rank[cut["completeness"]])
            if cut["availability"] != "present":
                result = 2
        for group in evidence["group_coverage"]:
            result = max(result, rank[group["completeness"]])
            if group["presence"] in {"unsupported", "unavailable", "corrupt", "unaudited", "unclassified"}:
                result = 2
        for family in evidence["family_coverage"]:
            result = max(result, rank[family["completeness"]])
            if family["presence"] in {"unsupported", "unavailable", "corrupt", "unaudited", "unclassified"}:
                result = 2
            for cut in family["cuts"]:
                result = max(result, rank[cut["completeness"]])
                if cut["presence"] in {"unsupported", "unavailable", "corrupt", "unaudited", "unclassified"}:
                    result = 2
    return {0: "exhaustive", 1: "sampled", 2: "incomplete"}[result]


def _validate_diagnostics(value: Mapping[str, Any]) -> None:
    _exact(value, {"total_count", "sample_limit", "sample"}, "report diagnostics")
    total = decimal_u64(value["total_count"], "diagnostics total_count")
    limit = decimal_u64(value["sample_limit"], "diagnostics sample_limit")
    sample = value["sample"]
    if not isinstance(sample, list):
        raise ContractError("diagnostic sample must be an array")
    expected_sample_count = min(total, limit)
    if len(sample) != expected_sample_count:
        raise ContractError("diagnostic sample length must equal min(total_count, sample_limit)")
    keys = []
    for index, entry in enumerate(sample):
        _exact(entry, {"owner_uuid", "family", "stable_key", "revision_or_event", "cut_id",
                       "expected_availability", "observed_availability", "expected_sha256", "observed_sha256"},
               f"diagnostic sample {index}")
        _uuid(entry["owner_uuid"], f"diagnostic sample {index} owner")
        _hash(entry["expected_sha256"], f"diagnostic sample {index} expected hash")
        _hash(entry["observed_sha256"], f"diagnostic sample {index} observed hash")
        _hash(entry["cut_id"], f"diagnostic sample {index} cut ID")
        _check_unicode(entry)
        if (entry["expected_availability"] == entry["observed_availability"]
                and entry["expected_sha256"] == entry["observed_sha256"]):
            raise ContractError("diagnostic sample row does not describe a discrepancy")
        keys.append((
            bytes.fromhex(entry["owner_uuid"]),
            entry["family"].encode("utf-8"),
            bytes.fromhex(entry["cut_id"]),
            entry["stable_key"].encode("utf-8"),
            entry["revision_or_event"].encode("utf-8"),
        ))
    if keys != sorted(set(keys)):
        raise ContractError("diagnostic sample must have unique rows in canonical owner/family/cut/key order")


def _witness_binding_hash(witness: Mapping[str, Any], semantic_hash: str, expected_role: str) -> str:
    _exact(witness, {"producer_role", "producer_code_sha256", "producer_binary_sha256", "expected_owner_catalog", "owners"},
           f"{expected_role} witness")
    if witness["producer_role"] != expected_role:
        raise ContractError(f"{expected_role} witness has the wrong producer role")
    _hash(witness["producer_code_sha256"], f"{expected_role} producer code hash")
    _hash(witness["producer_binary_sha256"], f"{expected_role} producer binary hash")
    catalog = sorted(witness["expected_owner_catalog"], key=_catalog_owner_key)
    if not catalog:
        raise ContractError(f"{expected_role} witness owner catalog is empty")
    # Versioned canonical commitment binds both independent reader identity and its
    # recomputed semantic digest to the full per-owner physical input catalog.
    body = {
        "schema_id": "reliquary-corpus-witness-binding-v1",
        "producer_role": expected_role,
        "producer_code_sha256": witness["producer_code_sha256"],
        "producer_binary_sha256": witness["producer_binary_sha256"],
        "semantic_corpus_sha256": semantic_hash,
        "owner_catalog": catalog,
    }
    return hashlib.sha256(canonical_json_bytes(body)).hexdigest()


def _witness_coverage(owners: Sequence[Mapping[str, Any]]) -> dict[str, str]:
    return {
        "owner_count": str(len(owners)),
        "family_count": str(sum(len(item["family_coverage"]) for item in owners)),
        "cut_count": str(sum(len(item["descriptor"]["supported_cuts"]) for item in owners)),
        "row_count": str(sum(len(rows) for item in owners for rows in item["family_rows"].values())),
        "completeness": _report_completeness(owners),
    }


def _catalog_bindings(catalog: Sequence[Mapping[str, Any]]) -> list[dict[str, str]]:
    ordered = sorted(catalog, key=_catalog_owner_key)
    return [
        {key: entry[key] for key in ("owner_uuid", "owner_kind", "scope", "source_file_sha256")}
        for entry in ordered
    ]


def _validate_comparator_witness_bundle(
    evidence: Mapping[str, Any], report: Mapping[str, Any], root: Path
) -> tuple[dict[str, str], set[str]]:
    _exact(evidence, {"source_witness", "target_witness", "custody"}, "comparator witness evidence bundle")
    groups, records, carriers = load_registries(root)
    domains = domain_ids_from_registry(records)
    source = evidence["source_witness"]
    target = evidence["target_witness"]
    witness_fields = {"producer_role", "producer_code_sha256", "producer_binary_sha256", "expected_owner_catalog", "owners"}
    _exact(source, witness_fields, "source_reader witness")
    _exact(target, witness_fields, "target_reader witness")
    source_pre, source_hash = corpus_digest(
        source["owners"], source["expected_owner_catalog"], groups, records, domains, carriers
    )
    target_pre, target_hash = corpus_digest(
        target["owners"], target["expected_owner_catalog"], groups, records, domains, carriers
    )
    # Parse and bind reader role/code/binary identity plus each independently
    # recomputed corpus digest and exact physical owner catalog.
    source_binding_hash = _witness_binding_hash(source, source_hash, "source_reader")
    target_binding_hash = _witness_binding_hash(target, target_hash, "target_reader")
    source_catalog = _catalog_bindings(source["expected_owner_catalog"])
    target_catalog = _catalog_bindings(target["expected_owner_catalog"])
    source_keys = [owner_key({key: item[key] for key in ("owner_uuid", "owner_kind", "scope")}) for item in source_catalog]
    target_keys = [owner_key({key: item[key] for key in ("owner_uuid", "owner_kind", "scope")}) for item in target_catalog]
    if source_keys != target_keys:
        raise ContractError("source and target witness owner catalogs differ")
    if report["source_owner_bindings"] != source_catalog:
        raise ContractError("report source owner bindings differ from source witness catalog")
    if report["source_hashes"] != [item["source_file_sha256"] for item in source_catalog]:
        raise ContractError("report source hashes differ from source witness catalog")
    _, custody_hash = custody_digest(evidence["custody"], records)
    custody_files = sorted(
        evidence["custody"]["owner_files"],
        key=lambda row: owner_key({key: row[key] for key in ("owner_uuid", "owner_kind", "scope")}),
    )
    source_files = [
        {key: item[key] for key in ("owner_uuid", "owner_kind", "scope", "source_file_sha256")}
        for item in custody_files
    ]
    target_files = [
        {"owner_uuid": item["owner_uuid"], "owner_kind": item["owner_kind"], "scope": item["scope"],
         "source_file_sha256": item["target_file_sha256"]}
        for item in custody_files
    ]
    if source_catalog != source_files or target_catalog != target_files:
        raise ContractError("source/target witness catalogs differ from per-owner custody files")
    source_coverage = _witness_coverage(source["owners"])
    target_coverage = _witness_coverage(target["owners"])
    if report["coverage"] != source_coverage:
        raise ContractError("report coverage differs from recomputed source witness")
    code_hashes = {
        source["producer_code_sha256"], target["producer_code_sha256"], report["producer_code_sha256"]
    }
    binary_hashes = {
        source["producer_binary_sha256"], target["producer_binary_sha256"], report["producer_binary_sha256"]
    }
    if len(code_hashes) != 3 or len(binary_hashes) != 3:
        raise ContractError("comparator requires distinct source-reader, target-reader and comparator builds")
    if report["status"] == "PASS":
        if not source_catalog or not source["owners"] or not target["owners"]:
            raise ContractError("comparator PASS requires nonempty source and target witness evidence")
        source_row_count = sum(len(rows) for item in source["owners"] for rows in item["family_rows"].values())
        target_row_count = sum(len(rows) for item in target["owners"] for rows in item["family_rows"].values())
        if source_row_count == 0 or target_row_count == 0:
            raise ContractError("comparator PASS requires nonempty semantic expected rows on both witnesses")
        if source_coverage != target_coverage:
            raise ContractError("comparator PASS source/target witness coverage differs")
        if source_hash != target_hash:
            raise ContractError("comparator PASS source and target semantic corpus digests differ")
        if report["diagnostics"]["total_count"] != "0" or report["diagnostics"]["sample"]:
            raise ContractError("comparator PASS requires zero discrepancy diagnostics")
    required_parents = {source_hash, target_hash, custody_hash, source_binding_hash, target_binding_hash}
    if not required_parents.issubset(set(report["parent_artifact_hashes"])):
        raise ContractError("report parent hashes omit a source/target witness binding or custody digest")
    return source_coverage, required_parents


def validate_artifact_report(report: Mapping[str, Any], root: Path, evidence: Mapping[str, Any] | None = None) -> None:
    try:
        import jsonschema
        from importlib.metadata import version
    except ImportError as exc:
        raise ContractError("artifact report validation requires the pinned jsonschema test dependency") from exc
    if version("jsonschema") != "4.26.0":
        raise ContractError("artifact report validation requires jsonschema==4.26.0")
    schema = json.loads((root / "schemas" / "artifact-report-v1.schema.json").read_text(encoding="utf-8"))
    validator = jsonschema.Draft202012Validator(schema)
    validator.check_schema(schema)
    try:
        validator.validate(dict(report))
    except jsonschema.ValidationError as exc:
        raise ContractError(f"artifact report schema violation: {exc.message}") from exc
    if dict(report["registry_hashes"]) != registry_hashes(root):
        raise ContractError("artifact report registry hashes do not match the reviewed bundle")
    allowed_scopes = {
        "source_reader": {"authentic_source", "synthetic_fixture"},
        "converter": {"synthetic_fixture", "target_candidate"},
        "target_reader": {"synthetic_fixture", "target_candidate"},
        "comparator": {"synthetic_fixture", "target_candidate"},
        "staging": {"synthetic_fixture", "target_candidate"},
        "benchmark": {"synthetic_fixture", "target_candidate", "benchmark"},
    }
    if report["evidence_scope"] not in allowed_scopes[report["producer_role"]]:
        raise ContractError("producer_role cannot claim this evidence_scope")
    _validate_diagnostics(report["diagnostics"])
    bindings = report["source_owner_bindings"]
    binding_keys = []
    for index, binding in enumerate(bindings):
        _exact(binding, {"owner_uuid", "owner_kind", "scope", "source_file_sha256"}, f"report source owner binding {index}")
        binding_keys.append(owner_key({key: binding[key] for key in ("owner_uuid", "owner_kind", "scope")}))
        _hash(binding["source_file_sha256"], f"report source owner binding {index} source hash")
    if binding_keys != sorted(set(binding_keys)):
        raise ContractError("report source owner bindings must be unique and ordered by canonical full owner key")
    if report["source_hashes"] != [binding["source_file_sha256"] for binding in bindings]:
        raise ContractError("report source_hashes differ from ordered per-owner source bindings")
    if decimal_u64(report["coverage"]["owner_count"], "report owner_count") != len(bindings):
        raise ContractError("report owner_count differs from per-owner source bindings")
    if report["status"] == "PASS" and evidence is None:
        raise ContractError("PASS report requires recomputable owner/custody evidence")
    if report["status"] == "PASS" and report["producer_role"] == "comparator":
        if report["diagnostics"]["total_count"] != "0" or report["diagnostics"]["sample"]:
            raise ContractError("comparator PASS requires zero discrepancy diagnostics")
        if not isinstance(evidence, Mapping) or set(evidence) != {"source_witness", "target_witness", "custody"}:
            raise ContractError("comparator PASS requires separate source and target witness evidence")
        _validate_comparator_witness_bundle(evidence, report, root)
    if report["status"] == "PASS":
        if not bindings:
            raise ContractError("PASS report requires a nonempty owner witness catalog")
        if report["coverage"]["completeness"] != "exhaustive" or report["exit_status"] != "0":
            raise ContractError("PASS report requires exhaustive coverage and zero exit status")
    if evidence is not None and not (report["status"] == "PASS" and report["producer_role"] == "comparator"):
        if not isinstance(evidence, Mapping):
            raise ContractError("report evidence bundle must be an object")
        full_witness_bundle = isinstance(evidence, Mapping) and set(evidence) == {"source_witness", "target_witness", "custody"}
        if full_witness_bundle:
            _validate_comparator_witness_bundle(evidence, report, root)
            return
        # The source-only form remains useful for incomplete/source-admission reports.
        _exact(evidence, {"expected_owner_catalog", "owners", "custody"}, "report evidence bundle")
        groups, records, carriers = load_registries(root)
        domains = domain_ids_from_registry(records)
        _, semantic_hash = corpus_digest(evidence["owners"], evidence["expected_owner_catalog"], groups, records, domains, carriers)
        _, custody_hash = custody_digest(evidence["custody"], records)
        catalog = _catalog_bindings(evidence["expected_owner_catalog"])
        custody_files = sorted(evidence["custody"]["owner_files"], key=lambda row: owner_key({key: row[key] for key in ("owner_uuid", "owner_kind", "scope")}))
        custody_bindings = [{"owner_uuid": item["owner_uuid"], "owner_kind": item["owner_kind"], "scope": item["scope"], "source_file_sha256": item["source_file_sha256"]} for item in custody_files]
        if catalog != custody_bindings or bindings != catalog:
            raise ContractError("report, source owner catalog and custody owner-file bindings differ")
        expected_coverage = _witness_coverage(evidence["owners"])
        if dict(report["coverage"]) != expected_coverage:
            raise ContractError("report coverage counts/completeness differ from recomputed evidence")
        if not {semantic_hash, custody_hash}.issubset(set(report["parent_artifact_hashes"])):
            raise ContractError("report parent hashes omit recomputed semantic or custody evidence")
    if report["evidence_scope"] == "authentic_source":
        groups, records, carriers = load_registries(root)
        validate_authentic_admission(groups, records, carriers)

def domain_ids_from_registry(records: Mapping[str, Any]) -> set[str]:
    return set(records["domain_ids"])


def main() -> int:
    import argparse
    parser = argparse.ArgumentParser(description="PG-C01 evidence-contract validation helpers")
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parent)
    parser.add_argument("--authentic", action="store_true")
    parser.add_argument("--print-bundle-sha256", action="store_true")
    args = parser.parse_args()
    groups, records, carriers = load_registries(args.root)
    if args.authentic:
        validate_authentic_admission(groups, records, carriers)
    if args.print_bundle_sha256:
        print(bundle_digest(args.root)[1])
    return 0


if __name__ == "__main__":
    raise SystemExit(main())