"""Export pinned Entity publications and translate exact retained Resolution decisions."""
import argparse
from collections import Counter
import hashlib
from pathlib import Path

from f03_export_memory_transfer import Wire
from f03_legacy_frame_clock import LegacyFrameClock
from f03_verify_archived_oracle import PINS, require
from f03_verify_dream_journal_frontiers import NATIVE_DOMAIN, encode, framed, sha, u64
from f03_verify_independent_repeat import (
    MANIFEST, SCHEMA_V2, decimal, open_header, read_document, read_frame, regular, snapshot, take_stream,
)
from f03_verify_prefix_availability import write_jsonl
from f03_verify_resolution_cuts import (
    DELTA, FULL, HEADER_END, INERT, LITERAL, REVISION, STATUS, decode_delta, decode_inert, signed,
)

ENTITY = {"CVAENC02", "CVAEND02", "CVAETD02", "CVAETM02"}
ENTITY_FIELDS = ("global_id", "canonical_name", "aliases", "kind", "summary", "updated_at_ns")
RESOLUTION_DOMAIN = "clock_neutral_history"


class EntityWire(Wire):
    def global_id(self):
        return self.take(16).hex() if self.flag() else None

    def aliases(self):
        count = self.number(2)
        require(count <= (len(self.value) - self.offset) // 4, "invalid Entity alias count")
        return [self.text() for _ in range(count)]


def entity_value(body, global_, time, current):
    reader = EntityWire(body)
    marker, identity = reader.take(8), reader.identity()
    base = current.get(identity)
    if marker == b"CVAENC02":
        global_id, local, mutation = reader.global_id(), reader.number(), reader.text()
        value = {"id": identity, "global_id": global_id, "revision": 1, "entity_version": local,
                 "mutation_id": mutation, "canonical_name": reader.text(), "aliases": reader.aliases(),
                 "kind": reader.text(), "summary": reader.text(), "created_at_ns": reader.signed(),
                 "updated_at_ns": reader.signed()}
        require(base is None, "Entity creation repeats an active identity")
    elif marker == b"CVAEND02":
        previous, revision, local, mutation, mask = reader.number(), reader.number(), reader.number(), reader.text(), reader.number()
        require(base is not None and previous == base["revision"] and revision == previous + 1
                and mask < 1 << len(ENTITY_FIELDS), "Entity delta has no valid preceding revision")
        readers = (reader.global_id, reader.text, reader.aliases, reader.text, reader.text, reader.signed)
        fields = {field: parse() for i, (field, parse) in enumerate(zip(ENTITY_FIELDS, readers)) if mask & (1 << i)}
        require(all(base[field] != value for field, value in fields.items()), "Entity delta repeats an inherited value")
        value = {**base, **fields, "revision": revision, "entity_version": local, "mutation_id": mutation}
    else:
        require(marker in (b"CVAETD02", b"CVAETM02"), "unknown Entity publication")
        revision, local = reader.number(), reader.number()
        replacement = reader.identity() if marker == b"CVAETM02" else None
        reader.end()
        require(base is not None and revision == base["revision"] + 1 and replacement != identity,
                "Entity terminal has no valid active identity")
        return "terminal", {"id": identity, "revision": revision, "entity_version": local,
                            "global_version": global_, "transaction_time_ns": time, "replacement": replacement}
    require(not reader.flag(), "Entity description prune outside the pinned conversion scope")
    reader.end()
    require(bool(value["mutation_id"]), "empty Entity mutation identity")
    return "revision", {**value, "global_version": global_, "transaction_time_ns": time}


def native_revision(value):
    keys = ("canonical_name", "aliases", "summary")
    return {**{key: item for key, item in value.items() if key not in keys},
            "payload": {key: value[key] for key in keys}, "pruned": False}


def decision_fields(decision):
    status, evidence = decision.status, decision.evidence
    result = {"kind": STATUS[status]}
    if status == 1:
        result.update({"entity_id": evidence[0].hex(), "reason": evidence[1][0]})
    elif status == 2:
        result["reason"] = evidence[0][0]
    elif status in (3, 4):
        raw = evidence[0]
        result.update({
            "candidate_entity_ids": [raw[1 + index * 32:33 + index * 32].hex() for index in range(raw[0])],
            "reason": evidence[1][0], "candidate_set_fingerprint": evidence[2].hex(),
            "context_fingerprint": evidence[3].hex(), "first_seen_at_ns": signed(evidence[4]),
            "last_attempt_at_ns": signed(evidence[5]), "attempt_count": int.from_bytes(evidence[6], "little"),
        })
        if status == 4:
            result["dormant_at_ns"] = signed(evidence[7])
    return result


def verify_entity(owner, expected, revisions, terminals, current):
    rows = [("revision", value["entity_version"].to_bytes(8, "big"), native_revision(value)) for value in revisions]
    rows.extend(("terminal", bytes.fromhex(value["id"]), value) for value in terminals)
    rows.extend(("current", bytes.fromhex(identity), value["revision"]) for identity, value in current.items())
    head = len(revisions) + len(terminals)
    digest = hashlib.sha256(NATIVE_DOMAIN + b"\1\0\0\0" + bytes.fromhex(owner["owner_uuid"])
                            + framed(b"entity") + u64(head) + b"\0" + u64(len(rows)))
    for family, key, value in sorted(rows):
        canonical = encode(value)
        record = expected.pop(("entity", family, sha(key)), None)
        require(record is not None and record["canonical_value_sha256"] == sha(canonical)
                and decimal(record["canonical_byte_length"]) == len(canonical)
                and record["availability"] == ("tombstoned" if family == "terminal" else "retained"),
                f"typed Entity {family} differs from the native commitment")
        digest.update(framed(family.encode()) + framed(key) + framed(canonical))
    proof = next(row for row in owner["native"]["native_history_projections"] if row["domain"] == "entity")
    require(proof == {"domain": "entity", "local_cut": str(head), "event_cut": None, "rows": str(len(rows)),
                      "sha256": digest.hexdigest()}, "Entity head differs from its native commitment")
    return proof


def observe(output, artifacts, owner, source_path):
    native = owner["native"]
    expected = {}
    for _, row in take_stream(output, artifacts, owner, "records", native["record_stream"], schema_id=SCHEMA_V2):
        domain, family = row["projection_domain"], row["family"]
        if domain == "entity" or (domain == RESOLUTION_DOMAIN and family in (REVISION, INERT)):
            key = domain, family, row["stable_key_sha256"]
            require(key not in expected, "duplicate Entity or Resolution source row")
            expected[key] = row
    uuid = bytes.fromhex(owner["owner_uuid"])
    kind = {"reliquary": 1, "phylactery": 2}[native["owner_kind"]]
    clock = LegacyFrameClock()
    entity_rows, resolution_frames, resolution_handled, resolution_values = [], [], [], []
    current, resolved = {}, {}
    revisions, terminals = [], []
    resolution_counts = Counter()
    status_counts = Counter()
    before = regular(source_path)
    with source_path.open("rb") as source:
        digest = open_header(source, kind, owner)
        for ordinal, row in take_stream(output, artifacts, owner, "frames", owner["frame_stream"], schema_id=SCHEMA_V2):
            payload = read_frame(source, row, digest, before.st_size)
            frontier = clock.consume(payload)
            require(frontier == decimal(row["latest_global"]), "Entity physical clock differs from its witness")
            marker = row["carrier"]
            if marker in ENTITY:
                require(payload.startswith(b"CVAPUB01"), "Entity publication lacks native source envelope")
                global_ = int.from_bytes(payload[8:16], "little")
                mode = payload[16]
                body = payload[25 if mode == 1 else 17:]
                time = clock.times.get(global_)
                family, value = entity_value(body, global_, None if time is None else int(time), current)
                require(value["entity_version"] == len(revisions) + len(terminals) + 1,
                        "Entity publication differs from its ordered local coordinate")
                if family == "revision":
                    current[value["id"]] = value
                    revisions.append(value)
                else:
                    del current[value["id"]]
                    terminals.append(value)
                entity_rows.append({"source_ordinal": str(ordinal), "kind": "entity", "carrier": marker, "family": family,
                                    "bound_reservation": mode == 2, "value": value, "source_payload_sha256": row["payload_sha256"]})
            elif marker == "CVAENTF1":
                require(payload == b"CVAENTF1", "invalid Entity source format marker")
                entity_rows.append({"source_ordinal": str(ordinal), "kind": "embedded", "carrier": marker})
            elif payload.startswith((DELTA, LITERAL)):
                if payload.startswith(DELTA):
                    key, decision, value = decode_delta(payload, resolved)
                    if decision.status:
                        resolved[key] = decision
                    else:
                        resolved.pop(key, None)
                    family, status = REVISION, STATUS[decision.status]
                    comparison = FULL + payload[8:HEADER_END] + b"".join(decision.evidence)
                    require(sha(comparison) == value["comparison_sha256"], "Resolution canonical decision differs")
                    body = b"RELERH01" + uuid + u64(frontier) + u64(ordinal) + comparison[8:]
                    typed = {**value, "decision": decision_fields(decision)}
                else:
                    value = decode_inert(payload)
                    family, status = INERT, "inert"
                    raw = payload[12:-32]
                    body = b"RELERL01" + uuid + u64(frontier) + u64(ordinal) + payload[8:12] + raw
                    body += hashlib.sha256(body).digest()
                    typed = {**value, "retained_payload_hex": raw.hex()}
                slot = resolution_counts[family]
                canonical = encode(value)
                record = expected.pop((RESOLUTION_DOMAIN, family, sha(slot.to_bytes(8, "big"))), None)
                require(record is not None and record["canonical_value_sha256"] == sha(canonical)
                        and decimal(record["canonical_byte_length"]) == len(canonical)
                        and record["availability"] == value["availability"],
                        "Resolution retained decision differs from its native commitment")
                resolution_counts[family] += 1
                status_counts[status] += 1
                resolution_frames.append({"source_ordinal": str(ordinal), "kind": "frame", "body_hex": body.hex()})
                resolution_handled.append({"source_ordinal": str(ordinal), "source_carrier": marker,
                                           "disposition": "typed_retained_resolution"})
                resolution_values.append({"source_ordinal": str(ordinal), "physical_global_frontier": str(frontier),
                                          "source_payload_sha256": row["payload_sha256"], **typed})
            else:
                require(not marker.startswith(("CVAEN", "CVAET", "CVAERS", "CVAERL", "CVAEPRN")),
                        "Entity carrier outside reviewed conversion scope")
        require(source.tell() == before.st_size and digest.hexdigest() == owner["source_sha256"]
                and snapshot(before) == snapshot(regular(source_path)), "Entity source differs from the reviewed pin")
    proof = verify_entity(owner, expected, revisions, terminals, current)
    require(not expected, "Entity or Resolution retained commitments were omitted")
    return proof, entity_rows, resolution_frames, resolution_handled, resolution_values, dict(sorted(status_counts.items()))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("source-copies", "source-coverage", "output-directory"):
        parser.add_argument(f"--{name}", type=Path, required=True)
    args = parser.parse_args()
    output = args.source_coverage.parent
    coverage, coverage_sha = read_document(args.source_coverage, 2 * 1024 * 1024)
    manifest, manifest_sha = read_document(output / MANIFEST, 2 * 1024 * 1024)
    require(coverage["schema_id"] == manifest["schema_id"] == SCHEMA_V2
            and coverage_sha == manifest["source_coverage_sha256"], "Entity source witness custody differs")
    owners = {row["source_sha256"]: row for row in coverage["sources"]}
    require(len(coverage["sources"]) == len(PINS) and owners.keys() == set(PINS.values()), "expected six pinned owners")
    destination = args.output_directory.resolve()
    regular(destination.parent, directory=True)
    regular(args.source_copies, directory=True)
    require(not any(destination.is_relative_to(path.resolve()) for path in (output, args.source_copies)),
            "Entity transfer output overlaps protected inputs")
    artifacts = dict(manifest["artifact_sha256"])
    results = [(owners[pin], observe(output, artifacts, owners[pin], args.source_copies / fixture))
               for fixture, pin in sorted(PINS.items())]
    destination.mkdir()
    entity, resolution = [], []
    for owner, (proof, entity_rows, frames, handled, typed, counts) in results:
        copy = owner["copy_id"]
        names = {"frames": f"{copy}-resolution-frames.jsonl", "handled": f"{copy}-resolution-handled.jsonl",
                 "typed": f"{copy}-resolution-typed.jsonl"}
        common = {"copy_id": copy, "source_sha256": owner["source_sha256"], "owner_uuid": owner["owner_uuid"]}
        resolution.append({
            **common, **names, "status_counts": counts,
            **{f"{name}_sha256": write_jsonl(destination / names[name], data)
               for name, data in zip(names, (frames, handled, typed))},
        })
        filename = f"{copy}-entity-transfer.jsonl"
        entity.append({**common, "native_entity_head": proof, "transfer": filename,
                       "transfer_sha256": write_jsonl(destination / filename, entity_rows)})
    for name, scope, sources in (
        ("entity-source-transfer.json", "retained typed source Entity publications and terminals", entity),
        ("resolution-transfer-receipt.json", "full retained active and inert Entity Resolution history", resolution),
    ):
        receipt = {"scope": scope, "source_coverage_sha256": coverage_sha, "source_manifest_sha256": manifest_sha,
                   "target_conversion_verified": False, "sources": sources}
        with (destination / name).open("xb") as file:
            file.write(encode(receipt) + b"\n")


if __name__ == "__main__":
    main()
