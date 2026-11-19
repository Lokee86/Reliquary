"""Compare current Memory history commitments with one hash-pinned F03 owner."""
from __future__ import annotations

import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import subprocess
import sys
from uuid import UUID

from archive_binding_evidence import hex_digest
from authentic_witness import index_rows, read_source, require, retained_records, unsigned
from compare_archive_bindings import digest_file
from compare_clocked_prefix import pinned_rows
from evidence_io import WitnessInputError, strict_json_document

READER_SCHEMA = "reliquary-d02-memory-owner-history-v1"
REPORT_SCHEMA = "reliquary-d02-authentic-memory-history-comparison-v1"
OBSERVED = {"revision", "body", "current", "terminal", "routing"}
OTHER_SOURCE = {"external_evidence"}


def source_memory(coverage_path, coverage_digest, manifest_digest, source_digest):
    source, owner, artifacts = read_source(coverage_path, coverage_digest, manifest_digest, source_digest)
    rows = index_rows(
        (row for row in retained_records(coverage_path.parent, source, artifacts)
         if row["projection_domain"] == "memory"),
        "memory", OBSERVED | OTHER_SOURCE,
    )
    proofs = [proof for proof in source["native"]["native_history_projections"] if proof["domain"] == "memory"]
    require(len(proofs) == 1 and unsigned(proofs[0]["rows"]) == len(rows),
            "source Memory head proof and commitments differ")
    return owner, proofs[0], rows


def run_reader(reader: Path, candidate: Path, kind: str) -> dict:
    process = subprocess.run(
        [str(reader), "--kind", kind.lower(), "--candidate", str(candidate), "--memory-owner-history"],
        capture_output=True, timeout=300, check=False,
    )
    require(process.returncode == 2 and not process.stderr, "unexpected current Memory reader exit/output")
    return strict_json_document(process.stdout, "current Memory reader")


