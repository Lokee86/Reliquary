"""Compare the exported native head projections to the immutable archived oracle."""
import argparse
import hashlib
import json
from pathlib import Path

from f03_verify_archived_oracle import PINS, require
from f03_verify_independent_repeat import MANIFEST, SCHEMA, read_document, take_stream

ARCHIVED_SHA256 = "d3c1cae685932927d30dd4a0b4189280b4acd0487397e89632022fff0c2b7d34"


def comparable(oracle):
    common = oracle["common"]
    result = {domain: common[domain]["fingerprints"][-1]
              for domain in ("memory", "entity", "relationship", "graph", "ego")}
    result["memory_graph"] = common["memory_graph"][-1]
    result["dream_pairs"] = oracle["dream_pairs"]
    result["dream_processing"] = oracle["dream_processing"]
    if "archive" in oracle:
        result["archive"] = next(row for row in oracle["archive"]["snapshots"]
                                 if row["cut"] == oracle["archive"]["head"])
        result["vector_generation"] = oracle["vectors"]["fingerprints"][-1]
        result["freshness"] = oracle["freshness"]["fingerprints"][-1]
        result["dream_proof"] = oracle["dream_proof"]
    return result


def global_ordering(owner, output, artifacts):
    hash_ = hashlib.sha256()
    metadata = owner["native"]["global_stream"]
    count = 0
    for ordinal, row in take_stream(output, artifacts, owner, "global", metadata):
        require(ordinal == int(row["global_version"]), "out-of-order global coordinate")
        domains = row["domain_versions"]
        point = [ordinal, None if row["transaction_time_ns"] is None else int(row["transaction_time_ns"])]
        point.extend(int(domains.get(name, "0")) for name in (
            "archive", "memory", "entity", "relationship", "graph", "memory_graph", "vector_generation"
        ))
        hash_.update(json.dumps(point, separators=(",", ":")).encode("ascii"))
        count += 1
    return {"exhaustive": True, "points": count, "sha256": hash_.hexdigest()}


def ledger(archived, coverage, output, artifacts):
    require(coverage["schema_id"] == SCHEMA and coverage["authentic_admission"] is False,
            "unsupported historical exporter coverage schema")
    sources = coverage["sources"]
    rows = {row["fixture"]: row for row in archived["fixtures"]}
    require(len(sources) == len(PINS) and len(rows) == len(PINS)
            and rows.keys() == PINS.keys(), "six-owner witness is required")
    matches = 0
    matching_orderings = 0
    matching_global_points = 0
    differences = []
    remaining = []
    unrepresented = []
    for (relative, pinned), owner in zip(sorted(PINS.items()), sources):
        require(owner["source_sha256"] == pinned, "source does not match reviewed F03 custody")
        oracle = rows[relative]["oracle"]
        actual = owner["native"]["native_history_projections"]
        by_domain = {row["domain"]: row for row in actual}
        expected = comparable(oracle)
        require(len(actual) == len(by_domain) and expected.keys() <= by_domain.keys()
                and by_domain.keys() - expected.keys() <= {
                    "archive_publication_origins", "project_operational", "vector_objects",
                    "insomnia_import_journal", "clock_neutral_history"
                },
                f"unexpected projection domains for {owner['copy_id']}")
        for domain in sorted(by_domain.keys() - expected.keys()):
            unrepresented.append({"copy_id": owner["copy_id"], "domain": domain,
                                  "status": "not_represented_by_archived_oracle"})
        for domain, want in expected.items():
            got = by_domain[domain]
            projected = {"cut": int(got["local_cut"]), "rows": int(got["rows"]),
                         "event_cut": None if got["event_cut"] is None else int(got["event_cut"]),
                         "sha256": got["sha256"]}
            if want == projected:
                matches += 1
            else:
                differences.append({
                    "copy_id": owner["copy_id"], "domain": domain,
                    "status": "unexplained_projection_difference", "archived": want, "frs07": projected,
                })
        ordering = global_ordering(owner, output, artifacts)
        if oracle["global_ordering"] == ordering:
            matching_orderings += 1
            matching_global_points += ordering["points"]
        else:
            differences.append({
                "copy_id": owner["copy_id"], "domain": "global_ordering",
                "status": "unexplained_projection_difference",
                "archived": oracle["global_ordering"], "frs07": ordering,
            })
        if relative.endswith(".rel"):
            remaining.append({
                "copy_id": owner["copy_id"],
                "whole_owner_archived_cuts": rows[relative]["global_projection_points"],
                "archive_archived_cut_count": len(oracle["archive"]["snapshots"]),
                "archive_archived_cuts_exhaustive": oracle["archive"]["snapshot_queries_exhaustive"],
                "freshness_archived_event_cut_count": len(oracle["freshness_events"]),
                "freshness_cut_basis": "archived event query uses current MemoryGraph head; FRS07 source witness uses each event's recorded proof coordinate",
                "disposition": "separate archived projection replay required; no new equivalence assumed",
            })
    return {
        "status": "FAIL" if differences else "INCOMPLETE",
        "scope": "archived native semantic head projection comparison",
        "authentic_admission": False,
        "matching_heads": matches,
        "matching_global_orderings": matching_orderings,
        "matching_global_points": matching_global_points,
        "unexplained_differences": differences,
        "outstanding_historical_coordinates": remaining,
        "source_projections_without_archived_coverage": unrepresented,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--archive", type=Path, required=True)
    parser.add_argument("--source-coverage", type=Path, required=True)
    args = parser.parse_args()
    archive, archive_hash = read_document(args.archive, 128 * 1024 * 1024)
    require(archive_hash == ARCHIVED_SHA256, "archived oracle SHA-256 mismatch")
    coverage, coverage_hash = read_document(args.source_coverage, 2 * 1024 * 1024)
    manifest, _ = read_document(args.source_coverage.parent / MANIFEST, 2 * 1024 * 1024)
    require(manifest["source_coverage_sha256"] == coverage_hash, "coverage manifest SHA-256 mismatch")
    report = ledger(archive, coverage, args.source_coverage.parent, manifest["artifact_sha256"])
    report["archived_oracle_sha256"] = archive_hash
    report["source_coverage_sha256"] = coverage_hash
    print(json.dumps(report, indent=2))
    return 1 if report["status"] == "FAIL" else 2


if __name__ == "__main__":
    raise SystemExit(main())
