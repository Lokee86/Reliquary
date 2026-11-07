"""Fingerprint pinned pre-settlement Freshness journal cuts without native prefix replay."""
import argparse
import hashlib
import json
from pathlib import Path

from f03_verify_archived_oracle import PINS, require
from f03_verify_dream_journal_frontiers import NATIVE_DOMAIN, encode, framed, u64
from f03_verify_freshness_birth_only import PLAN_SHA256, bind_points
from f03_verify_freshness_prefixes import POLICY, scan_owner, verify_commitments
from f03_verify_independent_repeat import MANIFEST, SCHEMA_V2, read_document, regular
from f03_verify_prefix_availability import write_jsonl


def native_row(family, key, value):
    return framed(family.encode()) + framed(key) + framed(encode(value))


class JournalProjection:
    """All pinned admissions and positive Dream effects occur at the queried turn."""

    def __init__(self, uuid, activity, births):
        self.uuid = uuid
        self.activity = activity
        self.births = tuple(sorted((bytes(value["memory_id"]), value) for value in births))
        require(self.births and all(value["source_turn"] <= activity and not value["admitted_at_birth"]
                                    for _, value in self.births), "unexpected journal birth clock or enrollment")
        self.unadmitted = {}
        self.admitted = {}
        self.initialized = bytearray()
        for identity, value in self.births:
            self.initialized.extend(native_row("initialize", identity, value))
            self.unadmitted[identity] = self.score_row(identity, None)
            self.admitted[identity] = self.score_row(identity, activity)
        self.activation = native_row("activation", b"current", {"seen": False, "activation": None})
        self.cut = native_row("cut", b"cut", {"mode": "activity", "rel_turn": activity,
                                             "event_id_inclusive": None, "graph_version": None})
        self.policy = native_row("policy", b"policy", POLICY)

    @staticmethod
    def score_row(identity, admission):
        # No decay interval elapses between admission, effects, and the query.
        # Positive effects saturate the initialized score at 100.
        return native_row("record", identity, {"score": 100, "admitted_at_turn": admission,
                                               "accounted_turn": admission})

    def encode_entry(self, end, family, key, value):
        require(value["source_turn"] == self.activity, "journal receipt has a different activity coordinate")
        if family == "admit":
            require(key in self.admitted, "journal admission refers to a missing birth")
        else:
            require(family == "event" and value["provenance"]["producer_kind"] == 1
                    and value["semantic"] is None and value["effects"]
                    and all(0 < amount <= 100 and bytes(identity) in self.unadmitted
                            for identity, amount in value["effects"]),
                    "journal contains effects outside the exact positive Dream scope")
        return end, family, key, native_row(family, key, value)

    def fingerprint(self, admissions, events):
        rows = 3 + 2 * len(self.births) + len(admissions) + len(events)
        digest = hashlib.sha256(NATIVE_DOMAIN + b"\1\0\0\0" + self.uuid + framed(b"freshness")
                                + u64(self.activity) + b"\0" + u64(rows))
        digest.update(self.activation)
        for identity in sorted(admissions):
            digest.update(admissions[identity])
        digest.update(self.cut)
        for identity in sorted(events):
            digest.update(events[identity])
        digest.update(self.initialized)
        digest.update(self.policy)
        for identity, _ in self.births:
            digest.update(self.admitted[identity] if identity in admissions else self.unadmitted[identity])
        return {"cut": self.activity, "event_cut": None, "rows": rows, "sha256": digest.hexdigest()}


