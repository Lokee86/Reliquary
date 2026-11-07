"""Export exact typed Memory publications from the six protected F03 2.0 owner copies."""
import argparse
from collections import Counter
import hashlib
from pathlib import Path

from f03_legacy_frame_clock import LegacyFrameClock
from f03_verify_archived_oracle import PINS, require
from f03_verify_dream_journal_frontiers import NATIVE_DOMAIN, Reader, encode, framed, sha, u64
from f03_verify_independent_repeat import (
    MANIFEST, SCHEMA_V2, decimal, open_header, read_document, read_frame, regular, snapshot, take_stream,
)
from f03_verify_prefix_availability import write_jsonl

FIELDS = (
    ("category", "text"), ("memory_type", "text"), ("authority_kind", "text"),
    ("temporal_status", "text"), ("scope", "text"), ("lifecycle_state", "text"), ("archived", "flag"),
    ("superseded_by", "optional_id"), ("parent_id", "optional_id"), ("source_node_id", "optional_text"),
    ("content_source_conversation_id", "optional_text"), ("content_source_node_id", "optional_text"),
    ("grounding_source_conversation_id", "optional_text"), ("grounding_source_node_id", "optional_text"),
    ("source_episode_id", "optional_id"), ("source_time_ns", "optional_signed"), ("source_ref", "source_ref"),
    ("temporal_inference", "temporal"), ("created_at_ns", "signed"), ("updated_at_ns", "signed"),
)
TEMPORAL_KINDS = {1: "explicit", 2: "calendar", 3: "relative", 4: "boundary", 5: "recurrence", 6: "duration"}
STANDALONE = {"CVAMC002", "CVAMD002", "CVAMT002"}
ALLOWED_MEMORY = STANDALONE | {"CVAMBDY1", "CVAMRTE2", "CVAMEMF2", "CVAMVEC1", "CVAMVFM1"}


class Wire(Reader):
    def flag(self):
        flag = self.number(1)
        require(flag in (0, 1), "invalid Memory boolean or optional flag")
        return bool(flag)

    def identity(self):
        return self.take(32).hex()

    def signed(self):
        return int.from_bytes(self.take(8), "little", signed=True)

    def optional_id(self):
        return self.identity() if self.flag() else None

    def optional_signed(self):
        return self.signed() if self.flag() else None

    def optional_text(self):
        return self.text() if self.flag() else None

    def source_ref(self):
        if not self.flag():
            return None
        return {
            "owner_id": self.text(), "principal_id": self.optional_text(), "source_episode_id": self.identity(),
            "source_node_id": self.text(), "content_source_conversation_id": self.optional_text(),
            "content_source_node_id": self.optional_text(), "grounding_source_conversation_id": self.optional_text(),
            "grounding_source_node_id": self.optional_text(),
        }

    def temporal(self):
        if not self.flag():
            return None
        body, at, model, contract = self.identity(), self.optional_signed(), self.text(), self.text()
        count = self.number(1)
        require(0 < count <= 32 and model.strip() and contract.strip(), "invalid Memory temporal inference header")
        resolutions = []
        for _ in range(count):
            start, end, kind = self.number(4), self.number(4), self.number(1)
            evidence, expression = self.text(), self.text()
            require(start < end and kind in TEMPORAL_KINDS and evidence and expression, "invalid temporal inference span")
            resolutions.append({"start": start, "end": end, "kind": TEMPORAL_KINDS[kind],
                                "evidence": evidence, "canonical_expression": expression})
        return {"body_id": body, "source_time_ns": at, "model": model, "contract_version": contract,
                "resolutions": resolutions}


def read_fields(reader, mask):
    require(mask < (1 << len(FIELDS)), "unknown Memory delta field")
    return {name: getattr(reader, codec)() for bit, (name, codec) in enumerate(FIELDS) if mask & (1 << bit)}


def creation(payload, global_):
    reader = Wire(payload)
    require(reader.take(8) == (b"CVAMC001" if global_ is None else b"CVAMC002"), "invalid Memory creation carrier")
    identity, body_id = reader.identity(), reader.identity()
    if global_ is None:
        global_ = reader.number()
    local, mutation = reader.number(), reader.text()
    fields = read_fields(reader, (1 << len(FIELDS)) - 1)
    reader.end()
    return {"id": identity, "body_id": body_id, "revision": 1, "global_version": global_, "memory_version": local,
            "mutation_id": mutation, **fields}


