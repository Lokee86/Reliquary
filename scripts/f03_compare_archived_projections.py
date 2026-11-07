"""Compare the exported native head projections to the immutable archived oracle."""
import argparse
import hashlib
import json
from pathlib import Path

from f03_verify_archived_oracle import PINS, require
from f03_verify_independent_repeat import MANIFEST, SCHEMAS, read_document, take_stream

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


def global_ordering(owner, output, artifacts, schema_id):
    hash_ = hashlib.sha256()
    metadata = owner["native"]["global_stream"]
    count = 0
    for ordinal, row in take_stream(output, artifacts, owner, "global", metadata, schema_id=schema_id):
        require(ordinal == int(row["global_version"]), "out-of-order global coordinate")
        domains = row["domain_versions"]
        point = [ordinal, None if row["transaction_time_ns"] is None else int(row["transaction_time_ns"])]
        point.extend(int(domains.get(name, "0")) for name in (
            "archive", "memory", "entity", "relationship", "graph", "memory_graph", "vector_generation"
        ))
        hash_.update(json.dumps(point, separators=(",", ":")).encode("ascii"))
        count += 1
    return {"exhaustive": True, "points": count, "sha256": hash_.hexdigest()}



def native_proof(proof):
    return {"cut": int(proof["local_cut"]), "rows": int(proof["rows"]),
            "event_cut": None if proof["event_cut"] is None else int(proof["event_cut"]),
            "sha256": proof["sha256"]}


def freshness_activity(oracle, owner, output, artifacts, differences, schema_id):
    expected = oracle["freshness"]["fingerprints"]
    proofs = owner["native"].get("freshness_activity_proofs")
    if not proofs or proofs["coverage"] != "all_activity_cuts":
        return 0, len(expected)
    metadata = proofs["stream"]
    require(len(expected) == int(metadata["rows"]), "Freshness activity cut population differs")
    matches = 0
    for ordinal, row in take_stream(output, artifacts, owner, "freshness-activity", metadata, schema_id=schema_id):
        require(row["activity"] == str(ordinal), "Freshness activity is missing or out of order")
        state = row["state"]
        if (state["availability"] == "supported" and state["proof"]["domain"] == "freshness"
                and native_proof(state["proof"]) == expected[ordinal]):
            matches += 1
        else:
            differences.append({
                "copy_id": owner["copy_id"], "domain": "freshness_activity", "activity": str(ordinal),
                "status": "unexplained_projection_difference",
                "archived": expected[ordinal], "frs07": state,
            })
    return matches, 0


def freshness_events(oracle, owner, output, artifacts, differences, schema_id):
    expected = {}
    for row in oracle["freshness_events"]:
        key = str(row["turn"]), hashlib.sha256(row["event"].encode("utf-8")).hexdigest()
        require(key not in expected, "duplicate archived Freshness event cut")
        expected[key] = row["outcome"]
    archived_graph = str(oracle["common"]["memory_graph"][-1]["cut"])
    metadata = owner["native"]["freshness_stream"]
    comparable = 0
    reconciled = []
    pending = []
    for _, row in take_stream(output, artifacts, owner, "freshness", metadata, schema_id=schema_id):
        if row["kind"] == "settlement":
            continue
        require(row["kind"] == "accepted_event_cut", "unknown Freshness stream entry")
        key = row["activity"], row["event_id_sha256"]
        require(key in expected, "unknown or duplicate retained Freshness event cut")
        want = expected.pop(key)
        got = row["state"]
        coordinates = {
            "copy_id": owner["copy_id"], "activity": key[0], "event_id_sha256": key[1],
            "archived_memory_graph_version": archived_graph,
            "frs07_memory_graph_version": row["memory_graph_version"],
        }
        if ("rejected" in want and
                want["rejected"] == "import settlement cut does not retain an intra-settlement trajectory" and
                got == {"availability": "unavailable", "reason": "intra_settlement_trajectory_not_retained"}):
            comparable += 1
            reconciled.append({
                **coordinates, "archived_schema": "archived-canonical-history-v1",
                "frs07_schema": schema_id, "availability": "unavailable",
                "reason": "intra_settlement_trajectory_not_retained",
            })
        elif (row["memory_graph_version"] == archived_graph and got["availability"] == "supported"
              and "rejected" not in want and native_proof(got["proof"]) == want):
            comparable += 1
        elif row["memory_graph_version"] != archived_graph and got["availability"] == "supported" and "rejected" not in want:
            pending.append({**coordinates, "reason": "supported_event_requires_common_graph_coordinate"})
        else:
            differences.append({**coordinates, "domain": "freshness_event",
                                "status": "unexplained_projection_difference",
                                "archived": want, "frs07": got})
    require(not expected, "retained Freshness event stream omitted archived identities")
    return comparable, reconciled, pending

