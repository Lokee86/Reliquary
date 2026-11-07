"""Reduce pinned physical-prefix Freshness queries to exact component proofs and replay plans."""
import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import struct

from f03_compare_archived_projections import ARCHIVED_SHA256
from f03_legacy_frame_clock import LegacyFrameClock
from f03_verify_archived_oracle import PINS, require, rows
from f03_verify_dream_journal_frontiers import (
    JOURNAL_TAGS, NATIVE_DOMAIN, Population, Reader, decode_completion, encode, framed, sha, u64,
)
from f03_verify_graph_frontiers import mutations, node_value
from f03_verify_independent_repeat import (
    COVERAGE, MANIFEST, SCHEMA_V2, decimal, exact_object, open_header, read_document,
    read_frame, regular, snapshot, take_stream,
)
from f03_verify_prefix_availability import write_jsonl

INPUT_DOMAIN = b"reliquary:f03:freshness-prefix-inputs:v1\0"
POLICY = dict(zip(
    ("policy_version", "decay_turns_per_point", "access_principal", "linkage_principal",
     "local_hop_cost", "outside_hop_cost"),
    (1, 10, 25, 50, 5, 10),
))
KNOWN_OTHER_CARRIERS = set("""
    CVAAFMT2 CVAAVEC1 CVAAVFM1 CVABRC02 CVACOMM1 CVACON04 CVACONT1 CVACPFM1 CVACPRO1
    CVACPRT1 CVADRM01 CVADRP02 CVAENC02 CVAEND02 CVAENTF1 CVAEPI02 CVAERL01 CVAERS03
    CVAETM02 CVAFRA02 CVAGCB02 CVAGCM02 CVAGFMT1 CVAGNOD2 CVAGNODE CVAINSF1 CVAINSW1
    CVAMD002 CVAMEMF2 CVAMRTE2 CVAMVEC1 CVAMVFM1 CVAPVEC1 CVAPVFM1 CVARELM1 CVATUR03
    CVAVERS1 CVAVERS2 CVAVGEN2 CVAVGFM2 CVAVGRC2 PRJCOR02 PRJFILE1
""".split())


def optional_turn(reader):
    flag = reader.number(1)
    require(flag in (0, 1), "invalid Freshness optional coordinate")
    return reader.number() if flag else None


