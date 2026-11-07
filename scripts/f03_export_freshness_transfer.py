"""Decode pinned FRS07 settlement authority and its exact retained Freshness journal."""
import argparse
import hashlib
import json
from pathlib import Path

from f03_export_memory_transfer import Wire
from f03_verify_archived_oracle import PINS, require
from f03_verify_dream_journal_frontiers import encode, sha
from f03_verify_freshness_prefixes import POLICY, scan_owner, verify_commitments
from f03_verify_independent_repeat import (
    MANIFEST, SCHEMA_V2, decimal, read_document, read_exact, regular, snapshot, take_stream,
)
from f03_verify_prefix_availability import write_jsonl

SETTLEMENT_DOMAIN = b"reliquary-semantic-import-settlement-v7"
EVENT_DOMAIN = b"reliquary-import-accepted-event-v1"


class Compact(Wire):
    def var(self):
        value = 0
        for index in range(10):
            byte = self.number(1)
            require(index < 9 or byte <= 1, "compact integer overflow")
            value |= (byte & 127) << (index * 7)
            if byte < 128:
                require(index == 0 or byte != 0, "nonminimal compact integer")
                return value
        raise ValueError("unterminated compact integer")

    def count(self, minimum):
        count = self.var()
        require(count <= min((len(self.value) - self.offset) // minimum, (1 << 32) - 1),
                "compact count exceeds available bytes")
        return count

    def index(self, items):
        index = self.var()
        require(index < len(items), "compact index is outside its dictionary")
        return items[index]

    def signed_scalar(self):
        raw = self.var()
        value = (raw >> 1) ^ -(raw & 1)
        require(-(1 << 15) <= value < (1 << 15), "compact signed scalar overflow")
        return value

    def indexed(self, ids, minimum):
        index = -1
        for _ in range(self.count(minimum)):
            delta = self.var()
            index += delta
            require(delta > 0 and index < len(ids), "invalid compact ordinal delta")
            yield ids[index]


def strictly_ordered(values, message):
    require(all(left < right for left, right in zip(values, values[1:])), message)
    return values


def record_value(raw):
    reader = Wire(raw)
    score = int.from_bytes(reader.take(2), "little", signed=True)
    admitted, accounted = [reader.number() if reader.flag() else None for _ in range(2)]
    reader.end()
    require(-100 <= score <= 100 and ((admitted is None and accounted is None)
            or (admitted is not None and accounted is not None and admitted <= accounted)),
            "invalid compact score anchor")
    return {"score": score, "admitted_at_turn": admitted, "accounted_turn": accounted}


def settle(record, turn):
    accounted = record["accounted_turn"]
    if accounted is not None:
        require(accounted <= turn, "settlement contribution regresses its score anchor")
        steps = (turn - accounted) // POLICY["decay_turns_per_point"]
        record["accounted_turn"] += steps * POLICY["decay_turns_per_point"]
        record["score"] = max(-100, record["score"] - steps)


def reconstruct(initial, contributions, corrections, cut):
    records = {identity: value.copy() for identity, value in initial}
    previous = 0
    for contribution in contributions:
        position, effects = contribution["position"], contribution["effects"]
        require(0 < position <= cut and position >= previous and effects, "invalid causal contribution order")
        previous = position
        for identity, amount in effects:
            require(identity in records and 0 < abs(amount) <= 100, "invalid causal contribution")
            record = records[identity]
            settle(record, position)
            record["score"] = min(100, max(-100, record["score"] + amount))
    for record in records.values():
        settle(record, cut)
    for identity, amount in corrections:
        require(identity in records and 0 < abs(amount) <= 200, "invalid causal score correction")
        records[identity]["score"] += amount
        require(-100 <= records[identity]["score"] <= 100, "corrected score is outside policy range")
    return list(records.items())


def settlement(payload, uuid, leaves):
    require(len(payload) >= 329 and payload.startswith(b"CVAFRS07")
            and hashlib.sha256(SETTLEMENT_DOMAIN + payload[:-32]).digest() == payload[-32:],
            "FRS07 source settlement checksum mismatch")
    reader = Compact(payload[:-32], 8)
    require(reader.take(16).hex() == uuid, "FRS07 owner identity differs")
    import_id, start, corpus, manifest, input_ = [reader.identity() for _ in range(5)]
    historical_policy, semantic_policy = reader.identity(), reader.identity()
    base, cut, global_, archive, graph, memory_graph = [reader.number() for _ in range(6)]
    require(reader.number(1) == 0 and base <= cut and memory_graph <= graph <= global_ and archive <= global_,
            "invalid FRS07 clocks or reserved tag")
    ids = strictly_ordered([reader.identity() for _ in range(reader.count(32))], "unordered FRS07 Memory dictionary")
    texts = []
    last = b""
    for _ in range(reader.count(2)):
        prefix = reader.var()
        suffix = reader.take(reader.count(1))
        require(prefix <= len(last) and (prefix == len(last) or not suffix or last[prefix] != suffix[0]),
                "invalid compact text prefix")
        value = last[:prefix] + suffix
        require(value > last, "unordered compact text dictionary")
        texts.append(value.decode())
        last = value
    sets = strictly_ordered([list(reader.indexed(ids, 1)) for _ in range(reader.count(1))], "unordered FRS07 set dictionary")
    born, recipients = reader.index(sets), reader.index(sets)
    nodes = strictly_ordered([(reader.index(texts), reader.index(texts)) for _ in range(reader.count(2))],
                             "unordered FRS07 source nodes")
    accepted = []
    for _ in range(reader.count(41)):
        event, digest, turn = reader.index(texts), reader.identity(), reader.number()
        leaf = leaves.get(event)
        require(leaf is not None and digest == leaf["payload_digest"] and turn == leaf["accepted_turn"]
                and (not accepted or accepted[-1][0] < event), "FRS07 authority has a changed or missing accepted event")
        accepted.append((event, leaf))
    bitmap = reader.take((len(accepted) + 7) // 8)
    require(len(accepted) % 8 == 0 or bitmap[-1] >> (len(accepted) % 8) == 0, "nonzero FRS07 bitmap padding")
    superseded = [event for index, (event, _) in enumerate(accepted) if bitmap[index // 8] & (1 << (index % 8))]
    cohort, record_ids = reader.index(sets), reader.index(sets)
    anchors = strictly_ordered([reader.take(reader.count(1)) for _ in range(reader.count(5))],
                               "unordered FRS07 score anchor dictionary")
    values = [record_value(value) for value in anchors]
    require(reader.count(1) == len(record_ids), "FRS07 anchor count differs from records")
    initial = [(identity, reader.index(values)) for identity in record_ids]
    position = 0
    contributions = []
    for _ in range(reader.count(4)):
        position += reader.var()
        effects = [(identity, reader.signed_scalar()) for identity in reader.indexed(ids, 2)]
        contributions.append({"position": position, "effects": effects})
    corrections = [(identity, reader.signed_scalar()) for identity in reader.indexed(ids, 2)]
    reader.end()
    records = reconstruct(initial, contributions, corrections, cut)
    require(cohort == record_ids and set(born) <= set(cohort) and set(recipients) <= set(cohort),
            "FRS07 replacement cohort differs from retained records")
    result = {
        "owner": uuid, "import_id": import_id, "start_evidence_digest": start, "corpus_digest": corpus,
        "manifest_digest": manifest, "input_digest": input_, "historical_policy_digest": historical_policy,
        "semantic_policy_digest": semantic_policy, "base_cut": base, "settlement_cut": cut,
        "global_version": global_, "archive_version": archive, "graph_version": graph,
        "memory_graph_version": memory_graph, "born_memories": born, "affected_recipients": recipients,
        "source_nodes": nodes, "accepted_events": accepted, "superseded": superseded, "cohort": cohort,
        "records": records, "causal_basis": {"initial": initial, "contributions": contributions, "corrections": corrections},
    }
    return result


def native_settlement(result):
    arrays = lambda values: [list(bytes.fromhex(value)) for value in values]
    digest_fields = ("owner", "import_id", "start_evidence_digest", "corpus_digest", "manifest_digest", "input_digest")
    integers = ("base_cut", "settlement_cut", "global_version", "archive_version", "graph_version", "memory_graph_version")
    scope = {key: list(bytes.fromhex(result[key])) for key in digest_fields[:3]}
    scope.update({key: result[key] for key in ("base_cut", "settlement_cut", "source_nodes", "superseded")})
    scope.update({key: arrays(result[key]) for key in ("born_memories", "affected_recipients")})
    scope["predecessor_digest"] = None
    scope["accepted_events"] = [(event, {**leaf, "payload_digest": list(bytes.fromhex(leaf["payload_digest"])),
                                        "affected_recipients": arrays(leaf["affected_recipients"])})
                                 for event, leaf in result["accepted_events"]]
    return {
        "kind": "import_settlement", "epoch_scope": None, "semantic_scope": scope,
        **{key: list(bytes.fromhex(result[key])) for key in digest_fields},
        **{key: result[key] for key in integers}, "cohort": arrays(result["cohort"]),
        "records": [(list(bytes.fromhex(identity)), value) for identity, value in result["records"]],
        "superseded": result["superseded"],
    }


def observe(output, artifacts, owner, path):
    _, _, commitments, _, _ = scan_owner(output, dict(artifacts), owner, path)
    families = verify_commitments(output, dict(artifacts), owner, dict(commitments))
    uuid = owner["owner_uuid"]
    before = regular(path)
    leaves = {}
    journal = []
    settlement_row = None
    with path.open("rb") as source:
        for ordinal, frame in take_stream(output, dict(artifacts), owner, "frames", owner["frame_stream"],
                                          schema_id=SCHEMA_V2):
            marker = frame["carrier"]
            if marker not in ("CVAFRP01", "CVAFRS02", "CVAFRS07"):
                continue
            offset, end = decimal(frame["offset"]), decimal(frame["end_offset"])
            source.seek(offset)
            size = int.from_bytes(read_exact(source, 8), "little")
            require(size == end - offset - 8 and 8 <= size <= 16 * 1024 * 1024, "invalid Freshness frame extent")
            payload = read_exact(source, size)
            require(sha(payload) == frame["payload_sha256"] and payload[8:24].hex() == uuid,
                    "Freshness transfer payload differs from the independently verified bytes")
            row = {"physical_ordinal": ordinal, "physical_end_offset": end,
                   "physical_global_frontier": decimal(frame["latest_global"]),
                   "carrier": marker, "source_payload_sha256": frame["payload_sha256"]}
            if marker == "CVAFRP01":
                journal.append({**row, "family": "policy", "value": POLICY})
            elif marker == "CVAFRS02":
                require(settlement_row is None and payload[24] in (1, 2), "unsupported retained Freshness journal record")
                family, key = (("admit", payload[25:57]) if payload[24] == 1
                               else ("event", Wire(payload, 25).text().encode()))
                value = json.loads(commitments.pop((family, sha(key))))
                if family == "event":
                    leaves[value["event_id"]] = {
                        "event_id": value["event_id"], "accepted_turn": value["source_turn"],
                        "payload_digest": sha(EVENT_DOMAIN + payload[8:24] + payload[25:]),
                        "affected_recipients": [bytes(identity).hex() for identity, _ in value["effects"]],
                    }
                journal.append({**row, "family": family, "value": value})
            else:
                require(settlement_row is None, "unexpected additional FRS07 settlement")
                value = settlement(payload, uuid, leaves)
                require(value["global_version"] == row["physical_global_frontier"], "FRS07 global binding differs")
                settlement_row = {**row, "settlement": value}
    require(snapshot(before) == snapshot(regular(path)) and settlement_row is not None
            and all(family == "initialize" for family, _ in commitments), "incomplete or changed Freshness source input")
    native = owner["native"]
    expected = [row for _, row in take_stream(output, dict(artifacts), owner, "records", native["record_stream"],
                                             schema_id=SCHEMA_V2)
                if row["projection_domain"] == "freshness" and row["family"] == "import_settlement"]
    value = settlement_row["settlement"]
    body = encode(native_settlement(value))
    require(len(expected) == 1 and expected[0]["stable_key_sha256"] == sha(bytes.fromhex(value["import_id"]))
            and expected[0]["canonical_value_sha256"] == sha(body) and expected[0]["availability"] == "retained"
            and decimal(expected[0]["canonical_byte_length"]) == len(body), "typed FRS07 authority differs from native commitment")
    settlement_row["native_canonical_value_sha256"] = sha(body)
    return journal, settlement_row, families


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
    require(len(coverage["sources"]) == len(PINS) and owners.keys() == set(PINS.values()), "expected the exact reviewed pins")
    destination = args.output_directory.resolve()
    regular(destination.parent, directory=True)
    regular(args.source_copies, directory=True)
    require(not any(destination.is_relative_to(p.resolve()) for p in (output, args.source_copies)),
            "Freshness destination overlaps protected inputs")
    results = []
    for relative, pin in sorted(PINS.items()):
        owner = owners[pin]
        if not relative.endswith(".rel"):
            require(not any(key.startswith(("CVAFRP", "CVAFRS", "CVAFRB", "CVAFSA"))
                            for key in owner["physical_carrier_counts"]), "PHY Freshness is outside this transfer")
            continue
        journal, authority, families = observe(output, manifest["artifact_sha256"], owner, args.source_copies / relative)
        results.append(({
            "copy_id": owner["copy_id"], "fixture": relative, "source_sha256": pin, "owner_uuid": owner["owner_uuid"],
            "native_freshness_head": next(row for row in owner["native"]["native_history_projections"]
                                          if row["domain"] == "freshness"),
            "accepted_admissions": families["admit"], "accepted_events": families["event"],
            "grouped_births_bound_in_memory_transfer": families["initialize"],
            "settlement_records": len(authority["settlement"]["records"]),
            "settlement_native_value_sha256": authority["native_canonical_value_sha256"],
        }, journal, authority))
    destination.mkdir()
    for summary, journal, authority in results:
        copy = summary["copy_id"]
        summary["journal_sha256"] = write_jsonl(destination / f"{copy}-freshness-journal.jsonl", journal)
        payload = encode(authority) + b"\n"
        with (destination / f"{copy}-freshness-frs07-authority.json").open("xb") as file:
            file.write(payload)
        summary["settlement_sha256"] = sha(payload)
    receipt = {
        "schema_id": "reliquary-f03-freshness-source-transfer-v1",
        "scope": "retained source Freshness journal and exact FRS07 settlement authority",
        "source_manifest_sha256": manifest_sha, "source_coverage_sha256": coverage_sha,
        "target_conversion_verified": False, "sources": [summary for summary, _, _ in results],
    }
    with (destination / "freshness-source-transfer.json").open("xb") as file:
        file.write(encode(receipt) + b"\n")


if __name__ == "__main__":
    main()
