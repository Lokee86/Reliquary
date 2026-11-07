"""Synthetic unit controls for the authentic retained-Memory comparator."""
import copy
from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from compare_authentic_memory import READER_SCHEMA, verify_observation
from evidence_io import WitnessInputError


OWNER = {"kind": "PHY", "owner_uuid": "11" * 16, "scope": "unspecified"}
HEAD = {"local_cut": "2"}
CANDIDATE = "22" * 32
ROW = {
    "projection_domain": "memory", "family": "revision", "stable_key_sha256": "33" * 32,
    "canonical_value_sha256": "44" * 32, "canonical_byte_length": "100",
    "origin_coordinate": {"memory_version": "1", "global_version": "2", "transaction_time_ns": "-9007199254740995"},
    "availability": "retained",
}
ROUTING = {
    **ROW, "family": "routing", "origin_coordinate": {},
}


class AuthenticMemoryComparisonTests(unittest.TestCase):
    def setUp(self):
        self.expected = {("revision", "33" * 32): ROW, ("routing", "33" * 32): ROUTING}
        self.report = {
            "schema_id": READER_SCHEMA, "status": "INCOMPLETE", "observation_status": "OBSERVED",
            "memory_head": "2", "candidate_sha256_before": CANDIDATE, "candidate_sha256_after": CANDIDATE,
            "candidate_unchanged": True, "rows": [copy.deepcopy(ROW)],
            "owner": {**OWNER, "history_generation_sha256": "55" * 32},
        }

    def check(self):
        return verify_observation(self.report, OWNER, HEAD, CANDIDATE, self.expected)

    def test_retained_revision_matches_with_unobserved_routing(self):
        generation, counts = self.check()
        self.assertEqual(generation, "55" * 32)
        self.assertEqual(counts, {"revision": 1})

    def test_changed_intermediate_coordinate_rejected_at_same_head(self):
        self.report["rows"][0]["origin_coordinate"]["transaction_time_ns"] = "-9007199254740994"
        with self.assertRaisesRegex(WitnessInputError, "1 altered"):
            self.check()

    def test_missing_and_unexpected_rows_rejected(self):
        self.report["rows"] = [{**ROW, "stable_key_sha256": "66" * 32}]
        with self.assertRaisesRegex(WitnessInputError, "1 missing, 1 extra"):
            self.check()

    def test_duplicate_row_rejected(self):
        self.report["rows"].append(ROW)
        with self.assertRaisesRegex(WitnessInputError, "duplicate"):
            self.check()

    def test_scope_and_read_only_custody_are_required(self):
        self.report["candidate_unchanged"] = False
        with self.assertRaisesRegex(WitnessInputError, "custody"):
            self.check()
        self.report["candidate_unchanged"] = True
        self.report["owner"]["scope"] = "project"
        with self.assertRaisesRegex(WitnessInputError, "scope"):
            self.check()


if __name__ == "__main__":
    unittest.main()
