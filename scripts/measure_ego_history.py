#!/usr/bin/env python3
"""Read-only inventory of Ego carriers and exact repeated text in pinned fixtures."""

from __future__ import annotations

import argparse
import hashlib
import json
import struct
from collections import Counter, defaultdict
from pathlib import Path
from typing import Any


CONTAINER_MAGIC = b"CVA\0\r\n\x1a\n"
EGO_MAGICS = {
    b"CVAEIDN1": "legacy_identity",
    b"CVAEIDN2": "identity",
    b"CVAEIDA1": "identity_activation",
    b"CVAEPER1": "personality",
    b"CVAEANC1": "anchor",
    b"CVAESYN1": "synthesis",
}
REFERENCE_ID_WIDTHS = (4, 8, 16)
REFERENCE_TAG_BYTES = 1


class MeasurementError(ValueError):
    pass


def u16(data: bytes, at: int) -> int:
    return struct.unpack_from("<H", data, at)[0]


def u32(data: bytes, at: int) -> int:
    return struct.unpack_from("<I", data, at)[0]


def u64(data: bytes, at: int) -> int:
    return struct.unpack_from("<Q", data, at)[0]


def read_string(payload: bytes, cursor: int) -> tuple[str, int, int]:
    if cursor + 4 > len(payload):
        raise MeasurementError("truncated Ego string length")
    size = u32(payload, cursor)
    start = cursor + 4
    end = start + size
    if end > len(payload):
        raise MeasurementError("truncated Ego string")
    raw = payload[start:end]
    try:
        value = raw.decode("utf-8")
    except UnicodeDecodeError as error:
        raise MeasurementError("invalid Ego UTF-8") from error
    return value, len(raw), end


def parse_ego_payload(payload: bytes) -> dict[str, Any] | None:
    if len(payload) < 8 or payload[:8] not in EGO_MAGICS:
        return None
    kind = EGO_MAGICS[payload[:8]]
    cursor = 8
    result: dict[str, Any] = {"kind": kind, "text_fields": []}

    def need(size: int, label: str) -> None:
        if cursor + size > len(payload):
            raise MeasurementError(f"truncated {kind} {label}")

    if kind == "legacy_identity":
        need(16, "coordinates")
        result["ego_version"] = u64(payload, cursor)
        result["revision"] = u64(payload, cursor + 8)
        cursor += 16
        value, size, cursor = read_string(payload, cursor)
        if not value.strip():
            raise MeasurementError("empty legacy Identity text rejected by Ego codec")
        result["text_fields"].append({"field": "text", "text": value, "bytes": size})
    elif kind == "identity":
        need(34, "identity and flags")
        result["ego_version"] = u64(payload, cursor)
        result["id"] = payload[cursor + 8 : cursor + 24].hex()
        result["revision"] = u64(payload, cursor + 24)
        deleted, activate = payload[cursor + 32 : cursor + 34]
        if deleted not in (0, 1) or activate not in (0, 1):
            raise MeasurementError("invalid Identity boolean flags")
        result["deleted"] = bool(deleted)
        result["activate"] = bool(activate)
        cursor += 34
        name, name_size, cursor = read_string(payload, cursor)
        text, text_size, cursor = read_string(payload, cursor)
        if not deleted and (not name.strip() or not text.strip()):
            raise MeasurementError("empty live Identity text rejected by Ego codec")
        result["text_fields"].extend(
            [
                {"field": "name", "text": name, "bytes": name_size},
                {"field": "text", "text": text, "bytes": text_size},
            ]
        )
        if deleted and (activate or name or text):
            raise MeasurementError("deleted Identity is not canonical")
    elif kind == "identity_activation":
        need(24, "coordinates")
        result["ego_version"] = u64(payload, cursor)
        result["id"] = payload[cursor + 8 : cursor + 24].hex()
        cursor += 24
    elif kind in ("personality", "synthesis"):
        need(24, "coordinates")
        result["ego_version"] = u64(payload, cursor)
        result["revision"] = u64(payload, cursor + 8)
        result["source_memory_version"] = u64(payload, cursor + 16)
        cursor += 24
        text, size, cursor = read_string(payload, cursor)
        if not text.strip():
            raise MeasurementError(f"empty {kind} text rejected by Ego codec")
        result["text_fields"].append({"field": "text", "text": text, "bytes": size})
    elif kind == "anchor":
        need(34, "identity and flags")
        result["ego_version"] = u64(payload, cursor)
        result["id"] = payload[cursor + 8 : cursor + 24].hex()
        result["revision"] = u64(payload, cursor + 24)
        deleted, priority = payload[cursor + 32 : cursor + 34]
        if deleted not in (0, 1) or priority not in (1, 2, 3):
            raise MeasurementError("invalid Anchor deletion or priority value")
        result["deleted"] = bool(deleted)
        result["priority"] = {1: "high", 2: "normal", 3: "low"}[priority]
        cursor += 34
        text, size, cursor = read_string(payload, cursor)
        if not deleted and (not text.strip() or len(text) > 2500):
            raise MeasurementError("Anchor text violates the current Ego codec limits")
        result["text_fields"].append({"field": "text", "text": text, "bytes": size})
        if deleted and text:
            raise MeasurementError("deleted Anchor has a nonempty body")
    if cursor != len(payload):
        raise MeasurementError(f"{kind} payload has {len(payload) - cursor} trailing bytes")
    result["payload_sha256"] = hashlib.sha256(payload).hexdigest()
    return result


