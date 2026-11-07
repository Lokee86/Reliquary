"""Verify F03 v2 clocked memberships and compositions independently of Rust."""
import argparse
from collections import defaultdict
import hashlib
from itertools import zip_longest
import json
from pathlib import Path
import re

from f03_verify_independent_repeat import (
    COVERAGE, MANIFEST, decimal, read_document, require, take_stream,
)

SCHEMA = "reliquary-f03-historical-state-v2"
SCOPE = "retained_clocked_native_history"
COMMON = {"memory", "entity", "relationship", "graph", "memory_graph", "ego"}
REL = {"archive", "archive_publication_origins", "vector_generation"}
LOCAL_DOMAIN = b"reliquary:f03:retained-local-prefix:v1\0"


def canonical(value):
    return json.dumps(value, ensure_ascii=False, allow_nan=False, sort_keys=True,
                      separators=(",", ":")).encode("utf-8")


def framed(data):
    return len(data).to_bytes(8, "little") + data


def digest(value):
    return hashlib.sha256(value).hexdigest()


def verify_interval_shape(interval, domains, previous):
    require(interval.keys() == {"row", "first_cut", "last_cut"}, "invalid membership interval")
    row = interval["row"]
    require(row.keys() == {"projection_domain", "family", "stable_key_sha256",
                           "canonical_value_sha256", "availability"}, "invalid membership row")
    domain = row["projection_domain"]
    family = row["family"]
    key = row["stable_key_sha256"]
    require(domain in domains and isinstance(family, str) and bool(family)
            and all(isinstance(row[field], str) and re.fullmatch(r"[0-9a-f]{64}", row[field])
                    for field in ("stable_key_sha256", "canonical_value_sha256"))
            and row["availability"] in {"retained", "pruned", "tombstoned", "inert"},
            "unclassified membership row or digest")
    first = decimal(interval["first_cut"])
    last = decimal(interval["last_cut"])
    head = decimal(domains[domain]["head"])
    require(first <= last <= head, "membership interval exceeds the declared local head")
    identity = (domain, family, key)
    require(previous.get(identity, -1) < first, "overlapping or unordered membership interval")
    previous[identity] = last

    # These native projection families use the local publication version as
    # their key. Their starting coordinate can be checked without payload text.
    if family in {"publication", "publication_origin", "revision", "generation"}:
        require(first > 0 and key == digest(first.to_bytes(8, "big")) and last == head,
                "immutable native publication membership differs from its local key")
    return domain, first, last, identity, row


def verify_owner(output, artifacts, owner):
    native = owner["native"]
    evidence = native["clocked_cut_membership"]
    domains = evidence["domains"]
    expected = COMMON | (REL if native["owner_kind"] == "reliquary" else set())
    require(evidence["scope"] == SCOPE and domains.keys() == expected,
            "clocked membership does not cover the exact native domains")
    head_proofs = {row["domain"]: decimal(row["local_cut"]) for row in native["native_history_projections"]}
    require(all(decimal(facts["head"]) == head_proofs[domain] for domain, facts in domains.items()),
            "declared domain heads differ from retained native proofs")

    previous = {}
    changes = defaultdict(lambda: defaultdict(list))
    emitted_head = {}
    counts = defaultdict(int)
    graph_positions = [0]
    member_count = 0
    for _, interval in take_stream(output, artifacts, owner, "clocked-membership",
                                   evidence["stream"], schema_id=SCHEMA):
        domain, first, last, identity, row = verify_interval_shape(interval, domains, previous)
        counts[domain] += 1
        member_count += 1
        head = head_proofs[domain]
        value = canonical(row)
        changes[domain][first].append(b"+" + value)
        if last < head:
            changes[domain][last + 1].append(b"-" + value)
        else:
            emitted_head[identity] = (row["canonical_value_sha256"], row["availability"])
        if domain == "memory_graph" and row["family"] == "publication":
            graph_positions.append(first)

    expected_head = {}
    for _, row in take_stream(output, artifacts, owner, "records", native["record_stream"],
                              schema_id=SCHEMA):
        if row["projection_domain"] in domains:
            key = (row["projection_domain"], row["family"], row["stable_key_sha256"])
            require(key not in expected_head, "duplicate native head key")
            expected_head[key] = (row["canonical_value_sha256"], row["availability"])
    require(expected_head == emitted_head, "membership endpoints do not reproduce the complete native head")

    roots = {}
    for domain, facts in domains.items():
        head = head_proofs[domain]
        require(decimal(facts["intervals"]) == counts[domain], "wrong membership interval count")
        sparse = domain == "memory_graph"
        require(facts["cut_shape"] == ("sparse_graph_publications" if sparse else "inclusive_dense")
                and facts["qualifying_cuts"] == ([str(n) for n in graph_positions] if sparse else None)
                and (not sparse or graph_positions == sorted(set(graph_positions))
                     and graph_positions[-1] == head),
                "declared cuts differ from the accepted local publication history")
        series = []
        chain = hashlib.sha256(LOCAL_DOMAIN + framed(domain.encode("utf-8")))
        for cut in range(head + 1):
            for delta in sorted(changes[domain].get(cut, ())):
                chain.update(cut.to_bytes(8, "big"))
                chain.update(framed(delta))
            series.append(chain.hexdigest())
        roots[domain] = series

    globals_ = take_stream(output, artifacts, owner, "global", native["global_stream"], schema_id=SCHEMA)
    composed = take_stream(output, artifacts, owner, "clocked-composition",
                           native["clocked_composition_stream"], schema_id=SCHEMA)
    points = 0
    sparse_positions = {str(cut) for cut in graph_positions}
    for pair in zip_longest(globals_, composed):
        require(all(item is not None for item in pair), "incomplete composed global coverage")
        (ordinal, global_), (second, projection) = pair
        require(ordinal == second == decimal(global_["global_version"]), "misordered global cut")
        coordinates = global_["domain_versions"]
        local = {}
        for domain, series in roots.items():
            if domain == "ego":
                continue
            clock = "archive" if domain == "archive_publication_origins" else domain
            cut = decimal(coordinates[clock])
            require(cut < len(series), "global cut points beyond an observed local history")
            require(domain != "memory_graph" or str(cut) in sparse_positions,
                    "global cut points at a nonqualifying MemoryGraph publication")
            local[domain] = series[cut]
        expected_sha = digest(canonical({"global_cut": global_, "clocked_prefixes": local}))
        require(projection == {"global_version": str(ordinal), "scope": SCOPE,
                               "clocked_history_sha256": expected_sha},
                "composed global digest differs from independently evaluated interval history")
        points += 1
    return {"copy_id": owner["copy_id"], "memberships": member_count, "global_points": points,
            "domains": sorted(domains)}


