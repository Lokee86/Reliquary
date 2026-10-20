#!/usr/bin/env python3
"""Read-only inventory of global tickets and owner publication links."""

from __future__ import annotations

import argparse
import hashlib
import json
import struct
from collections import Counter, defaultdict
from pathlib import Path
from typing import Any

CONTAINER_MAGIC = b"CVA\0\r\n\x1a\n"
GLOBAL = {
    b"CVAVERS1": ("container_ticket_v1", "Container", 16, 8, 0, 0, 0),
    b"CVAVERS2": ("container_ticket_v2", "Container", 24, 8, 0, 8, 0),
}
OWNER_LINKS = {
    b"CVAMEMV1": ("Memory", "memory_link", 40, 8, 8, 0, 16),
    b"CVAENTV1": ("Entity", "entity_link", 40, 8, 8, 0, 16),
    b"CVARELV1": ("Relationship", "relationship_link", 40, 8, 8, 0, 16),
    b"CVAGVER1": ("Graph", "graph_link", 40, 8, 8, 0, 16),
    b"CVAAREC1": ("Archive", "archive_link", 40, 8, 8, 0, 16),
}
LOCAL_CARRIERS = {
    b"CVAMC001": ("Memory", "creation", 72, 8, 8),
    b"CVAMD001": ("Memory", "delta", 56, 16, 8),
    b"CVAENC01": ("Entity", "creation", None, None, 8),
    b"CVAEND01": ("Entity", "delta", 56, 16, 8),
    b"CVARNC01": ("Relationship", "creation", 40, 8, 8),
    b"CVARND01": ("Relationship", "delta", 56, 16, 8),
    b"CVAECR01": ("Ego", "creation", 9, 0, 8),
    b"CVAEDL01": ("Ego", "delta", 9, 0, 8),
    b"CVAETM01": ("Ego", "terminal", 9, 0, 8),
}
LEGACY_EGO = {
    b"CVAEIDN1", b"CVAEIDN2", b"CVAEIDA1", b"CVAEPER1", b"CVAEANC1", b"CVAESYN1"
}
OWNER_VERSION_PREFIXES = (b"CVAMEMV", b"CVAENTV", b"CVARELV", b"CVAGVER", b"CVAAREC")
GLOBAL_PREFIXES = (b"CVAVERS",)
LOCAL_PREFIXES = tuple(magic[:6] for magic in LOCAL_CARRIERS)
COMPLETION_MAGICS = {f"CVAINSC{i}".encode("ascii") for i in range(1, 8)} | {b"CVAINSC8"}


class MeasurementError(ValueError):
    pass


def u16(data: bytes, at: int) -> int:
    return struct.unpack_from("<H", data, at)[0]


def u32(data: bytes, at: int) -> int:
    return struct.unpack_from("<I", data, at)[0]


def parse_container(data: bytes) -> tuple[int, list[tuple[int, bytes]]]:
    if len(data) < 16 or data[:8] != CONTAINER_MAGIC:
        raise MeasurementError("invalid or truncated Container header")
    version = (u16(data, 8), u16(data, 10))
    header_len = u32(data, 12)
    if version != (1, 0):
        raise MeasurementError(f"expected pinned V1.0 Container, found {version}")
    if header_len not in (16, 24, 40) or header_len > len(data):
        raise MeasurementError(f"invalid Container header length {header_len}")
    if header_len >= 24 and (data[16] not in (1, 2) or data[17] not in (0, 1, 2, 3) or data[18:24] != bytes(6)):
        raise MeasurementError("invalid typed Container identity")
    if header_len >= 24 and data[16] == 2 and data[17] != 0:
        raise MeasurementError("invalid Phylactery Container scope")
    records = []
    cursor = header_len
    while cursor < len(data):
        if cursor + 8 > len(data):
            raise MeasurementError(f"truncated chunk length at {cursor}")
        size = struct.unpack_from("<Q", data, cursor)[0]
        start, end = cursor + 8, cursor + 8 + size
        if end > len(data):
            raise MeasurementError(f"truncated chunk at {cursor}")
        records.append((cursor, data[start:end]))
        cursor = end
    return header_len, records


