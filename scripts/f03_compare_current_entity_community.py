"""Compare current Entity, Community and retained Resolution to the old native witness."""
import argparse
from collections import Counter
import hashlib
from pathlib import Path
import struct

from f03_legacy_frame_clock import LegacyFrameClock
from f03_verify_archived_oracle import PINS, require
from f03_verify_community_cuts import decode_snapshot, derive_lineage
from f03_verify_dream_journal_frontiers import Reader, encode, sha
from f03_verify_independent_repeat import (
    MANIFEST, SCHEMA_V2, decimal, read_document, read_exact, regular, snapshot, take_stream,
)
from f03_verify_resolution_cuts import HEADER, INERT, REVISION, signed

CURRENT_HEADER = b"REL\0\r\n\x1a\n\x02\0\x01\0\x28\0\0\0"
COMMUNITY = {"community_snapshot", "community_lineage", "community_name"}
RESOLUTION = {REVISION, INERT}
ENTITY = {"CVAENC02", "CVAEND02", "CVAETD02", "CVAETM02"}


class CurrentClock(LegacyFrameClock):
    def consume_current(self, payload):
        if payload.startswith((b"RELPUB", b"RELVERS")):
            return self.consume(b"CVA" + payload[3:])
        if payload.startswith(b"RELINSC8"):
            return self.consume(b"CVA" + payload[3:])
        if payload.startswith(b"RELINSC7"):
            require(len(payload) >= 84, "truncated target grouped allocation")
            first, count, time = struct.unpack_from("<QIq", payload, 64)
            self.allocate(first, count, time)
        elif payload.startswith(b"RELXMEM2"):
            require(len(payload) >= 68 and sha(payload[:-32]) == payload[-32:].hex(),
                    "invalid target external grouped allocation")
            length, first, count, time = struct.unpack_from("<QQIq", payload, 8)
            require(length + 68 == len(payload) and count <= 128, "invalid target external grouped extent")
            self.allocate(first, count, time)
        return self.head


def check_value(expected, domain, family, key, value, availability):
    raw = encode(value)
    record = expected.pop((domain, family, sha(key)), None)
    require(record is not None and record["canonical_value_sha256"] == sha(raw)
            and decimal(record["canonical_byte_length"]) == len(raw) and record["availability"] == availability,
            f"current {family} differs from exact retained source value")
    return sha(raw)


def community(raw, uuid, expected, previous):
    require(raw.startswith(b"RELCOM02") and raw[8:24] == uuid, "target Community lacks exact derivation metadata")
    value = decode_snapshot(b"CVACOMM1\x01\0\0\0" + raw[24:], uuid)
    generation = value["generation"]
    require(generation == (1 if previous is None else previous["generation"] + 1),
            "target Community generation differs from source sequence")
    key = generation.to_bytes(8, "big")
    digests = {"snapshot": check_value(expected, "clock_neutral_history", "community_snapshot", key,
                                      value, "retained")}
    if previous is not None:
        digests["lineage"] = check_value(expected, "clock_neutral_history", "community_lineage", key,
                                         derive_lineage(previous, value), "retained")
    return value, digests