def match_retained_baseline(output, manifest, coverage, baseline):
    old_coverage, old_digest = read_document(baseline / COVERAGE, 2 * 1024 * 1024)
    old_manifest, _ = read_document(baseline / MANIFEST, 2 * 1024 * 1024)
    require(old_manifest["source_coverage_sha256"] == old_digest, "baseline coverage digest mismatch")
    new_artifacts = dict(manifest["artifact_sha256"])
    old_artifacts = dict(old_manifest["artifact_sha256"])
    total = 0
    require(len(coverage["sources"]) == len(old_coverage["sources"]) == 6,
            "baseline copy population differs")
    for current, old in zip(coverage["sources"], old_coverage["sources"]):
        require(current["copy_id"] == old["copy_id"] and current["source_sha256"] == old["source_sha256"]
                and current["native"]["native_history_projections"] == old["native"]["native_history_projections"],
                "baseline native projections differ")
        for name, key in [("frames", "frame_stream"), ("global", "global_stream"),
                          ("freshness", "freshness_stream"), ("records", "record_stream")]:
            left = (current if name == "frames" else current["native"])[key]
            right = (old if name == "frames" else old["native"])[key]
            if "rows" not in left:
                require(left == right, "baseline applicability changed")
                continue
            rows = take_stream(output, new_artifacts, current, name, left, schema_id=SCHEMA)
            old_rows = take_stream(baseline, old_artifacts, old, name, right)
            for a, b in zip_longest(rows, old_rows):
                require(a is not None and a == b, f"retained baseline differs in {name}")
                total += 1
    return total


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-output", type=Path, required=True)
    parser.add_argument("--baseline-output", type=Path)
    args = parser.parse_args()
    manifest, _ = read_document(args.source_output / MANIFEST, 2 * 1024 * 1024)
    coverage, coverage_sha = read_document(args.source_output / COVERAGE, 2 * 1024 * 1024)
    require(manifest["schema_id"] == coverage["schema_id"] == SCHEMA
            and manifest["source_coverage_sha256"] == coverage_sha
            and manifest["artifact_sha256"][COVERAGE] == coverage_sha,
            "invalid v2 manifest provenance")
    require(len(coverage["sources"]) == 6, "expected the six source copies")
    artifacts = dict(manifest["artifact_sha256"])
    results = [verify_owner(args.source_output, artifacts, owner) for owner in coverage["sources"]]
    retained = None
    if args.baseline_output:
        retained = match_retained_baseline(args.source_output, manifest, coverage, args.baseline_output)
    print(json.dumps({"status": "PASS", "scope": SCOPE, "authentic_admission": False,
                      "source_coverage_sha256": coverage_sha, "baseline_rows_equal": retained,
                      "owners": results}, indent=2))


if __name__ == "__main__":
    main()