def delta(payload, global_, current):
    reader = Wire(payload, 8)
    identity = reader.identity()
    previous, revision, local, mutation, mask = reader.number(), reader.number(), reader.number(), reader.text(), reader.number()
    fields = read_fields(reader, mask)
    reader.end()
    base = current.get(identity)
    require(base is not None and previous == base["revision"] and revision == previous + 1
            and mutation != base["mutation_id"] and all(base[key] != value for key, value in fields.items()),
            "invalid or noncanonical Memory delta")
    return {**base, **fields, "revision": revision, "global_version": global_, "memory_version": local,
            "mutation_id": mutation}


def body_value(identity, raw):
    reader = Wire(raw)
    title = reader.take(reader.number()).decode()
    content = reader.take(reader.number()).decode()
    reader.end()
    require(sha(raw) == identity, "Memory body does not match its content ID")
    return {"title": title, "content": content}


def routing_value(payload):
    reader = Wire(payload)
    require(reader.take(8) == b"CVAMRTE2", "unexpected Memory routing carrier")
    memory_id, body_id = reader.identity(), reader.identity()
    count = reader.count(13)
    require(count <= 64, "too many Memory mentions")
    mentions = []
    for _ in range(count):
        field, start, end, value = reader.number(1), reader.number(4), reader.number(4), reader.text()
        require(field in (1, 2) and start < end, "invalid Memory routing span")
        mentions.append({"field": field, "start": start, "end": end, "text": value})
    reader.end()
    return {"memory_id": memory_id, "body_id": body_id, "mentions": mentions}


def completion(payload):
    reader = Wire(payload, 8)
    layout = reader.number(1)
    require(layout in (3, 5, 6), "grouped Memory completion layout outside reviewed corpus")
    group = {
        "layout": layout, "episode_id": reader.identity(), "attempt": reader.number(4),
        "started_at_ns": reader.signed(), "completed_at_ns": reader.signed(), "rejected_count": reader.number(4),
        "global_version_start": reader.number(), "global_version_count": reader.number(4),
        "transaction_time_ns": reader.signed() if layout >= 5 else None,
        "extractor_model": reader.text(), "extractor_version": reader.text(),
    }
    group["memory_ids"] = [reader.identity() for _ in range(reader.count(32))]
    group["external_memory_refs"] = [{"owner_id": reader.text(), "memory_id": reader.identity()}
                                     for _ in range(reader.count(36))]
    bodies = [{"body_id": identity, **body_value(identity, reader.blob())}
              for identity in (reader.identity() for _ in range(reader.count(36)))]
    revisions = [creation(reader.blob(), None) for _ in range(reader.count(4))]
    routing = [routing_value(reader.blob()) for _ in range(reader.count(4))] if layout >= 5 else []
    group["freshness_birth"] = None
    if layout == 6:
        turn, policy = reader.number(), reader.number()
        ids = [reader.identity() for _ in range(reader.count(32))]
        require(policy == 1 and len(ids) == len(revisions) and set(ids) == {r["id"] for r in revisions},
                "grouped Freshness birth cohort differs from first revisions")
        group["freshness_birth"] = {"activity": turn, "policy_version": policy, "memory_ids": ids}
    reader.end()
    require(len(revisions) == group["global_version_count"]
            and all(r["global_version"] == group["global_version_start"] + i for i, r in enumerate(revisions)),
            "grouped Memory global allocation differs from accepted records")
    return group, revisions, bodies, routing