def parse_record(payload: bytes) -> dict[str, Any] | None:
    if len(payload) < 8:
        return None
    magic = payload[:8]
    if magic in GLOBAL:
        name, owner, expected, global_at, local_bytes, time_bytes, ref_bytes = GLOBAL[magic]
        if len(payload) != expected:
            raise MeasurementError(f"invalid {name} length")
        transaction_time_ns = struct.unpack_from("<q", payload, 16)[0] if time_bytes else None
        return {"family": name, "owner": owner, "payload_bytes": len(payload),
                "global_version_bytes": global_at and 8 or 0,
                "owner_local_version_bytes": local_bytes, "transaction_time_bytes": time_bytes,
                "object_ref_bytes": ref_bytes, "range_count_bytes": 0,
                "global_ticket_version": struct.unpack_from("<Q", payload, global_at)[0],
                "transaction_time_ns": transaction_time_ns,
                "field_offsets": {"global_version": global_at}}
    if magic in OWNER_LINKS:
        owner, name, expected, global_at, local_bytes, time_bytes, ref_bytes = OWNER_LINKS[magic]
        if len(payload) != expected:
            raise MeasurementError(f"invalid {name} length")
        return {"family": name, "owner": owner, "payload_bytes": len(payload),
                "global_version_bytes": 8, "owner_local_version_bytes": 8,
                "transaction_time_bytes": 0, "object_ref_bytes": 16,
                "range_count_bytes": 0,
                "global_versions": [struct.unpack_from("<Q", payload, global_at)[0]],
                "field_offsets": {"global_version": global_at, "owner_local_version": 16, "object_ref": 24}}
    if magic in LOCAL_CARRIERS:
        owner, form, fixed_prefix, unused, local_bytes = LOCAL_CARRIERS[magic]
        if fixed_prefix is not None and len(payload) < fixed_prefix + local_bytes:
            raise MeasurementError(f"truncated {owner} {form} carrier")
        global_bytes = 8 if owner in ("Memory", "Entity", "Relationship") else 0
        if owner == "Entity" and form == "creation":
            if len(payload) < 41 or payload[40] not in (0, 1):
                raise MeasurementError("invalid Entity compact creation optional-global-ID tag")
            global_at = 41 + (16 if payload[40] else 0)
            local_at = global_at + 8
            if len(payload) < local_at + 8:
                raise MeasurementError("truncated Entity compact creation clocks")
        elif owner in ("Memory", "Entity", "Relationship"):
            global_at = (72 if owner == "Memory" and form == "creation" else
                         40 if owner == "Relationship" and form == "creation" else
                         56)
            local_at = global_at + 8
        else:
            global_at = None
            local_at = 9
        global_versions = [struct.unpack_from("<Q", payload, global_at)[0]] if global_at is not None else []
        return {"family": f"{owner.lower()}_compact_{form}", "owner": owner,
                "payload_bytes": len(payload), "global_version_bytes": global_bytes,
                "owner_local_version_bytes": local_bytes, "transaction_time_bytes": 0,
                "object_ref_bytes": 0, "range_count_bytes": 0,
                "global_versions": global_versions,
                "field_offsets": {"global_version": global_at,
                "owner_local_version": local_at}}
    if magic in LEGACY_EGO:
        if len(payload) < 16:
            raise MeasurementError("truncated legacy Ego record")
        return {"family": "ego_legacy_record", "owner": "Ego", "payload_bytes": len(payload),
                "global_version_bytes": 0, "owner_local_version_bytes": 8,
                "transaction_time_bytes": 0, "object_ref_bytes": 0, "range_count_bytes": 0,
                "field_offsets": {"owner_local_version": 8}}
    if magic in COMPLETION_MAGICS:
        version = int(chr(magic[-1]))
        offset = 0
        if version == 8:
            if len(payload) < 9 or payload[8] not in range(1, 7):
                raise MeasurementError("invalid CVAINSC8 legacy completion variant")
            version = payload[8]
            offset = 1
        if version == 1:
            return {"family": "insomnia_completion_v1" if offset == 0 else "insomnia_completion_v8_v1",
                    "owner": "Insomnia", "payload_bytes": len(payload), "global_version_bytes": 0,
                    "owner_local_version_bytes": 0, "transaction_time_bytes": 0,
                    "object_ref_bytes": 0, "range_count_bytes": 0, "global_ranges": [],
                    "field_offsets": {}}
        range_at = 64 + offset
        minimum = (84 if version >= 4 else 76) + offset
        if version == 7:
            minimum = 85 + offset
            birth_flag_at = 84 + offset
            if len(payload) < minimum or payload[birth_flag_at] not in (0, 1):
                raise MeasurementError("invalid/truncated Insomnia v7 completion header")
        elif len(payload) < minimum:
            raise MeasurementError("truncated Insomnia completion range")
        start = struct.unpack_from("<Q", payload, range_at)[0]
        count = struct.unpack_from("<I", payload, range_at + 8)[0]
        has_time = version >= 4
        transaction_time = struct.unpack_from("<q", payload, range_at + 12)[0] if has_time else None
        if count and start == 0:
            raise MeasurementError("invalid Insomnia completion range")
        return {"family": f"insomnia_completion_v{8 if offset else version}", "owner": "Insomnia",
                "payload_bytes": len(payload), "global_version_bytes": 8,
                "owner_local_version_bytes": 0, "transaction_time_bytes": 8 if has_time else 0,
                "object_ref_bytes": 0, "range_count_bytes": 4,
                "global_ranges": [{"start": start, "count": count, "transaction_time_ns": transaction_time}],
                "field_offsets": {"global_range_start": range_at, "global_range_count": range_at + 8,
                                  **({"transaction_time_ns": range_at + 12} if has_time else {})}}
    if magic in (b"CVAXMEM1", b"CVAXMEM2"):
        if len(payload) < 68:
            raise MeasurementError("truncated external-source publication range")
        body_len = struct.unpack_from("<Q", payload, 8)[0]
        start = struct.unpack_from("<Q", payload, 16)[0]
        count = struct.unpack_from("<I", payload, 24)[0]
        transaction_time = struct.unpack_from("<q", payload, 28)[0]
        if body_len > 8 * 1024 * 1024 or body_len + 68 != len(payload) or hashlib.sha256(payload[:-32]).digest() != payload[-32:]:
            raise MeasurementError("invalid external-source publication length/checksum")
        if count > 128 or (count and start == 0):
            raise MeasurementError("invalid external-source publication range")
        try:
            wire = json.loads(payload[36:-32])
        except (UnicodeDecodeError, json.JSONDecodeError) as error:
            raise MeasurementError("invalid external-source publication JSON") from error
        expected_keys = {"window", "global_start", "transaction_ns", "birth_turn", "records", "bodies", "bindings", "routing"}
        if (not isinstance(wire, dict) or set(wire) != expected_keys
                or wire["global_start"] != start or wire["transaction_ns"] != transaction_time
                or not isinstance(wire["records"], list) or len(wire["records"]) != count):
            raise MeasurementError("external-source publication JSON/range mismatch")
        return {"family": "external_source_publication_v" + magic[-1:].decode(),
                "owner": "ExternalSource", "payload_bytes": len(payload), "global_version_bytes": 8,
                "owner_local_version_bytes": 0, "transaction_time_bytes": 8,
                "object_ref_bytes": 0, "range_count_bytes": 4,
                "global_ranges": [{"start": start, "count": count, "transaction_time_ns": transaction_time}],
                "field_offsets": {"json_length": 8, "global_range_start": 16,
                                  "global_range_count": 24, "transaction_time_ns": 28}}
    for prefix in GLOBAL_PREFIXES:
        if magic.startswith(prefix):
            raise MeasurementError(f"unsupported global ticket version {magic!r}")
    if magic.startswith(b"CVAINSC"):
        raise MeasurementError(f"unsupported Insomnia completion version {magic!r}")
    if magic.startswith(b"CVAXMEM"):
        raise MeasurementError(f"unsupported external-source publication version {magic!r}")
    for prefix in OWNER_VERSION_PREFIXES:
        if magic.startswith(prefix):
            raise MeasurementError(f"unsupported owner link version {magic!r}")
    if any(magic.startswith(prefix) for prefix in LOCAL_PREFIXES):
        raise MeasurementError(f"unsupported compact owner publication version {magic!r}")
    return None


