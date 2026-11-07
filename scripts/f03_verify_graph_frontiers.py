"""Independently verify retained Graph values and visible relations at source cuts."""
import argparse
from collections import defaultdict
import hashlib
import json
from pathlib import Path
import struct

from f03_legacy_frame_clock import LegacyFrameClock
from f03_verify_archived_oracle import PINS, require
from f03_verify_independent_repeat import (
    MANIFEST, SCHEMA_V2, open_header, read_document, read_frame, regular, snapshot, take_stream,
)

ARCHIVED_ORACLE_SHA256 = "d3c1cae685932927d30dd4a0b4189280b4acd0487397e89632022fff0c2b7d34"
NATIVE = b"reliquary:historical-owner-state:v1\0"
PHYSICAL = b"reliquary:f03:graph-physical-publications:v1\0"
VISIBLE = b"reliquary:f03:graph-visible-rows:v1\0"
CUTS = b"reliquary:f03:graph-retained-frontiers:v1\0"
GRAPH_DOMAINS = ("graph", "memory_graph")
ENDPOINT_DOMAINS = {"memory": 1, "entity": 2, "relationship": 3}
# Archived REL composition includes twelve component rows and one lineage row.
REL_COMPONENT_ROWS = 13


def encode(value):
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()


def le(value):
    return value.to_bytes(8, "little")


def framed(value):
    return le(len(value)) + value


def sha(value):
    return hashlib.sha256(value).hexdigest()


def native_prefix(uuid, domain, local, rows):
    return hashlib.sha256(NATIVE + b"\1\0\0\0" + uuid + framed(domain.encode()) + le(local) + b"\0" + le(rows))


def native_row(family, key, canonical):
    return framed(family.encode()) + framed(key) + framed(canonical)


def relation_key(key):
    source, target, kind = key
    return bytes([source[0]]) + source[1] + bytes([target[0]]) + target[1] + kind.to_bytes(2, "big")


def relation_value(key, origin, graph, global_):
    source, target, kind = key
    return {"source_kind": source[0], "source": list(source[1]), "target_kind": target[0], "target": list(target[1]),
            "kind": kind, "origin": origin, "graph_version": graph, "global_version": global_}


def node_value(payload):
    marker = payload[:8]
    if marker == b"CVAGNODE":
        require(len(payload) == 44, "invalid retained Graph Memory node")
        return struct.unpack_from("<I", payload, 40)[0], (1, payload[8:40])
    require(marker == b"CVAGNOD2" and len(payload) == 45 and payload[8] in (1, 2),
            "invalid retained typed Graph node")
    return struct.unpack_from("<I", payload, 41)[0], (payload[8], payload[9:41])


def mutations(body, nodes):
    marker = body[:8]
    if marker == b"CVAGCM02":
        start, count = 16, 1
    else:
        require(marker == b"CVAGCB02" and len(body) >= 20, "invalid retained Graph batch header")
        start, count = 20, struct.unpack_from("<I", body, 16)[0]
    require(count > 0 and len(body) == start + 12 * count, "invalid retained Graph mutation framing")
    result = {}
    for source, target, kind, active, origin in struct.iter_unpack("<IIHBB", body[start:]):
        require(source < len(nodes) and target < len(nodes) and source != target and active in (0, 1),
                "invalid Graph mutation endpoint or active flag")
        source, target = nodes[source], nodes[target]
        memory_edge = 1 <= kind <= 8 and (source[0], target[0]) == (1, 1) and origin in (1, 2)
        entity_edge = kind in (100, 101) and (source[0], target[0]) == (1, 2) and origin in (2, 3)
        key = source, target, kind
        require((memory_edge or entity_edge) and key not in result, "invalid or repeated typed Graph relation")
        result[key] = bool(active), origin
    return result