class History:
    def __init__(self):
        self.current = {}
        self.bodies = {}
        self.routing = {}
        self.revisions = []

    def accept(self, revisions, bodies, routing, clock):
        for body in bodies:
            identity = body["body_id"]
            value = {key: body[key] for key in ("title", "content")}
            require(self.bodies.setdefault(identity, value) == value, "conflicting Memory body")
        published = []
        for record in revisions:
            identity, body_id = record["id"], record["body_id"]
            require(record["memory_version"] == len(self.revisions) + 1 and record["mutation_id"]
                    and 0 < record["global_version"] <= clock.head and body_id in self.bodies
                    and (record["revision"] == 1) == (identity not in self.current),
                    "Memory history order, body, or identity mismatch")
            stamp = clock.times.get(record["global_version"])
            record = {**record, "transaction_time_ns": None if stamp is None else int(stamp)}
            self.current[identity] = record
            self.revisions.append(record)
            published.append({"record": record, "body": self.bodies[body_id]})
        for row in routing:
            identity = row["memory_id"]
            require(row["body_id"] in self.bodies and self.routing.setdefault(identity, row) == row,
                    "conflicting Memory routing or missing body")
            for mention in row["mentions"]:
                field = "title" if mention["field"] == 1 else "content"
                raw = self.bodies[row["body_id"]][field].encode()
                require(mention["end"] <= len(raw)
                        and raw[mention["start"]:mention["end"]].decode() == mention["text"],
                        "Memory routing does not match literal body bytes")
        return published

    def native_rows(self):
        used_bodies = {row["body_id"] for row in self.revisions}
        require(used_bodies == self.bodies.keys() and self.routing.keys() <= self.current.keys(),
                "unrecognized orphaned Memory body or routing")
        for body_id, body in self.bodies.items():
            yield "body", bytes.fromhex(body_id), body
        for identity, record in self.current.items():
            route = self.routing.get(identity)
            require(route is None or route["body_id"] == record["body_id"], "routing points to a different Memory body")
            key = bytes.fromhex(identity)
            yield "current", key, record["revision"]
            yield "routing", key, route
        for record in self.revisions:
            yield "revision", record["memory_version"].to_bytes(8, "big"), record


def observe(output, artifacts, owner, path):
    kind = {"reliquary": 1, "phylactery": 2}[owner["native"]["owner_kind"]]
    history, clock = History(), LegacyFrameClock()
    transfer, carriers = [], Counter()
    before = regular(path)
    with path.open("rb") as source:
        digest = open_header(source, kind, owner)
        for ordinal, frame in take_stream(output, artifacts, owner, "frames", owner["frame_stream"], schema_id=SCHEMA_V2):
            payload = read_frame(source, frame, digest, before.st_size)
            frontier = clock.consume(payload)
            require(frontier == decimal(frame["latest_global"]), "Memory physical frontier differs from the witness")
            marker = frame["carrier"]
            revisions, bodies, routing = [], [], []
            group, clock_binding = None, None
            if marker in STANDALONE:
                require(payload.startswith(b"CVAPUB01"), "standalone Memory revision lacks accepted envelope")
                mode = payload[16]
                at = 25 if mode == 1 else 17
                global_ = int.from_bytes(payload[8:16], "little")
                clock_binding = {"global_version": global_, "clock_mode": "bind_reserved" if mode == 2 else "allocate",
                                 "transaction_time_ns_in_envelope": int.from_bytes(payload[17:25], "little", signed=True)
                                 if mode == 1 else None}
                if marker == "CVAMC002":
                    revisions = [creation(payload[at:], global_)]
                else:
                    require(marker == "CVAMD002", "Memory tombstone outside reviewed conversion cohort")
                    revisions = [delta(payload[at:], global_, history.current)]
            elif marker == "CVAMBDY1":
                reader = Wire(payload, 8)
                identity = reader.identity()
                bodies = [{"body_id": identity, **body_value(identity, reader.blob())}]
                reader.end()
            elif marker == "CVAMRTE2":
                routing = [routing_value(payload)]
            elif marker == "CVAINSC8":
                require(kind == 1, "grouped Insomnia Memory in a PHY owner")
                group, revisions, bodies, routing = completion(payload)
            else:
                require(not marker.startswith(("CVAM", "CVAXMEM", "CVAINSC")) or marker in ALLOWED_MEMORY,
                        "Memory carrier outside reviewed conversion cohort")
                continue
            carriers[marker] += 1
            transfer.append({
                "physical_ordinal": ordinal, "physical_end_offset": source.tell(), "physical_global_frontier": frontier,
                "carrier": marker, "source_payload_sha256": frame["payload_sha256"],
                "clock_binding": clock_binding, "group": group, "bodies": bodies, "routing": routing,
                "revisions": history.accept(revisions, bodies, routing, clock),
            })
        require(source.tell() == before.st_size and digest.hexdigest() == owner["source_sha256"]
                and snapshot(before) == snapshot(regular(path)), "source differs from the pinned Memory owner")
    return history, transfer, dict(sorted(carriers.items()))


