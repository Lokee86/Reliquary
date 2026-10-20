#!/usr/bin/env python3
"""Read-only field and byte inventory for Graph records in pinned V1.0 owners."""

from __future__ import annotations

import argparse
import hashlib
import json
import struct
from collections import Counter
from pathlib import Path
from typing import Any

CONTAINER_MAGIC = b"CVA\0\r\n\x1a\n"
GRAPH_MAGICS = {
    b"CVAGFMT1": "format_marker",
    b"CVAGNODE": "memory_node_mapping_v1",
    b"CVAGNOD2": "semantic_node_mapping_v2",
    b"CVAGVER1": "version_link",
    b"CVAGMUT1": "memory_mutation_v1",
    b"CVAGMUT2": "semantic_mutation_v2",
    b"CVAGBAT1": "memory_batch_v1",
    b"CVAGBAT2": "semantic_batch_v2",
    b"CVAGBL01": "rewrite_baseline",
}
NODE_KIND_CODES = {1: "memory", 2: "entity", 3: "observation"}
RELATION_CODES = {
    1: "topical", 2: "factual", 3: "causal", 4: "recurrent",
    5: "references", 6: "duplicate_of", 7: "supersedes", 8: "structural_parent",
    100: "entity_association", 101: "principal_association",
}
ORIGIN_CODES = {1: "dream", 2: "user", 3: "perception"}
ORDINAL_WIDTHS = (1, 2, 4, 8)


class MeasurementError(ValueError):
    pass


def u16(data: bytes, at: int) -> int:
    return struct.unpack_from("<H", data, at)[0]


def u32(data: bytes, at: int) -> int:
    return struct.unpack_from("<I", data, at)[0]


def u64(data: bytes, at: int) -> int:
    return struct.unpack_from("<Q", data, at)[0]


def recognized_magic(payload: bytes) -> bytes | None:
    magic = payload[:8]
    if magic in GRAPH_MAGICS:
        return magic
    # Every routed Graph family has an explicit version suffix. A future or
    # damaged version must not vanish from this inventory as an unknown chunk.
    if any(magic.startswith(prefix) for prefix in (b"CVAGMUT", b"CVAGBAT", b"CVAGVER", b"CVAGNOD", b"CVAGFMT", b"CVAGBL")):
        raise MeasurementError(f"unsupported Graph record version: {magic!r}")
    return None


def parse_node_ref(payload: bytes, offset: int) -> tuple[tuple[int, bytes], dict[str, Any]]:
    if offset + 33 > len(payload):
        raise MeasurementError("truncated semantic Graph node reference")
    code = payload[offset]
    if code not in NODE_KIND_CODES:
        raise MeasurementError(f"unknown Graph semantic node kind {code}")
    node_id = payload[offset + 1 : offset + 33]
    return (code, node_id), {"kind": NODE_KIND_CODES[code], "id": node_id.hex()}


def parse_mutation_body(payload: bytes, offset: int, typed: bool, has_origin: bool) -> tuple[dict[str, Any], list[tuple[int, bytes]]]:
    if typed:
        source_key, source = parse_node_ref(payload, offset)
        target_key, target = parse_node_ref(payload, offset + 33)
        kind_at = offset + 66
        active_at = kind_at + 2
        origin_at = active_at + 1
        semantic_bytes = 66 + 2 + 1 + int(has_origin)
        if origin_at + int(has_origin) > len(payload):
            raise MeasurementError("truncated typed Graph mutation")
    else:
        if offset + 67 + int(has_origin) > len(payload):
            raise MeasurementError("truncated Memory Graph mutation")
        source_id, target_id = payload[offset : offset + 32], payload[offset + 32 : offset + 64]
        source_key, target_key = (1, source_id), (1, target_id)
        source = {"kind": "memory", "id": source_id.hex()}
        target = {"kind": "memory", "id": target_id.hex()}
        kind_at = offset + 64
        active_at = kind_at + 2
        origin_at = active_at + 1
        semantic_bytes = 64 + 2 + 1 + int(has_origin)

    code = u16(payload, kind_at)
    if code not in RELATION_CODES:
        raise MeasurementError(f"unknown Graph relation kind {code}")
    active = payload[active_at]
    if active not in (0, 1):
        raise MeasurementError(f"invalid Graph active flag {active}")
    if has_origin:
        origin_code = payload[origin_at]
        if origin_code not in ORIGIN_CODES:
            raise MeasurementError(f"unknown Graph relation origin {origin_code}")
        origin = ORIGIN_CODES[origin_code]
    else:
        origin = "dream (implicit in V1 no-origin encoding)"
    return ({
        "source": source,
        "target": target,
        "relation_kind": RELATION_CODES[code],
        "active": bool(active),
        "origin": origin,
        "semantic_field_bytes": semantic_bytes,
        "framing_bytes": 8 if typed else 8,
    }, [source_key, target_key])


