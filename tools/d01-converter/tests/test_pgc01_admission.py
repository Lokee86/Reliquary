import json
import os
import sys
import tempfile
import unittest
from dataclasses import replace
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from admission import (
    AdmissionDenied,
    AdmissionScope,
    CarrierClassification,
    CarrierKind,
    admit_for_scope,
    classify_source_marker,
    classify_synthetic_frame,
    require_source_admission,
)
from evidence import build_incomplete_report, validate_incomplete_report
from pgc01_adapter import (
    PINNED_COMMIT,
    PINNED_REGISTRY_HASHES,
    ContractSnapshotError,
    load_pinned_snapshot,
)
from synthetic import build_frame


_CONTRACT_ROOT = os.environ.get("RELIQUARY_PG_C01_ROOT")


@unittest.skipUnless(_CONTRACT_ROOT, "set RELIQUARY_PG_C01_ROOT to the PG-C01 contract root")
class PGC01AdmissionTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.contract_root = Path(_CONTRACT_ROOT)
        cls.snapshot = load_pinned_snapshot(cls.contract_root)
        cls.tool_root = Path(__file__).resolve().parents[1]

    def test_current_contract_pin_is_exact_and_authentic_admission_stays_closed(self):
        self.assertEqual(self.snapshot.revision, PINNED_COMMIT)
        self.assertEqual(self.snapshot.registry_hashes, PINNED_REGISTRY_HASHES)
        self.assertEqual(self.snapshot.carriers["status"], "unclassified-no-authenticated-inventory")
        self.assertEqual(self.snapshot.carriers["entries"], [])
        self.assertEqual(self.snapshot.carriers["unclassified_carriers"], [])
        self.assertFalse(self.snapshot.authentic_admission)

    def test_candidate_marker_is_unclassified_not_a_synthetic_or_authentic_carrier(self):
        result = classify_source_marker("CVAAFMT2", self.snapshot)
        self.assertEqual(result.kind, CarrierKind.UNCLASSIFIED_SOURCE_MARKER)
        self.assertIsNone(result.family_id)
        self.assertIsNone(result.target_schema_id)
        self.assertFalse(result.source_admissible)
        with self.assertRaisesRegex(AdmissionDenied, "unclassified source markers"):
            admit_for_scope(result, self.snapshot, AdmissionScope.SYNTHETIC_FIXTURE_TEST)
        with self.assertRaisesRegex(AdmissionDenied, "F03-V04"):
            require_source_admission(result, self.snapshot)

    def test_known_synthetic_fixture_maps_to_contract_schema_but_never_source_admits(self):
        frame = build_frame(
            "synthetic.archive_turn.v1",
            {"conversation_id": "fixture", "node_id": "node", "text": "not source"},
        )
        result = classify_synthetic_frame(frame, self.snapshot)
        self.assertEqual(result.kind, CarrierKind.SYNTHETIC_FIXTURE)
        self.assertEqual(result.marker, "D01SYN01")
        self.assertEqual(result.carrier_version, 1)
        self.assertEqual(result.target_schema_id, "fixture.archive_turn_revision.v1")
        self.assertFalse(result.source_admissible)

        grant = admit_for_scope(result, self.snapshot, AdmissionScope.SYNTHETIC_FIXTURE_TEST)
        self.assertTrue(grant.allowed_for_synthetic_tests)
        self.assertFalse(grant.authentic_source_admission)
        self.assertEqual(grant.contract_revision, PINNED_COMMIT)
        self.assertEqual(
            grant.contract_bundle_sha256,
            PINNED_REGISTRY_HASHES["contract_bundle"],
        )
        with self.assertRaisesRegex(AdmissionDenied, "F03-V04"):
            require_source_admission(result, self.snapshot)

    def test_forged_classification_cannot_expand_synthetic_grant(self):
        frame = build_frame(
            "synthetic.memory_revision.v1",
            {"memory_id": "fixture-m", "revision": "1", "title": "title", "body": "body"},
        )
        result = classify_synthetic_frame(frame, self.snapshot)
        unsealed = CarrierClassification(
            kind=CarrierKind.SYNTHETIC_FIXTURE,
            marker="D01SYN01",
            family_id="synthetic.memory_revision.v1",
            target_schema_id="fixture.memory_revision.v1",
            carrier_version=1,
            evidence_scope="synthetic_fixture",
        )
        with self.assertRaisesRegex(AdmissionDenied, "not produced by the closed D01 fixture parser"):
            admit_for_scope(unsealed, self.snapshot, AdmissionScope.SYNTHETIC_FIXTURE_TEST)
        forged = replace(result, source_admissible=True)
        with self.assertRaisesRegex(AdmissionDenied, "closed D01 fixture map"):
            admit_for_scope(forged, self.snapshot, AdmissionScope.SYNTHETIC_FIXTURE_TEST)

    def test_incomplete_report_uses_official_contract_and_has_no_source_bindings(self):
        report = build_incomplete_report(
            self.snapshot,
            self.tool_root,
            captured_at_utc="2026-11-07T00:00:00Z",
        )
        validate_incomplete_report(report, self.snapshot)
        self.assertEqual(report["registry_hashes"], PINNED_REGISTRY_HASHES)
        self.assertEqual(report["producer_role"], "converter")
        self.assertEqual(report["evidence_scope"], "synthetic_fixture")
        self.assertEqual(report["status"], "INCOMPLETE")
        self.assertEqual(report["source_hashes"], [])
        self.assertEqual(report["source_owner_bindings"], [])
        self.assertEqual(report["coverage"]["owner_count"], "0")
        self.assertEqual(report["coverage"]["completeness"], "incomplete")
        report["status"] = "PASS"
        with self.assertRaisesRegex(ValueError, "D01 synthetic evidence"):
            validate_incomplete_report(report, self.snapshot)
        with self.assertRaisesRegex(
            self.snapshot.api.ContractError, "PASS report requires recomputable owner/custody evidence"
        ):
            self.snapshot.api.validate_artifact_report(report, self.snapshot.contract_root)

    def test_synthetic_pass_example_does_not_grant_source_admission(self):
        example_path = (
            self.contract_root / "fixtures" / "report-example-pass-synthetic.v1.json"
        )
        example = json.loads(example_path.read_text(encoding="utf-8"))
        self.assertEqual(example["status"], "PASS")
        self.assertEqual(example["evidence_scope"], "synthetic_fixture")
        result = classify_source_marker("example-only-marker", self.snapshot)
        with self.assertRaises(AdmissionDenied):
            admit_for_scope(result, self.snapshot, AdmissionScope.SYNTHETIC_FIXTURE_TEST)
        with self.assertRaises(AdmissionDenied):
            require_source_admission(result, self.snapshot)

    def test_modified_pin_is_rejected(self):
        binding = Path(__file__).resolve().parents[1] / "pgc01-binding.provisional.json"
        changed = json.loads(binding.read_text(encoding="utf-8"))
        changed["pg_c01_commit"] = "0" * 40
        with tempfile.TemporaryDirectory() as temporary:
            temporary_path = Path(temporary) / "binding.json"
            temporary_path.write_text(json.dumps(changed), encoding="utf-8")
            with self.assertRaises(ContractSnapshotError):
                load_pinned_snapshot(self.contract_root, temporary_path)


if __name__ == "__main__":
    unittest.main()