def walk_container(data: bytes) -> tuple[int, int, int, int | None, list[dict[str, Any]]]:
    if len(data) < 16 or data[:8] != CONTAINER_MAGIC:
        raise MeasurementError("invalid or truncated Container header")
    major, minor = u16(data, 8), u16(data, 10)
    header_len = u32(data, 12)
    if (major, minor) != (1, 0):
        raise MeasurementError(f"expected pinned V1.0 fixture, found V{major}.{minor}")
    if header_len not in (16, 24, 40) or header_len > len(data):
        raise MeasurementError(f"unsupported Container header length {header_len}")
    if header_len > 16:
        if len(data) < 24 or data[18:24] != b"\0" * 6:
            raise MeasurementError("invalid typed Container identity padding")
        if data[16] not in (1, 2) or data[17] not in (0, 1, 2, 3):
            raise MeasurementError("invalid typed Container identity")
        if data[16] == 2 and data[17] != 0:
            raise MeasurementError("invalid Phylactery scope")
        owner_kind: int | None = data[16]
    else:
        owner_kind = None
    chunks: list[dict[str, Any]] = []
    chunk_count = 0
    cursor = header_len
    while cursor < len(data):
        if cursor + 8 > len(data):
            raise MeasurementError(f"truncated chunk length at byte {cursor}")
        payload_len = u64(data, cursor)
        start = cursor + 8
        end = start + payload_len
        if end > len(data):
            raise MeasurementError(f"truncated chunk at byte {cursor}")
        payload = data[start:end]
        chunk_count += 1
        parsed = parse_ego_payload(payload)
        if parsed is not None:
            if owner_kind == 1 and parsed["kind"] in {
                "legacy_identity",
                "identity",
                "identity_activation",
                "personality",
            }:
                raise MeasurementError(
                    f"REL owner contains forbidden {parsed['kind']} Ego carrier"
                )
            parsed["payload_bytes"] = len(payload)
            parsed["framed_bytes"] = len(payload) + 8
            chunks.append(parsed)
        cursor = end
    return header_len, major, chunk_count, owner_kind, chunks