def resolution(raw, uuid, expected, counts, source_frame, frontier):
    require(len(raw) >= 40 and raw[8:24] == uuid, "target Resolution owner differs")
    global_, ordinal = struct.unpack_from("<QQ", raw, 24)
    require((ordinal, global_) == source_frame[:2] and global_ == frontier,
            "target Resolution physical source coordinate differs")
    if raw.startswith(b"RELERL01"):
        require(source_frame[2] == "CVAERL01" and len(raw) >= 84, "inert Resolution carrier or extent differs")
        size = int.from_bytes(raw[40:44], "little")
        literal = raw[44:-32]
        require(8 <= size <= 8 * 1024 * 1024 and len(literal) == size and literal.startswith(b"CVAERS02")
                and sha(raw[:-32]) == raw[-32:].hex(), "target inert Resolution payload or checksum differs")
        family = INERT
        value = {"availability": "inert", "retained_codec": "CVAERS02",
                 "retained_payload_sha256": sha(literal), "retained_payload_byte_length": size}
    else:
        require(raw.startswith(b"RELERH01") and source_frame[2] == "CVAERS03" and len(raw) >= 40 + HEADER.size,
                "target Resolution decision carrier differs")
        memory, field, start, end, revision, at, status = HEADER.unpack_from(raw, 40)
        require(field in (1, 2) and 0 <= start < end and revision > 0 and status <= 4,
                "invalid typed Resolution header")
        evidence = Reader(raw, 40 + HEADER.size)
        if status == 1:
            evidence.take(32)
        if status in (1, 2):
            require(1 <= evidence.number(1) <= 15, "unknown Resolution reason")
        if status in (3, 4):
            count = evidence.number(1)
            require(count <= 8, "too many target Resolution candidates")
            candidates = [evidence.take(32) for _ in range(count)]
            require(candidates == sorted(set(candidates)) and 1 <= evidence.number(1) <= 15,
                    "invalid target Resolution candidates or reason")
            evidence.take(64)
            first, last = signed(evidence.take(8)), signed(evidence.take(8))
            require(first <= last and evidence.number(2) > 0
                    and (status != 4 or signed(evidence.take(8)) >= last), "invalid Resolution attempt chronology")
        evidence.end()
        family = REVISION
        value = {"availability": "retained" if status else "tombstoned", "memory_id": memory.hex(),
                 "field": field, "start_byte": start, "end_byte": end, "revision": revision, "updated_at_ns": at,
                 "comparison_sha256": sha(b"CVAERS01" + raw[40:])}
    key = counts[family].to_bytes(8, "big")
    counts[family] += 1
    check_value(expected, "clock_neutral_history", family, key, value, value["availability"])


class Entities:
    def __init__(self):
        self.current = {}
        self.terminals = set()
        self.local = 0
        self.revisions = 0
        self.rows = 0

    def accept(self, raw, body, uuid, expected, source_frame, clock):
        require(raw.startswith(b"RELPUB01") and body.startswith(b"RELEPB01"), "missing current Entity binding")
        global_ = int.from_bytes(raw[8:16], "little")
        mode = raw[16]
        time_in_envelope = signed(raw[17:25]) if mode == 1 else None
        frontier, carrier, source = source_frame
        require(global_ == decimal(source["global_version"]) and clock.head == frontier
                and source["clock"] == ("bind_reserved" if mode == 2 else "allocate")
                and source["transaction_time_ns_in_envelope"] == (None if time_in_envelope is None else str(time_in_envelope)),
                "target Entity allocation or reservation differs from source")
        time = clock.times.get(global_)
        stamp = None if time is None else int(time)

        reader = Reader(body, 8)
        variant, present = reader.number(1), reader.number(1)
        require(variant in (1, 2) and present in (0, 1), "invalid current Entity body flags")
        extra = reader.take(16 if variant == 1 else 32).hex() if present else None
        marker = reader.take(8)
        if variant == 1:
            require(carrier in ("CVAENC02", "CVAEND02") and marker == b"RELPER01" and reader.number(1) == 1,
                    "current Entity revision carrier differs from source")
            local = reader.number()
            require(reader.take(16) == uuid, "target Entity record owner differs")
            identity, revision = reader.take(32), reader.number()
            name = reader.text()
            alias_count = reader.number(1)
            require(alias_count <= 32, "too many retained Entity aliases")
            aliases = [reader.text() for _ in range(alias_count)]
            kind, summary, mutation = reader.text(), reader.text(), reader.text()
            created, updated = signed(reader.take(8)), signed(reader.take(8))
            require(identity not in self.terminals and revision == self.current.get(identity, 0) + 1
                    and (revision == 1) == (carrier == "CVAENC02"), "target Entity revision order differs")
            value = {
                "id": identity.hex(), "global_id": extra, "revision": revision, "kind": kind,
                "payload": {"canonical_name": name, "aliases": aliases, "summary": summary}, "pruned": False,
                "mutation_id": mutation, "created_at_ns": created, "updated_at_ns": updated,
                "global_version": global_, "entity_version": local, "transaction_time_ns": stamp,
            }
            check_value(expected, "entity", "revision", local.to_bytes(8, "big"), value, "retained")
            self.current[identity] = revision
            self.revisions += 1
        else:
            require(carrier in ("CVAETD02", "CVAETM02") and marker == b"RELETM01" and reader.take(16) == uuid,
                    "current Entity terminal carrier or owner differs")
            identity, revision, local = reader.take(32), reader.number(), reader.number()
            require(identity in self.current and revision == self.current[identity] + 1
                    and (extra is None) == (carrier == "CVAETD02") and extra != identity.hex(),
                    "current Entity replacement or terminal differs from source")
            value = {"id": identity.hex(), "revision": revision, "entity_version": local, "global_version": global_,
                     "transaction_time_ns": stamp, "replacement": extra}
            check_value(expected, "entity", "terminal", identity, value, "tombstoned")
            del self.current[identity]
            self.terminals.add(identity)
        reader.end()
        require(local == self.local + 1, "target Entity local history is discontinuous")
        self.local = local

    def finish(self, owner, expected):
        for key, revision in self.current.items():
            check_value(expected, "entity", "current", key, revision, "retained")
        proof = next(row for row in owner["native"]["native_history_projections"] if row["domain"] == "entity")
        rows = self.local + len(self.current)
        require(decimal(proof["rows"]) == rows and decimal(proof["local_cut"]) == self.local,
                "target Entity head or retained row count differs from its source")
        return rows


