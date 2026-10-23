#!/usr/bin/env python3
"""Read-only CVAFRS06 field accounting and compact-shape estimates.

Reads the six frozen canonical files named by the Phase 3 baseline receipt.
It validates Container framing and CVAFRS06's checksum/field boundaries, then
estimates two proposed wire shapes. It never opens a Container writer or writes
to any input. Candidate sizes are models, not format selections or acceptance.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import struct
from collections import Counter
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
BASELINE = ROOT / "benchmarks" / "dependency-convergence-phase3-baseline-2026-10-22.json"
MAGIC = b"CVAFRS06"
CONTAINER_MAGIC = b"CVA\0\r\n\x1a\n"
PUBLICATION_MAGIC = b"CVAPUB01"
FIXED_DIGEST_NAMES = (
    "import_id",
    "start_evidence_digest",
    "corpus_digest",
    "manifest_digest",
    "input_digest",
    "historical_policy_digest",
    "semantic_policy_digest",
)
FIXED_CUT_NAMES = (
    "base_cut",
    "settlement_cut",
    "global_version",
    "archive_version",
    "graph_version",
    "memory_graph_version",
)


class AuditError(Exception):
    pass


class Reader:
    def __init__(self, data: bytes, label: str):
        self.data = data
        self.at = 0
        self.label = label

    def take(self, length: int) -> bytes:
        if length < 0 or self.at + length > len(self.data):
            raise AuditError(f"{self.label}: truncated at byte {self.at}, need {length}")
        value = self.data[self.at : self.at + length]
        self.at += length
        return value

    def u8(self) -> int:
        return self.take(1)[0]

    def u16(self) -> int:
        return struct.unpack("<H", self.take(2))[0]

    def i16(self) -> int:
        return struct.unpack("<h", self.take(2))[0]

    def u32(self) -> int:
        return struct.unpack("<I", self.take(4))[0]

    def u64(self) -> int:
        return struct.unpack("<Q", self.take(8))[0]

    def bytes(self) -> bytes:
        return self.take(self.u32())

    def text(self) -> str:
        try:
            return self.bytes().decode("utf-8")
        except UnicodeDecodeError as exc:
            raise AuditError(f"{self.label}: invalid UTF-8 at byte {self.at}") from exc

    def ids(self) -> list[bytes]:
        count = self.u32()
        if count > (len(self.data) - self.at) // 32:
            raise AuditError(f"{self.label}: ID count exceeds remaining bytes")
        values = [self.take(32) for _ in range(count)]
        if values != sorted(set(values)):
            raise AuditError(f"{self.label}: IDs are unordered or duplicated")
        return values


def uleb_size(value: int) -> int:
    if value < 0:
        raise ValueError("ULEB128 cannot encode a negative value")
    size = 1
    while value >= 0x80:
        value >>= 7
        size += 1
    return size


def prefix_string_table(values: list[str]) -> tuple[int, dict[str, int]]:
    """ULEB count, then UTF-8 sorted entries as common-prefix and suffix lengths."""
    encoded = sorted({value.encode("utf-8") for value in values})
    indexes = {value.decode("utf-8"): i for i, value in enumerate(encoded)}
    total = uleb_size(len(encoded))
    previous = b""
    for value in encoded:
        common = 0
        limit = min(len(previous), len(value))
        while common < limit and previous[common] == value[common]:
            common += 1
        suffix = value[common:]
        total += uleb_size(common) + uleb_size(len(suffix)) + len(suffix)
        previous = value
    return total, indexes


def delta_set_size(values: tuple[bytes, ...], id_index: dict[bytes, int]) -> int:
    indexes = [id_index[value] for value in values]
    return uleb_size(len(indexes)) + sum(
        uleb_size(indexes[0] + 1 if i == 0 else indexes[i] - indexes[i - 1])
        for i in range(len(indexes))
    )


def parse_settlement(
    payload: bytes,
    path: str,
    frame_offset: int,
    physical_frame_bytes: int,
    envelope_bytes: int,
) -> dict[str, Any]:
    if len(payload) < 8 + 16 + 7 * 32 + 6 * 8 + 1 + 32 or not payload.startswith(MAGIC):
        raise AuditError(f"{path}@{frame_offset}: invalid/short CVAFRS06 payload")
    expected_checksum = hashlib.sha256(payload[:-32]).digest()
    if expected_checksum != payload[-32:]:
        raise AuditError(f"{path}@{frame_offset}: CVAFRS06 checksum mismatch")

    body = payload[:-32]
    r = Reader(body, f"{path}@{frame_offset}")
    field_ranges: dict[str, list[tuple[int, int]]] = {}

    def mark(name: str, fn):
        start = r.at
        value = fn()
        field_ranges.setdefault(name, []).append((start, r.at))
        return value

    mark("magic", lambda: r.take(8))
    owner = mark("owner_uuid", lambda: r.take(16))
    digests = {name: mark(name, lambda: r.take(32)) for name in FIXED_DIGEST_NAMES}
    cuts = {name: mark(name, r.u64) for name in FIXED_CUT_NAMES}
    reserved = mark("reserved", r.u8)
    if reserved != 0:
        raise AuditError(f"{path}@{frame_offset}: unsupported reserved tag {reserved}")

    born = mark("born_memory_ids", r.ids)
    affected = mark("affected_recipient_ids", r.ids)

    def read_sources():
        values = []
        count = r.u32()
        if count > (len(body) - r.at) // 8:
            raise AuditError(f"{path}@{frame_offset}: source-node count exceeds remaining bytes")
        for _ in range(count):
            values.append((r.text(), r.text()))
        if values != sorted(set(values)):
            raise AuditError(f"{path}@{frame_offset}: source nodes unordered or duplicated")
        return values

    sources = mark("source_nodes", read_sources)

    def read_events():
        values = []
        count_start = r.at
        count = r.u32()
        field_ranges.setdefault("accepted_event_count", []).append((count_start, r.at))
        if count > (len(body) - r.at) // 48:
            raise AuditError(f"{path}@{frame_offset}: event count exceeds remaining bytes")
        previous = None
        for _ in range(count):
            event_id = r.text()
            if not event_id or (previous is not None and previous >= event_id):
                raise AuditError(f"{path}@{frame_offset}: event IDs unordered/duplicated")
            previous = event_id
            digest = r.take(32)
            turn = r.u64()
            recipients = r.ids()
            values.append({"id": event_id, "digest": digest, "turn": turn, "recipients": recipients})
        return values

    events = mark("accepted_event_entries", read_events)

    def read_superseded():
        values = []
        count = r.u32()
        if count > (len(body) - r.at) // 4:
            raise AuditError(f"{path}@{frame_offset}: superseded count exceeds remaining bytes")
        for _ in range(count):
            values.append(r.text())
        if values != sorted(set(values)):
            raise AuditError(f"{path}@{frame_offset}: superseded IDs unordered/duplicated")
        return values

    superseded = mark("superseded_ids", read_superseded)
    cohort = mark("cohort_ids", r.ids)

    def read_records():
        count = r.u32()
        values = []
        if count > (len(body) - r.at) // 36:
            raise AuditError(f"{path}@{frame_offset}: record count exceeds remaining bytes")
        previous = None
        for _ in range(count):
            memory_id = r.take(32)
            if previous is not None and previous >= memory_id:
                raise AuditError(f"{path}@{frame_offset}: record IDs unordered/duplicated")
            previous = memory_id
            record_start = r.at
            value_start = r.at
            score = r.i16()
            admitted_tag = r.u8()
            if admitted_tag not in (0, 1):
                raise AuditError(f"{path}@{frame_offset}: invalid admitted-turn tag")
            admitted = r.u64() if admitted_tag else None
            accounted_tag = r.u8()
            if accounted_tag not in (0, 1):
                raise AuditError(f"{path}@{frame_offset}: invalid accounted-turn tag")
            accounted = r.u64() if accounted_tag else None
            if not -100 <= score <= 100 or (admitted is None) != (accounted is None):
                raise AuditError(f"{path}@{frame_offset}: invalid Freshness record")
            if admitted is not None and accounted < admitted:
                raise AuditError(f"{path}@{frame_offset}: record clock regressed")
            field_ranges.setdefault("record_values", []).append((record_start, r.at))
            values.append({
                "id": memory_id,
                "score": score,
                "admitted": admitted,
                "accounted": accounted,
                "value_wire": body[value_start:r.at],
            })
        return values

    record_count_start = r.at
    records = mark("records", read_records)
    field_ranges.setdefault("record_key_count", []).append((record_count_start, record_count_start + 4))
    if r.at != len(body):
        raise AuditError(f"{path}@{frame_offset}: trailing bytes before checksum")

    # Split accepted event entries into actual codec components for exact field accounting.
    event_id_bytes = 0
    event_digest_bytes = 32 * len(events)
    event_turn_bytes = 8 * len(events)
    event_recipient_bytes = 0
    event_recipient_occurrences = 0
    for event in events:
        event_id_bytes += 4 + len(event["id"].encode("utf-8"))
        event_recipient_bytes += 4 + 32 * len(event["recipients"])
        event_recipient_occurrences += len(event["recipients"])
    source_bytes = 4 + sum(8 + len(c.encode("utf-8")) + len(n.encode("utf-8")) for c, n in sources)
    superseded_bytes = 4 + sum(4 + len(v.encode("utf-8")) for v in superseded)

    field_bytes = {
        "magic": 8,
        "owner_uuid": 16,
        "identity_and_proof_digests": 7 * 32,
        "activity_and_owner_cuts": 6 * 8,
        "reserved": 1,
        "born_memory_ids": 4 + 32 * len(born),
        "affected_recipient_ids": 4 + 32 * len(affected),
        "source_nodes": source_bytes,
        "accepted_event_count": 4,
        "accepted_event_ids": event_id_bytes,
        "accepted_event_payload_digests": event_digest_bytes,
        "accepted_event_turns": event_turn_bytes,
        "accepted_event_recipient_sets": event_recipient_bytes,
        "superseded_ids": superseded_bytes,
        "cohort_ids": 4 + 32 * len(cohort),
        "record_count": 4,
        "record_keys": 32 * len(records),
        "record_values": sum(end - start for start, end in field_ranges.get("record_values", [])),
        "checksum": 32,
    }
    if sum(field_bytes.values()) != len(payload):
        raise AuditError(
            f"{path}@{frame_offset}: field accounting {sum(field_bytes.values())} != payload {len(payload)}"
        )

    all_ids = set(born) | set(affected) | set(cohort) | {r["id"] for r in records}
    for event in events:
        all_ids.update(event["recipients"])
    all_strings = set(superseded)
    all_strings.update(event["id"] for event in events)
    for conversation, node in sources:
        all_strings.add(conversation)
        all_strings.add(node)

    recipient_sets = [tuple(event["recipients"]) for event in events]
    record_values = [record["value_wire"] for record in records]

    return {
        "path": path,
        "frame_offset": frame_offset,
        "payload_bytes": len(payload),
        "physical_frame_bytes": physical_frame_bytes,
        "physical_frame_overhead_bytes": physical_frame_bytes - len(payload),
        "publication_envelope_bytes": envelope_bytes,
        "payload_sha256": hashlib.sha256(payload).hexdigest(),
        "owner_uuid": owner.hex(),
        "digests": {name: value.hex() for name, value in digests.items()},
        "cuts": cuts,
        "counts": {
            "born_memories": len(born),
            "affected_recipients": len(affected),
            "source_nodes": len(sources),
            "accepted_events": len(events),
            "event_recipient_occurrences": event_recipient_occurrences,
            "distinct_event_recipients": len(set().union(*(set(e["recipients"]) for e in events))) if events else 0,
            "superseded_events": len(superseded),
            "cohort": len(cohort),
            "freshness_records": len(records),
            "distinct_memory_ids_across_fields": len(all_ids),
            "distinct_text_values_across_fields": len(all_strings),
            "unique_event_recipient_sets": len(set(recipient_sets)),
            "duplicate_event_recipient_set_occurrences": len(recipient_sets) - len(set(recipient_sets)),
            "unique_freshness_record_values": len(set(record_values)),
            "duplicate_freshness_record_value_occurrences": len(record_values) - len(set(record_values)),
            "duplicate_source_node_pairs_within_settlement": len(sources) - len(set(sources)),
        },
        "field_bytes": field_bytes,
        "data": {
            "events": events,
            "born": born,
            "affected": affected,
            "sources": sources,
            "superseded": superseded,
            "cohort": cohort,
            "records": records,
            "record_values": record_values,
            "memory_ids": sorted(all_ids),
            "strings": sorted(all_strings),
        },
    }


def unwrap_publication(payload: bytes, path: str, offset: int) -> tuple[bytes, int]:
    if not payload.startswith(PUBLICATION_MAGIC):
        return payload, 0
    if len(payload) < 17:
        raise AuditError(f"{path}@{offset}: truncated publication envelope")
    tag = payload[16]
    if tag in (0, 2):
        return payload[17:], 17
    if tag == 1 and len(payload) >= 25:
        return payload[25:], 25
    raise AuditError(f"{path}@{offset}: malformed publication envelope clock tag {tag}")


def scan_container(path: Path, fixture: str, expected_sha256: str, expected_bytes: int) -> dict[str, Any]:
    raw = path.read_bytes()
    sha256 = hashlib.sha256(raw).hexdigest()
    if sha256 != expected_sha256:
        raise AuditError(f"{fixture}: SHA-256 mismatch: expected {expected_sha256}, got {sha256}")
    if len(raw) != expected_bytes:
        raise AuditError(f"{fixture}: size mismatch: expected {expected_bytes}, got {len(raw)}")
    if len(raw) < 16 or raw[:8] != CONTAINER_MAGIC:
        raise AuditError(f"{fixture}: invalid Container magic/header")
    major, minor, header_len = struct.unpack("<HHI", raw[8:16])
    if (major, minor) != (2, 0) or header_len not in (16, 40) or len(raw) < header_len:
        raise AuditError(f"{fixture}: unsupported/incomplete Container header")
    if header_len == 40:
        if raw[16] not in (1, 2) or raw[17] not in (0, 1, 2, 3) or any(raw[18:24]):
            raise AuditError(f"{fixture}: invalid identity header")

    settlements = []
    frame_count = 0
    wrapped_counts: Counter[str] = Counter()
    offset = header_len
    while offset < len(raw):
        if len(raw) - offset < 8:
            raise AuditError(f"{fixture}@{offset}: truncated chunk header")
        payload_len = struct.unpack("<Q", raw[offset : offset + 8])[0]
        frame_end = offset + 8 + payload_len
        if payload_len > len(raw) or frame_end > len(raw):
            raise AuditError(f"{fixture}@{offset}: invalid/truncated chunk length {payload_len}")
        payload = raw[offset + 8 : frame_end]
        body, envelope_len = unwrap_publication(payload, fixture, offset)
        if envelope_len:
            wrapped_counts[f"publication_envelope_{envelope_len}"] += 1
        if body.startswith(MAGIC):
            settlements.append(
                parse_settlement(body, fixture, offset, 8 + payload_len, envelope_len)
            )
        frame_count += 1
        offset = frame_end

    return {
        "fixture": fixture,
        "path": str(path.relative_to(ROOT)).replace("\\", "/"),
        "bytes": len(raw),
        "sha256": sha256,
        "header_bytes": header_len,
        "physical_frames": frame_count,
        "publication_envelope_frame_counts": dict(wrapped_counts),
        "settlements": settlements,
    }


def lossless_dictionary_size(settlement: dict[str, Any]) -> dict[str, int]:
    """Model a lossless codec using shared MemoryId/text tables and ULEB indices."""
    data = settlement["data"]
    ids = data["memory_ids"]
    strings = data["strings"]
    id_index = {value: index for index, value in enumerate(ids)}
    text_index = {value: index for index, value in enumerate(strings)}

    fixed = 8 + 16 + 7 * 32 + 6 * 8 + 1
    id_table = uleb_size(len(ids)) + 32 * len(ids)
    text_table = uleb_size(len(strings)) + sum(uleb_size(len(s.encode("utf-8"))) + len(s.encode("utf-8")) for s in strings)
    ids_set = lambda values: uleb_size(len(values)) + sum(uleb_size(id_index[value]) for value in values)
    source_nodes = uleb_size(len(data["sources"])) + sum(
        uleb_size(text_index[c]) + uleb_size(text_index[n]) for c, n in data["sources"]
    )
    accepted_events = uleb_size(len(data["events"])) + sum(
        uleb_size(text_index[e["id"]])
        + 32
        + 8
        + ids_set(e["recipients"])
        for e in data["events"]
    )
    superseded = uleb_size(len(data["superseded"])) + sum(uleb_size(text_index[s]) for s in data["superseded"])
    record_bytes = uleb_size(len(data["records"])) + sum(
        uleb_size(id_index[record["id"]]) + 20 for record in data["records"]
    )
    components = {
        "fixed_metadata": fixed,
        "memory_id_dictionary": id_table,
        "text_dictionary": text_table,
        "born_memory_id_indexes": ids_set(data["born"]),
        "affected_recipient_id_indexes": ids_set(data["affected"]),
        "source_node_text_indexes": source_nodes,
        "accepted_event_full_authority": accepted_events,
        "superseded_event_text_indexes": superseded,
        "cohort_id_indexes": ids_set(data["cohort"]),
        "record_keys_and_values": record_bytes,
        "checksum": 32,
    }
    return {"components": components, "total_bytes": sum(components.values())}


def lossless_prefix_delta_size(settlement: dict[str, Any]) -> dict[str, Any]:
    """Lossless full-authority schema with prefix strings and shared delta ID sets."""
    data = settlement["data"]
    ids = data["memory_ids"]
    id_index = {value: index for index, value in enumerate(ids)}
    strings = data["strings"]
    text_table, text_index = prefix_string_table(strings)

    id_sets = {tuple(data["born"]), tuple(data["affected"]), tuple(data["cohort"]),
               tuple(record["id"] for record in data["records"])}
    id_sets.update(tuple(event["recipients"]) for event in data["events"])
    ordered_sets = sorted(id_sets)
    set_index = {value: index for index, value in enumerate(ordered_sets)}
    set_table_bytes = uleb_size(len(ordered_sets)) + sum(delta_set_size(value, id_index) for value in ordered_sets)

    source_pairs = uleb_size(len(data["sources"])) + sum(
        uleb_size(text_index[conversation]) + uleb_size(text_index[node])
        for conversation, node in data["sources"]
    )
    events = uleb_size(len(data["events"])) + sum(
        uleb_size(text_index[event["id"]]) + 32 + 8 + uleb_size(set_index[tuple(event["recipients"])])
        for event in data["events"]
    )
    superseded = uleb_size(len(data["superseded"])) + sum(
        uleb_size(text_index[value]) for value in data["superseded"]
    )
    record_values = sorted(set(data["record_values"]))
    value_index = {value: index for index, value in enumerate(record_values)}
    record_value_table = uleb_size(len(record_values)) + sum(
        uleb_size(len(value)) + len(value) for value in record_values
    )
    record_refs = uleb_size(len(data["records"])) + sum(
        uleb_size(value_index[record["value_wire"]]) for record in data["records"]
    )
    fixed = 8 + 16 + 7 * 32 + 6 * 8 + 1
    components = {
        "fixed_metadata": fixed,
        "memory_id_dictionary": uleb_size(len(ids)) + 32 * len(ids),
        "prefix_compressed_text_dictionary": text_table,
        "shared_delta_id_set_table": set_table_bytes,
        "born_id_set_reference": uleb_size(set_index[tuple(data["born"])]),
        "affected_id_set_reference": uleb_size(set_index[tuple(data["affected"])]),
        "source_node_pair_text_indexes": source_pairs,
        "accepted_event_full_authority": events,
        "superseded_text_indexes": superseded,
        "cohort_id_set_reference": uleb_size(set_index[tuple(data["cohort"])]),
        "record_key_set_reference": uleb_size(set_index[tuple(record["id"] for record in data["records"])]) ,
        "deduplicated_record_value_table": record_value_table,
        "record_value_indexes": record_refs,
        "checksum": 32,
    }
    return {
        "components": components,
        "total_bytes": sum(components.values()),
        "table_counts": {
            "memory_ids": len(ids), "text_strings": len(text_index), "id_sets": len(ordered_sets),
            "unique_record_values": len(record_values),
        },
        "set_deduplication": {
            "recipient_set_occurrences": len(data["events"]),
            "unique_recipient_sets": len({tuple(event["recipients"]) for event in data["events"]}),
            "unique_sets_shared_across_all_fields": len(ordered_sets),
            "source_node_pairs_are_canonical_unique_within_settlement": True,
        },
    }


def event_spine_postimage_size(settlement: dict[str, Any]) -> dict[str, Any]:
    """Compact authority spine retaining all score postimages, avoiding replay cycles."""
    data = settlement["data"]
    record_key_set = tuple(record["id"] for record in data["records"])
    id_sets = {tuple(data["affected"]), tuple(data["cohort"]), record_key_set}
    ordered_sets = sorted(id_sets)
    needed_ids = sorted(set().union(*(set(value) for value in ordered_sets))) if ordered_sets else []
    id_index = {value: index for index, value in enumerate(needed_ids)}
    set_index = {value: index for index, value in enumerate(ordered_sets)}

    event_strings = [event["id"] for event in data["events"]]
    superseded = set(data["superseded"])
    if not superseded.issubset(set(event_strings)):
        raise AuditError("superseded IDs are not a subset of the retained accepted-event spine")
    text_table, text_index = prefix_string_table(event_strings)
    event_spine = uleb_size(len(data["events"])) + sum(
        uleb_size(text_index[event["id"]]) + 1 for event in data["events"]
    )
    set_table_bytes = uleb_size(len(ordered_sets)) + sum(delta_set_size(values, id_index) for values in ordered_sets)
    record_values = sorted(set(data["record_values"]))
    value_index = {value: index for index, value in enumerate(record_values)}
    record_refs = uleb_size(len(data["records"])) + sum(
        uleb_size(value_index[record["value_wire"]]) for record in data["records"]
    )
    fixed = 8 + 16 + 7 * 32 + 6 * 8 + 1
    components = {
        "fixed_metadata": fixed,
        "retained_memory_id_dictionary": uleb_size(len(needed_ids)) + 32 * len(needed_ids),
        "shared_delta_set_table_for_affected_cohort_record_keys": set_table_bytes,
        "affected_set_reference": uleb_size(set_index[tuple(data["affected"])]),
        "cohort_set_reference": uleb_size(set_index[tuple(data["cohort"])]),
        "record_key_set_reference": uleb_size(set_index[record_key_set]),
        "prefix_compressed_accepted_event_id_table": text_table,
        "event_spine_indexes_and_superseded_bits": event_spine,
        "canonical_event_leaf_commitment": 32,
        "deduplicated_exact_record_postimages": uleb_size(len(record_values)) + sum(
            uleb_size(len(value)) + len(value) for value in record_values
        ),
        "record_value_indexes": record_refs,
        "checksum": 32,
    }
    total = sum(components.values())
    return {
        "components": components,
        "total_bytes": total,
        "saved_bytes_vs_current": settlement["payload_bytes"] - total,
        "saved_percent_vs_current": 100.0 * (settlement["payload_bytes"] - total) / settlement["payload_bytes"],
        "table_counts": {
            "retained_memory_ids": len(needed_ids), "deduplicated_id_sets": len(ordered_sets),
            "accepted_event_ids": len(event_strings), "unique_record_values": len(record_values),
        },
        "reconstructed_from_retained_authority": [
            "source_nodes from the retained Archive ImportReceipt",
            "born Memory IDs from import completion, Episode path, revision-1 Memory publication, and first-mutation proof",
            "event digest, activity turn and recipient effects by exact accepted event ID lookup in retained CVAFRS02",
        ],
        "retained_to_avoid_circular_postimage_replay": [
            "all 20-byte FreshnessRecord postimages, keyed by the exact record-key ID set",
            "affected and cohort ID sets, exact accepted event ID spine, superseded bit per event, and aggregate event-leaf commitment",
        ],
        "proof_obligations": [
            "resolve every accepted event ID uniquely and verify its canonical digest, turn, recipient effects, and ordered aggregate commitment",
            "preserve the exact ID spine so same-activity-turn event selection is not inferred from cuts alone",
            "prove source-node and birth derivation from retained Archive receipt and Memory/Insomnia proofs after prune/reclaim",
            "update settlement completion hashing to the canonical compact payload and retain exact owner/import identity and cuts",
        ],
    }


def event_spine_reconstruction_size(settlement: dict[str, Any]) -> dict[str, Any]:
    """Estimate ID-spine+root shape; scores/effects derive from retained owner facts."""
    data = settlement["data"]
    # Keep exact scope identity, all digests/policy and cuts, the affected and
    # replacement cohorts. Receipt/birth proofs rederive source_nodes/born IDs.
    fixed = 8 + 16 + 7 * 32 + 6 * 8 + 1
    events = uleb_size(len(data["events"])) + sum(
        uleb_size(len(e["id"].encode("utf-8"))) + len(e["id"].encode("utf-8")) + 1
        for e in data["events"]
    )
    events += 32  # one domain-separated commitment over ordered event leaves
    affected = 4 + 32 * len(data["affected"])
    cohort = 4 + 32 * len(data["cohort"])
    checksum = 32
    components = {
        "fixed_metadata": fixed,
        "event_id_spine_and_superseded_tag": events,
        "affected_recipient_ids_retained": affected,
        "replacement_cohort_ids_retained": cohort,
        "checksum": checksum,
    }
    total = sum(components.values())
    current = settlement["payload_bytes"]
    return {
        "components": components,
        "total_bytes": total,
        "saved_bytes_vs_current": current - total,
        "saved_percent_vs_current": (100.0 * (current - total) / current) if current else 0.0,
        "reconstructed_fields": [
            "event payload digest, activity turn and recipient effects (resolve accepted event IDs in CVAFRS02)",
            "born Memory set (Import receipt plus retained Insomnia completion and first Memory publication proof)",
            "source Nodes (Archive ImportReceipt)",
            "full FreshnessRecord postimages (replay retained pre-settlement owner history and exact accepted event effects)",
        ],
        "retained_fields": [
            "owner/import/start identity, corpus/manifest/input/policy digests and owner/activity cuts",
            "accepted event IDs with superseded membership bit and aggregate canonical-leaf commitment",
            "affected-recipient and replacement-cohort Memory IDs",
        ],
        "requires": [
            "fail closed if any referenced accepted event is missing or its recomputed leaf root differs",
            "a replay path that excludes the target settlement to avoid circular postimage reconstruction",
            "retained event history and required base state through prune/repack/rewrite",
            "preserve accepted same-turn attribution by exact event IDs; cuts alone are insufficient",
        ],
    }


def summarize_file(file_result: dict[str, Any]) -> dict[str, Any]:
    settlements = file_result.pop("settlements")
    total_field_bytes: Counter[str] = Counter()
    totals = {
        "current_payload_bytes": 0,
        "current_physical_frame_bytes": 0,
        "lossless_dictionary_bytes": 0,
        "lossless_prefix_delta_deduplicated_bytes": 0,
        "event_spine_reconstruction_bytes": 0,
        "event_spine_retained_postimages_bytes": 0,
    }
    items = []
    for settlement in settlements:
        dictionary = lossless_dictionary_size(settlement)
        optimized = lossless_prefix_delta_size(settlement)
        reconstruction = event_spine_reconstruction_size(settlement)
        postimage = event_spine_postimage_size(settlement)
        for field, amount in settlement["field_bytes"].items():
            total_field_bytes[field] += amount
        totals["current_payload_bytes"] += settlement["payload_bytes"]
        totals["current_physical_frame_bytes"] += settlement["physical_frame_bytes"]
        totals["lossless_dictionary_bytes"] += dictionary["total_bytes"]
        totals["lossless_prefix_delta_deduplicated_bytes"] += optimized["total_bytes"]
        totals["event_spine_reconstruction_bytes"] += reconstruction["total_bytes"]
        totals["event_spine_retained_postimages_bytes"] += postimage["total_bytes"]
        items.append({
            "frame_offset": settlement["frame_offset"],
            "payload_bytes": settlement["payload_bytes"],
            "physical_frame_bytes": settlement["physical_frame_bytes"],
            "physical_frame_overhead_bytes": settlement["physical_frame_overhead_bytes"],
            "publication_envelope_bytes": settlement["publication_envelope_bytes"],
            "payload_sha256": settlement["payload_sha256"],
            "counts": settlement["counts"],
            "field_bytes": settlement["field_bytes"],
            "lossless_dictionary_candidate": dictionary,
            "lossless_prefix_delta_deduplicated_candidate": optimized,
            "event_id_spine_reconstruction_candidate": reconstruction,
            "event_id_spine_retained_postimages_candidate": postimage,
        })
    return {
        **file_result,
        "settlement_count": len(items),
        "settlement_field_bytes": dict(total_field_bytes),
        "settlement_totals": totals,
        "settlements": items,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline", type=Path, default=BASELINE)
    parser.add_argument("--output", type=Path, default=ROOT / ".verification-tmp" / "phase3-s3-field-audit.json")
    args = parser.parse_args()
    baseline = json.loads(args.baseline.read_text(encoding="utf-8"))
    files = []
    for item in baseline["canonical_sources"]:
        candidate = ROOT / item["frozen_copy"].replace("C:\\!bin\\workspace\\Reliquary\\", "")
        # Baseline stores absolute Windows paths; verify the resolved target remains in this repo.
        candidate = Path(str(candidate).replace("\\", "/"))
        if not candidate.is_absolute():
            candidate = ROOT / candidate
        try:
            candidate.resolve().relative_to(ROOT.resolve())
        except ValueError as exc:
            raise AuditError(f"frozen copy escapes repository: {candidate}") from exc
        result = scan_container(candidate, item["fixture"], item["sha256"], item["bytes"])
        files.append(summarize_file(result))

    aggregate_fields: Counter[str] = Counter()
    aggregate_counts: Counter[str] = Counter()
    aggregate_totals = Counter()
    for file_result in files:
        aggregate_fields.update(file_result["settlement_field_bytes"])
        aggregate_totals.update(file_result["settlement_totals"])
        for settlement in file_result["settlements"]:
            aggregate_counts.update(settlement["counts"])

    output = {
        "schema": 2,
        "purpose": "read-only physical CVAFRS06 field audit and modeled candidate sizes; no acceptance claim",
        "baseline": str(args.baseline.relative_to(ROOT)).replace("\\", "/"),
        "baseline_source_commit": baseline.get("source_commit"),
        "baseline_file_count": len(files),
        "container_framing": {
            "header": "CVA magic, v2.0, little-endian header length (16 or 40 bytes)",
            "chunk": "u64 little-endian payload length followed by exact payload bytes",
            "publication": "CVAPUB01 envelope, body offset 17 for clock tag 0/2 or 25 for tag 1",
        },
        "codec_basis": {
            "source": "src/freshness/freshness_semantic_settlement.rs encode/decode; src/freshness_storage.rs put_record",
            "record_wire_bytes": "i16 score; optional admitted and accounted turns (each tag+u64); exact observed bytes are retained per record, including tag-dependent lengths",
            "candidate_size_status": "modeled exact byte counts for the described schemas; no new codec implemented",
        },
        "aggregate": {
            "current_payload_bytes": aggregate_totals["current_payload_bytes"],
            "current_physical_frame_bytes": aggregate_totals["current_physical_frame_bytes"],
            "lossless_dictionary_bytes": aggregate_totals["lossless_dictionary_bytes"],
            "lossless_prefix_delta_deduplicated_bytes": aggregate_totals["lossless_prefix_delta_deduplicated_bytes"],
            "event_spine_reconstruction_bytes": aggregate_totals["event_spine_reconstruction_bytes"],
            "event_spine_retained_postimages_bytes": aggregate_totals["event_spine_retained_postimages_bytes"],
            "field_bytes": dict(aggregate_fields),
            "counts": dict(aggregate_counts),
        },
        "candidate_comparison": {
            "lossless_dictionary": "Full semantic authority retained. Sorted unique MemoryId and UTF-8 text dictionaries replace repeated inline values with ULEB128 indexes; all event digests/turns/effects, cohorts and complete records remain. No semantic reconstruction dependency.",
            "lossless_prefix_delta_deduplicated": "Full semantic authority retained. Adds UTF-8 byte prefix-compressed strings, sorted MemoryId dictionary, a deduplicated shared table of sorted per-field ID sets encoded as delta ULEB indexes, and a deduplicated table of exact variable-length record-value bytes. Event recipient sets are re-used when equal to another field/event set.",
            "event_id_spine_reconstruction": "Aggressive optimistic bound: retains selected event IDs plus a superseded tag and one ordered leaf commitment, but drops FreshnessRecord postimages and requires excluded-settlement replay to reconstruct them. This has a circularity/replay risk and is not the smallest justified no-semantic-loss candidate.",
            "event_id_spine_retained_postimages": "Conditional framed-scope candidate only: preserves all exact FreshnessRecord postimages, affected/cohort sets, accepted event IDs, superseded tags, and a commitment; derives source_nodes and born IDs from retained framed proofs and resolves event details by exact IDs. It avoids circular postimage replay. Not justified for all current supported scopes because an initial/unframed legacy predecessor has no ImportStart/receipt from which exact source_nodes can be derived, yet Freshness validation and scope consumers still use them.",
            "smallest_currently_justified_without_semantic_reconstruction": {
                "candidate": "lossless_prefix_delta_deduplicated",
                "bytes": aggregate_totals["lossless_prefix_delta_deduplicated_bytes"],
                "reason": "The current implementation explicitly supports initial/unframed settlements. Their source_nodes cannot be assumed derivable from an Archive ImportReceipt; preserving all authority fields is the only measured candidate here that is safe across framed and initial scopes without a separately proven initial-source derivation or contract change.",
            },
            "two_stage_hydration_feasibility": {
                "framed_scopes": "Architecturally feasible without codec callbacks or Archive/Insomnia logic in Freshness: Freshness decodes a compact envelope retaining postimages, cuts, sets, event IDs and root; Freshness resolves exact event IDs/leaves using its own journal. After owner stores are finished, History derives source_nodes from ImportStart+Archive ImportReceipt and born IDs from Insomnia completion/Episode/Memory first-mutation proofs, then validates/hydrates before exposing the final settlement projection.",
                "initial_unframed_scopes": "Not proven feasible for the 207,071-byte candidate. `history/epoch_scope_proof.rs` explicitly takes an InitialHistory branch when no epoch exists, requiring base_cut=0 and born_memories=cohort but with no receipt to reconstruct source_nodes. `freshness_import_events.rs::validate_semantic_import_evidence` uses source_nodes to authorize superseded source events, and `scope_identity()` exposes the exact set to callers. Retain source_nodes for this branch or establish a new exact derivation and update/prove all consumers before counting these savings.",
                "minimal_seam": "Use a distinct decoded compact-envelope type in Freshness; locally validate exact event IDs/root and apply retained record postimages; defer the source/birth-dependent supersession check. Add a History-owned finalizer after Archive/Memory/epoch reconstruction and before final validation/public exposure. History must hydrate a complete semantic settlement, with the initial branch retaining source_nodes (or a separately proven equivalent). Completion hashing remains over exact physical compact bytes.",
                "current_blockers": [
                    "FreshnessStore::ingest_import_settlement currently decodes full semantic authority and calls validate_semantic_import_evidence immediately.",
                    "History::validate_settlement_scope requires semantic Some, then validates framed receipt/birth proofs or the explicit InitialHistory path.",
                    "Import readiness and reconciliation consume scope_identity source_nodes/born_memories; they cannot receive an unresolved projection.",
                    "Completion proof decodes the settlement before final History hydration and binds the raw payload digest; the compact envelope must expose owner/import/start/cut identity during that scan.",
                ],
            },
            "retained_evidence_and_transforms": {
                "prune_reclaim": "Current REL prune/reclaim validation compares transformed-copy semantic settlements, ImportEpoch status, Freshness policy/current projection, owner evidence digest, and clocks. A compact format can survive only if Archive ImportReceipt plus required Episode/Node/Insomnia/Memory birth proofs and referenced CVAFRS02 event frames remain available; current transformed-copy proof is the validation seam.",
                "rewrite": "Current rewrite emits a new owner generation and does not copy ImportEpochStore/settlement frames; it is a generation boundary rather than preservation of CVAFRS06. Any requirement to inspect old settlement authority across rewrite needs explicit lineage/format scope.",
                "same_turn_attribution": "Cuts cannot by themselves distinguish exact events at the same activity turn. Preserve accepted event IDs explicitly and resolve each ID uniquely against retained CVAFRS02; recompute canonical digest/turn/recipient leaves and verify the aggregate commitment.",
                "source_functions": [
                    "src/freshness/freshness_semantic_settlement.rs encode/decode",
                    "src/freshness/freshness_import_events.rs::validate_semantic_import_evidence",
                    "src/freshness/freshness_import_settlement.rs::ingest_import_settlement and scope_identity",
                    "src/history/settlement_scope.rs::validate_settlement_scope",
                    "src/history/epoch_scope_proof.rs::prepare_semantic_import_scope and validate_semantic_import_births",
                    "src/history/transformed_copy.rs::historical_copy_preserves_import_authority",
                    "src/maintenance/reliquary_prune.rs and src/maintenance/reclamation.rs",
                    "src/maintenance/rewrite/file.rs::emit_rel",
                ],
            },
            "limitations": [
                "Dictionary candidate size assumes a new canonical schema with deterministic lexicographic dictionaries and ULEB128 indexes; bytes are exact for that stated layout, not measured runtime output.",
                "Reconstruction candidate savings are conditional. The postimage-retaining candidate avoids base-state replay but still requires retained owner proofs, exact event lookup, transform retention and completion-digest migration to be proven at S3/C3.",
                "The report keeps baseline source hashes and file sizes; a checksum validates each settlement payload. No corpus bytes are written.",
            ],
        },
        "files": files,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(output, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({
        "output": str(args.output),
        "files": len(files),
        "settlements": sum(f["settlement_count"] for f in files),
        "aggregate": output["aggregate"],
        "source_hashes": [{"fixture": f["fixture"], "sha256": f["sha256"], "bytes": f["bytes"]} for f in files],
    }, indent=2))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (AuditError, OSError, json.JSONDecodeError) as exc:
        raise SystemExit(f"audit failed: {exc}")