class Graph:
    def __init__(self, uuid, expected):
        self.uuid = uuid
        self.expected = expected
        self.nodes = []
        self.node_ids = set()
        self.transactions = {}
        self.last = 0
        self.memory_cut = 0
        self.counts = [0, 0]
        self.hash_parts = [bytearray(), bytearray()]
        self.prefixes = [{0: (0, 0, 0)}, {0: (0, 0, 0)}]
        self.format_seen = False
        self.physical_chain = hashlib.sha256(PHYSICAL + uuid)
        self.physical_frontiers = {}

    def check_record(self, domain, family, key, value):
        commitment = sha(value), str(len(value))
        require(self.expected.pop((domain, family, sha(key)), None) == commitment,
                f"physical {domain}/{family} differs from its native record")
        return commitment[0]

    def publish(self, body, global_, ordinal, clock):
        local = struct.unpack_from("<Q", body, 8)[0]
        require(self.format_seen and local == self.counts[0] + 1 and global_ > self.last,
                "Graph publication does not extend the retained local/global sequence")
        changes = mutations(body, self.nodes)
        values = []
        for key, (active, origin) in sorted(changes.items()):
            values.append({**relation_value(key, origin, local, global_), "active": active})
        memory_values = [value for value in values if value["kind"] <= 8]
        if memory_values:
            self.memory_cut = local
        stamp = int(clock.times[global_]) if global_ in clock.times else None
        key = local.to_bytes(8, "big")
        commitments = []
        for index, (domain, rows) in enumerate(zip(GRAPH_DOMAINS, (values, memory_values))):
            if rows:
                canonical = encode({"global_version": global_, "time_ns": stamp, "changes": rows})
                commitments.append(self.check_record(domain, "publication", key, canonical))
                self.hash_parts[index].extend(native_row("publication", key, canonical))
                self.counts[index] += 1
                self.prefixes[index][local] = self.counts[index], len(self.hash_parts[index]), self.memory_cut
        self.transactions[global_] = (local, self.memory_cut, changes)
        self.last = global_
        self.physical_chain.update(framed(encode([ordinal, clock.head, local, global_, *commitments])))
        self.physical_frontiers[clock.head] = (ordinal, self.physical_chain.hexdigest())

    def ingest(self, payload, ordinal, clock):
        marker = payload[:8]
        if marker == b"CVAGFMT1":
            require(payload[8:] == b"\1\0\0\0" and not self.format_seen, "invalid Graph format")
            self.format_seen = True
        elif marker in (b"CVAGNODE", b"CVAGNOD2"):
            node_ordinal, identity = node_value(payload)
            require(self.format_seen and node_ordinal == len(self.nodes) and identity not in self.node_ids,
                    "invalid Graph node ordinal or repeated typed identity")
            self.nodes.append(identity)
            self.node_ids.add(identity)
        elif marker == b"CVAPUB01":
            body = payload[25 if payload[16] == 1 else 17:]
            if body.startswith(b"CVAG"):
                require(body[:8] in (b"CVAGCM02", b"CVAGCB02") and len(body) >= 16,
                        "unknown versioned Graph carrier")
                self.publish(body, struct.unpack_from("<Q", payload, 8)[0], ordinal, clock)
        elif marker.startswith((b"CVAG", b"CVARGN")):
            raise ValueError("unknown or baseline Graph carrier outside pinned scope")

    def fingerprint(self, index, local):
        count, length, memory_cut = self.prefixes[index][local]
        rows = count + 1
        digest = native_prefix(self.uuid, GRAPH_DOMAINS[index], local, rows)
        digest.update(native_row("clock", b"memory_graph", encode(memory_cut)))
        digest.update(memoryview(self.hash_parts[index])[:length])
        return {"cut": local, "event_cut": None, "rows": rows, "sha256": digest.hexdigest()}

    def verify_heads(self, native, frozen):
        result = {}
        for index, (domain, local) in enumerate(zip(GRAPH_DOMAINS, (self.counts[0], self.memory_cut))):
            self.check_record(domain, "clock", b"memory_graph", encode(self.memory_cut))
            proof = self.fingerprint(index, local)
            native_head = native[domain]
            require(native_head["local_cut"] == str(local) and native_head["event_cut"] is None
                    and native_head["rows"] == str(proof["rows"]) and native_head["sha256"] == proof["sha256"]
                    and frozen[domain][local] == proof,
                    f"{domain} native or archived head differs from independent replay")
            result[domain] = proof["sha256"]
        require(not self.expected, "native Graph commitments were omitted")
        return result


def endpoint_intervals(output, artifacts, owner):
    member = owner["native"]["clocked_cut_membership"]["stream"]
    intervals = defaultdict(list)
    for _, interval in take_stream(output, artifacts, owner, "clocked-membership", member, schema_id=SCHEMA_V2):
        row = interval["row"]
        kind = ENDPOINT_DOMAINS.get(row["projection_domain"])
        if kind and row["family"] == "current":
            require(row["availability"] == "retained", "current Graph endpoint is not retained")
            key = kind, row["stable_key_sha256"]
            first, last = int(interval["first_cut"]), int(interval["last_cut"])
            ranges = intervals[key]
            require(0 < first <= last and (not ranges or ranges[-1][1] < first), "invalid endpoint lifetime")
            if ranges and ranges[-1][1] + 1 == first:
                ranges[-1] = ranges[-1][0], last
            else:
                ranges.append((first, last))
    return intervals


