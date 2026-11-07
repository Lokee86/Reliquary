"""Export exact typed Graph transaction batches from the six protected F03 owner copies."""
import argparse
from pathlib import Path

from f03_legacy_frame_clock import LegacyFrameClock
from f03_verify_archived_oracle import PINS, require, rows
from f03_verify_graph_frontiers import ARCHIVED_ORACLE_SHA256, GRAPH_DOMAINS, Graph, encode, global_cuts, node_value, sha
from f03_verify_independent_repeat import (
    MANIFEST, SCHEMA_V2, decimal, open_header, read_document, read_frame, regular, snapshot, take_stream,
)
from f03_verify_prefix_availability import write_jsonl

NODES = ("CVAGNODE", "CVAGNOD2")
PUBLICATIONS = ("CVAGCM02", "CVAGCB02")


def observe(output, artifacts, owner, path, archived):
    native = owner["native"]
    expected = {}
    for _, row in take_stream(output, artifacts, owner, "records", native["record_stream"], schema_id=SCHEMA_V2):
        if row["projection_domain"] in GRAPH_DOMAINS:
            key = row["projection_domain"], row["family"], row["stable_key_sha256"]
            require(key not in expected and row["availability"] == "retained", "invalid native Graph record")
            expected[key] = row["canonical_value_sha256"], row["canonical_byte_length"]
    kind = {"reliquary": 1, "phylactery": 2}[native["owner_kind"]]
    graph, clock = Graph(bytes.fromhex(owner["owner_uuid"]), expected), LegacyFrameClock()
    transfer, mappings = [], {}
    used = set()
    transition_count = 0
    before = regular(path)
    with path.open("rb") as source:
        digest = open_header(source, kind, owner)
        for ordinal, frame in take_stream(output, artifacts, owner, "frames", owner["frame_stream"], schema_id=SCHEMA_V2):
            payload = read_frame(source, frame, digest, before.st_size)
            require(clock.consume(payload) == decimal(frame["latest_global"]), "physical Graph clock differs")
            graph.ingest(payload, ordinal, clock)
            marker = frame["carrier"]
            if marker in (*NODES, "CVAGFMT1"):
                transfer.append({"source_ordinal": str(ordinal), "kind": "embedded", "carrier": marker})
                if marker in NODES:
                    mapping, identity = node_value(payload)
                    mappings[identity] = ordinal, mapping
            elif marker in PUBLICATIONS:
                require(payload.startswith(b"CVAPUB01"), "Graph publication has no retained envelope")
                global_ = int.from_bytes(payload[8:16], "little")
                local, _, changes = graph.transactions[global_]
                values = []
                for (origin, target, relation), (active, producer) in changes.items():
                    used.update((origin, target))
                    values.append([[origin[0], origin[1].hex()], [target[0], target[1].hex()],
                                   relation, active, producer])
                transition_count += len(values)
                transfer.append({
                    "source_ordinal": str(ordinal), "kind": "graph", "graph_version": str(local),
                    "global_version": str(global_), "bound_reservation": payload[16] == 2,
                    "transaction_time_ns": clock.times.get(global_), "changes": values,
                })
        require(source.tell() == before.st_size and digest.hexdigest() == owner["source_sha256"]
                and snapshot(before) == snapshot(regular(path)), "Graph source differs from the verified pin")
    common = archived["common"]
    frozen = {"graph": {row["cut"]: row for row in common["graph"]["fingerprints"]},
              "memory_graph": {row["cut"]: row for row in common["memory_graph"]}}
    heads = graph.verify_heads({row["domain"]: row for row in native["native_history_projections"]}, frozen)
    global_rows, _ = global_cuts(output, artifacts, owner, graph, clock)
    require(len(mappings) == len(graph.nodes) and used <= mappings.keys(), "Graph mutation lacks an accepted node mapping")
    unused = [{"source_ordinal": str(mappings[node][0]), "mapping_ordinal": str(mappings[node][1]),
               "node_kind": node[0], "node_id": node[1].hex()}
              for node in sorted(mappings.keys() - used)]
    return {
        "copy_id": owner["copy_id"], "source_sha256": owner["source_sha256"], "owner_uuid": owner["owner_uuid"],
        "owner_kind": native["owner_kind"], "verified_global_points": len(global_rows), "typed_nodes": len(mappings),
        "graph_publications": graph.counts[0], "memory_graph_publications": graph.counts[1],
        "relation_mutations": transition_count, "native_graph_head_digests": heads, "unreferenced_node_mappings": unused,
    }, transfer


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("source-copies", "source-coverage", "archived-oracle", "output-directory"):
        parser.add_argument(f"--{name}", type=Path, required=True)
    args = parser.parse_args()
    output = args.source_coverage.parent
    coverage, coverage_sha = read_document(args.source_coverage, 2 * 1024 * 1024)
    manifest, manifest_sha = read_document(output / MANIFEST, 2 * 1024 * 1024)
    archived, archive_sha = read_document(args.archived_oracle, 40 * 1024 * 1024)
    require(coverage["schema_id"] == manifest["schema_id"] == SCHEMA_V2
            and coverage_sha == manifest["source_coverage_sha256"] and archive_sha == ARCHIVED_ORACLE_SHA256,
            "Graph custody or frozen oracle differs")
    owners = {row["source_sha256"]: row for row in coverage["sources"]}
    require(len(coverage["sources"]) == len(PINS) and owners.keys() == set(PINS.values()),
            "Graph transfer requires the six reviewed owner pins")
    oracle = rows(archived)
    destination = args.output_directory.resolve()
    regular(destination.parent, directory=True)
    regular(args.source_copies, directory=True)
    require(not any(destination.is_relative_to(path.resolve()) for path in (output, args.source_copies)),
            "Graph transfer destination overlaps protected inputs")
    artifacts = dict(manifest["artifact_sha256"])
    results = []
    for relative, pin in sorted(PINS.items()):
        summary, transfer = observe(output, artifacts, owners[pin], args.source_copies / relative, oracle[relative]["oracle"])
        results.append(({**summary, "fixture": relative, "transfer_rows": len(transfer)}, transfer))
    destination.mkdir()
    for summary, transfer in results:
        summary["transfer_sha256"] = write_jsonl(destination / f'{summary["copy_id"]}-graph-transfer.jsonl', transfer)
    receipt = {
        "schema_id": "reliquary-f03-graph-source-transfer-v1",
        "scope": "retained typed Graph transactions in physical batch order",
        "source_manifest_sha256": manifest_sha, "source_coverage_sha256": coverage_sha,
        "archived_oracle_sha256": archive_sha, "target_conversion_verified": False,
        "sources": [summary for summary, _ in results],
    }
    with (destination / "graph-source-transfer.json").open("xb") as file:
        file.write(encode(receipt) + b"\n")


if __name__ == "__main__":
    main()