def parse_graph_payload(payload: bytes) -> dict[str, Any] | None:
    if len(payload) < 8:
        return None
    magic = recognized_magic(payload)
    if magic is None:
        return None
    kind = GRAPH_MAGICS[magic]
    result: dict[str, Any] = {
        "kind": kind,
        "magic": magic.decode("ascii"),
        "payload_bytes": len(payload),
        "framed_bytes": len(payload) + 8,
        "payload_sha256": hashlib.sha256(payload).hexdigest(),
    }
    if kind == "format_marker":
        if len(payload) != 12 or u32(payload, 8) != 1:
            raise MeasurementError("invalid Graph format marker or schema")
        result.update(schema_version=1, framing_bytes=8, semantic_field_bytes=4)
    elif kind in ("memory_node_mapping_v1", "semantic_node_mapping_v2"):
        expected = 44 if kind == "memory_node_mapping_v1" else 45
        if len(payload) != expected:
            raise MeasurementError(f"invalid {kind} length")
        if kind == "memory_node_mapping_v1":
            node = {"kind": "memory", "id": payload[8:40].hex()}
            node_id = u32(payload, 40)
            semantic_ref_bytes = 32
        else:
            code = payload[8]
            if code not in NODE_KIND_CODES:
                raise MeasurementError(f"unknown Graph node kind {code}")
            node = {"kind": NODE_KIND_CODES[code], "id": payload[9:41].hex()}
            node_id = u32(payload, 41)
            semantic_ref_bytes = 33
        result.update(node=node, internal_node_ordinal=node_id,
                      framing_bytes=8, semantic_id_bytes=semantic_ref_bytes,
                      derived_internal_ordinal_bytes=4)
    elif kind == "version_link":
        if len(payload) != 40:
            raise MeasurementError("invalid Graph version/link length")
        result.update(global_version=u64(payload, 8), graph_version=u64(payload, 16),
                      object_ref_offset=u64(payload, 24), object_ref_length=u64(payload, 32),
                      framing_bytes=8 + 16, semantic_clock_bytes=16,
                      reference_bytes=16)
    elif kind in ("memory_mutation_v1", "semantic_mutation_v2"):
        if kind == "memory_mutation_v1":
            if len(payload) not in (75, 76):
                raise MeasurementError("invalid V1 Graph mutation length")
            parsed, endpoints = parse_mutation_body(payload, 8, False, len(payload) == 76)
        else:
            if len(payload) != 78:
                raise MeasurementError("invalid V2 Graph mutation length")
            parsed, endpoints = parse_mutation_body(payload, 8, True, True)
        result.update(mutations=[parsed], endpoint_keys=endpoints,
                      framing_bytes=8, semantic_field_bytes=parsed["semantic_field_bytes"])
    elif kind in ("memory_batch_v1", "semantic_batch_v2"):
        if len(payload) < 12:
            raise MeasurementError("truncated Graph batch header")
        count = u32(payload, 8)
        if count == 0:
            raise MeasurementError("empty Graph batch")
        stride = ((68 if len(payload) == 12 + count * 68 else
                   67 if len(payload) == 12 + count * 67 else None)
                  if kind == "memory_batch_v1" else
                  70 if kind == "semantic_batch_v2" and len(payload) == 12 + count * 70 else None)
        if stride is None:
            raise MeasurementError("Graph batch count/length mismatch")
        typed = kind == "semantic_batch_v2"
        has_origin = stride in (68, 70)
        mutations = []
        endpoints: list[tuple[int, bytes]] = []
        for index in range(count):
            parsed, refs = parse_mutation_body(payload, 12 + index * stride, typed, has_origin)
            mutations.append(parsed)
            endpoints.extend(refs)
        result.update(mutations=mutations, endpoint_keys=endpoints, count=count,
                      framing_bytes=12, semantic_field_bytes=sum(m["semantic_field_bytes"] for m in mutations))
    elif kind == "rewrite_baseline":
        if len(payload) < 108:
            raise MeasurementError("truncated Graph rewrite baseline")
        count = u32(payload, 72)
        if 108 + count * 32 != len(payload):
            raise MeasurementError("Graph rewrite baseline count/length mismatch")
        if hashlib.sha256(payload[:-32]).digest() != payload[-32:]:
            raise MeasurementError("Graph rewrite baseline digest mismatch")
        if payload[8:24] == b"\0" * 16 or payload[24:40] == b"\0" * 16 or payload[40:72] == b"\0" * 32:
            raise MeasurementError("Graph rewrite baseline has an empty owner, generation, or policy digest")
        ids = [payload[76 + index * 32 : 108 + index * 32] for index in range(count)]
        if len(set(ids)) != count:
            raise MeasurementError("duplicate Memory in Graph rewrite baseline")
        result.update(owner_uuid=payload[8:24].hex(), generation=payload[24:40].hex(),
                      policy_digest=payload[40:72].hex(), memory_count=count,
                      semantic_field_bytes=64 + count * 32,
                      framing_bytes=8 + 4 + 32, integrity_digest_bytes=32,
                      ordered_memory_ids=[item.hex() for item in ids])
    return result


