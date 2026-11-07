"""Synthetic failure controls for exact Archive and project row comparison."""
from copy import deepcopy
from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from compare_authentic_archive import READER_SCHEMA, index_domains, verify_observation
from evidence_io import WitnessInputError


OWNER = {"kind": "REL", "owner_uuid": "11" * 16, "scope": "project"}
DIGEST = "22" * 32
PROOFS = {"archive": {"local_cut": "2"}, "project_operational": {"local_cut": "0"}}
PUBLICATION = {
    "projection_domain": "archive", "family": "publication", "stable_key_sha256": "33" * 32,
    "canonical_value_sha256": "44" * 32, "canonical_byte_length": "201",
    "origin_coordinate": {"archive_version": "1", "global_version": "2",
                          "transaction_time_ns": "-9007199254740995"},
    "availability": "retained",
}
PROJECT = {
    **PUBLICATION, "projection_domain": "project_operational", "family": "project_file_binding",
    "origin_coordinate": {}, "canonical_value_sha256": "66" * 32,
}


class AuthenticArchiveComparisonTests(unittest.TestCase):
    def setUp(self):
        self.expected = index_domains([PUBLICATION, PROJECT])
        self.report = {
            "schema_id": READER_SCHEMA, "status": "INCOMPLETE", "observation_status": "OBSERVED",
            "archive_head": "2", "candidate_sha256_before": DIGEST, "candidate_sha256_after": DIGEST,
            "candidate_unchanged": True, "rows": deepcopy([PUBLICATION, PROJECT]),
            "owner": {**OWNER, "history_generation_sha256": "55" * 32},
        }

    def check(self):
        return verify_observation(self.report, OWNER, PROOFS, DIGEST, self.expected)

    def test_typed_archive_and_project_populations_are_compared(self):
        generation, counts = self.check()
        self.assertEqual(generation, "55" * 32)
        self.assertEqual(counts, {"archive": {"publication": 1},
                                  "project_operational": {"project_file_binding": 1}})

    def test_altered_intermediate_publication_rejected_at_same_head(self):
        self.report["rows"][0]["canonical_value_sha256"] = "77" * 32
        with self.assertRaisesRegex(WitnessInputError, "archive differs.*1 altered"):
            self.check()

    def test_missing_embedded_project_reference_rejected(self):
        self.report["rows"].pop()
        with self.assertRaisesRegex(WitnessInputError, "project_operational differs.*1 missing"):
            self.check()

    def test_same_file_and_header_scope_are_required(self):
        self.report["candidate_sha256_after"] = "88" * 32
        with self.assertRaisesRegex(WitnessInputError, "custody"):
            self.check()
        self.report["candidate_sha256_after"] = DIGEST
        self.report["owner"]["scope"] = "unspecified"
        with self.assertRaisesRegex(WitnessInputError, "scope"):
            self.check()


if __name__ == "__main__":
    unittest.main()