def journal_entry(payload, uuid, population, admitted, activity, graph_head):
    reader = Reader(payload, 8)
    require(reader.take(16) == uuid, "Freshness journal owner differs")
    kind = reader.number(1)
    if kind == 1:
        identity = reader.take(32)
        score = reader.number(2)
        admission, accounted = optional_turn(reader), optional_turn(reader)
        require(identity in population.births and identity not in admitted and score == 100
                and admission == accounted and admission is not None and admission <= activity,
                "Freshness admission does not refer to a prior unadmitted birth")
        admitted.add(identity)
        value = {"memory_id": list(identity), "source_turn": admission}
        family, key = "admit", identity
    elif kind == 2:
        event_id, turn, producer = reader.text(), reader.number(), reader.number(1)
        source, origin, accepted, policy = reader.text(), reader.take(16), reader.number(), reader.number()
        require(bool(event_id) and bool(source.strip()) and origin == uuid
                and policy == 1 and turn <= activity and producer in (1, 2, 3, 4),
                "Freshness event provenance differs from its physical prefix")
        semantic = None
        if producer in (3, 4):
            owner_uuid = reader.take(16)
            fragment = None
            if producer == 4:
                fragment_id = list(reader.take(32))
                sources = [(reader.text(), reader.text()) for _ in range(reader.count(8))]
                require(bool(sources) and sources == sorted(set(sources))
                        and all(reference in population.turn_ids for reference in sources),
                        "Freshness fragment sources are not accepted Archive turns")
                fragment = {"fragment_id": fragment_id, "sources": sources}
            conversation, node, semantic_turn = reader.text(), reader.text(), reader.number()
            require(owner_uuid == origin and (conversation, node) in population.turn_ids
                    and semantic_turn == accepted == turn and source == event_id
                    and (fragment is None or (conversation, node) in fragment["sources"]),
                    "Freshness semantic event lacks accepted source provenance")
            semantic = {
                "owner_uuid": list(owner_uuid), "fragment": fragment, "conversation_id": conversation,
                "node_id": node, "accepted_turn": semantic_turn,
                "base_policy_digest": list(reader.take(32)), "semantic_policy_version": reader.number(2),
                "semantic_policy_digest": list(reader.take(32)), "compatibility_profile_id": list(reader.take(32)),
                "source_evidence_digest": list(reader.take(32)), "target_evidence_digest": list(reader.take(32)),
            }
        graph = reader.number()
        generation, community_graph = optional_turn(reader), optional_turn(reader)
        require(graph <= graph_head and ((generation is None and community_graph is None)
                or (generation is not None and community_graph == graph)),
                "Freshness event depends on an unavailable Graph proof")
        effects = [(reader.take(32), int.from_bytes(reader.take(2), "little", signed=True))
                   for _ in range(reader.count(34))]
        require(len({identity for identity, _ in effects}) == len(effects)
                and all(identity in population.births for identity, _ in effects)
                and (bool(effects) or semantic is not None), "Freshness event effect lacks a prior birth")
        if producer == 1:
            prefix = f"dream-initial:{origin.hex()}:"
            require(source == event_id and event_id.startswith(prefix)
                    and len(event_id) == len(prefix) + 64 and bytes.fromhex(event_id[len(prefix):]) in population.births,
                    "Freshness Dream source lacks an owner-bound birth")
        require(producer != 2, "accepted-use event requires receipt membership outside the pinned scope")
        value = {
            "event_id": event_id, "source_turn": turn,
            "effects": [[list(identity), effect] for identity, effect in sorted(effects)],
            "proof": {"graph_version": graph, "community_generation": generation,
                      "community_graph_version": community_graph},
            "provenance": {"producer_kind": producer, "source_id": source, "origin_owner": list(origin),
                           "accepted_turn": accepted, "policy_version": policy},
            "semantic": semantic,
        }
        family, key = "event", event_id.encode()
    else:
        raise ValueError("Freshness journal entry outside the pinned replay scope")
    reader.end()
    return family, key, value


def verify_commitments(output, artifacts, owner, committed):
    native = owner["native"]
    families = Counter()
    for _, row in take_stream(output, artifacts, owner, "records", native["record_stream"], schema_id=SCHEMA_V2):
        if row["projection_domain"] != "freshness":
            continue
        family = row["family"]
        families[family] += 1
        if family in ("initialize", "admit", "event"):
            value = committed.pop((family, row["stable_key_sha256"]), None)
            require(value is not None and sha(value) == row["canonical_value_sha256"]
                    and len(value) == decimal(row["canonical_byte_length"])
                    and row["availability"] == "retained", f"Freshness {family} input differs from the native row witness")
    freshness = next(row for row in native["native_history_projections"] if row["domain"] == "freshness")
    require(sum(families.values()) == decimal(freshness["rows"]) and not committed and set(families) <= {
        "activation", "admit", "cut", "event", "import_settlement", "initialize", "policy", "record",
    } and all(families[family] == 1 for family in ("activation", "cut", "import_settlement", "policy"))
        and families["initialize"] == families["record"], "incomplete or unsupported Freshness head rows")
    return dict(sorted(families.items()))