def reviewed_source_projection(expected, source_digest, coverage_digest, manifest_digest, paired):
    receipt_path, pinned_sha, rel_id, kind = paired
    require(digest_file(receipt_path) == pinned_sha, "Memory source transfer receipt changed")
    receipt = json.loads(receipt_path.read_bytes())
    require(receipt["source_coverage_sha256"] == coverage_digest
            and receipt["source_manifest_sha256"] == manifest_digest, "Memory transfer differs from the frozen witness")
    selected, = [row for row in receipt["sources"] if row["source_sha256"] == source_digest]
    transfer = receipt_path.with_name(f"{selected['copy_id']}-memory-transfer.jsonl")
    changes = Counter()
    for row in pinned_rows(transfer, selected["transfer_sha256"]):
        for item in row["revisions"]:
            record = item["record"]
            source = record["source_ref"]
            if source is not None and source["owner_id"] == f"proj-{UUID(rel_id)}":
                canonical_ref = dict(source, owner_id=rel_id)
                reason = "paired_rel_owner_uuid"
            elif (kind == "REL" and source is None and record["source_episode_id"] and record["source_node_id"]):
                fields = (
                    "source_episode_id", "source_node_id",
                    "content_source_conversation_id", "content_source_node_id",
                    "grounding_source_conversation_id", "grounding_source_node_id",
                )
                canonical_ref = dict(((field, record[field]) for field in fields), owner_id=rel_id, principal_id=None)
                reason = "archive_episode_and_node"
            else:
                continue
            key = ("revision", hashlib.sha256(record["memory_version"].to_bytes(8, "big")).hexdigest())
            original = json.dumps(record, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()
            row = expected[key]
            require(hashlib.sha256(original).hexdigest() == row["canonical_value_sha256"]
                    and str(len(original)) == row["canonical_byte_length"],
                    "typed Memory source differs from its original commitment")
            native = json.dumps(dict(record, source_ref=canonical_ref), sort_keys=True,
                                separators=(",", ":"), ensure_ascii=False).encode()
            expected[key] = dict(row, canonical_value_sha256=hashlib.sha256(native).hexdigest(),
                                 canonical_byte_length=str(len(native)))
            changes[reason] += 1
    return dict(changes)


def verify_observation(observed, owner, source_head, candidate_digest, expected):
    required = {
        "schema_id": READER_SCHEMA, "status": "INCOMPLETE", "observation_status": "OBSERVED",
        "memory_head": source_head["local_cut"], "candidate_sha256_before": candidate_digest,
        "candidate_sha256_after": candidate_digest, "candidate_unchanged": True,
    }
    require(all(observed.get(key) == value for key, value in required.items()),
            "current Memory history observation is incomplete or its candidate custody differs")
    actual_owner = observed.get("owner")
    require(isinstance(actual_owner, dict) and all(actual_owner.get(key) == value for key, value in owner.items()),
            "candidate owner kind, identity, or scope differs from source")
    hex_digest(actual_owner.get("history_generation_sha256"), 64, "candidate history generation SHA-256")
    actual = index_rows(observed["rows"], "memory", OBSERVED)
    comparable = {key: row for key, row in expected.items() if key[0] in OBSERVED}
    missing = comparable.keys() - actual.keys()
    extra = actual.keys() - comparable.keys()
    differences = [key for key in comparable.keys() & actual.keys() if comparable[key] != actual[key]]
    if missing or extra or differences:
        first = min([*missing, *extra, *differences])
        coordinate = (comparable.get(first) or actual[first])["origin_coordinate"]
        raise WitnessInputError(
            f"retained Memory history differs in {first[0]} at {coordinate}, "
            f"{len(missing)} missing, {len(extra)} extra, {len(differences)} altered"
        )
    return actual_owner["history_generation_sha256"], Counter(key[0] for key in actual)


def compare(reader, candidate, candidate_digest, coverage_path, coverage_digest, manifest_digest, source_digest,
            *, paired=None):
    owner, head, expected = source_memory(coverage_path, coverage_digest, manifest_digest, source_digest)
    mappings = reviewed_source_projection(expected, source_digest, coverage_digest, manifest_digest, paired) if paired else {}
    hex_digest(candidate_digest, 64, "candidate SHA-256")
    require(candidate_digest != source_digest and digest_file(candidate) == candidate_digest,
            "candidate SHA-256 differs or identifies the legacy source")
    reader_digest = digest_file(reader)
    observed = run_reader(reader, candidate, owner["kind"])
    generation, matched = verify_observation(observed, owner, head, candidate_digest, expected)
    require(digest_file(candidate) == candidate_digest and digest_file(reader) == reader_digest,
            "candidate or current reader changed during verification")
    unresolved = Counter(family for family, _ in expected if family in OTHER_SOURCE)
    return {
        "schema_id": REPORT_SCHEMA, "status": "INCOMPLETE" if unresolved else "PASS",
        "comparison_status": "PASS", "scope": "one_owner_retained_memory_revisions_bodies_current_terminals_and_routing",
        "cohort_parity_status": "INCOMPLETE", "owner": owner, "target_history_generation_sha256": generation,
        "source_file_sha256": source_digest, "candidate_sha256": candidate_digest, "reader_sha256": reader_digest,
        "source_coverage_sha256": coverage_digest, "source_manifest_sha256": manifest_digest,
        "source_memory_head_sha256": head["sha256"], "memory_head": head["local_cut"],
        "matched_rows_by_family": {key: str(value) for key, value in sorted(matched.items())},
        "unobserved_source_rows_by_family": {key: str(value) for key, value in sorted(unresolved.items())},
        "reviewed_provenance_mappings": mappings,
        "fresh_process_observations": "1",
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--reader", type=Path, required=True)
    parser.add_argument("--candidate", type=Path, required=True)
    parser.add_argument("--candidate-sha256", required=True)
    parser.add_argument("--source-coverage", type=Path, required=True)
    parser.add_argument("--source-coverage-sha256", required=True)
    parser.add_argument("--source-manifest-sha256", required=True)
    parser.add_argument("--source-file-sha256", required=True)
    args = parser.parse_args()
    try:
        result = compare(
            args.reader.resolve(strict=True), args.candidate.resolve(strict=True), args.candidate_sha256,
            args.source_coverage.resolve(strict=True), args.source_coverage_sha256,
            args.source_manifest_sha256, args.source_file_sha256,
        )
    except (OSError, ValueError, KeyError, TypeError, subprocess.SubprocessError) as error:
        result = {"schema_id": REPORT_SCHEMA, "status": "FAIL", "reason": str(error)}
    sys.stdout.write(json.dumps(result, ensure_ascii=False, sort_keys=True) + "\n")
    return 0 if result["status"] == "PASS" else 2


if __name__ == "__main__":
    raise SystemExit(main())
