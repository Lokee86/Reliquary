"""Verify one current REL against its exact retained Archive and project source commitments."""
from __future__ import annotations

import argparse
from collections import Counter
import json
from pathlib import Path
import subprocess
import sys

from archive_binding_evidence import hex_digest
from authentic_witness import index_rows, read_source, require, retained_records, unsigned
from compare_archive_bindings import digest_file
from evidence_io import WitnessInputError, strict_json_document

SCHEMA = "reliquary-d02-authentic-archive-history-comparison-v1"
READER_SCHEMA = "reliquary-d02-archive-owner-history-v1"
FAMILIES = {
    "archive": {"publication", "content"},
    "project_operational": {"project_revision_correlation", "project_file_binding"},
}


def index_domains(rows):
    partitions = {domain: [] for domain in FAMILIES}
    for row in rows:
        domain = row["projection_domain"]
        require(domain in partitions, "target reported an unexpected Archive domain")
        partitions[domain].append(row)
    return {
        domain: index_rows(partitions[domain], domain, families)
        for domain, families in FAMILIES.items()
    }


def source_archive(coverage_path, coverage_digest, manifest_digest, source_digest):
    source, owner, artifacts = read_source(coverage_path, coverage_digest, manifest_digest, source_digest)
    require(owner["kind"] == "REL", "retained Archive comparison requires a REL source")
    expected = index_domains(
        row for row in retained_records(coverage_path.parent, source, artifacts)
        if row["projection_domain"] in FAMILIES
    )
    proofs = {}
    for domain in FAMILIES:
        selected = [proof for proof in source["native"]["native_history_projections"] if proof["domain"] == domain]
        require(len(selected) == 1 and unsigned(selected[0]["rows"]) == len(expected[domain]),
                f"{domain} native source head proof and commitments differ")
        proofs[domain] = selected[0]
    return owner, proofs, expected


def run_reader(reader, candidate):
    process = subprocess.run(
        [str(reader), "--kind", "rel", "--candidate", str(candidate), "--archive-owner-history"],
        capture_output=True, timeout=300, check=False,
    )
    require(process.returncode == 2 and not process.stderr, "unexpected current Archive reader exit/output")
    return strict_json_document(process.stdout, "current Archive reader")


def verify_observation(observed, owner, proofs, candidate_digest, expected):
    required = {
        "schema_id": READER_SCHEMA, "status": "INCOMPLETE", "observation_status": "OBSERVED",
        "archive_head": proofs["archive"]["local_cut"],
        "candidate_sha256_before": candidate_digest, "candidate_sha256_after": candidate_digest,
        "candidate_unchanged": True,
    }
    require(all(observed.get(key) == value for key, value in required.items()),
            "current Archive history observation is incomplete or its candidate custody differs")
    actual_owner = observed.get("owner")
    require(isinstance(actual_owner, dict) and all(actual_owner.get(key) == value for key, value in owner.items()),
            "candidate owner kind, identity, or scope differs from source")
    generation = hex_digest(actual_owner.get("history_generation_sha256"), 64, "target generation SHA-256")
    actual = index_domains(observed["rows"])
    counts = {}
    for domain in FAMILIES:
        old, new = expected[domain], actual[domain]
        missing, extra = old.keys() - new.keys(), new.keys() - old.keys()
        differences = [key for key in old.keys() & new.keys() if old[key] != new[key]]
        if missing or extra or differences:
            first = min([*missing, *extra, *differences])
            coordinate = (old.get(first) or new[first])["origin_coordinate"]
            raise WitnessInputError(
                f"retained {domain} differs in {first[0]} at {coordinate}, "
                f"{len(missing)} missing, {len(extra)} extra, {len(differences)} altered"
            )
        counts[domain] = dict(sorted(Counter(family for family, _ in new).items()))
    return generation, counts


def compare(reader, candidate, candidate_digest, coverage_path, coverage_digest, manifest_digest, source_digest):
    owner, proofs, expected = source_archive(coverage_path, coverage_digest, manifest_digest, source_digest)
    hex_digest(candidate_digest, 64, "candidate SHA-256")
    require(candidate_digest != source_digest and digest_file(candidate) == candidate_digest,
            "candidate SHA-256 differs or identifies the legacy source")
    reader_digest = digest_file(reader)
    observed = run_reader(reader, candidate)
    generation, counts = verify_observation(observed, owner, proofs, candidate_digest, expected)
    require(observed == run_reader(reader, candidate), "fresh-process Archive observations differ")
    require(digest_file(candidate) == candidate_digest and digest_file(reader) == reader_digest,
            "candidate or current reader changed during verification")
    return {
        "schema_id": SCHEMA, "status": "PASS", "scope": "one_owner_retained_archive_and_project_operational",
        "cohort_parity_status": "INCOMPLETE", "owner": owner, "target_history_generation_sha256": generation,
        "source_file_sha256": source_digest, "candidate_sha256": candidate_digest, "reader_sha256": reader_digest,
        "source_coverage_sha256": coverage_digest, "source_manifest_sha256": manifest_digest,
        "source_head_proofs": proofs, "archive_head": proofs["archive"]["local_cut"],
        "matched_rows_by_domain_and_family": {
            domain: {family: str(count) for family, count in families.items()} for domain, families in counts.items()
        },
        "fresh_process_repeats": "2",
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
        result = {"schema_id": SCHEMA, "status": "FAIL", "reason": str(error)}
    sys.stdout.write(json.dumps(result, ensure_ascii=False, sort_keys=True) + "\n")
    return 0 if result["status"] == "PASS" else 2


if __name__ == "__main__":
    raise SystemExit(main())