def verify_memory(output, artifacts, owner, history):
    native = owner["native"]
    expected = {}
    for _, row in take_stream(output, artifacts, owner, "records", native["record_stream"], schema_id=SCHEMA_V2):
        if row["projection_domain"] == "memory":
            key = row["family"], row["stable_key_sha256"]
            require(key not in expected and row["availability"] == "retained", "invalid native Memory row")
            expected[key] = row["canonical_value_sha256"], decimal(row["canonical_byte_length"])
    rows = sorted(history.native_rows())
    head = len(history.revisions)
    digest = hashlib.sha256(NATIVE_DOMAIN + b"\1\0\0\0" + bytes.fromhex(owner["owner_uuid"])
                            + framed(b"memory") + u64(head) + b"\0" + u64(len(rows)))
    for family, key, value in rows:
        data = encode(value)
        require(expected.pop((family, sha(key)), None) == (sha(data), len(data)),
                f"typed Memory {family} differs from its native commitment")
        digest.update(framed(family.encode()) + framed(key) + framed(data))
    proof = next(row for row in native["native_history_projections"] if row["domain"] == "memory")
    require(not expected and proof == {"domain": "memory", "local_cut": str(head), "event_cut": None,
                                      "rows": str(len(rows)), "sha256": digest.hexdigest()},
            "independent retained Memory projection differs from its native head")
    return proof


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("source-copies", "source-coverage", "output-directory"):
        parser.add_argument(f"--{name}", type=Path, required=True)
    args = parser.parse_args()
    output = args.source_coverage.parent
    coverage, coverage_sha = read_document(args.source_coverage, 2 * 1024 * 1024)
    manifest, manifest_sha = read_document(output / MANIFEST, 2 * 1024 * 1024)
    require(coverage["schema_id"] == manifest["schema_id"] == SCHEMA_V2
            and coverage_sha == manifest["source_coverage_sha256"], "source witness custody differs")
    owners = {row["source_sha256"]: row for row in coverage["sources"]}
    require(len(coverage["sources"]) == len(PINS) and owners.keys() == set(PINS.values()),
            "Memory transfer requires the exact six reviewed owner pins")
    destination = args.output_directory.resolve()
    regular(destination.parent, directory=True)
    regular(args.source_copies, directory=True)
    require(not any(destination.is_relative_to(p.resolve()) for p in (output, args.source_copies)),
            "Memory transfer destination overlaps protected inputs")
    artifacts = dict(manifest["artifact_sha256"])
    results = []
    for relative, pin in sorted(PINS.items()):
        owner = owners[pin]
        history, transfer, carriers = observe(output, artifacts, owner, args.source_copies / relative)
        proof = verify_memory(output, artifacts, owner, history)
        results.append(({
            "copy_id": owner["copy_id"], "fixture": relative, "source_sha256": pin, "owner_uuid": owner["owner_uuid"],
            "owner_kind": owner["native"]["owner_kind"], "history_generation": owner["native"]["history_generation"],
            "native_memory_head": proof, "carriers": carriers, "transfer_rows": len(transfer),
            "literal_memory_bodies": len(history.bodies), "routing_records": len(history.routing),
        }, transfer))
    destination.mkdir()
    for summary, transfer in results:
        summary["transfer_sha256"] = write_jsonl(destination / f'{summary["copy_id"]}-memory-transfer.jsonl', transfer)
    receipt = {
        "schema_id": "reliquary-f03-memory-source-transfer-v1", "scope": "six exact retained source Memory histories",
        "source_manifest_sha256": manifest_sha, "source_coverage_sha256": coverage_sha,
        "target_conversion_verified": False, "sources": [summary for summary, _ in results],
    }
    with (destination / "memory-source-transfer.json").open("xb") as file:
        file.write(encode(receipt) + b"\n")


if __name__ == "__main__":
    main()
