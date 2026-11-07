"""Compare a current REL's retained Freshness authority and score anchors to the pinned F03 source."""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys

from archive_binding_evidence import hex_digest
from authentic_witness import require
from compare_archive_bindings import digest_file
from evidence_io import WitnessInputError, reject_json_number, strict_json_document, unique_object
from freshness_journal import compare_journal

SCHEMA = "reliquary-d02-authentic-freshness-comparison-v1"
READER_SCHEMA = "reliquary-d02-retained-freshness-v1"
ANCHOR_DOMAIN = b"reliquary:d02:retained-freshness-anchors:v1\0"
COORDINATES = ("base_cut", "settlement_cut", "global_version", "archive_version",
               "graph_version", "memory_graph_version")


def source_json(path: Path, expected: str, limit: int) -> dict:
    hex_digest(expected, 64, "Freshness source artifact SHA-256")
    with path.open("rb") as file:
        raw = file.read(limit + 1)
    require(0 < len(raw) <= limit and hashlib.sha256(raw).hexdigest() == expected,
            "Freshness source artifact size or SHA-256 differs")
    return json.loads(raw.decode("utf-8"), object_pairs_hook=unique_object,
                      parse_float=reject_json_number, parse_constant=reject_json_number)


def integer(value, minimum=0, maximum=(1 << 64) - 1):
    require(type(value) is int and minimum <= value <= maximum, "invalid Freshness source integer")
    return value


def records_sha256(rows):
    digest = hashlib.sha256(ANCHOR_DOMAIN)
    previous = ""
    for identity, record in rows:
        hex_digest(identity, 64, "Freshness Memory identity")
        require(identity > previous and record.keys() == {"score", "admitted_at_turn", "accounted_turn"},
                "Freshness source records must have ordered unique identities and exact anchors")
        previous = identity
        digest.update(bytes.fromhex(identity))
        digest.update(integer(record["score"], -100, 100).to_bytes(2, "little", signed=True))
        for key in ("admitted_at_turn", "accounted_turn"):
            anchor = record[key]
            digest.update(b"\0" if anchor is None else b"\1" + integer(anchor).to_bytes(8, "little"))
    return digest.hexdigest()


def source_summary(receipt_path, receipt_digest, source_digest):
    receipt = source_json(receipt_path, receipt_digest, 64 * 1024)
    require(receipt.get("schema_id") == "reliquary-f03-freshness-source-transfer-v1"
            and receipt.get("target_conversion_verified") is False, "wrong Freshness source receipt schema")
    selected = [row for row in receipt["sources"] if row["source_sha256"] == source_digest]
    require(len(selected) == 1 and bool(re.fullmatch(r"copy-[0-9]{4}", selected[0]["copy_id"])),
            "Freshness receipt does not identify exactly one F03 REL")
    return selected[0]


def expectation(receipt_path, receipt_digest, source_digest):
    summary = source_summary(receipt_path, receipt_digest, source_digest)
    path = receipt_path.with_name(f'{summary["copy_id"]}-freshness-frs07-authority.json')
    artifact = source_json(path, summary["settlement_sha256"], 16 * 1024 * 1024)
    value = artifact["settlement"]
    require(artifact["carrier"] == "CVAFRS07"
            and artifact["native_canonical_value_sha256"] == summary["settlement_native_value_sha256"]
            and value["owner"] == summary["owner_uuid"]
            and value["global_version"] == artifact["physical_global_frontier"]
            and len(value["records"]) == summary["settlement_records"] == summary["grouped_births_bound_in_memory_transfer"],
            "Freshness source authority provenance or record count differs")
    return {
        "owner_uuid": hex_digest(value["owner"], 32, "source owner UUID"),
        "import_id": hex_digest(value["import_id"], 64, "source import ID"),
        "source_payload_sha256": hex_digest(artifact["source_payload_sha256"], 64, "source proof SHA-256"),
        **{name: str(integer(value[name])) for name in COORDINATES},
        "journal_entries_included": str(integer(summary["accepted_admissions"])
                                       + integer(summary["accepted_events"]) + integer(summary["settlement_records"])),
        "records": str(len(value["records"])), "records_sha256": records_sha256(value["records"]),
    }