def observe(output, artifacts, owner, identity, candidate, target_sha):
    expected = {}
    for _, row in take_stream(output, artifacts, owner, "records", owner["native"]["record_stream"],
                              schema_id=SCHEMA_V2):
        family, domain = row["family"], row["projection_domain"]
        if domain == "entity" or (domain == "clock_neutral_history" and family in COMMUNITY | RESOLUTION):
            key = domain, family, row["stable_key_sha256"]
            require(key not in expected, "duplicate retained source value")
            expected[key] = row
    source_community, source_resolution, source_entities = [], [], []
    for ordinal, row in take_stream(output, artifacts, owner, "frames", owner["frame_stream"], schema_id=SCHEMA_V2):
        marker = row["carrier"]
        if marker == "CVACOMM1":
            source_community.append(decimal(row["latest_global"]))
        elif marker in ("CVAERS03", "CVAERL01"):
            source_resolution.append((ordinal, decimal(row["latest_global"]), marker))
        elif marker in ENTITY:
            source_entities.append((decimal(row["latest_global"]), marker, row["details"]))
    communities, counts, entities = [], Counter(), Entities()
    uuid = bytes.fromhex(owner["owner_uuid"])
    kind = {"REL": 1, "PHY": 2}[identity["kind"]]
    scope = {"unspecified": 0, "organization": 1, "project": 2, "connection": 3}[identity["scope"]]
    previous = None
    clock = CurrentClock()
    digest = hashlib.sha256()
    before = regular(candidate)
    with candidate.open("rb") as source:
        header = read_exact(source, 40)
        require(header == CURRENT_HEADER + bytes([kind, scope]) + b"\0" * 6 + uuid,
                "target current format, scope or owner identity differs")
        digest.update(header)
        ordinal = 0
        while source.tell() < before.st_size:
            length = read_exact(source, 8)
            size = int.from_bytes(length, "little")
            require(8 <= size <= 16 * 1024 * 1024 and source.tell() + size <= before.st_size,
                    "invalid target frame extent")
            raw = read_exact(source, size)
            digest.update(length)
            digest.update(raw)
            frontier = clock.consume_current(raw)
            target = raw[25 if raw[16] == 1 else 17:] if raw.startswith(b"RELPUB01") else raw
            if raw.startswith(b"RELCOM"):
                require(len(communities) < len(source_community) and frontier == source_community[len(communities)],
                        "target Community physical clock differs from its accepted source frontier")
                previous, values = community(raw, uuid, expected, previous)
                communities.append({"target_frame_ordinal": ordinal, "global_version": frontier,
                                    "generation": previous["generation"], "graph_version": previous["derived_graph_version"],
                                    "matched_source_value_sha256": values})
            elif raw.startswith((b"RELERH", b"RELERL")):
                index = sum(counts.values())
                require(index < len(source_resolution), "unexpected target Resolution record")
                resolution(raw, uuid, expected, counts, source_resolution[index], frontier)
            elif target.startswith(b"RELEPB"):
                require(entities.local < len(source_entities), "unexpected target Entity publication")
                entities.accept(raw, target, uuid, expected, source_entities[entities.local], clock)
            else:
                require(not target.startswith((b"RELPER", b"RELET")), "target Entity facts lack exact retained bindings")
            ordinal += 1
        require(source.tell() == before.st_size and digest.hexdigest() == target_sha
                and snapshot(before) == snapshot(regular(candidate)), "target copy differs from the reviewed candidate pin")
    entity_rows = entities.finish(owner, expected)
    require(not expected and len(communities) == len(source_community) and sum(counts.values()) == len(source_resolution)
            and entities.local == len(source_entities)
            and clock.head + 1 == decimal(owner["native"]["global_stream"]["rows"]),
            "retained source values or global clock were omitted")
    return {"entity_rows": entity_rows, "entity_revisions": entities.revisions, "entity_terminals": len(entities.terminals),
            "resolution_rows": dict(sorted(counts.items())), "community_generations": communities,
            "target_frames": ordinal, "target_global_head": clock.head}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("source-coverage", "verified-conversion-inputs", "candidate", "receipt"):
        parser.add_argument(f"--{name}", type=Path, required=True)
    parser.add_argument("--source-sha256", required=True)
    parser.add_argument("--target-sha256", required=True)
    args = parser.parse_args()
    output = args.source_coverage.parent
    coverage, coverage_sha = read_document(args.source_coverage, 2 * 1024 * 1024)
    manifest, manifest_sha = read_document(output / MANIFEST, 2 * 1024 * 1024)
    custody, custody_sha = read_document(args.verified_conversion_inputs, 2 * 1024 * 1024)
    require(coverage["schema_id"] == manifest["schema_id"] == SCHEMA_V2
            and coverage_sha == manifest["source_coverage_sha256"] == custody["source_coverage_sha256"]
            and manifest_sha == custody["source_manifest_sha256"] and args.source_sha256 in PINS.values(),
            "retained source or copy custody differs")
    owners = [row for row in coverage["sources"] if row["source_sha256"] == args.source_sha256]
    identities = [row for row in custody["sources"] if row["source_sha256"] == args.source_sha256]
    require(len(owners) == len(identities) == 1 and owners[0]["owner_uuid"] == identities[0]["owner_uuid"]
            and identities[0]["verified_disposable_sha256"] == args.source_sha256, "unbound source owner identity")
    owner = owners[0]
    result = observe(output, dict(manifest["artifact_sha256"]), owner, identities[0], args.candidate, args.target_sha256)
    receipt = {
        "status": "PASS", "scope": "current typed Entity, Resolution and Community retention",
        "copy_id": owner["copy_id"], "source_sha256": args.source_sha256, "target_sha256": args.target_sha256,
        "source_coverage_sha256": coverage_sha, "source_manifest_sha256": manifest_sha,
        "verified_conversion_inputs_sha256": custody_sha, "whole_owner_verified": False, **result,
    }
    regular(args.receipt.parent, directory=True)
    with args.receipt.open("xb") as file:
        file.write(encode(receipt) + b"\n")


if __name__ == "__main__":
    main()