def global_cuts(output, artifacts, owner, graph, clock):
    cuts = []
    first_global = {name: [0] for name in ENDPOINT_DOMAINS}
    graph_cut = memory_graph_cut = 0
    stream = owner["native"]["global_stream"]
    for version, row in take_stream(output, artifacts, owner, "global", stream, schema_id=SCHEMA_V2):
        require(version == int(row["global_version"]) and row["transaction_time_ns"] == clock.times.get(version),
                "global point differs from independently reconstructed clock")
        if version in graph.transactions:
            graph_cut, memory_graph_cut, _ = graph.transactions[version]
        values = row["domain_versions"]
        require((graph_cut, memory_graph_cut) == tuple(int(values[name]) for name in GRAPH_DOMAINS),
                "Graph local cut differs from reconstructed publication order")
        for name, first in first_global.items():
            local = int(values[name])
            require(local >= len(first) - 1, "endpoint local clock regressed")
            first.extend([version] * (local + 1 - len(first)))
        cuts.append((graph_cut, memory_graph_cut))
    require(len(cuts) == clock.head + 1, "missing global Graph frontier")
    return cuts, {kind: first_global[name] for name, kind in ENDPOINT_DOMAINS.items()}


def endpoint_events(intervals, first_global):
    events = defaultdict(dict)
    for (kind, key), ranges in intervals.items():
        first = first_global[kind]
        for local_start, local_end in ranges:
            require(local_end < len(first), "endpoint lifetime exceeds global timeline")
            start = first[local_start]
            end = first[local_end + 1] if local_end + 1 < len(first) else None
            if start != end:
                events[start][(kind, key)] = True
                if end is not None:
                    events[end][(kind, key)] = False
    return events


def visible_frontiers(graph, cuts, events, probes, frozen):
    node_hash = {node: (node[0], sha(node[1])) for node in graph.nodes}
    endpoints, active, visible = set(), {}, {}
    incident = defaultdict(set)
    history = hashlib.sha256(VISIBLE + graph.uuid)
    cut_chain = hashlib.sha256(CUTS + graph.uuid)
    physical = (None, hashlib.sha256(PHYSICAL + graph.uuid).hexdigest())
    samples = []
    state_changes = 0
    semantic_changes = 0
    for global_, (local, memory_cut) in enumerate(cuts):
        dirty = set()
        for endpoint, present in events.get(global_, {}).items():
            dirty.update(incident.get(endpoint, ()))
            if present:
                endpoints.add(endpoint)
            else:
                endpoints.discard(endpoint)
        _, _, changes = graph.transactions.get(global_, (None, None, {}))
        for key, (enabled, origin) in changes.items():
            dirty.add(key)
            source, target = (node_hash[node] for node in key[:2])
            if enabled:
                active[key] = encode(relation_value(key, origin, local, global_))
                incident[source].add(key)
                incident[target].add(key)
            else:
                active.pop(key, None)
                incident[source].discard(key)
                incident[target].discard(key)
        semantic_changes += len(changes)
        for key in sorted(dirty):
            previous = visible.get(key)
            present = all(node_hash[node] in endpoints for node in key[:2])
            current = active.get(key) if present else None
            if current != previous:
                state_changes += 1
                history.update(framed(encode([global_, sha(relation_key(key)),
                                              None if current is None else sha(current)])))
                if current is None:
                    del visible[key]
                else:
                    visible[key] = current
        physical = graph.physical_frontiers.get(global_, physical)
        cut_chain.update(framed(encode([global_, local, memory_cut, len(active), len(visible),
                                        history.hexdigest(), physical])))
        if global_ in probes:
            require(probes[global_]["rows"] == REL_COMPONENT_ROWS + len(visible),
                    "visible Graph row count differs from supported archived probe")
            proofs = {}
            for index, (domain, cut) in enumerate(zip(GRAPH_DOMAINS, (local, memory_cut))):
                proof = graph.fingerprint(index, cut)
                require(frozen[domain][cut] == proof, "Graph history differs at supported archived probe")
                proofs[domain] = proof["sha256"]
            samples.append({"global_version": global_, "visible_rows": len(visible), "graph_digests": proofs})
    require(len(samples) == len(probes), "supported archived probe outside retained global history")
    head = hashlib.sha256(VISIBLE + b"head\0" + graph.uuid + le(len(visible)))
    for key, value in sorted(visible.items()):
        head.update(framed(relation_key(key)) + framed(value))
    return {"semantic_relation_changes": semantic_changes, "visible_relation_changes": state_changes,
            "active_relation_head_rows": len(active), "visible_relation_head_rows": len(visible),
            "visible_head_sha256": head.hexdigest(), "visible_transition_sha256": history.hexdigest(),
            "frontiers_sha256": cut_chain.hexdigest(),
            "archived_supported_probes": samples}


