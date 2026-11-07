"""Reconstruct retained Dream journal proofs from the pinned historical copies."""
import argparse
from bisect import bisect_left
from collections import Counter
import hashlib
import json
from pathlib import Path

from f03_legacy_frame_clock import LegacyFrameClock
from f03_verify_archived_oracle import PINS, require
from f03_verify_independent_repeat import (
    MANIFEST, SCHEMAS, open_header, read_document, read_frame, regular, snapshot, take_stream,
)

NATIVE_DOMAIN = b"reliquary:historical-owner-state:v1\0"
POPULATION_DOMAIN = b"reliquary:dream-population:v1\0"
TRANSITION_DOMAIN = b"reliquary:f03:dream-journal-transitions:v1\0"
BIRTH_DOMAIN = b"reliquary:f03:dream-journal-grouped-births:v1\0"
JOURNAL_TAGS = {1: "roots", 2: "settled", 6: "replay_settled", 8: "accept_graph"}
OTHER_PUBLICATIONS = {b"CVAGCB02", b"CVAGCM02", b"CVAENC02", b"CVAEND02", b"CVAETM02"}
OTHER_ARCHIVE = {b"CVABRC02", b"CVACON04", b"CVAEPI02", b"CVAFRA02"}


def encode(value):
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()


def u64(value):
    return value.to_bytes(8, "little")


def framed(value):
    return u64(len(value)) + value


def sha(value):
    return hashlib.sha256(value).hexdigest()