def scan_owner(output, artifacts, owner, path):
    global_rows = [r for _, r in take_stream(output, artifacts, owner, "global", owner["native"]["global_stream"],
                                             schema_id=SCHEMA_V2)]
    require(all(decimal(row["global_version"]) == index for index, row in enumerate(global_rows)),
            "retained global coordinate stream is not dense")
    uuid = bytes.fromhex(owner["owner_uuid"])
    pop = Population(uuid)
    chain = hashlib.sha256(INPUT_DOMAIN + uuid)
    admitted, commitments, frontiers = set(), {}, {}
    graph_nodes, graph_head, memory_graph_head = [], 0, 0
    counts = Counter()
    policy_digest = None
    settlement = None
    clock = LegacyFrameClock()
    before = regular(path)
    with path.open("rb") as source:
        source_hash = open_header(source, 1, owner)
        for ordinal, frame in take_stream(output, artifacts, owner, "frames", owner["frame_stream"],
                                          schema_id=SCHEMA_V2):
            payload = read_frame(source, frame, source_hash, before.st_size)
            g = clock.consume(payload)
            require(g == decimal(frame["latest_global"]) and g < len(global_rows)
                    and decimal(global_rows[g]["global_version"]) == g,
                    "Freshness prefix clock differs from the retained global stream")
            carrier = frame["carrier"]
            relevant = False
            if payload.startswith(b"CVAPUB01"):
                require(carrier in KNOWN_OTHER_CARRIERS, "unknown enveloped Freshness-prefix input")
                pop.publish(payload)
                if carrier in ("CVAGCB02", "CVAGCM02"):
                    body = payload[25 if payload[16] == 1 else 17:]
                    local = struct.unpack_from("<Q", body, 8)[0]
                    require(local == graph_head + 1, "noncontiguous physical Graph publication")
                    graph_head = local
                    if any(kind <= 8 for _, _, kind in mutations(body, graph_nodes)):
                        memory_graph_head = local
            elif carrier in ("CVAGNODE", "CVAGNOD2"):
                ordinal_in_graph, identity = node_value(payload)
                require(ordinal_in_graph == len(graph_nodes), "noncontiguous physical Graph node")
                graph_nodes.append(identity)
            elif carrier == "CVAINSC8":
                previous = len(pop.births)
                pop.group(payload, ordinal, g)
                added = len(pop.births) - previous
                if added:
                    require(not admitted and not counts["CVAFRS02:event"], "grouped birth after Freshness journal")
                    _, _, birth_turn, ids = decode_completion(payload)
                    require(len(ids) == added, "grouped birth list differs from accepted first revisions")
                    for identity in ids:
                        commitments["initialize", sha(identity)] = encode({
                            "memory_id": list(identity), "source_turn": birth_turn, "admitted_at_birth": False,
                        })
                    counts["CVAINSC8:nonempty_birth"] += 1
                    relevant = True
            elif carrier == "CVAFRP01":
                require(policy_digest is None and payload[8:24] == uuid and len(payload) == 42
                        and struct.unpack("<HQhhhh", payload[24:]) == tuple(POLICY.values()),
                        "explicit policy differs from the immutable default")
                policy_digest = frame["payload_sha256"]
                counts[carrier] += 1
            elif carrier == "CVAFRS02":
                require(policy_digest is not None and settlement is None, "journal outside the expected policy/settlement interval")
                family, key, value = journal_entry(payload, uuid, pop, admitted, len(pop.activity_globals), memory_graph_head)
                identity = family, sha(key)
                require(identity not in commitments, "repeated Freshness receipt identity outside the pinned scope")
                commitments[identity] = encode(value)
                counts[f"{carrier}:{family}"] += 1
                relevant = True
            elif carrier == "CVAFRS07":
                require(settlement is None, "multiple semantic settlements outside the pinned scope")
                settlement = g
                counts[carrier] += 1
                relevant = True
            elif carrier in ("CVDREAM1", "CVDREAM2"):
                tags = JOURNAL_TAGS if carrier == "CVDREAM1" else {0}
                require(len(payload) > 24 and payload[8:24] == uuid and payload[24] in tags,
                        "Dream record may change birth admission outside the pinned scope")
            else:
                require(carrier in KNOWN_OTHER_CARRIERS, "unknown Freshness-prefix carrier")
            if relevant:
                chain.update(framed(encode([ordinal, g, frame["payload_sha256"]])))
            stage = ("terminal" if settlement is not None else "journal" if admitted or counts["CVAFRS02:event"] else
                     "birth_only" if pop.births else "no_input")
            frontiers[g] = {
                "end_offset": source.tell(), "physical_prefix_sha256": source_hash.hexdigest(),
                "activity": len(pop.activity_globals), "input_chain_sha256": chain.hexdigest(),
                "stage": stage, "physical_frame_count": ordinal + 1,
            }
        require(source.tell() == before.st_size and source_hash.hexdigest() == owner["source_sha256"]
                and snapshot(before) == snapshot(regular(path)), "Freshness source differs from the reviewed bytes")
    require(policy_digest is not None and settlement == clock.head == len(global_rows) - 1
            and decimal(owner["native"]["rel_activity_head"]) == len(pop.activity_globals),
            "source did not reach the exact reviewed terminal Freshness state")
    return frontiers, global_rows, commitments, counts, policy_digest