def projected_pool_savings(text_values: list[tuple[str, bytes]], id_width: int) -> dict[str, int]:
    """Exact-value pool model; unique values stay inline.

    A pooled entry is one framed Ego object with an 8-byte magic, 8-byte owner
    sequence, ID, u32 length, and bytes. Each use stores a one-byte
    inline/reference tag and ID.
    Every use stores a one-byte inline/reference tag. Inline text then stores
    a u32 length and UTF-8 bytes; references store the ID.
    """
    counts = Counter(value for _, value in text_values)
    current = sum(4 + len(value) for _, value in text_values)
    projected = current + len(text_values) * REFERENCE_TAG_BYTES
    pooled_values = 0
    pooled_occurrences = 0
    for value, count in counts.items():
        if count < 2:
            continue
        inline_bytes = count * (4 + len(value))
        entry_bytes = 8 + 8 + 8 + id_width + 4 + len(value)
        refs_bytes = count * (REFERENCE_TAG_BYTES + id_width)
        candidate_bytes = entry_bytes + refs_bytes
        tagged_inline_bytes = inline_bytes + count * REFERENCE_TAG_BYTES
        if candidate_bytes < tagged_inline_bytes:
            pooled_values += 1
            pooled_occurrences += count
            projected += candidate_bytes - tagged_inline_bytes
    return {
        "current_inline_text_bytes": current,
        "current_tagged_inline_text_bytes": current + len(text_values) * REFERENCE_TAG_BYTES,
        "projected_text_and_pool_bytes": projected,
        "net_savings_bytes": current - projected,
        "pooled_exact_values": pooled_values,
        "pooled_occurrences": pooled_occurrences,
        "reference_id_width_bytes": id_width,
        "tag_bytes_per_text_use": REFERENCE_TAG_BYTES,
        "inline_length_prefix_bytes_per_use": 4,
        "pool_entry_fixed_overhead_bytes": 8 + 8 + 8 + id_width + 4,
        "pool_entry_framing_bytes": 8,
        "pool_entry_magic_bytes": 8,
        "pool_entry_owner_sequence_bytes": 8,
        "pool_entry_length_prefix_bytes": 4,
    }