class Reader:
    def __init__(self, value, offset=0):
        self.value = value
        self.offset = offset

    def take(self, count):
        end = self.offset + count
        require(end <= len(self.value), "truncated Dream proof carrier")
        result = self.value[self.offset:end]
        self.offset = end
        return result

    def number(self, size=8):
        return int.from_bytes(self.take(size), "little")

    def count(self, minimum_size):
        count = self.number(4)
        require(count <= (len(self.value) - self.offset) // minimum_size, "truncated Dream proof list")
        return count

    def blob(self):
        return self.take(self.number(4))

    def text(self):
        return self.blob().decode("utf-8")

    def ids(self):
        return [self.take(32) for _ in range(self.count(32))]

    def end(self):
        require(self.offset == len(self.value), "trailing Dream proof carrier bytes")


def creation_prefix(data):
    reader = Reader(data)
    require(reader.take(8) == b"CVAMC001", "unsupported grouped first Memory carrier")
    identity = reader.take(32)
    reader.take(32)  # Body ID. The birth proof does not observe body fields.
    global_ = reader.number()
    local = reader.number()
    mutation = reader.text()
    require(bool(mutation) and reader.offset < len(data), "invalid first Memory identity")
    return identity, global_, local, sha(mutation.encode())


def decode_completion(payload):
    reader = Reader(payload, 8)
    layout = reader.number(1)
    require(layout in (3, 5, 6), "grouped Memory carrier outside pinned Dream proof scope")
    reader.take(56)  # Episode, attempt, started, completed and rejected fields.
    start, count = reader.number(), reader.number(4)
    if layout >= 5:
        reader.take(8)
    reader.text()  # Extractor identity.
    reader.text()  # Extractor version.
    reader.ids()
    for _ in range(reader.count(36)):
        require(bool(reader.text().strip()), "empty external Memory owner")
        reader.take(32)
    for _ in range(reader.count(36)):
        reader.take(32)
        reader.blob()
    records = [creation_prefix(reader.blob()) for _ in range(reader.count(4))]
    require(len(records) == count and all(g == start + i and g > 0 for i, (_, g, _, _) in enumerate(records)),
            "invalid grouped first Memory global coordinates")
    if layout >= 5:
        routing_count = reader.count(4)
        require(routing_count <= 64, "too many grouped routing records")
        for _ in range(routing_count):
            require(reader.blob().startswith(b"CVAMRTE2"), "unexpected grouped routing carrier")
    turn, births = None, set()
    if layout == 6:
        turn = reader.number()
        require(reader.number() == 1, "unsupported grouped birth policy")
        ids = reader.ids()
        births = set(ids)
        require(len(births) == len(ids) == len(records)
                and births == {identity for identity, *_ in records}, "grouped birth identity differs")
    reader.end()
    return start, records, turn, births


class Population:
    def __init__(self, uuid):
        self.memory_head = 0
        self.first = {}
        self.revisions = {}
        self.births = {}
        self.turn_ids = set()
        self.activity_globals = []
        self.archive_local = 0
        self.archive_global = 0
        self.cohorts = 0
        self.birth_chain = hashlib.sha256(BIRTH_DOMAIN + uuid)
        self.birth_frontiers = {}

    def publish(self, payload):
        global_ = int.from_bytes(payload[8:16], "little")
        body = payload[25 if payload[16] == 1 else 17:]
        marker = body[:8]
        if marker == b"CVAMD002":
            reader = Reader(body, 8)
            identity = reader.take(32)
            previous, revision, local = reader.number(), reader.number(), reader.number()
            require(self.revisions.get(identity) == previous and revision == previous + 1
                    and local == self.memory_head + 1, "Memory delta does not extend the physical prefix")
            self.revisions[identity] = revision
            self.memory_head = local
        elif marker == b"CVATUR03" or marker in OTHER_ARCHIVE:
            reader = Reader(body, 8)
            local = reader.number()
            require(local == self.archive_local + 1 and global_ > self.archive_global,
                    "Archive order differs from pinned Dream proof scope")
            self.archive_local, self.archive_global = local, global_
            if marker == b"CVATUR03":
                reader.take(40)
                node, conversation = reader.text(), reader.text()
                reader.text()  # Parent.
                role = reader.text()
                identity = conversation, node
                require(identity not in self.turn_ids, "repeated Archive turn outside pinned Dream proof scope")
                self.turn_ids.add(identity)
                if role in ("user", "assistant"):
                    self.activity_globals.append(global_)
        elif marker not in OTHER_PUBLICATIONS:
            raise ValueError("standalone publication outside pinned REL Dream proof scope")

    def group(self, payload, ordinal, frontier):
        start, records, turn, births = decode_completion(payload)
        if turn is not None:
            require(turn <= len(self.activity_globals)
                    and (not births or turn == bisect_left(self.activity_globals, start)),
                    "grouped birth activity cut differs from accepted Archive prefix")
            self.cohorts += 1
            proof = {"ordinal": ordinal, "global_frontier": frontier, "global_start": start,
                     "turn": turn, "members": [(sha(i), g, l, m) for i, g, l, m in records]}
            self.birth_chain.update(framed(encode(proof)))
            self.birth_frontiers[frontier] = (ordinal, self.birth_chain.hexdigest())
        for identity, global_, local, _ in records:
            require(identity not in self.first and local == self.memory_head + 1,
                    "grouped first Memory does not extend the physical prefix")
            self.memory_head = local
            self.first[identity] = global_
            self.revisions[identity] = 1
            if identity in births:
                self.births[identity] = ordinal

    def materialize(self, source, cut):
        require(cut > 0 and cut == self.memory_head and source in self.first,
                "Dream Begin disagrees with its physical Memory prefix")
        cohort = self.births.get(source)
        return sorted(identity for identity, origin in self.births.items()
                      if origin != cohort and self.first[identity] < self.first[source])


class Journal:
    def __init__(self, uuid):
        self.uuid = uuid
        self.pending = {}
        self.accepted = {}
        self.settled = set()
        self.completed = {}
        self.counts = Counter()
        self.chain = hashlib.sha256(TRANSITION_DOMAIN + uuid)
        self.frontiers = {}

    def admit(self, payload, population, ordinal, frontier):
        marker = payload[:8]
        reader = Reader(payload, 8)
        require(reader.take(16) == self.uuid, "Dream journal owner mismatch")
        tag = reader.number(1)
        pass_id = reader.text()
        require(bool(pass_id), "empty Dream pass identity")
        if marker == b"CVDREAM2":
            require(tag == 0, "noncompact Dream Begin outside pinned scope")
            source = reader.take(32)
            turn, graph, cut = reader.number(), reader.number(), reader.number()
            expected = reader.take(32)
            ids = population.materialize(source, cut)
            actual = hashlib.sha256(POPULATION_DOMAIN + u64(len(ids)) + b"".join(ids)).digest()
            require(actual == expected, "compact Dream population digest differs from physical birth prefix")
            state = {
                "pass_id": pass_id, "source_id": list(source), "first_revision": 1,
                "turn": turn, "accepted_settlement_turn": None,
                "preexisting_ids": [list(identity) for identity in ids],
                "graph_version_before": graph, "roots": [],
            }
            if pass_id in self.pending:
                existing = self.pending[pass_id]
                keys = state.keys() - {"roots", "accepted_settlement_turn"}
                require(all(existing[key] == state[key] for key in keys), "conflicting Dream Begin identity")
            else:
                self.pending[pass_id] = state
            kind = "compact_begin"
            detail = [sha(source), cut, population.first[source], actual.hex(), turn, graph]
        else:
            require(marker == b"CVDREAM1" and tag in JOURNAL_TAGS and bool(pass_id.strip()),
                    "Dream journal tag outside pinned scope")
            kind = JOURNAL_TAGS[tag]
            state = self.pending.get(pass_id)
            detail = None
            if kind == "roots":
                roots = reader.ids()
                require(state is not None and len(set(roots)) == len(roots)
                        and set(roots) <= {bytes(identity) for identity in state["preexisting_ids"]},
                        "Dream roots differ from the begun population")
                state["roots"] = [list(identity) for identity in
                                  sorted(set(roots) | {bytes(identity) for identity in state["roots"]})]
                detail = [sha(identity) for identity in sorted(roots)]
            elif kind == "accept_graph":
                turn, graph, proof_graph = reader.number(), reader.number(), reader.number()
                has_community = reader.number(1)
                require(has_community in (0, 1), "invalid Dream Community proof flag")
                generation, community_graph = (reader.number(), reader.number()) if has_community else (None, None)
                proof = [pass_id, graph, proof_graph, generation, community_graph]
                require(state is not None and turn >= state["turn"] and graph >= state["graph_version_before"]
                        and community_graph in (None, proof_graph)
                        and state["accepted_settlement_turn"] in (None, turn)
                        and self.accepted.get(pass_id, proof) == proof, "inconsistent Dream graph acceptance")
                state["accepted_settlement_turn"] = turn
                self.accepted[pass_id] = proof
                detail = [turn, *proof[1:]]
            else:
                if kind == "settled":
                    require(state is not None and state["accepted_settlement_turn"] is not None
                            and pass_id in self.accepted, "Dream settlement lacks accepted graph evidence")
                    source = bytes(state["source_id"])
                    require(self.completed.get(source, state) == state, "conflicting completed Dream proof")
                    self.completed[source] = state
                    detail = [sha(source), sha(encode(state))]
                self.pending.pop(pass_id, None)
                self.settled.add(pass_id)
        reader.end()
        self.counts[kind] += 1
        self.chain.update(framed(encode([ordinal, frontier, kind, sha(pass_id.encode()), detail])))
        self.frontiers[frontier] = (ordinal, self.chain.hexdigest())

    def native_rows(self, population):
        yield "resume_state", b"current", {
            "pending": [self.pending[key] for key in sorted(self.pending)],
            "settled": sorted(self.settled),
            "pending_initializations": [],
            "publication_births": [list(identity) for identity in sorted(population.births)],
            "accepted_graph_cuts": [self.accepted[key] for key in sorted(self.accepted)],
        }
        for identity, value in self.completed.items():
            yield "completed", identity, value


def verify_native(output, artifacts, owner, schema, kind, journal, population):
    native = owner["native"]
    expected = {}
    for _, row in take_stream(output, artifacts, owner, "records", native["record_stream"], schema_id=schema):
        if row["projection_domain"] == "dream_proof":
            key = row["family"], row["stable_key_sha256"]
            require(key not in expected and row["availability"] == "retained", "invalid native Dream record")
            expected[key] = row["canonical_value_sha256"], row["canonical_byte_length"]
    projections = [row for row in native["native_history_projections"] if row["domain"] == "dream_proof"]
    if kind == 2:
        require(not projections and not expected, "PHY contains a native Dream proof projection")
        return None
    rows = sorted(journal.native_rows(population))
    digest = hashlib.sha256(NATIVE_DOMAIN + b"\1\0\0\0" + journal.uuid + framed(b"dream_proof")
                            + b"\0" * 9 + u64(len(rows)))
    for family, key, value in rows:
        canonical = encode(value)
        require(expected.pop((family, sha(key)), None) == (sha(canonical), str(len(canonical))),
                f"physical Dream {family} differs from its typed native record")
        digest.update(framed(family.encode()) + framed(key) + framed(canonical))
    require(not expected and len(projections) == 1 and projections[0]["rows"] == str(len(rows))
            and projections[0]["sha256"] == digest.hexdigest(), "Dream projection differs from native head")
    return digest.hexdigest()


def observe(output, artifacts, owner, schema, path, kind):
    uuid = bytes.fromhex(owner["owner_uuid"])
    clock = LegacyFrameClock()
    population, journal = Population(uuid), Journal(uuid)
    before = regular(path)
    with path.open("rb") as source:
        source_hash = open_header(source, kind, owner)
        frames = take_stream(output, artifacts, owner, "frames", owner["frame_stream"], schema_id=schema)
        for ordinal, frame in frames:
            payload = read_frame(source, frame, source_hash, before.st_size)
            frontier = clock.consume(payload)
            require(str(frontier) == frame["latest_global"], "Dream physical global frontier differs")
            marker = payload[:8]
            if marker.startswith(b"CVDREAM"):
                require(kind == 1, "unexpected Dream journal in PHY")
                journal.admit(payload, population, ordinal, frontier)
            elif kind == 1:
                if marker == b"CVAINSC8":
                    population.group(payload, ordinal, frontier)
                elif marker == b"CVAPUB01":
                    population.publish(payload)
                else:
                    require(not marker.startswith((b"CVAMC", b"CVAMD", b"CVAMT", b"CVATUR", b"CVANOD")),
                            "unversioned first Memory or Archive activity carrier outside pinned scope")
        require(source.tell() == before.st_size and source_hash.hexdigest() == owner["source_sha256"]
                and snapshot(before) == snapshot(regular(path)), "Dream source differs from its pins")
    digest = verify_native(output, artifacts, owner, schema, kind, journal, population)
    cut_chain = hashlib.sha256(TRANSITION_DOMAIN + b"cuts\0" + uuid)
    event = (None, hashlib.sha256(TRANSITION_DOMAIN + uuid).hexdigest())
    birth = (None, hashlib.sha256(BIRTH_DOMAIN + uuid).hexdigest())
    for cut in range(clock.head + 1):
        event = journal.frontiers.get(cut, event)
        birth = population.birth_frontiers.get(cut, birth)
        cut_chain.update(framed(encode([cut, event, birth])))
    return {
        "source_sha256": owner["source_sha256"], "global_frontiers": clock.head + 1,
        "event_counts": dict(journal.counts), "completed": len(journal.completed),
        "pending": len(journal.pending), "accepted_graph_cuts": len(journal.accepted),
        "distinct_settled": len(journal.settled), "grouped_birth_cohorts": population.cohorts,
        "grouped_birth_memories": len(population.births), "direct_birth_carriers": 0,
        "native_head_sha256": digest, "transition_sha256": journal.chain.hexdigest(),
        "frontiers_sha256": cut_chain.hexdigest(), "grouped_birth_sha256": population.birth_chain.hexdigest(),
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-copies", type=Path, required=True)
    parser.add_argument("--source-coverage", type=Path, required=True)
    args = parser.parse_args()
    output = args.source_coverage.parent
    coverage, coverage_sha = read_document(args.source_coverage, 2 * 1024 * 1024)
    manifest, _ = read_document(output / MANIFEST, 2 * 1024 * 1024)
    schema = coverage["schema_id"]
    require(schema in SCHEMAS and manifest["schema_id"] == schema
            and manifest["source_coverage_sha256"] == coverage_sha, "invalid source coverage provenance")
    owners = {row["source_sha256"]: row for row in coverage["sources"]}
    require(len(coverage["sources"]) == len(PINS) and owners.keys() == set(PINS.values()),
            "Dream journal cohort differs from reviewed pins")
    results = []
    for relative, pinned in sorted(PINS.items()):
        kind = 1 if relative.endswith(".rel") else 2
        results.append({"fixture": relative, **observe(output, manifest["artifact_sha256"], owners[pinned],
                        schema, args.source_copies / relative, kind)})
    print(json.dumps({
        "status": "PASS", "scope": "retained Dream journal and grouped birth carrier frontiers",
        "frontier_scope": "ordered physical prefixes at retained global cuts",
        "native_scope": "clock-neutral head and every Dream proof row",
        "authentic_admission": False, "source_coverage_sha256": coverage_sha, "fixtures": results,
    }, indent=2))


if __name__ == "__main__":
    main()