def no_input_proof(uuid, activity):
    records = (
        ("activation", b"current", {"seen": False, "activation": None}),
        ("cut", b"cut", {"mode": "activity", "rel_turn": activity, "event_id_inclusive": None, "graph_version": None}),
        ("policy", b"policy", POLICY),
    )
    digest = hashlib.sha256(NATIVE_DOMAIN + b"\1\0\0\0" + uuid + framed(b"freshness")
                            + u64(activity) + b"\0" + u64(len(records)))
    for family, key, value in records:
        digest.update(framed(family.encode()) + framed(key) + framed(encode(value)))
    return {"cut": activity, "event_cut": None, "rows": len(records), "sha256": digest.hexdigest()}


def availability_rows(path, receipt):
    before = regular(path)
    require(0 < before.st_size <= 64 * 1024 * 1024, "invalid prefix-availability stream length")
    data = path.read_bytes()
    require(snapshot(before) == snapshot(regular(path)) and sha(data) == receipt["cut_rows_sha256"],
            "prefix-availability stream differs from its checked receipt")
    return [json.loads(line, object_pairs_hook=exact_object) for line in data.splitlines()]


def make_plan(owner, frontiers, global_rows, availability, archived):
    copy = owner["copy_id"]
    native = owner["native"]
    terminal = next(p for p in native["native_history_projections"] if p["domain"] == "freshness")
    require(len(availability) == len(global_rows) and archived["exhaustive"]
            and len(archived["fingerprints"]) == archived["head"] + 1, "incomplete retained Freshness cut evidence")
    cut_proofs = archived["fingerprints"]
    head = len(global_rows) - 1
    representatives, points, counts, empty = {}, [], Counter(), {}
    frontier = frontiers[0]
    for g, availability_row in enumerate(availability):
        frontier = frontiers.get(g, frontier)
        activity, stage = frontier["activity"], frontier["stage"]
        require(availability_row["global_version"] == str(g)
                and availability_row["physical_frontier_end"] == str(frontier["end_offset"])
                and availability_row["physical_frame_count"] == str(frontier["physical_frame_count"])
                and availability_row["physical_prefix_sha256"] == frontier["physical_prefix_sha256"],
                "physical prefix differs from the independently verified backing gate")
        if availability_row["archive_backing_gate"] == "blocked":
            counts["archive_backing_blocked"] += 1
            continue
        require(availability_row["archive_backing_gate"] == "satisfied"
                and decimal(global_rows[g]["domain_versions"]["rel_activity"]) == activity,
                "backing status or physical activity differs from its accepted coordinates")
        row = {"global": g, **{k: v for k, v in frontier.items() if k != "physical_frame_count"}}
        counts[stage] += 1
        if stage == "no_input":
            proof = empty.get(activity)
            if proof is None:
                proof = no_input_proof(bytes.fromhex(owner["owner_uuid"]), activity)
                empty[activity] = proof
            require(cut_proofs[0] == empty[0], "empty source differs from archived zero-activity proof")
            row["freshness_fingerprint"] = proof
        elif stage == "terminal":
            proof = cut_proofs[-1]
            require(g == head and frontier["end_offset"] == decimal(owner["source_byte_length"])
                    and frontier["physical_prefix_sha256"] == owner["source_sha256"]
                    and proof == {"cut": activity, "event_cut": None, "rows": decimal(terminal["rows"]),
                                  "sha256": terminal["sha256"]} and decimal(terminal["local_cut"]) == activity,
                    "terminal prefix differs from the native and archived full owner")
            row["freshness_fingerprint"] = proof
        else:
            key = activity, frontier["input_chain_sha256"]
            representative = representatives.setdefault(key, {**row, "members": 0})
            require(representative["stage"] == stage, "distinct stages shared an input chain")
            representative["members"] += 1
            row["representative_global"] = representative["global"]
        points.append(row)
    return {
        "copy_id": copy, "source_sha256": owner["source_sha256"],
        "counts": dict(sorted(counts.items())), "representatives": list(representatives.values()),
        "no_input_distinct_activity": len(empty),
        "no_input_archive_matches": sum(cut_proofs[cut] == proof for cut, proof in empty.items()),
    }, points


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("source-copies", "source-coverage", "archived-oracle", "prefix-availability",
                 "prefix-availability-directory", "output-directory"):
        parser.add_argument(f"--{name}", type=Path, required=True)
    args = parser.parse_args()
    output = args.source_coverage.parent
    coverage, coverage_sha = read_document(args.source_coverage, 2 * 1024 * 1024)
    manifest, manifest_sha = read_document(output / MANIFEST, 2 * 1024 * 1024)
    archived, archived_sha = read_document(args.archived_oracle, 128 * 1024 * 1024)
    prefix, prefix_sha = read_document(args.prefix_availability, 2 * 1024 * 1024)
    require(coverage["schema_id"] == manifest["schema_id"] == SCHEMA_V2
            and coverage_sha == manifest["source_coverage_sha256"] == manifest["artifact_sha256"][COVERAGE]
            == prefix["source_coverage_sha256"] and manifest_sha == prefix["source_manifest_sha256"]
            and archived_sha == prefix["archived_oracle_sha256"] == ARCHIVED_SHA256 and prefix["status"] == "PASS",
            "source evidence or archive custody differs from the checked prefix receipt")
    owners = {owner["source_sha256"]: owner for owner in coverage["sources"]}
    prefixes = {owner["source_sha256"]: owner for owner in prefix["owners"]}
    require(len(coverage["sources"]) == len(prefix["owners"]) == len(PINS)
            and owners.keys() == prefixes.keys() == set(PINS.values()), "expected the exact pinned fixture cohort")
    archives = rows(archived)
    destination = args.output_directory.resolve()
    regular(destination.parent, directory=True)
    regular(args.source_copies, directory=True)
    regular(args.prefix_availability_directory, directory=True)
    protected = (args.source_copies, output, args.prefix_availability_directory)
    require(not any(destination.is_relative_to(path.resolve()) for path in protected),
            "Freshness evidence destination overlaps protected input custody")
    artifacts = dict(manifest["artifact_sha256"])
    sources = []
    for relative, pinned in sorted(PINS.items()):
        if not relative.endswith(".rel"):
            continue
        owner, receipt = owners[pinned], prefixes[pinned]
        require(owner["native"]["owner_kind"] == "reliquary" and owner["copy_id"] == receipt["copy_id"],
                "prefix availability is bound to another source")
        availability = availability_rows(args.prefix_availability_directory / f'{owner["copy_id"]}-prefix-availability.jsonl',
                                         receipt)
        frontiers, global_rows, commitments, counts, policy_digest = scan_owner(
            output, artifacts, owner, args.source_copies / relative)
        families = verify_commitments(output, artifacts, owner, commitments)
        plan, points = make_plan(owner, frontiers, global_rows, availability, archives[relative]["oracle"]["freshness"])
        sources.append(({**plan, "fixture": relative, "source_prefix_stream_sha256": receipt["cut_rows_sha256"],
                         "carriers": dict(sorted(counts.items())), "native_head_families": families,
                         "policy_carrier_sha256": policy_digest}, points))
    destination.mkdir()
    for source, points in sources:
        source["point_stream_sha256"] = write_jsonl(destination / f'{source["copy_id"]}-freshness-prefix-points.jsonl', points)
    result = {
        "schema_id": "reliquary-f03-freshness-prefix-reduction-v1",
        "scope": "exact empty and terminal Freshness components and nonempty physical-prefix replay candidates",
        "authentic_admission": False, "whole_owner_positive_availability_assessed": False,
        "interior_native_freshness_evaluated": False, "source_coverage_sha256": coverage_sha,
        "source_manifest_sha256": manifest_sha, "archived_oracle_sha256": archived_sha,
        "prefix_availability_receipt_sha256": prefix_sha, "sources": [source for source, _ in sources],
    }
    with (destination / "freshness-prefix-plan.json").open("xb") as file:
        file.write(encode(result) + b"\n")


if __name__ == "__main__":
    main()