def run_reader(reader, candidate):
    process = subprocess.run(
        [str(reader), "--kind", "rel", "--candidate", str(candidate), "--retained-freshness"],
        capture_output=True, timeout=300, check=False,
    )
    require(process.returncode == 2 and not process.stderr, "unexpected current Freshness reader exit/output")
    return strict_json_document(process.stdout, "current Freshness reader")


def compare(reader, candidate, candidate_digest, receipt, receipt_digest, source_digest):
    hex_digest(candidate_digest, 64, "candidate SHA-256")
    hex_digest(source_digest, 64, "legacy owner SHA-256")
    expected = expectation(receipt, receipt_digest, source_digest)
    require(candidate_digest != source_digest and digest_file(candidate) == candidate_digest,
            "candidate SHA-256 differs or identifies the legacy source")
    reader_digest = digest_file(reader)
    observed = run_reader(reader, candidate)
    required = {
        "schema_id": READER_SCHEMA, "status": "INCOMPLETE", "observation_status": "OBSERVED",
        "candidate_sha256_before": candidate_digest, "candidate_sha256_after": candidate_digest,
        "candidate_unchanged": True,
    }
    require(all(observed.get(key) == value for key, value in required.items()),
            "current Freshness observation is incomplete or its candidate custody differs")
    owner = observed["owner"]
    require(owner["kind"] == "REL" and owner["owner_uuid"] == expected["owner_uuid"],
            "Freshness target owner differs from the pinned source")
    actual = observed.get("projection")
    require(isinstance(actual, dict) and actual.keys() == expected.keys(), "current Freshness projection shape differs")
    differences = [key for key, value in expected.items() if actual[key] != value]
    require(not differences, f"retained Freshness values differ in {', '.join(differences)}")
    summary = source_summary(receipt, receipt_digest, source_digest)
    journal_rows = compare_journal(
        candidate, owner, candidate_digest, expected,
        receipt.with_name(f'{summary["copy_id"]}-freshness-journal.jsonl'), summary["journal_sha256"],
    )
    require(observed == run_reader(reader, candidate), "fresh-process Freshness observations differ")
    require(digest_file(candidate) == candidate_digest and digest_file(reader) == reader_digest,
            "candidate or current reader changed during verification")
    return {
        "schema_id": SCHEMA, "status": "PASS", "scope": "one_owner_retained_freshness_authority_journal_and_score_anchors",
        "cohort_parity_status": "INCOMPLETE", "source_file_sha256": source_digest,
        "candidate_sha256": candidate_digest, "reader_sha256": reader_digest,
        "source_transfer_receipt_sha256": receipt_digest, "projection": expected,
        "direct_journal_rows": str(journal_rows), "fresh_process_repeats": "2",
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--reader", type=Path, required=True)
    parser.add_argument("--candidate", type=Path, required=True)
    parser.add_argument("--candidate-sha256", required=True)
    parser.add_argument("--source-transfer-receipt", type=Path, required=True)
    parser.add_argument("--source-transfer-receipt-sha256", required=True)
    parser.add_argument("--source-file-sha256", required=True)
    args = parser.parse_args()
    try:
        result = compare(
            args.reader.resolve(strict=True), args.candidate.resolve(strict=True), args.candidate_sha256,
            args.source_transfer_receipt.resolve(strict=True), args.source_transfer_receipt_sha256,
            args.source_file_sha256,
        )
    except (OSError, ValueError, KeyError, TypeError, subprocess.SubprocessError) as error:
        result = {"schema_id": SCHEMA, "status": "FAIL", "reason": str(error)}
    sys.stdout.write(json.dumps(result, ensure_ascii=False, sort_keys=True) + "\n")
    return 0 if result["status"] == "PASS" else 2


if __name__ == "__main__":
    raise SystemExit(main())