def ledger(archived, coverage, output, artifacts, archive_replay_sha=None):
    schema_id = coverage["schema_id"]
    require(schema_id in SCHEMAS and coverage["authentic_admission"] is False,
            "unsupported historical exporter coverage schema")
    sources = coverage["sources"]
    rows = {row["fixture"]: row for row in archived["fixtures"]}
    require(len(sources) == len(PINS) and len(rows) == len(PINS)
            and rows.keys() == PINS.keys(), "six-owner witness is required")
    matches = 0
    matching_orderings = 0
    matching_global_points = 0
    matching_freshness_activity = 0
    matching_freshness_events = 0
    unavailable_freshness_events = []
    pending_freshness_events = []
    differences = []
    remaining = []
    unavailable_archive_projections = []
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
        ordering = global_ordering(owner, output, artifacts, schema_id)
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
            activity_matches, activity_omitted = freshness_activity(oracle, owner, output, artifacts, differences, schema_id)
            event_matches, reconciled, pending = freshness_events(oracle, owner, output, artifacts, differences, schema_id)
            matching_freshness_activity += activity_matches
            matching_freshness_events += event_matches
            unavailable_freshness_events.extend(reconciled)
            pending_freshness_events.extend(pending)
            for projection in oracle["global_projection"]:
                if "rejected" in projection:
                    require(projection["rejected"] == "missing Archive reference",
                            "archived composed cut has an unclassified rejection")
                    unavailable_archive_projections.append({
                        "copy_id": owner["copy_id"], "global_version": str(projection["cut"]),
                        "availability": "unavailable", "reason": "missing Archive reference",
                        "fresh_legacy_replay_verified": archive_replay_sha is not None,
                    })
            remaining.append({
                "copy_id": owner["copy_id"],
                "whole_owner_archived_cuts": rows[relative]["global_projection_points"],
                "archive_archived_cut_count": len(oracle["archive"]["snapshots"]),
                "archive_archived_cuts_exhaustive": oracle["archive"]["snapshot_queries_exhaustive"],
                "freshness_archived_event_cut_count": len(oracle["freshness_events"]),
                "freshness_archived_activity_cuts_omitted": activity_omitted,
                "freshness_cut_basis": "archived event query uses current MemoryGraph head; FRS07 source witness uses each event's recorded proof coordinate",
                "disposition": ("archived projection reproduced at the frozen coordinates"
                                if archive_replay_sha else "fresh archived projection replay required"),
            })
    return {
        "status": "FAIL" if differences else "INCOMPLETE",
        "scope": "archived native semantic head projection comparison",
        "authentic_admission": False,
        "fresh_archived_replay_audit_sha256": archive_replay_sha,
        "archive_projection_reconciliation": (
            "FAIL" if differences else "PASS" if archive_replay_sha and not pending_freshness_events else "INCOMPLETE"
        ),
        "matching_heads": matches,
        "matching_global_orderings": matching_orderings,
        "matching_global_points": matching_global_points,
        "matching_freshness_activity_cuts": matching_freshness_activity,
        "matching_freshness_event_cuts": matching_freshness_events,
        "matched_unavailable_freshness_events": unavailable_freshness_events,
        "pending_freshness_event_differences": pending_freshness_events,
        "unexplained_differences": differences,
        "archived_projection_coverage": remaining,
        "archived_unavailable_global_projections": unavailable_archive_projections,
        "source_projections_without_archived_coverage": unrepresented,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--archive", type=Path, required=True)
    parser.add_argument("--source-coverage", type=Path, required=True)
    parser.add_argument("--audited-archive-replay", type=Path)
    args = parser.parse_args()
    archive, archive_hash = read_document(args.archive, 128 * 1024 * 1024)
    require(archive_hash == ARCHIVED_SHA256, "archived oracle SHA-256 mismatch")
    coverage, coverage_hash = read_document(args.source_coverage, 2 * 1024 * 1024)
    manifest, _ = read_document(args.source_coverage.parent / MANIFEST, 2 * 1024 * 1024)
    require(manifest["source_coverage_sha256"] == coverage_hash, "coverage manifest SHA-256 mismatch")
    replay_sha = None
    if args.audited_archive_replay:
        replay, replay_sha = read_document(args.audited_archive_replay, 64 * 1024)
        require(replay["status"] == "PASS"
                and replay["scope"] == "fresh optimized six-file archived oracle replay"
                and replay["archived_oracle_sha256"] == archive_hash
                and replay["fixture_count"] == len(PINS) and replay["source_sha256"] == PINS,
                "audited archive replay does not bind this fixture population")
    report = ledger(archive, coverage, args.source_coverage.parent, manifest["artifact_sha256"], replay_sha)
    report["archived_oracle_sha256"] = archive_hash
    report["source_coverage_sha256"] = coverage_hash
    print(json.dumps(report, indent=2))
    return 1 if report["status"] == "FAIL" else 2


if __name__ == "__main__":
    raise SystemExit(main())
