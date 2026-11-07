import hashlib
from dataclasses import replace
import os
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from admission import (
    AdmissionDenied,
    AdmissionScope,
    admit_for_scope,
    classify_synthetic_frame,
)
from coverage_map import (
    COVERAGE_MAP_SCHEMA,
    COVERAGE_MAP_STATUS,
    D01_T01_ACCEPTANCE,
    PINNED_COVERAGE_MAP_SHA256,
    ProvisionalCoverageMap,
    CoverageMapError,
    load_provisional_coverage_map,
)
from pgc01_adapter import load_pinned_snapshot
from synthetic import build_frame


_CONTRACT_ROOT = os.environ.get("RELIQUARY_PG_C01_ROOT")
_TOOL_ROOT = Path(__file__).resolve().parents[1]
_MAP = _TOOL_ROOT / "codec-coverage.provisional.json"


@unittest.skipUnless(_CONTRACT_ROOT, "set RELIQUARY_PG_C01_ROOT to the PG-C01 contract root")
class ProvisionalCoverageMapTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.snapshot = load_pinned_snapshot(Path(_CONTRACT_ROOT))

    def test_exact_map_bytes_drive_closed_synthetic_family_mapping(self):
        coverage = load_provisional_coverage_map(_MAP, self.snapshot)
        self.assertEqual(coverage.schema_id, COVERAGE_MAP_SCHEMA)
        self.assertEqual(coverage.status, COVERAGE_MAP_STATUS)
        self.assertEqual(coverage.t01_acceptance, D01_T01_ACCEPTANCE)
        self.assertEqual(coverage.sha256, PINNED_COVERAGE_MAP_SHA256)
        self.assertEqual(coverage.sha256, hashlib.sha256(_MAP.read_bytes()).hexdigest())
        self.assertEqual(coverage.contract_revision, self.snapshot.revision)
        self.assertEqual(
            coverage.registry_hashes,
            tuple(sorted(self.snapshot.registry_hashes.items())),
        )
        self.assertEqual(
            coverage.target_schema_for("synthetic.archive_turn.v1"),
            "fixture.archive_turn_revision.v1",
        )
        self.assertEqual(
            coverage.target_schema_for("synthetic.memory_revision.v1"),
            "fixture.memory_revision.v1",
        )
        self.assertIsNone(coverage.target_schema_for("synthetic.unlisted.v1"))

    def test_classification_and_admission_reject_replaced_snapshot_binding(self):
        coverage = load_provisional_coverage_map(_MAP, self.snapshot)
        frame = build_frame(
            "synthetic.archive_turn.v1",
            {"conversation_id": "fixture-c", "node_id": "fixture-n", "text": "synthetic"},
        )
        classification = classify_synthetic_frame(frame, self.snapshot, coverage)
        replacements = [("revision", replace(self.snapshot, revision="0" * 40))]
        for name, original_hash in self.snapshot.registry_hashes.items():
            changed_hashes = dict(self.snapshot.registry_hashes)
            changed_hashes[name] = "0" * 64 if original_hash != "0" * 64 else "f" * 64
            replacements.append(
                (name, replace(self.snapshot, registry_hashes=changed_hashes))
            )

        for name, substituted_snapshot in replacements:
            with self.subTest(binding=name):
                with self.assertRaisesRegex(AdmissionDenied, "supplied PG-C01 snapshot"):
                    classify_synthetic_frame(frame, substituted_snapshot, coverage)
                with self.assertRaisesRegex(AdmissionDenied, "supplied PG-C01 snapshot"):
                    admit_for_scope(
                        classification,
                        substituted_snapshot,
                        AdmissionScope.SYNTHETIC_FIXTURE_TEST,
                        coverage,
                    )

    def test_missing_map_fails_with_stable_diagnostic_code(self):
        with tempfile.TemporaryDirectory() as workspace:
            missing = Path(workspace) / "missing-map.json"
            with self.assertRaises(CoverageMapError) as raised:
                load_provisional_coverage_map(missing, self.snapshot)
        self.assertEqual(raised.exception.error_code, "coverage_map_unavailable")
        self.assertEqual(
            raised.exception.message,
            "provisional synthetic coverage map is missing",
        )

    def test_forged_map_value_cannot_drive_classification(self):
        forged = ProvisionalCoverageMap(
            schema_id=COVERAGE_MAP_SCHEMA,
            status=COVERAGE_MAP_STATUS,
            t01_acceptance=D01_T01_ACCEPTANCE,
            sha256=PINNED_COVERAGE_MAP_SHA256,
            contract_revision=self.snapshot.revision,
            registry_hashes=tuple(sorted(self.snapshot.registry_hashes.items())),
            fixture_schemas=(("synthetic.archive_turn.v1", "fixture.archive_turn_revision.v1"),),
        )
        frame = build_frame(
            "synthetic.archive_turn.v1",
            {"conversation_id": "fixture-c", "node_id": "fixture-n", "text": "synthetic"},
        )
        with self.assertRaisesRegex(AdmissionDenied, "supplied PG-C01 snapshot"):
            classify_synthetic_frame(frame, self.snapshot, forged)

    def test_changed_map_bytes_fail_before_the_mapping_is_used(self):
        changed_bytes = _MAP.read_bytes() + b" "
        with tempfile.TemporaryDirectory() as workspace:
            changed = Path(workspace) / "changed-map.json"
            changed.write_bytes(changed_bytes)
            with self.assertRaises(CoverageMapError) as raised:
                load_provisional_coverage_map(changed, self.snapshot)
        self.assertEqual(raised.exception.error_code, "coverage_map_changed")
        self.assertEqual(
            raised.exception.message,
            "provisional synthetic coverage map differs from its exact pin",
        )


if __name__ == "__main__":
    unittest.main()