def walk_container(data: bytes) -> tuple[int, int, int, list[dict[str, Any]]]:
    if len(data) < 16 or data[:8] != CONTAINER_MAGIC:
        raise MeasurementError("invalid or truncated Container header")
    major, minor = u16(data, 8), u16(data, 10)
    header_len = u32(data, 12)
    if (major, minor) != (1, 0):
        raise MeasurementError(f"expected pinned V1.0 fixture, found V{major}.{minor}")
    if header_len not in (16, 24, 40) or header_len > len(data):
        raise MeasurementError(f"unsupported Container header length {header_len}")
    if header_len >= 24:
        if data[16] not in (1, 2) or data[17] not in (0, 1, 2, 3) or data[18:24] != b"\0" * 6:
            raise MeasurementError("invalid typed Container identity")
        if data[16] == 2 and data[17] != 0:
            raise MeasurementError("invalid Phylactery scope")
    records: list[dict[str, Any]] = []
    cursor = header_len
    while cursor < len(data):
        if cursor + 8 > len(data):
            raise MeasurementError(f"truncated chunk length at byte {cursor}")
        size = u64(data, cursor)
        start, end = cursor + 8, cursor + 8 + size
        if end > len(data):
            raise MeasurementError(f"truncated chunk at byte {cursor}")
        payload = data[start:end]
        record = parse_graph_payload(payload)
        if record is not None:
            record["chunk_offset"] = cursor
            record["chunk_payload_offset"] = start
            records.append(record)
        cursor = end
    return header_len, major, len(records), records


def ordinal_width(unique_count: int) -> int:
    for width in ORDINAL_WIDTHS:
        if unique_count <= (1 << (8 * width)):
            return width
    raise MeasurementError("Graph endpoint dictionary exceeds u64 ordinal capacity")


