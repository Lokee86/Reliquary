"""Verify the exact pinned Insomnia journal and its clock-neutral frontier state."""
import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path

from f03_legacy_frame_clock import LegacyFrameClock
from f03_verify_archived_oracle import PINS, require
from f03_verify_independent_repeat import (
    MANIFEST, SCHEMA_V2, decimal, open_header, read_document, read_frame, regular, snapshot, take_stream,
)


FORMAT = b"CVAINSF1"
WORK = b"CVAINSW1"
COMPLETION = b"CVAINSC8"
NATIVE = b"reliquary:historical-owner-state:v1\0"
DOMAIN = "insomnia_import_journal"
PREFIX = b"reliquary:f03:insomnia-retained-journal:v1\0"
CURRENT = b"reliquary:f03:insomnia-current:v1\0"


def encode(value):
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode("utf-8")


def sha(value):
    return hashlib.sha256(value).hexdigest()


def framed(value):
    return len(value).to_bytes(8, "little") + value


class Wire:
    def __init__(self, data):
        self.data = data
        self.offset = 0

    def take(self, size):
        end = self.offset + size
        require(end <= len(self.data), "truncated Insomnia value")
        value = self.data[self.offset:end]
        self.offset = end
        return value

    def number(self, size=8, *, signed=False):
        return int.from_bytes(self.take(size), "little", signed=signed)

    def text(self):
        return self.blob().decode("utf-8")

    def blob(self):
        return self.take(self.number(4))

    def optional(self, read):
        flag = self.number(1)
        require(flag in (0, 1), "invalid Insomnia option flag")
        return read() if flag else None

    def items(self, minimum, read):
        count = self.number(4)
        require(count <= (len(self.data) - self.offset) // minimum, "truncated Insomnia item list")
        return [read() for _ in range(count)]

    def ids(self):
        return self.items(32, lambda: list(self.take(32)))

    def end(self):
        require(self.offset == len(self.data), "Insomnia carrier has trailing bytes")


def read_work(reader):
    result = {
        "episode_id": list(reader.take(32)), "priority": reader.number(1), "state": reader.number(1),
        "attempt_count": reader.number(4), "updated_at_ns": reader.number(signed=True),
        "lease_owner": reader.optional(reader.text),
        "lease_token": reader.optional(lambda: list(reader.take(32))),
        "lease_expires_ns": reader.optional(lambda: reader.number(signed=True)),
        "retry_after_ns": reader.optional(lambda: reader.number(signed=True)),
        "last_error": reader.optional(reader.text),
    }
    require(1 <= result["priority"] <= 3 and 1 <= result["state"] <= 5, "invalid Insomnia work tag")
    return result


def body_value(reader):
    identity = reader.take(32)
    value = reader.blob()
    require(hashlib.sha256(value).digest() == identity, "grouped Memory body identity differs")
    return {"body_id": list(identity), "bytes": len(value), "body_sha256": identity.hex()}


def memory_link(reader):
    record = Wire(reader.blob())
    require(record.take(8) == b"CVAMC001", "grouped Insomnia record is not a compact Memory creation")
    identity, body_id = record.take(32), record.take(32)
    global_version, memory_version = record.number(), record.number()
    require(global_version > 0 and memory_version > 0 and bool(record.text())
            and record.offset < len(record.data), "invalid grouped Memory creation prefix")
    # Only the creation identity and version links belong to the journal row.
    return {"memory_id": list(identity), "revision": 1, "body_id": list(body_id),
            "global_version": global_version, "memory_version": memory_version}


def route_value(reader):
    route = Wire(reader.blob())
    require(route.take(8) == b"CVAMRTE2", "unknown grouped routing carrier")
    memory, body = route.take(32), route.take(32)
    mentions = route.items(13, lambda: {
        "field": route.number(1), "start_byte": route.number(4),
        "end_byte": route.number(4), "text": route.text(),
    })
    require(len(mentions) <= 64 and all(mention["field"] in (1, 2) for mention in mentions),
            "invalid grouped Memory mention shape")
    route.end()
    return {"memory_id": list(memory), "body_id": list(body), "mentions": mentions}


def external_reference(reader):
    owner = reader.text()
    require(bool(owner.strip()), "empty external Memory owner")
    return {"owner_id": owner, "memory_id": list(reader.take(32))}


def read_completion(reader):
    layout = reader.number(1)
    require(layout in (3, 5, 6), "Insomnia completion shape outside the pinned population")
    value = {
        "episode_id": list(reader.take(32)), "attempt": reader.number(4),
        "started_at_ns": reader.number(signed=True), "completed_at_ns": reader.number(signed=True),
        "rejected_count": reader.number(4), "global_start": reader.number(), "global_count": reader.number(4),
        "transaction_time_ns": reader.number(signed=True) if layout >= 5 else None,
        "extractor_model": reader.text(), "extractor_version": reader.text(), "memory_ids": reader.ids(),
        "external_memory_refs": reader.items(36, lambda: external_reference(reader)),
        "bodies": reader.items(36, lambda: body_value(reader)),
        "memory_revision_links": reader.items(4, lambda: memory_link(reader)),
        "routing": reader.items(4, lambda: route_value(reader)) if layout >= 5 else [],
        "freshness_birth_turn": reader.number() if layout == 6 else None,
        "freshness_policy_version": reader.number() if layout == 6 else None,
        "freshness_birth_memory_ids": reader.ids() if layout == 6 else [],
    }
    links = value["memory_revision_links"]
    require(value["attempt"] > 0 and len(value["routing"]) <= 64
            and len(links) == value["global_count"]
            and all(link["global_version"] == value["global_start"] + index
                    for index, link in enumerate(links)), "invalid grouped Insomnia publication coordinates")
    if layout == 6:
        birth_ids = [bytes(identity) for identity in value["freshness_birth_memory_ids"]]
        require(value["freshness_policy_version"] == 1 and len(birth_ids) == len(set(birth_ids))
                and set(birth_ids) == {bytes(link["memory_id"]) for link in links},
                "grouped Freshness birth cohort differs from first revisions")
    return value


def decode(payload, carrier):
    marker = payload[:8]
    if marker not in (FORMAT, WORK, COMPLETION):
        require(not carrier.startswith(("CVAINS", "CVAIEP", "CVAIFR")),
                "unexpected Insomnia attempt or import-epoch carrier")
        return None
    reader = Wire(payload)
    reader.take(8)
    if marker == FORMAT:
        family, value = "format", {"schema": "CVAINSF1"}
    elif marker == WORK:
        family, value = "work", read_work(reader)
    else:
        family, value = "completion", read_completion(reader)
    reader.end()
    return family, value


def expected_records(output, artifacts, owner):
    expected = {}
    for _, row in take_stream(output, artifacts, owner, "records", owner["native"]["record_stream"],
                              schema_id=SCHEMA_V2):
        if row["projection_domain"] == DOMAIN:
            key = row["family"], row["stable_key_sha256"]
            require(key not in expected, "duplicate native Insomnia commitment")
            expected[key] = row
    return expected


def native_head(uuid, rows):
    digest = hashlib.sha256(NATIVE + (1).to_bytes(4, "little") + uuid + framed(DOMAIN.encode()))
    digest.update(b"\0" * 9 + len(rows).to_bytes(8, "little"))
    for family, ordinal, canonical in sorted(rows):
        digest.update(framed(family.encode()) + framed(ordinal) + framed(canonical))
    return digest.hexdigest()


def scan(owner, path, output, artifacts, kind):
    before = regular(path)
    require(before.st_size == decimal(owner["source_byte_length"]), "Insomnia source length differs")
    expected = expected_records(output, artifacts, owner)
    clock = LegacyFrameClock()
    journal, native_rows = [], []
    counts = Counter()
    with path.open("rb") as source:
        source_hash = open_header(source, kind, owner)
        for ordinal, frame in take_stream(output, artifacts, owner, "frames", owner["frame_stream"],
                                          schema_id=SCHEMA_V2):
            payload = read_frame(source, frame, source_hash, before.st_size)
            cut = clock.consume(payload)
            require(str(cut) == frame["latest_global"], "Insomnia physical clock differs")
            decoded = decode(payload, frame["carrier"])
            if decoded is None:
                continue
            family, value = decoded
            require(kind == 1 and (family != "format" or not counts["format"]),
                    "Insomnia journal in wrong owner or duplicate format")
            sequence = len(journal)
            key = sequence.to_bytes(8, "big")
            canonical = encode({"accepted_journal_ordinal": sequence,
                                "accepted_global_frontier": cut, "record": value})
            row = {
                "projection_domain": DOMAIN, "family": family, "stable_key_sha256": sha(key),
                "canonical_value_sha256": sha(canonical), "canonical_byte_length": str(len(canonical)),
                "origin_coordinate": {}, "availability": "retained",
            }
            require(expected.pop((family, sha(key)), None) == row,
                    f"Insomnia record differs from native commitment at frame {ordinal}")
            retained = {
                **row, "accepted_journal_ordinal": str(sequence), "first_global": str(cut),
                "physical_ordinal": str(ordinal),
            }
            native_rows.append((family, key, canonical))
            journal.append((cut, family, value, retained))
            counts[family] += 1
        require(source.tell() == before.st_size and source_hash.hexdigest() == owner["source_sha256"]
                and snapshot(before) == snapshot(regular(path)), "Insomnia source differs from pins")
    require(not expected and clock.head + 1 == decimal(owner["native"]["global_stream"]["rows"]),
            "Insomnia commitment or global cut population differs")
    proofs = [row for row in owner["native"]["native_history_projections"] if row["domain"] == DOMAIN]
    digest = native_head(bytes.fromhex(owner["owner_uuid"]), native_rows) if kind == 1 else None
    if kind == 1:
        require(counts["format"] == 1 and len(proofs) == 1 and proofs[0]["local_cut"] == "0"
                and proofs[0]["event_cut"] is None and proofs[0]["rows"] == str(len(journal))
                and proofs[0]["sha256"] == digest, "Insomnia native head digest differs")
    else:
        require(not proofs and not journal, "PHY has a retained Insomnia projection")
    return clock.head, journal, counts, digest


def apply(family, value, work, attempts):
    if family == "format":
        return
    episode = bytes(value["episode_id"])
    if family == "work":
        work[episode] = value
        return
    previous = work.get(episode)
    if previous is not None:
        require(value["attempt"] >= previous["attempt_count"], "grouped Insomnia attempt regressed")
        work[episode] = {
            **previous, "state": 3, "attempt_count": value["attempt"],
            "lease_owner": None, "lease_token": None, "lease_expires_ns": None,
            "retry_after_ns": None, "last_error": None, "updated_at_ns": value["completed_at_ns"],
        }
    attempts.pop(episode, None)
    attempts[episode] = {key: value[key] for key in (
        "episode_id", "attempt", "started_at_ns", "completed_at_ns", "extractor_model",
        "extractor_version", "memory_ids", "external_memory_refs", "rejected_count",
    )} | {"state": 3, "error": None}


def frontier(uuid, head, journal):
    work, attempts = {}, {}
    digest = hashlib.sha256(PREFIX + uuid)
    state = sha(CURRENT + uuid + encode({"work": [], "attempts": []}))
    position = 0
    for cut in range(head + 1):
        start = position
        while position < len(journal) and journal[position][0] == cut:
            _, family, value, retained = journal[position]
            digest.update(framed(encode(retained)))
            apply(family, value, work, attempts)
            position += 1
        if start != position:
            state = sha(CURRENT + uuid + encode({
                "work": [item for _, item in sorted(work.items())], "attempts": list(attempts.values()),
            }))
        yield {"global_version": str(cut), "retained_rows": str(position), "retained_prefix_sha256": digest.hexdigest(),
               "work_rows": str(len(work)), "completed_attempt_rows": str(len(attempts)), "current_state_sha256": state}
    require(position == len(journal), "Insomnia journal contains an uncovered physical frontier")


def write_rows(path, rows):
    digest = hashlib.sha256()
    count, last = 0, None
    with path.open("xb") as output:
        for row in rows:
            line = encode(row) + b"\n"
            output.write(line)
            digest.update(line)
            count += 1
            last = row
    return {"file": path.name, "sha256": digest.hexdigest(), "rows": count}, last


def observe(owner, path, output, artifacts, private, kind):
    head, journal, counts, digest = scan(owner, path, output, artifacts, kind)
    copy = owner["copy_id"]
    retained, _ = write_rows(private / f"{copy}-insomnia-journal.jsonl", (row[3] for row in journal))
    cuts, final = write_rows(private / f"{copy}-insomnia-cuts.jsonl",
                             frontier(bytes.fromhex(owner["owner_uuid"]), head, journal))
    completions = [value for _, family, value, _ in journal if family == "completion"]
    return {
        "copy_id": copy, "source_sha256": owner["source_sha256"], "native_head_sha256": digest,
        "carrier_rows": dict(counts), "absent_from_pinned_owner_bytes": ["standalone_attempt", "native_import_epoch"],
        "grouped_freshness_birth_cohorts": sum(value["freshness_birth_turn"] is not None for value in completions),
        "grouped_memory_revision_links": sum(len(value["memory_revision_links"]) for value in completions),
        "retained": retained, "cuts": cuts, "final_cut": final,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-copies", type=Path, required=True)
    parser.add_argument("--source-coverage", type=Path, required=True)
    parser.add_argument("--private-output", type=Path, required=True)
    args = parser.parse_args()
    output = args.source_coverage.parent
    coverage, coverage_sha = read_document(args.source_coverage, 2 * 1024 * 1024)
    manifest, manifest_sha = read_document(output / MANIFEST, 2 * 1024 * 1024)
    require(coverage["schema_id"] == manifest["schema_id"] == SCHEMA_V2
            and manifest["source_coverage_sha256"] == coverage_sha, "invalid v2 source evidence provenance")
    owners = {owner["source_sha256"]: owner for owner in coverage["sources"]}
    require(len(coverage["sources"]) == len(PINS) and owners.keys() == set(PINS.values()),
            "Insomnia cohort differs from reviewed pins")
    fixtures = sorted(PINS.items())
    for index, (_, digest) in enumerate(fixtures, 1):
        require(owners[digest]["copy_id"] == f"copy-{index:04d}", "unexpected Insomnia copy identifier")
    require(not args.private_output.resolve().is_relative_to(args.source_copies.resolve(strict=True)),
            "Insomnia output must be outside source copies")
    regular(args.private_output.parent, directory=True)
    args.private_output.mkdir()
    artifacts = manifest["artifact_sha256"].copy()
    results = [observe(owners[digest], args.source_copies / fixture, output, artifacts,
                       args.private_output, 1 if fixture.endswith(".rel") else 2) for fixture, digest in fixtures]
    print(json.dumps({
        "schema_id": "reliquary-f03-insomnia-frontier-verification-v1",
        "status": "PASS", "authentic_admission": False,
        "scope": "retained Insomnia journal and clock-neutral work/attempt state in pinned owner bytes",
        "verifier_sha256": sha(Path(__file__).read_bytes()),
        "source_coverage_sha256": coverage_sha, "source_manifest_sha256": manifest_sha,
        "retained_rows": sum(result["retained"]["rows"] for result in results),
        "global_cuts": sum(result["cuts"]["rows"] for result in results), "owners": results,
    }, indent=2))


if __name__ == "__main__":
    main()
