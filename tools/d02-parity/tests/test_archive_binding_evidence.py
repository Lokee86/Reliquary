from __future__ import annotations

from copy import deepcopy
from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from archive_binding_evidence import EXPECTATION_SCHEMA, READER_SCHEMA, validate_expectation
from compare_archive_bindings import verify_observation
from evidence_io import WitnessInputError

from check_bound_archive_candidate import OWNER, expected_binding_projection


def expectation() -> dict:
    return {
        "schema_id": EXPECTATION_SCHEMA,
        "source_file_sha256": "a" * 64,
        "owner": {field: OWNER[field] for field in ("kind", "owner_uuid", "scope")},
        "binding_projection": expected_binding_projection(6),
    }


def observed(expected: dict) -> dict:
    return {
        "schema_id": READER_SCHEMA,
        "status": "INCOMPLETE",
        "archive_observation_status": "OBSERVED",
        "archive_binding_observation_status": "OBSERVED",
        "requested_archive_version": "6",
        "archive_head": "6",
        "owner": dict(OWNER),
        "candidate_sha256_before": "b" * 64,
        "candidate_sha256_after": "b" * 64,
        "candidate_unchanged": True,
        "binding_projection": deepcopy(expected["binding_projection"]),
        "binding_projection_sha256": validate_expectation(expected),
    }


class ArchiveBindingEvidenceTests(unittest.TestCase):
    def test_valid_nonmonotonic_global_order_and_signed_times(self) -> None:
        expected = expectation()
        self.assertEqual(expected["binding_projection"]["bindings"][1]["global_version"], "1")
        self.assertEqual(expected["binding_projection"]["bindings"][1]["transaction_time_ns"], "-9007199254740995")
        verify_observation(expected, validate_expectation(expected), observed(expected), "b" * 64)

    def test_recomputed_digest_does_not_hide_binding_changes(self) -> None:
        expected = expectation()
        for field, value, local in (
            ("transaction_time_ns", "-9007199254740994", 2),
            ("clock_mode", "bind_reserved", 5),
        ):
            with self.subTest(field=field):
                altered = expectation()
                altered["binding_projection"]["bindings"][local - 1][field] = value
                with self.assertRaisesRegex(WitnessInputError, f"differs at local version {local}"):
                    verify_observation(expected, validate_expectation(expected), observed(altered), "b" * 64)

    def test_repeat_must_point_directly_to_the_first_applying_row(self) -> None:
        expected = expectation()
        expected["binding_projection"]["bindings"][5]["first_applying_archive_version"] = "4"
        expected["binding_projection"]["bindings"][5]["first_applying_global_version"] = "3"
        with self.assertRaisesRegex(WitnessInputError, "does not resolve"):
            validate_expectation(expected)

    def test_omission_and_oversized_signed_clock_are_rejected(self) -> None:
        expected = expectation()
        expected["binding_projection"]["bindings"].pop(2)
        with self.assertRaisesRegex(WitnessInputError, "complete local cut"):
            validate_expectation(expected)
        expected = expectation()
        expected["binding_projection"]["bindings"][1]["transaction_time_ns"] = "-9223372036854775809"
        with self.assertRaisesRegex(WitnessInputError, "outside the supported integer range"):
            validate_expectation(expected)

    def test_candidate_custody_and_raw_source_scope_are_required(self) -> None:
        expected = expectation()
        report = observed(expected)
        report["candidate_sha256_after"] = "c" * 64
        with self.assertRaisesRegex(WitnessInputError, "custody differs"):
            verify_observation(expected, validate_expectation(expected), report, "b" * 64)
        report = observed(expected)
        report["owner"]["scope"] = "unspecified"
        with self.assertRaisesRegex(WitnessInputError, "scope differs"):
            verify_observation(expected, validate_expectation(expected), report, "b" * 64)


if __name__ == "__main__":
    unittest.main()