def summarize(path: Path, fixture: str, expected_size: int, expected_hash: str) -> dict[str, Any]:
    before = path.read_bytes()
    digest = hashlib.sha256(before).hexdigest()
    if len(before) != expected_size or digest != expected_hash:
        raise MeasurementError(f"source identity mismatch for {fixture}: {len(before)} bytes, {digest}")
    header, chunks = parse_container(before)
    parsed = []
    for offset, payload in chunks:
        row = parse_record(payload)
        if row is not None:
            row["container_offset"] = offset
            row["container_framed_bytes"] = len(payload) + 8
            parsed.append(row)
    after_hash = hashlib.sha256(path.read_bytes()).hexdigest()
    if after_hash != digest:
        raise MeasurementError(f"source changed during measurement: {fixture}")
    counts = Counter(row["family"] for row in parsed)
    grouped: dict[str, dict[str, int]] = {}
    for family in sorted(counts):
        rows = [row for row in parsed if row["family"] == family]
        grouped[family] = {
            "record_count": len(rows),
            "payload_bytes": sum(row["payload_bytes"] for row in rows),
            "container_framed_bytes": sum(row["container_framed_bytes"] for row in rows),
            "global_version_bytes": sum(row["global_version_bytes"] for row in rows),
            "owner_local_version_bytes": sum(row["owner_local_version_bytes"] for row in rows),
            "transaction_time_bytes": sum(row["transaction_time_bytes"] for row in rows),
            "object_ref_bytes": sum(row["object_ref_bytes"] for row in rows),
            "range_count_bytes": sum(row.get("range_count_bytes", 0) for row in rows),
            "field_offsets": rows[0]["field_offsets"],
        }
    shapes: dict[str, dict[str, int]] = {}
    for family, values in grouped.items():
        count = values["record_count"]
        shape = (values["global_version_bytes"] // count, values["owner_local_version_bytes"] // count,
                 values["transaction_time_bytes"] // count, values["object_ref_bytes"] // count,
                 values["range_count_bytes"] // count)
        label = f"global={shape[0]};local={shape[1]};time={shape[2]};object_ref={shape[3]};range_count={shape[4]}"
        entry = shapes.setdefault(label, {"record_count": 0, "payload_bytes": 0,
                                          "container_framed_bytes": 0, "family_count": 0})
        entry["record_count"] += values["record_count"]
        entry["payload_bytes"] += values["payload_bytes"]
        entry["container_framed_bytes"] += values["container_framed_bytes"]
        entry["family_count"] += 1
    # Unknown versions in recognized families have already failed in parse_record.
    # Explicit zeroes make that absence visible in the inventory.
    unknown_version_counts = {prefix.decode("ascii"): 0 for prefix in
                              (*GLOBAL_PREFIXES, *OWNER_VERSION_PREFIXES, b"CVAINSC", b"CVAXMEM")}
    owner_counts = {owner: sum(row["owner"] == owner for row in parsed)
                    for owner in ("Memory", "Entity", "Relationship", "Ego", "Archive", "Graph", "Container", "Insomnia", "ExternalSource")}
    ticket_attribution = attribute_global_tickets(parsed)
    return {"fixture": fixture, "source_bytes": len(before), "source_sha256_before": digest,
            "source_sha256_after": after_hash, "source_unchanged": True,
            "container_header_bytes": header, "container_record_count": len(chunks),
            "recognized_publication_record_count": len(parsed), "recognized_counts_by_owner": owner_counts,
            "owner_link_family_counts": {family: counts.get(family, 0) for family in
                ("memory_link", "entity_link", "relationship_link", "archive_link", "graph_link")},
            "counts_by_family": dict(sorted(counts.items())), "bytes_by_family": grouped,
            "publication_shapes": dict(sorted(shapes.items())),
            "recognized_payload_bytes": sum(r["payload_bytes"] for r in parsed),
            "recognized_container_framed_bytes": sum(r["container_framed_bytes"] for r in parsed),
            "unknown_version_family_counts": unknown_version_counts,
            "unclassified_record_count": len(chunks) - len(parsed),
            "unclassified_magic_counts": dict(sorted(Counter(
                payload[:8].decode("ascii", errors="backslashreplace")
                for _, payload in chunks if parse_record(payload) is None
            ).items())),
            "global_ticket_attribution": ticket_attribution}


def attribute_global_tickets(records: list[dict[str, Any]]) -> dict[str, Any]:
    tickets: dict[int, list[dict[str, Any]]] = defaultdict(list)
    direct: dict[int, set[str]] = defaultdict(set)
    direct_counts: Counter[str] = Counter()
    ranges: list[tuple[str, dict[str, Any]]] = []
    for record in records:
        if "global_ticket_version" in record:
            tickets[record["global_ticket_version"]].append(record)
        for version in record.get("global_versions", []):
            direct[version].add(record["owner"])
            direct_counts[record["owner"]] += 1
        for version_range in record.get("global_ranges", []):
            ranges.append((record["owner"], version_range))

    range_owners: dict[int, set[str]] = defaultdict(set)
    range_coverage_by_owner: Counter[str] = Counter()
    version_times: dict[int, set[int]] = defaultdict(set)
    for owner, version_range in ranges:
        start, count = version_range["start"], version_range["count"]
        range_coverage_by_owner[owner] += count
        timestamp = version_range["transaction_time_ns"]
        for version in range(start, start + count):
            range_owners[version].add(owner)
            if timestamp is not None:
                version_times[version].add(timestamp)

    ticket_owners: dict[int, set[str]] = {}
    for version in tickets:
        ticket_owners[version] = direct.get(version, set()) | range_owners.get(version, set())
    ticket_owner_counts: Counter[str] = Counter()
    for owners in ticket_owners.values():
        for owner in owners:
            ticket_owner_counts[owner] += 1
    ticket_timestamps = {
        version: {record.get("transaction_time_ns") for record in rows
                  if record.get("transaction_time_ns") is not None}
        for version, rows in tickets.items()
    }
    time_mismatch_count = sum(
        1 for version, times in version_times.items()
        if version in ticket_timestamps and ticket_timestamps[version]
        and not times.issubset(ticket_timestamps[version])
    )
    unmatched_direct_refs = sum(
        1 for version in direct if version not in tickets and version not in range_owners
    )
    range_family_counts: Counter[str] = Counter()
    range_counts_by_owner: Counter[str] = Counter()
    range_ticket_total = 0
    for record in records:
        for version_range in record.get("global_ranges", []):
            range_family_counts[record["family"]] += 1
            range_counts_by_owner[record["owner"]] += version_range["count"]
            range_ticket_total += version_range["count"]
    exclusive_ticket_bytes: dict[str, dict[str, int]] = defaultdict(lambda: {
        "record_count": 0, "payload_bytes": 0, "container_framed_bytes": 0,
    })
    shared_bucket = {"record_count": 0, "payload_bytes": 0, "container_framed_bytes": 0}
    orphan_bucket = {"record_count": 0, "payload_bytes": 0, "container_framed_bytes": 0}
    for version, rows in tickets.items():
        owners = ticket_owners[version]
        bucket = (orphan_bucket if not owners else shared_bucket if len(owners) > 1 else
                  exclusive_ticket_bytes[next(iter(owners))])
        for record in rows:
            bucket["record_count"] += 1
            bucket["payload_bytes"] += record["payload_bytes"]
            bucket["container_framed_bytes"] += record["payload_bytes"] + 8
    return {
        "individual_container_ticket_records": sum(len(rows) for rows in tickets.values()),
        "distinct_individual_ticket_ids": len(tickets),
        "individual_ticket_ids_by_referencing_owner": dict(sorted(ticket_owner_counts.items())),
        "individual_ticket_ids_with_no_publication_reference": sum(not owners for owners in ticket_owners.values()),
        "individual_ticket_ids_shared_by_multiple_owners": sum(len(owners) > 1 for owners in ticket_owners.values()),
        "exclusive_individual_ticket_bytes_by_owner": dict(sorted(exclusive_ticket_bytes.items())),
        "shared_ticket_bucket": shared_bucket,
        "orphan_ticket_bucket": orphan_bucket,
        "ticket_byte_partition_rule": "Each individual CVAVERS record is counted once: assigned to its sole attributed owner, the shared bucket if multiple owners/ranges claim it, or the orphan bucket if no claim exists.",
        "direct_owner_link_reference_occurrences": sum(direct_counts.values()),
        "direct_owner_link_references_by_owner": dict(sorted(direct_counts.items())),
        "direct_owner_link_distinct_ids_with_no_ticket_or_explicit_range": unmatched_direct_refs,
        "embedded_version_range_records_by_family": dict(sorted(range_family_counts.items())),
        "embedded_version_range_ticket_ids_by_owner": dict(sorted(range_counts_by_owner.items())),
        "embedded_version_range_ticket_id_occurrences": range_ticket_total,
        "ticket_transaction_time_disagreements_with_explicit_ranges": time_mismatch_count,
        "attribution_rule": "Owner links are joined to CVAVERS records by explicit global_version value. Insomnia/external-source reservations expand only their encoded start/count range. Unclaimed means a CVAVERS ID has no matching owner link or range; no record adjacency is used.",
    }


def main() -> None:
    repo = Path(__file__).resolve().parents[1]
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", type=Path, default=repo / "benchmarks" / "authoritative-history-rewrite-step62-2026-10-18.json")
    parser.add_argument("--fixture-root", type=Path, default=repo.parent / "reliquary-fixtures" / "local" / "authoritative")
    parser.add_argument("--output", type=Path, default=repo / "benchmarks" / "publication-framing-byte-inventory-2026-10-20.json")
    args = parser.parse_args()
    manifest = json.loads(args.manifest.read_text(encoding="utf-8"))
    files = []
    for fixture, digest in manifest["authoritative_hashes"].items():
        path = args.fixture_root / Path(fixture)
        files.append(summarize(path, fixture, path.stat().st_size, digest))
    aggregate: dict[str, dict[str, int]] = defaultdict(lambda: defaultdict(int))
    aggregate_shapes: dict[str, dict[str, int]] = defaultdict(lambda: defaultdict(int))
    ticket_owner_totals: Counter[str] = Counter()
    range_owner_totals: Counter[str] = Counter()
    exclusive_ticket_bytes_totals: dict[str, Counter[str]] = defaultdict(Counter)
    shared_ticket_bytes_total: Counter[str] = Counter()
    orphan_ticket_bytes_total: Counter[str] = Counter()
    for item in files:
        for family, measurements in item["bytes_by_family"].items():
            for key, value in measurements.items():
                if isinstance(value, int):
                    aggregate[family][key] += value
        for shape, values in item["publication_shapes"].items():
            for key, value in values.items():
                aggregate_shapes[shape][key] += value
        ticket_data = item["global_ticket_attribution"]
        ticket_owner_totals.update(ticket_data["individual_ticket_ids_by_referencing_owner"])
        range_owner_totals.update(ticket_data["embedded_version_range_ticket_ids_by_owner"])
        for owner, values in ticket_data["exclusive_individual_ticket_bytes_by_owner"].items():
            exclusive_ticket_bytes_totals[owner].update(values)
        shared_ticket_bytes_total.update(ticket_data["shared_ticket_bucket"])
        orphan_ticket_bytes_total.update(ticket_data["orphan_ticket_bucket"])
    ticket_totals = {
        "individual_container_ticket_records_across_files": sum(f["global_ticket_attribution"]["individual_container_ticket_records"] for f in files),
        "per_file_distinct_individual_ticket_ids_sum": sum(f["global_ticket_attribution"]["distinct_individual_ticket_ids"] for f in files),
        "individual_ticket_ids_by_referencing_owner": dict(sorted(ticket_owner_totals.items())),
        "individual_ticket_ids_with_no_publication_reference": sum(f["global_ticket_attribution"]["individual_ticket_ids_with_no_publication_reference"] for f in files),
        "individual_ticket_ids_shared_by_multiple_owners": sum(f["global_ticket_attribution"]["individual_ticket_ids_shared_by_multiple_owners"] for f in files),
        "exclusive_individual_ticket_bytes_by_owner": {owner: dict(values) for owner, values in sorted(exclusive_ticket_bytes_totals.items())},
        "shared_ticket_bucket": dict(shared_ticket_bytes_total),
        "orphan_ticket_bucket": dict(orphan_ticket_bytes_total),
        "direct_owner_link_reference_occurrences": sum(f["global_ticket_attribution"]["direct_owner_link_reference_occurrences"] for f in files),
        "direct_owner_link_references_by_owner": {
            owner: sum(f["global_ticket_attribution"]["direct_owner_link_references_by_owner"].get(owner, 0) for f in files)
            for owner in sorted({owner for f in files for owner in f["global_ticket_attribution"]["direct_owner_link_references_by_owner"]})
        },
        "direct_owner_link_distinct_ids_with_no_ticket_or_explicit_range": sum(f["global_ticket_attribution"]["direct_owner_link_distinct_ids_with_no_ticket_or_explicit_range"] for f in files),
        "embedded_version_range_ticket_ids_by_owner": dict(sorted(range_owner_totals.items())),
        "embedded_version_range_ticket_id_occurrences": sum(f["global_ticket_attribution"]["embedded_version_range_ticket_id_occurrences"] for f in files),
    }
    artifact = {
        "schema_version": 1, "measurement_date": "2026-10-20", "plan_step": 166,
        "decision": "inventory_only_no_compaction_projection",
        "manifest": str(args.manifest), "fixture_root": str(args.fixture_root),
        "authoritative_sources_unchanged": all(f["source_unchanged"] for f in files),
        "method": {
            "container": "Read-only pinned V1.0 chunk scan; each chunk is an 8-byte little-endian payload length followed by payload.",
            "global_tickets": "CVAVERS1 is {global_version:u64}; CVAVERS2 is {global_version:u64, transaction_time_ns:i64}.",
            "owner_links": "Memory CVAMEMV1, Entity CVAENTV1, Relationship CVARELV1, Graph CVAGVER1, and Archive CVAAREC1 each carry global u64, owner-local u64, and 16-byte ObjectRef. Compact Memory/Entity/Relationship carriers inline their clocks; Ego carries only Ego-local sequence and has no global ticket or ObjectRef; compact Graph ordinal frames carry no clocks or references.",
            "size_accounting": "Payload field bytes are separated from per-chunk 8-byte Container framing. This is measurement only and assumes no replacement format or savings.",
            "unknown_families": "Recognized ticket/link/local-carrier prefixes with unknown suffixes fail closed. Unknown unrelated chunks are counted as unclassified, not inferred as publication records.",
        },
        "fixtures": files,
        "aggregate": {"fixture_count": len(files), "publication_bytes_by_family": {k: dict(v) for k, v in sorted(aggregate.items())},
                      "publication_shapes": {k: dict(v) for k, v in sorted(aggregate_shapes.items())},
                      "global_ticket_attribution": ticket_totals,
                      "unknown_version_family_counts": {p.decode("ascii"): sum(f["unknown_version_family_counts"][p.decode("ascii")] for f in files)
                                                        for p in (*GLOBAL_PREFIXES, *OWNER_VERSION_PREFIXES, b"CVAINSC", b"CVAXMEM")},
                      "unclassified_record_count": sum(f["unclassified_record_count"] for f in files),
                      "zero_count_owner_families": [owner for owner in ("Memory", "Entity", "Relationship", "Ego", "Archive", "Graph", "Container", "Insomnia", "ExternalSource") if sum(f["recognized_counts_by_owner"][owner] for f in files) == 0]},
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(artifact, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"output": str(args.output), "aggregate": artifact["aggregate"]}, indent=2))


if __name__ == "__main__":
    main()