def summarize_file(path: Path, fixture: str, expected_bytes: int, expected_sha: str) -> dict[str, Any]:
    with path.open("rb") as source:
        data = source.read()
    actual_sha = hashlib.sha256(data).hexdigest()
    if len(data) != expected_bytes or actual_sha != expected_sha:
        raise MeasurementError(
            f"source identity mismatch for {fixture}: bytes={len(data)} sha256={actual_sha}"
        )
    header_len, major, chunk_count, owner_kind, records = walk_container(data)
    ego_payload_hashes: dict[str, Counter[str]] = defaultdict(Counter)
    type_counts = Counter(record["kind"] for record in records)
    type_payload_bytes = Counter()
    type_framed_bytes = Counter()
    text_by_field: dict[str, list[bytes]] = defaultdict(list)
    text_all: list[tuple[str, bytes]] = []
    for record in records:
        ego_payload_hashes[record["kind"]][record["payload_sha256"]] += 1
        type_payload_bytes[record["kind"]] += record["payload_bytes"]
        type_framed_bytes[record["kind"]] += record["framed_bytes"]
        for field in record["text_fields"]:
            raw = field["text"].encode("utf-8")
            text_by_field[f"{record['kind']}.{field['field']}"].append(raw)
            text_all.append((f"{record['kind']}.{field['field']}", raw))

    def text_stats(values: list[bytes]) -> dict[str, int]:
        counts = Counter(values)
        nonempty_counts = Counter(value for value in values if value)
        return {
            "occurrences": len(values),
            "distinct_exact_values": len(counts),
            "total_utf8_bytes": sum(map(len, values)),
            "repeated_occurrences": sum(count - 1 for count in counts.values()),
            "repeatable_utf8_bytes": sum(len(value) * (count - 1) for value, count in counts.items()),
            "nonempty_repeated_occurrences": sum(count - 1 for count in nonempty_counts.values()),
            "nonempty_repeatable_utf8_bytes": sum(
                len(value) * (count - 1) for value, count in nonempty_counts.items()
            ),
            "empty_occurrences": counts.get(b"", 0),
            "inline_bytes_including_u32_lengths": sum(4 + len(value) for value in values),
        }

    field_stats = {field: text_stats(values) for field, values in sorted(text_by_field.items())}
    owner_value_stats = text_stats([value for _, value in text_all])
    whole_text_payloads: dict[str, Counter[tuple[bytes, ...]]] = defaultdict(Counter)
    for record in records:
        if record["text_fields"]:
            signature = tuple(field["text"].encode("utf-8") for field in record["text_fields"])
            whole_text_payloads[record["kind"]][signature] += 1
    after_sha = hashlib.sha256(path.read_bytes()).hexdigest()
    if after_sha != actual_sha:
        raise MeasurementError(f"fixture changed during measurement: {fixture}")
    return {
        "fixture": fixture,
        "source_path": str(path),
        "source_bytes": len(data),
        "source_sha256": actual_sha,
        "source_sha256_after_measurement": after_sha,
        "container_version": f"{major}.0",
        "header_bytes": header_len,
        "compact_40_byte_header_delta_bytes": 40 - header_len,
        "container_chunk_count": chunk_count,
        "container_owner_kind": {1: "reliquary", 2: "phylactery"}.get(owner_kind, "legacy-untyped"),
        "ego_record_count": len(records),
        "ego_record_counts_by_type": dict(sorted(type_counts.items())),
        "ego_payload_bytes_by_type": dict(sorted(type_payload_bytes.items())),
        "ego_framed_bytes_by_type": dict(sorted(type_framed_bytes.items())),
        "ego_payload_bytes_total": sum(type_payload_bytes.values()),
        "ego_framed_bytes_total": sum(type_framed_bytes.values()),
        "identical_whole_ego_carrier_duplicates_by_type": {
            kind: sum(count - 1 for count in signatures.values())
            for kind, signatures in sorted(ego_payload_hashes.items())
            if any(count > 1 for count in signatures.values())
        },
        "text_fields": field_stats,
        "owner_wide_exact_text_values": owner_value_stats,
        "exact_whole_text_payload_duplicates_by_type": {
            kind: sum(count - 1 for count in signatures.values())
            for kind, signatures in sorted(whole_text_payloads.items())
            if any(count > 1 for count in signatures.values())
        },
        "pool_estimates": {
            "within_field": {
                str(width): {
                    field: projected_pool_savings([(field, value) for value in values], width)
                    for field, values in sorted(text_by_field.items())
                }
                for width in REFERENCE_ID_WIDTHS
            },
            "owner_wide_cross_field": {
                str(width): projected_pool_savings(text_all, width)
                for width in REFERENCE_ID_WIDTHS
            },
        },
    }


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--manifest",
        type=Path,
        default=Path(__file__).resolve().parents[1]
        / "benchmarks"
        / "development-format-interruption-inventory-2026-10-19.json",
    )
    parser.add_argument("--fixture-root", type=Path)
    parser.add_argument(
        "--output",
        type=Path,
        default=Path(__file__).resolve().parents[1]
        / "benchmarks"
        / "ego-payload-repetition-2026-10-20.json",
    )
    args = parser.parse_args()
    manifest = json.loads(args.manifest.read_text(encoding="utf-8"))
    fixture_root = args.fixture_root or Path(__file__).resolve().parents[2] / "reliquary-fixtures"
    results = []
    for fixture in manifest["retained_files"]:
        fixture_relpath = Path(fixture["fixture"].replace("\\", "/"))
        source = fixture_root / "local" / "authoritative" / fixture_relpath.parent.name / fixture_relpath.name
        results.append(
            summarize_file(source, fixture["fixture"], fixture["bytes"], fixture["sha256"])
        )

    aggregate_counts = Counter()
    for result in results:
        for kind, count in result["ego_record_counts_by_type"].items():
            aggregate_counts[kind] += count
    aggregate = {
        "ego_record_count": sum(result["ego_record_count"] for result in results),
        "ego_record_counts_by_type": dict(sorted(aggregate_counts.items())),
        "ego_payload_bytes_total": sum(result["ego_payload_bytes_total"] for result in results),
        "ego_framed_bytes_total": sum(result["ego_framed_bytes_total"] for result in results),
        "owner_local_projected_net_savings_bytes": {
            str(width): sum(
                result["pool_estimates"]["owner_wide_cross_field"][str(width)]["net_savings_bytes"]
                for result in results
            )
            for width in REFERENCE_ID_WIDTHS
        },
        "owner_local_projected_net_savings_after_compact_header_delta_bytes": {
            str(width): sum(
                result["pool_estimates"]["owner_wide_cross_field"][str(width)]["net_savings_bytes"]
                - result["compact_40_byte_header_delta_bytes"]
                for result in results
            )
            for width in REFERENCE_ID_WIDTHS
        },
        "compact_40_byte_header_delta_bytes_total": sum(
            result["compact_40_byte_header_delta_bytes"] for result in results
        ),
    }
    output = {
        "schema_version": 1,
        "measurement_date": "2026-10-20",
        "plan_step": 126,
        "plan_gate": "Measurement-backed yes/no decision recorded; decision is intentionally left to the plan owner.",
        "method": {
            "container_framing": "Read-only parser follows src/container/container.rs: V1.0 header length from bytes 12..16; each chunk is u64 little-endian payload length followed by payload, with 8-byte chunk framing.",
            "ego_admission": "Recognizes only the six exact Ego magic families from src/ego/ego_codec.rs and validates their fixed layout, nonempty live text, anchor limit, flags, priority and trailing-byte exhaustion. It also applies the current REL prohibition on Identity and Personality families. Unknown non-Ego payloads are skipped. This is codec-shape and owner-family validation, not a full EgoStore replay/admission pass.",
            "text_repetition": "Counts exact UTF-8 string values; substring similarity is not credited. Owner-wide estimates pool across text fields within each individual source file; no cross-file pooling is credited.",
            "pool_projection": "Repeated exact values only; unique values remain inline. Every text field carries a 1-byte inline/reference tag. Inline uses additionally pay a 4-byte UTF-8 length and text; reference uses pay the ID. Each hypothetical payload entry costs one 8-byte Container chunk prefix, 8-byte Ego magic, 8-byte owner sequence, reference ID, 4-byte UTF-8 length, and text. The fixed 40-byte identity-header delta from each measured header is reported separately and included in aggregate savings-after-header metrics. Allocator/version semantics, dictionary index, deletion/retention rules, and migration-proof transport remain unmodeled.",
            "references": "Input paths, byte counts and SHA-256 values come from benchmarks/development-format-interruption-inventory-2026-10-19.json; the script requires exact equality before parsing.",
        },
        "scope_limitations": [
            "Only the six retained authoritative REL/PHY fixtures from the pinned manifest are measured.",
            "Fixture bytes and SHA-256 are verified against the inventory manifest before parsing and rehashed after measurement; source files are opened read-only.",
            "This is a physical-carrier inventory, not a successful V1-to-compact migration measurement.",
            "Current fixtures use V1.0 encoding with typed 40-byte identity headers. This report measures the existing physical layout; it does not claim compact V2 migration equivalence.",
            "The parser does not repair or modify truncated tails; it fails closed on framing errors.",
            "The projection models exact repeated text payloads only; it does not model partial/common-substring encoding or choose Ego creation/delta semantics.",
            "No promotion, compatibility exception, or Step 126 yes/no decision is asserted by this artifact.",
        ],
        "sources": results,
        "aggregate": aggregate,
    }
    args.output.write_text(json.dumps(output, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    print(f"wrote {args.output}")
    print(json.dumps(aggregate, indent=2))


if __name__ == "__main__":
    main()