def journal_proofs(owner, representatives, frontiers, commitments, receipts):
    activity = representatives[0]["activity"]
    births = (json.loads(value) for (family, _), value in commitments.items() if family == "initialize")
    projection = JournalProjection(bytes.fromhex(owner["owner_uuid"]), activity, births)
    entries = iter(projection.encode_entry(*row) for row in receipts)
    current = next(entries, None)
    admissions, events, results = {}, {}, {}
    evidence_chain = hashlib.sha256()
    previous_end = 0
    for row in representatives:
        g, end = row["global"], row["end_offset"]
        require(end >= previous_end and row["activity"] == activity
                and all(row[key] == frontiers[g][key] for key in
                        ("end_offset", "physical_prefix_sha256", "input_chain_sha256", "activity", "stage")),
                "journal representative differs from its exact physical inputs")
        while current is not None and current[0] <= end:
            _, family, key, encoded = current
            selected = admissions if family == "admit" else events
            require(key not in selected, "repeated journal evidence identity")
            selected[key] = encoded
            current = next(entries, None)
        require(bool(events), "expected a retained Dream event at every journal representative")
        proof = projection.fingerprint(admissions, events)
        evidence_chain.update(framed(encode([g, row["physical_prefix_sha256"], proof])))
        require(g not in results, "repeated journal representative coordinate")
        results[g] = {**row, "freshness_fingerprint": proof}
        previous_end = end
    return results, evidence_chain.hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("source-copies", "source-coverage", "prefix-plan", "output-directory"):
        parser.add_argument(f"--{name}", type=Path, required=True)
    args = parser.parse_args()
    output = args.source_coverage.parent
    coverage, coverage_sha = read_document(args.source_coverage, 2 * 1024 * 1024)
    manifest, manifest_sha = read_document(output / MANIFEST, 2 * 1024 * 1024)
    plan, plan_sha = read_document(args.prefix_plan, 2 * 1024 * 1024)
    require(plan_sha == PLAN_SHA256 and manifest_sha == plan["source_manifest_sha256"]
            and coverage_sha == manifest["source_coverage_sha256"] == plan["source_coverage_sha256"]
            and coverage["schema_id"] == manifest["schema_id"] == SCHEMA_V2,
            "source witness or replay plan provenance differs")
    destination = args.output_directory.resolve()
    regular(destination.parent, directory=True)
    regular(args.source_copies, directory=True)
    require(not any(destination.is_relative_to(path.resolve())
                    for path in (output, args.source_copies, args.prefix_plan.parent)),
            "journal evidence destination overlaps protected inputs")
    owners = {row["copy_id"]: row for row in coverage["sources"]}
    artifacts = dict(manifest["artifact_sha256"])
    sources = []
    for item in plan["sources"]:
        copy, fixture = item["copy_id"], item["fixture"]
        owner = owners[copy]
        require(owner["source_sha256"] == item["source_sha256"] == PINS[fixture]
                and owner["native"]["owner_kind"] == "reliquary", "journal source identity differs")
        receipts = []

        def retain_journal(end, family, key, value):
            receipts.append((end, family, key, value))

        frontiers, _, commitments, _, _ = scan_owner(output, artifacts, owner, args.source_copies / fixture,
                                                       observe_journal=retain_journal)
        families = verify_commitments(output, artifacts, owner, dict(commitments))
        representatives = [row for row in item["representatives"] if row["stage"] == "journal"]
        results, digest = journal_proofs(owner, representatives, frontiers, commitments, receipts)
        points = bind_points(args.prefix_plan.parent / f"{copy}-freshness-prefix-points.jsonl",
                             item, results, stage="journal")
        comparison = [{
            **{k: value for k, value in row.items() if k != "freshness_fingerprint"},
            "rows": row["freshness_fingerprint"]["rows"], "sha256": row["freshness_fingerprint"]["sha256"],
        } for row in results.values()]
        sources.append(({
            "copy_id": copy, "source_sha256": owner["source_sha256"],
            "accepted_input_rows_verified": {family: families[family] for family in ("initialize", "admit", "event")},
            "source_point_stream_sha256": item["point_stream_sha256"], "journal_cuts": len(points),
            "representative_chain_sha256": digest, "representatives": comparison,
        }, points))
    destination.mkdir()
    for source, points in sources:
        source["point_stream_sha256"] = write_jsonl(
            destination / f'{source["copy_id"]}-freshness-journal-points.jsonl', points)
    result = {
        "schema_id": "reliquary-f03-freshness-journal-prefix-v1",
        "scope": "independent exact Freshness components from pinned pre-settlement positive Dream journal",
        "authentic_admission": False, "whole_owner_positive_availability_assessed": False,
        "native_interior_replay_checked": False, "prefix_plan_sha256": plan_sha,
        "source_manifest_sha256": manifest_sha, "source_coverage_sha256": coverage_sha,
        "sources": [source for source, _ in sources],
    }
    with (destination / "freshness-journal-fingerprints.json").open("xb") as file:
        file.write(encode(result) + b"\n")


if __name__ == "__main__":
    main()