def observe(output, artifacts, owner, source_path, kind, archived):
    native = owner["native"]
    expected = {}
    for _, row in take_stream(output, artifacts, owner, "records", native["record_stream"], schema_id=SCHEMA_V2):
        if row["projection_domain"] in GRAPH_DOMAINS:
            key = row["projection_domain"], row["family"], row["stable_key_sha256"]
            require(key not in expected and row["availability"] == "retained", "invalid native Graph record")
            expected[key] = row["canonical_value_sha256"], row["canonical_byte_length"]
    uuid = bytes.fromhex(owner["owner_uuid"])
    require(native["history_generation"] == "0" * 32 and native["rewrite_genesis"] is None,
            "lineage outside pinned Graph replay scope")
    graph = Graph(uuid, expected)
    clock = LegacyFrameClock()
    before = regular(source_path)
    with source_path.open("rb") as source:
        source_hash = open_header(source, kind, owner)
        for ordinal, frame in take_stream(output, artifacts, owner, "frames", owner["frame_stream"], schema_id=SCHEMA_V2):
            payload = read_frame(source, frame, source_hash, before.st_size)
            require(str(clock.consume(payload)) == frame["latest_global"], "physical Graph clock differs")
            graph.ingest(payload, ordinal, clock)
        require(source.tell() == before.st_size and source_hash.hexdigest() == owner["source_sha256"]
                and snapshot(before) == snapshot(regular(source_path)), "Graph source differs from its pins")
    native_heads = {row["domain"]: row for row in native["native_history_projections"]}
    common = archived["common"]
    frozen = {
        "graph": {row["cut"]: row for row in common["graph"]["fingerprints"]},
        "memory_graph": {row["cut"]: row for row in common["memory_graph"]},
    }
    graph_digests = graph.verify_heads(native_heads, frozen)
    intervals = endpoint_intervals(output, artifacts, owner)
    cuts, first_global = global_cuts(output, artifacts, owner, graph, clock)
    probes = {probe["cut"]: probe for probe in archived.get("global_projection", []) if "rejected" not in probe}
    return {
        "source_sha256": owner["source_sha256"], "global_cuts": len(cuts), "typed_nodes": len(graph.nodes),
        "graph_publications": graph.counts[0], "memory_graph_publications": graph.counts[1],
        "native_graph_head_digests": graph_digests, "physical_publications_sha256": graph.physical_chain.hexdigest(),
        **visible_frontiers(graph, cuts, endpoint_events(intervals, first_global), probes, frozen),
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-copies", type=Path, required=True)
    parser.add_argument("--source-coverage", type=Path, required=True)
    parser.add_argument("--archived-oracle", type=Path, required=True)
    args = parser.parse_args()
    output = args.source_coverage.parent
    coverage, coverage_sha = read_document(args.source_coverage, 2 * 1024 * 1024)
    manifest, manifest_sha = read_document(output / MANIFEST, 2 * 1024 * 1024)
    require(coverage["schema_id"] == manifest["schema_id"] == SCHEMA_V2
            and manifest["source_coverage_sha256"] == coverage_sha, "invalid source coverage provenance")
    owners = {row["source_sha256"]: row for row in coverage["sources"]}
    require(len(coverage["sources"]) == len(PINS) and owners.keys() == set(PINS.values()), "unexpected Graph custody")
    archived, archive_sha = read_document(args.archived_oracle, 40 * 1024 * 1024)
    require(archive_sha == ARCHIVED_ORACLE_SHA256, "unexpected archived Graph oracle")
    oracle = {row["fixture"]: row["oracle"] for row in archived["fixtures"]}
    require(oracle.keys() == PINS.keys(), "archived Graph owner population differs")
    results = []
    for relative, pinned in sorted(PINS.items()):
        kind = 1 if relative.endswith(".rel") else 2
        results.append({"fixture": relative, **observe(output, manifest["artifact_sha256"], owners[pinned],
                        args.source_copies / relative, kind, oracle[relative])})
    print(json.dumps({
        "status": "PASS", "scope": "retained Graph publications and endpoint-filtered visible relations",
        "endpoint_scope": "native retained current membership at each mapped local cut",
        "whole_owner_positive_availability_assessed": False, "authentic_admission": False,
        "source_coverage_sha256": coverage_sha, "source_manifest_sha256": manifest_sha,
        "archived_oracle_sha256": archive_sha, "fixtures": results,
    }, indent=2))


if __name__ == "__main__":
    main()
