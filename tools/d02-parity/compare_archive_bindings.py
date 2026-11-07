"""Compare a hash-pinned expected Archive binding history with one current REL."""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import sys

from archive_binding_evidence import (
    READER_SCHEMA,
    hex_digest,
    validate_binding_projection,
    validate_expectation,
)
from evidence_io import WitnessInputError, strict_json_document

REPORT_SCHEMA = "reliquary-d02-archive-binding-comparison-v1"


def digest_file(path: Path) -> str:
    with path.open("rb") as file:
        return hashlib.file_digest(file, "sha256").hexdigest()


def run_reader(reader: Path, candidate: Path, cut: str) -> dict:
    process = subprocess.run(
        [str(reader), "--kind", "rel", "--candidate", str(candidate), "--archive-binding-cut", cut],
        capture_output=True,
        timeout=300,
        check=False,
    )
    if process.returncode != 2 or process.stderr:
        raise WitnessInputError("unexpected Archive binding reader exit/output")
    return strict_json_document(process.stdout, "Archive binding reader")


def verify_observation(
    expectation: dict, expected_digest: str, observed: dict, candidate_sha256: str
) -> None:
    expected = expectation["binding_projection"]
    cut = expected["archive_version"]
    required = {
        "schema_id": READER_SCHEMA,
        "status": "INCOMPLETE",
        "archive_observation_status": "OBSERVED",
        "archive_binding_observation_status": "OBSERVED",
        "requested_archive_version": cut,
        "archive_head": cut,
        "candidate_sha256_before": candidate_sha256,
        "candidate_sha256_after": candidate_sha256,
        "candidate_unchanged": True,
    }
    if any(observed.get(key) != value for key, value in required.items()):
        raise WitnessInputError("Archive binding observation is incomplete or candidate custody differs")
    owner = observed.get("owner")
    if not isinstance(owner, dict) or any(owner.get(key) != value for key, value in expectation["owner"].items()):
        raise WitnessInputError("Archive binding owner identity or scope differs")
    hex_digest(owner.get("history_generation_sha256"), 64, "target history generation SHA-256")

    observed_projection = observed.get("binding_projection")
    actual_digest = validate_binding_projection(observed_projection)
    if observed.get("binding_projection_sha256") != actual_digest:
        raise WitnessInputError("Archive binding reader digest does not match its projection")
    if observed_projection["archive_version"] != cut:
        raise WitnessInputError("Archive binding projection cut differs")
    if actual_digest != expected_digest or observed_projection != expected:
        for index, (old, new) in enumerate(zip(expected["bindings"], observed_projection["bindings"]), 1):
            if old != new:
                raise WitnessInputError(f"Archive binding differs at local version {index}")
        raise WitnessInputError("Archive binding projection differs")


def compare(
    reader: Path, candidate: Path, candidate_sha256: str, expected_bytes: bytes, expected_sha256: str
) -> dict:
    hex_digest(candidate_sha256, 64, "candidate SHA-256")
    hex_digest(expected_sha256, 64, "expectation SHA-256")
    if hashlib.sha256(expected_bytes).hexdigest() != expected_sha256:
        raise WitnessInputError("Archive binding expectation SHA-256 differs")
    expectation = strict_json_document(expected_bytes, "Archive binding expectation")
    expected_digest = validate_expectation(expectation)
    if digest_file(candidate) != candidate_sha256:
        raise WitnessInputError("Archive binding candidate SHA-256 differs")
    reader_sha256 = digest_file(reader)
    cut = expectation["binding_projection"]["archive_version"]
    observed = run_reader(reader, candidate, cut)
    if observed != run_reader(reader, candidate, cut):
        raise WitnessInputError("fresh-process Archive binding observations differ")
    verify_observation(expectation, expected_digest, observed, candidate_sha256)
    if digest_file(candidate) != candidate_sha256 or digest_file(reader) != reader_sha256:
        raise WitnessInputError("Archive binding candidate or reader changed during verification")
    return {
        "schema_id": REPORT_SCHEMA,
        "status": "PASS",
        "scope": "one_owner_archive_publication_bindings_only",
        "cohort_parity_status": "INCOMPLETE",
        "owner": expectation["owner"],
        "target_history_generation_sha256": observed["owner"]["history_generation_sha256"],
        "source_file_sha256": expectation["source_file_sha256"],
        "expectation_sha256": expected_sha256,
        "candidate_sha256": candidate_sha256,
        "reader_sha256": reader_sha256,
        "archive_version": cut,
        "binding_projection_sha256": expected_digest,
        "fresh_process_repeats": "2",
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--reader", type=Path, required=True)
    parser.add_argument("--candidate", type=Path, required=True)
    parser.add_argument("--candidate-sha256", required=True)
    parser.add_argument("--expectation", type=Path, required=True)
    parser.add_argument("--expectation-sha256", required=True)
    args = parser.parse_args()
    try:
        result = compare(
            args.reader.resolve(strict=True),
            args.candidate.resolve(strict=True),
            args.candidate_sha256,
            args.expectation.read_bytes(),
            args.expectation_sha256,
        )
    except (OSError, WitnessInputError, subprocess.SubprocessError) as error:
        result = {"schema_id": REPORT_SCHEMA, "status": "FAIL", "reason": str(error)}
    sys.stdout.write(json.dumps(result, ensure_ascii=False, sort_keys=True) + "\n")
    return 0 if result["status"] == "PASS" else 2


if __name__ == "__main__":
    raise SystemExit(main())