def dictionary_projection(records: list[dict[str, Any]], width: int | None = None) -> dict[str, Any]:
    endpoints = [key for record in records for key in record.get("endpoint_keys", [])]
    distinct = set(endpoints)
    chosen = ordinal_width(len(distinct)) if width is None else width
    if chosen not in ORDINAL_WIDTHS:
        raise MeasurementError(f"unsupported ordinal width {chosen}")
    current_inline = sum(
        (33 if record["kind"] in ("semantic_mutation_v2", "semantic_batch_v2") else 32)
        for record in records
        for _ in record.get("endpoint_keys", [])
    )
    feasible = len(distinct) <= (1 << (8 * chosen))
    # Treat every entry as a typed (kind + 32-byte ID) reference, including
    # Memory, to avoid relying on external kind context. Include the full outer
    # Container frame and the dictionary's own magic/count bytes.
    dictionary_entry_bytes = 33 * len(distinct)
    dictionary_header_bytes = 8 + 8 + 4 if distinct else 0
    ordinal_bytes = len(endpoints) * chosen if feasible else None
    projected = dictionary_header_bytes + dictionary_entry_bytes + ordinal_bytes if feasible else None
    savings = current_inline - projected if feasible else None
    return {
        "endpoint_occurrences": len(endpoints),
        "distinct_owner_local_endpoints": len(distinct),
        "chosen_ordinal_width_bytes": chosen,
        "ordinal_width_capacity": 1 << (8 * chosen),
        "ordinal_width_feasible": feasible,
        "current_inline_endpoint_bytes": current_inline,
        "projected_dictionary_entry_bytes": dictionary_entry_bytes if feasible else None,
        "projected_dictionary_header_bytes": dictionary_header_bytes if feasible else None,
        "projected_dictionary_container_frame_bytes": 8 if distinct else 0,
        "projected_dictionary_magic_bytes": 8 if distinct else 0,
        "projected_dictionary_count_bytes": 4 if distinct else 0,
        "projected_typed_entry_kind_tag_bytes": len(distinct) if feasible else None,
        "projected_dictionary_id_bytes": 32 * len(distinct) if feasible else None,
        "projected_ordinal_reference_bytes": ordinal_bytes,
        "projected_total_bytes": projected,
        "projected_net_savings_bytes": savings,
        "break_even": None if not feasible else current_inline > projected,
        "model": "Hypothetical owner-local endpoint dictionary; one framed record with an 8-byte Container length, 8-byte magic, 4-byte count, and one kind-tag + 32-byte ID per unique endpoint (including Memory). Each occurrence uses a fixed-width zero-based ordinal. Existing node mapping frames remain unchanged; reuse is not assumed.",
    }


def summarize_file(path: Path, fixture: str, expected_bytes: int, expected_sha: str) -> dict[str, Any]:
    data = path.read_bytes()
    actual_sha = hashlib.sha256(data).hexdigest()
    if len(data) != expected_bytes or actual_sha != expected_sha:
        raise MeasurementError(f"source identity mismatch for {fixture}: bytes={len(data)} sha256={actual_sha}")
    header, major, graph_count, records = walk_container(data)
    after_sha = hashlib.sha256(path.read_bytes()).hexdigest()
    if after_sha != actual_sha:
        raise MeasurementError(f"fixture changed during measurement: {fixture}")
    counts = Counter(record["kind"] for record in records)
    by_type: dict[str, dict[str, int]] = {}
    for kind in sorted(GRAPH_MAGICS.values()):
        group = [record for record in records if record["kind"] == kind]
        if not group:
            continue
        by_type[kind] = {
            "record_count": len(group),
            "payload_bytes": sum(record["payload_bytes"] for record in group),
            "container_framed_bytes": sum(record["framed_bytes"] for record in group),
            "semantic_field_bytes": sum(record.get("semantic_field_bytes", 0) for record in group),
            "codec_framing_bytes": sum(record.get("framing_bytes", 0) for record in group),
            "derived_internal_ordinal_bytes": sum(record.get("derived_internal_ordinal_bytes", 0) for record in group),
            "integrity_digest_bytes": sum(record.get("integrity_digest_bytes", 0) for record in group),
        }
    return {
        "fixture": fixture,
        "source_path": str(path),
        "source_bytes": len(data),
        "source_sha256_before_measurement": actual_sha,
        "source_sha256_after_measurement": after_sha,
        "source_unchanged": True,
        "container_version": f"{major}.0",
        "container_header_bytes": header,
        "container_chunk_framing_bytes_per_record": 8,
        "graph_record_count": graph_count,
        "graph_record_counts_by_family": dict(sorted(counts.items())),
        "graph_payload_bytes_by_family": by_type,
        "graph_payload_bytes_total": sum(record["payload_bytes"] for record in records),
        "graph_container_framed_bytes_total": sum(record["framed_bytes"] for record in records),
        "endpoint_dictionary_projection": {
            "chosen_width": dictionary_projection(records),
            "width_sensitivity": {str(width): dictionary_projection(records, width) for width in ORDINAL_WIDTHS},
        },
    }


def main() -> None:
    repo = Path(__file__).resolve().parents[1]
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", type=Path, default=repo / "benchmarks" / "development-format-interruption-inventory-2026-10-19.json")
    parser.add_argument("--fixture-root", type=Path)
    parser.add_argument("--output", type=Path, default=repo / "benchmarks" / "graph-history-byte-inventory-2026-10-20.json")
    args = parser.parse_args()
    manifest = json.loads(args.manifest.read_text(encoding="utf-8"))
    fixture_root = args.fixture_root or repo.parent / "reliquary-fixtures"
    results = []
    for item in manifest["retained_files"]:
        relative = Path(item["fixture"].replace("\\", "/"))
        source = fixture_root / "local" / "authoritative" / relative.parent.name / relative.name
        results.append(summarize_file(source, item["fixture"], item["bytes"], item["sha256"]))
    family_counts: Counter[str] = Counter()
    family_payload: Counter[str] = Counter()
    family_framed: Counter[str] = Counter()
    for result in results:
        for family, values in result["graph_payload_bytes_by_family"].items():
            family_counts[family] += values["record_count"]
            family_payload[family] += values["payload_bytes"]
            family_framed[family] += values["container_framed_bytes"]
    output = {
        "schema_version": 1,
        "measurement_date": "2026-10-20",
        "plan_step": 139,
        "decision": "not_made; this artifact inventories bytes and break-even projections only",
        "method": {
            "container_framing": "Read-only V1.0 parser; each chunk is u64 little-endian payload length followed by payload, so physical chunk framing is eight bytes per record.",
            "graph_families": "Exact current magic definitions from src/semantic_graph/graph_codec.rs, graph_codec_mutation.rs, graph_rewrite_baseline.rs and Graph open routing; unknown version suffixes in known families fail closed.",
            "semantic_reference_widths": "Memory-only mutation endpoints are 32-byte IDs; typed semantic endpoints are a 1-byte kind tag plus 32-byte ID (33 bytes). Graph version ObjectRef is 16 bytes (offset+length), not a semantic endpoint.",
            "dictionary_projection": "Hypothetical owner-local unique endpoint dictionary and fixed-width ordinals. Reports widths 1/2/4/8 and automatic smallest width. Includes dictionary header and entry overhead; leaves existing node mapping frames unchanged. This is not a format decision.",
        },
        "fixture_root": str(fixture_root),
        "authoritative_sources_unchanged": all(result["source_unchanged"] for result in results),
        "fixtures": results,
        "aggregate": {
            "fixture_count": len(results),
            "graph_record_counts_by_family": dict(sorted(family_counts.items())),
            "graph_payload_bytes_by_family": dict(sorted(family_payload.items())),
            "graph_container_framed_bytes_by_family": dict(sorted(family_framed.items())),
            "graph_payload_bytes_total": sum(family_payload.values()),
            "graph_container_framed_bytes_total": sum(family_framed.values()),
            "dictionary_projected_net_savings_by_width": {
                str(width): sum(
                    item["endpoint_dictionary_projection"]["width_sensitivity"][str(width)]["projected_net_savings_bytes"]
                    for item in results
                    if item["endpoint_dictionary_projection"]["width_sensitivity"][str(width)]["ordinal_width_feasible"]
                )
                for width in ORDINAL_WIDTHS
            },
            "dictionary_projection_excluded_fixture_counts_by_width": {
                str(width): sum(
                    not item["endpoint_dictionary_projection"]["width_sensitivity"][str(width)]["ordinal_width_feasible"]
                    for item in results
                )
                for width in ORDINAL_WIDTHS
            },
        },
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(output, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"output": str(args.output), "aggregate": output["aggregate"]}, indent=2))


if __name__ == "__main__":
    main()
