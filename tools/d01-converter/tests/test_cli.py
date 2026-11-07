import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from cli import CliFailure, _coverage_map
from pgc01_adapter import load_pinned_snapshot
from synthetic import _frame_bytes_unchecked_for_tests, build_frame


_CONTRACT_ROOT = os.environ.get("RELIQUARY_PG_C01_ROOT")
_CLI = Path(__file__).resolve().parents[1] / "cli.py"
_COVERAGE_MAP = _CLI.with_name("codec-coverage.provisional.json")


def _coverage_map_sha256() -> str:
    return hashlib.sha256(_COVERAGE_MAP.read_bytes()).hexdigest()


def _fixture(text: str) -> bytes:
    return build_frame(
        "synthetic.archive_turn.v1",
        {"conversation_id": "fixture-c", "node_id": "fixture-n", "text": text},
    )


def _memory_fixture(text: str) -> bytes:
    return build_frame(
        "synthetic.memory_revision.v1",
        {"memory_id": "fixture-m", "revision": "1", "title": "fixture title", "body": text},
    )


@unittest.skipUnless(_CONTRACT_ROOT, "set RELIQUARY_PG_C01_ROOT to the PG-C01 contract root")
class SyntheticCommandTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.snapshot = load_pinned_snapshot(Path(_CONTRACT_ROOT))

    def run_cli(self, *args: str) -> subprocess.CompletedProcess[str]:
        environment = os.environ.copy()
        environment["RELIQUARY_PG_C01_ROOT"] = str(_CONTRACT_ROOT)
        return subprocess.run(
            [sys.executable, "-B", str(_CLI), *args],
            check=False,
            capture_output=True,
            text=True,
            encoding="utf-8",
            env=environment,
            timeout=20,
        )

    def test_cli_reports_missing_and_changed_map_failures_stably(self):
        with tempfile.TemporaryDirectory() as workspace:
            root = Path(workspace)
            missing = root / "missing-map.json"
            with self.assertRaises(CliFailure) as missing_error:
                _coverage_map(self.snapshot, missing)
            self.assertEqual(missing_error.exception.error_code, "coverage_map_unavailable")

            changed = root / "changed-map.json"
            changed.write_bytes(_COVERAGE_MAP.read_bytes() + b" ")
            with self.assertRaises(CliFailure) as changed_error:
                _coverage_map(self.snapshot, changed)
            self.assertEqual(changed_error.exception.error_code, "coverage_map_changed")

    def test_inspect_is_separate_process_and_never_emits_fixture_payload(self):
        secret_fixture_text = "synthetic-only-private-looking-payload"
        data = _fixture(secret_fixture_text)
        with tempfile.TemporaryDirectory() as workspace:
            source = Path(workspace) / "input.d01synthetic"
            source.write_bytes(data)
            result = self.run_cli(
                "inspect",
                "--synthetic-fixture",
                "--input",
                str(source),
                "--contract-root",
                str(_CONTRACT_ROOT),
            )
        self.assertEqual(result.returncode, 0, result.stderr)
        report = json.loads(result.stdout)
        self.assertEqual(report["schema_id"], "reliquary-d01-synthetic-command-report-v2")
        self.assertEqual(report["command"], "inspect")
        self.assertEqual(report["status"], "INCOMPLETE")
        self.assertEqual(report["coverage_map_schema"], "reliquary-d01-codec-coverage-provisional-v0")
        self.assertEqual(report["coverage_map_status"], "provisional-incomplete")
        self.assertEqual(report["coverage_map_sha256"], _coverage_map_sha256())
        self.assertEqual(report["d01_t01_acceptance"], "not-accepted")
        self.assertEqual(report["classification"], "synthetic_fixture_only")
        self.assertEqual(report["family_id"], "synthetic.archive_turn.v1")
        self.assertEqual(report["target_schema_id"], "fixture.archive_turn_revision.v1")
        self.assertFalse(report["authentic_source_admission"])
        self.assertFalse(report["payload_emitted"])
        self.assertNotIn(secret_fixture_text, result.stdout)
        self.assertNotIn(secret_fixture_text, result.stderr)

    def test_convert_and_verify_structure_are_separate_processes_and_no_replace(self):
        secret_fixture_text = "synthetic-copy-not-conversion"
        data = _fixture(secret_fixture_text)
        with tempfile.TemporaryDirectory() as workspace:
            root = Path(workspace)
            source = root / "input.d01synthetic"
            output = root / "output.d01synthetic"
            receipt_path = root / "receipt.json"
            source.write_bytes(data)
            convert = self.run_cli(
                "convert",
                "--synthetic-fixture",
                "--input",
                str(source),
                "--expected-sha256",
                hashlib.sha256(data).hexdigest(),
                "--output",
                str(output),
                "--contract-root",
                str(_CONTRACT_ROOT),
            )
            self.assertEqual(convert.returncode, 0, convert.stderr)
            receipt = json.loads(convert.stdout)
            self.assertEqual(receipt["schema_id"], "reliquary-d01-synthetic-command-receipt-v2")
            self.assertEqual(receipt["coverage_map_sha256"], _coverage_map_sha256())
            self.assertEqual(receipt["coverage_map_schema"], "reliquary-d01-codec-coverage-provisional-v0")
            self.assertEqual(receipt["coverage_map_status"], "provisional-incomplete")
            self.assertEqual(receipt["d01_t01_acceptance"], "not-accepted")
            self.assertEqual(receipt["status"], "INCOMPLETE")
            self.assertEqual(receipt["artifact_state"], "synthetic-test-artifact")
            self.assertEqual(receipt["source_sha256_before"], receipt["source_sha256_after"])
            self.assertEqual(receipt["output_sha256"], hashlib.sha256(data).hexdigest())
            self.assertFalse(receipt["target_candidate"])
            self.assertFalse(receipt["authentic_source_admission"])
            self.assertFalse(receipt["payload_emitted"])
            self.assertEqual(output.read_bytes(), data)
            self.assertNotIn(secret_fixture_text, convert.stdout)
            self.assertNotIn(secret_fixture_text, convert.stderr)

            receipt_path.write_text(convert.stdout, encoding="utf-8")
            verify = self.run_cli(
                "verify-structure",
                "--synthetic-fixture",
                "--candidate",
                str(output),
                "--receipt",
                str(receipt_path),
                "--contract-root",
                str(_CONTRACT_ROOT),
            )
            self.assertEqual(verify.returncode, 0, verify.stderr)
            structure = json.loads(verify.stdout)
            self.assertEqual(structure["schema_id"], "reliquary-d01-synthetic-command-report-v2")
            self.assertTrue(structure["structure_valid"])
            self.assertEqual(structure["coverage_map_sha256"], _coverage_map_sha256())
            self.assertEqual(structure["d01_t01_acceptance"], "not-accepted")
            self.assertFalse(structure["target_candidate"])
            self.assertFalse(structure["current_only_owner_reopen"])
            self.assertFalse(structure["payload_emitted"])
            self.assertNotIn(secret_fixture_text, verify.stdout)
            self.assertNotIn(secret_fixture_text, verify.stderr)

            retry = self.run_cli(
                "convert",
                "--synthetic-fixture",
                "--input",
                str(source),
                "--expected-sha256",
                hashlib.sha256(data).hexdigest(),
                "--output",
                str(output),
                "--contract-root",
                str(_CONTRACT_ROOT),
            )
            self.assertEqual(retry.returncode, 5)
            diagnostic = json.loads(retry.stderr)
            self.assertEqual(diagnostic["error_code"], "conflict_collision")
            self.assertEqual(output.read_bytes(), data)

    def test_memory_revision_round_trips_through_separate_cli_processes(self):
        secret_fixture_text = "synthetic-memory-body"
        data = _memory_fixture(secret_fixture_text)
        with tempfile.TemporaryDirectory() as workspace:
            root = Path(workspace)
            source = root / "memory.d01synthetic"
            output = root / "memory-copy.d01synthetic"
            receipt_path = root / "memory-receipt.json"
            source.write_bytes(data)

            inspect = self.run_cli(
                "inspect",
                "--synthetic-fixture",
                "--input",
                str(source),
                "--contract-root",
                str(_CONTRACT_ROOT),
            )
            self.assertEqual(inspect.returncode, 0, inspect.stderr)
            inspection = json.loads(inspect.stdout)
            self.assertEqual(inspection["family_id"], "synthetic.memory_revision.v1")
            self.assertEqual(inspection["target_schema_id"], "fixture.memory_revision.v1")
            self.assertEqual(inspection["coverage_map_sha256"], _coverage_map_sha256())
            self.assertEqual(inspection["d01_t01_acceptance"], "not-accepted")
            self.assertNotIn(secret_fixture_text, inspect.stdout)
            self.assertNotIn(secret_fixture_text, inspect.stderr)

            convert = self.run_cli(
                "convert",
                "--synthetic-fixture",
                "--input",
                str(source),
                "--expected-sha256",
                hashlib.sha256(data).hexdigest(),
                "--output",
                str(output),
                "--contract-root",
                str(_CONTRACT_ROOT),
            )
            self.assertEqual(convert.returncode, 0, convert.stderr)
            receipt = json.loads(convert.stdout)
            self.assertEqual(receipt["family_id"], "synthetic.memory_revision.v1")
            self.assertEqual(receipt["target_schema_id"], "fixture.memory_revision.v1")
            self.assertEqual(receipt["output_sha256"], hashlib.sha256(data).hexdigest())
            self.assertFalse(receipt["target_candidate"])
            self.assertFalse(receipt["authentic_source_admission"])
            self.assertFalse(receipt["payload_emitted"])
            self.assertNotIn(secret_fixture_text, convert.stdout)
            self.assertNotIn(secret_fixture_text, convert.stderr)
            receipt_path.write_text(convert.stdout, encoding="utf-8")

            verify = self.run_cli(
                "verify-structure",
                "--synthetic-fixture",
                "--candidate",
                str(output),
                "--receipt",
                str(receipt_path),
                "--contract-root",
                str(_CONTRACT_ROOT),
            )
            self.assertEqual(verify.returncode, 0, verify.stderr)
            structure = json.loads(verify.stdout)
            self.assertTrue(structure["structure_valid"])
            self.assertEqual(structure["family_id"], "synthetic.memory_revision.v1")
            self.assertEqual(structure["target_schema_id"], "fixture.memory_revision.v1")
            self.assertFalse(structure["target_candidate"])
            self.assertFalse(structure["payload_emitted"])
            self.assertNotIn(secret_fixture_text, verify.stdout)
            self.assertNotIn(secret_fixture_text, verify.stderr)

    def test_verify_rejects_tampered_receipt_evidence_fields(self):
        secret_fixture_text = "tamper-matrix-synthetic-only"
        data = _fixture(secret_fixture_text)
        with tempfile.TemporaryDirectory() as workspace:
            root = Path(workspace)
            source = root / "input.d01synthetic"
            output = root / "output.d01synthetic"
            receipt_path = root / "receipt.json"
            source.write_bytes(data)
            convert = self.run_cli(
                "convert",
                "--synthetic-fixture",
                "--input",
                str(source),
                "--expected-sha256",
                hashlib.sha256(data).hexdigest(),
                "--output",
                str(output),
                "--contract-root",
                str(_CONTRACT_ROOT),
            )
            self.assertEqual(convert.returncode, 0, convert.stderr)
            receipt = json.loads(convert.stdout)
            mutations = (
                ("schema_id", "unknown-receipt-v9"),
                ("command", "resume"),
                ("status", "PASS"),
                ("evidence_scope", "authentic_source"),
                ("artifact_state", "owner-candidate"),
                ("target_candidate", True),
                ("authentic_source_admission", True),
                ("source_sha256_before", "0" * 64),
                ("source_sha256_after", "0" * 64),
                ("output_sha256", "0" * 64),
                ("byte_count", "0"),
                ("output_name", "different.d01synthetic"),
                ("carrier_marker", "CVAAFMT2"),
                ("carrier_version", "99"),
                ("family_id", "synthetic.memory_revision.v1"),
                ("target_schema_id", "fixture.memory_revision.v1"),
                ("contract_revision", "0" * 40),
                ("registry_hashes", {"semantic_groups": "0" * 64}),
                ("coverage_map_schema", "unknown-map-v9"),
                ("coverage_map_status", "accepted"),
                ("coverage_map_sha256", "0" * 64),
                ("d01_t01_acceptance", "accepted"),
                ("producer_code_sha256", "0" * 64),
                ("payload_emitted", True),
                ("current_only_owner_reopen", True),
            )
            for field, value in mutations:
                with self.subTest(field=field):
                    changed = dict(receipt)
                    changed[field] = value
                    receipt_path.write_text(json.dumps(changed), encoding="utf-8")
                    verify = self.run_cli(
                        "verify-structure",
                        "--synthetic-fixture",
                        "--candidate",
                        str(output),
                        "--receipt",
                        str(receipt_path),
                        "--contract-root",
                        str(_CONTRACT_ROOT),
                    )
                    self.assertEqual(verify.returncode, 6, f"{field}: {verify.stderr}")
                    diagnostic = json.loads(verify.stderr)
                    self.assertEqual(diagnostic["error_code"], "target_mismatch")
                    self.assertFalse(diagnostic["payload_emitted"])
                    self.assertNotIn(secret_fixture_text, verify.stdout)
                    self.assertNotIn(secret_fixture_text, verify.stderr)

            duplicate = json.dumps(receipt)[:-1] + ',"command":"resume"}'
            receipt_path.write_text(duplicate, encoding="utf-8")
            duplicate_result = self.run_cli(
                "verify-structure",
                "--synthetic-fixture",
                "--candidate",
                str(output),
                "--receipt",
                str(receipt_path),
                "--contract-root",
                str(_CONTRACT_ROOT),
            )
            self.assertEqual(duplicate_result.returncode, 6)
            self.assertEqual(
                json.loads(duplicate_result.stderr)["error_code"], "receipt_invalid"
            )
            self.assertNotIn(secret_fixture_text, duplicate_result.stderr)

            missing_field = dict(receipt)
            missing_field.pop("family_id")
            receipt_path.write_text(json.dumps(missing_field), encoding="utf-8")
            missing_result = self.run_cli(
                "verify-structure",
                "--synthetic-fixture",
                "--candidate",
                str(output),
                "--receipt",
                str(receipt_path),
                "--contract-root",
                str(_CONTRACT_ROOT),
            )
            self.assertEqual(missing_result.returncode, 6)
            self.assertEqual(
                json.loads(missing_result.stderr)["error_code"], "target_mismatch"
            )

    def test_well_framed_unknown_family_and_version_fail_in_cli(self):
        unknown_family = _frame_bytes_unchecked_for_tests(
            "synthetic.unlisted_family.v1", b'{"value":"unknown-family-secret"}'
        )
        unsupported_version = _frame_bytes_unchecked_for_tests(
            "synthetic.archive_turn.v1",
            b'{"conversation_id":"fixture-c","node_id":"fixture-n","text":"version-secret"}',
            version=2,
        )
        with tempfile.TemporaryDirectory() as workspace:
            root = Path(workspace)
            unknown_path = root / "well-framed-unknown.d01synthetic"
            unknown_path.write_bytes(unknown_family)
            unknown_result = self.run_cli(
                "inspect",
                "--synthetic-fixture",
                "--input",
                str(unknown_path),
                "--contract-root",
                str(_CONTRACT_ROOT),
            )
            self.assertEqual(unknown_result.returncode, 4)
            unknown_diagnostic = json.loads(unknown_result.stderr)
            self.assertEqual(unknown_diagnostic["error_code"], "unsupported_family")
            self.assertFalse(unknown_diagnostic["payload_emitted"])
            self.assertNotIn("unknown-family-secret", unknown_result.stdout)
            self.assertNotIn("unknown-family-secret", unknown_result.stderr)

            version_path = root / "unsupported-version.d01synthetic"
            output = root / "must-not-be-created.d01synthetic"
            version_path.write_bytes(unsupported_version)
            version_result = self.run_cli(
                "convert",
                "--synthetic-fixture",
                "--input",
                str(version_path),
                "--expected-sha256",
                hashlib.sha256(unsupported_version).hexdigest(),
                "--output",
                str(output),
                "--contract-root",
                str(_CONTRACT_ROOT),
            )
            self.assertEqual(version_result.returncode, 4)
            version_diagnostic = json.loads(version_result.stderr)
            self.assertEqual(
                version_diagnostic["error_code"], "unclassified_or_malformed_input"
            )
            self.assertFalse(version_diagnostic["payload_emitted"])
            self.assertFalse(output.exists())
            self.assertNotIn("version-secret", version_result.stdout)
            self.assertNotIn("version-secret", version_result.stderr)

    def test_unclassified_marker_and_wrong_expected_hash_fail_structured(self):
        with tempfile.TemporaryDirectory() as workspace:
            root = Path(workspace)
            source = root / "not-synthetic.bin"
            source.write_bytes(b"CVAAFMT2")
            result = self.run_cli(
                "inspect",
                "--synthetic-fixture",
                "--input",
                str(source),
                "--contract-root",
                str(_CONTRACT_ROOT),
            )
            self.assertEqual(result.returncode, 4)
            diagnostic = json.loads(result.stderr)
            self.assertEqual(diagnostic["error_code"], "unclassified_or_malformed_input")
            self.assertFalse(diagnostic["payload_emitted"])
            self.assertNotIn("CVAAFMT2", result.stderr)

            good = root / "fixture.bin"
            data = _fixture("fixture")
            good.write_bytes(data)
            output = root / "wrong-hash-output.bin"
            mismatch = self.run_cli(
                "convert",
                "--synthetic-fixture",
                "--input",
                str(good),
                "--expected-sha256",
                "0" * 64,
                "--output",
                str(output),
                "--contract-root",
                str(_CONTRACT_ROOT),
            )
            self.assertEqual(mismatch.returncode, 3)
            self.assertEqual(json.loads(mismatch.stderr)["error_code"], "input_admission_mismatch")
            self.assertFalse(output.exists())

    def test_output_outside_system_temp_is_refused_before_write(self):
        with tempfile.TemporaryDirectory() as workspace:
            source = Path(workspace) / "fixture.d01synthetic"
            data = _fixture("fixture")
            source.write_bytes(data)
            outside = Path(__file__).resolve().parents[1] / "must-not-be-created-by-cli-test"
            self.assertFalse(outside.exists())
            result = self.run_cli(
                "convert",
                "--synthetic-fixture",
                "--input",
                str(source),
                "--expected-sha256",
                hashlib.sha256(data).hexdigest(),
                "--output",
                str(outside),
                "--contract-root",
                str(_CONTRACT_ROOT),
            )
            self.assertEqual(result.returncode, 3)
            self.assertEqual(json.loads(result.stderr)["error_code"], "output_outside_temporary_scope")
            self.assertFalse(outside.exists())

    def test_gap_ledger_records_f03_and_d02_blockers_without_claiming_t04(self):
        tool_root = Path(__file__).resolve().parents[1]
        ledger_path = tool_root / "target-primitive-gaps.provisional.json"
        ledger = json.loads(ledger_path.read_text(encoding="utf-8"))
        self.assertEqual(ledger["status"], "open-provisional")
        self.assertIn("F03-V02", ledger["source_evidence_gate"]["pending"])
        self.assertIn("F03-V04", ledger["source_evidence_gate"]["pending"])
        phy = next(item for item in ledger["gaps"] if item["id"] == "phy-current-only-profile-prune-authority")
        self.assertEqual(phy["status"], "blocked-on-D02-owned-fix-and-reverification")
        transaction = next(item for item in ledger["gaps"] if item["id"] == "create-only-crash-safe-publication")
        self.assertEqual(transaction["status"], "later-ticket-not-accepted")
        coverage_path = tool_root / "codec-coverage.provisional.json"
        binding = json.loads(coverage_path.read_text(encoding="utf-8"))["command_surface"]["coverage_map_contract"]
        self.assertEqual(binding["status"], "provisional-incomplete")
        self.assertEqual(binding["binding"], "pinned-exact-file-bytes")
        self.assertEqual(binding["t01_acceptance"], "not-accepted")
    def test_resume_refuses_with_distinct_structured_failure(self):
        result = self.run_cli("resume")
        self.assertEqual(result.returncode, 3)
        self.assertEqual(result.stdout, "")
        diagnostic = json.loads(result.stderr)
        self.assertEqual(diagnostic["error_code"], "resume_unsupported")
        self.assertEqual(
            diagnostic["message"],
            "checkpoint resume is unsupported; no resumable synthetic checkpoint exists",
        )
        self.assertFalse(diagnostic["payload_emitted"])

    def test_missing_synthetic_mode_returns_structured_argument_error(self):
        with tempfile.TemporaryDirectory() as workspace:
            source = Path(workspace) / "fixture.d01synthetic"
            source.write_bytes(_fixture("fixture"))
            result = self.run_cli(
                "inspect",
                "--input",
                str(source),
                "--contract-root",
                str(_CONTRACT_ROOT),
            )
        self.assertEqual(result.returncode, 2)
        self.assertEqual(json.loads(result.stderr)["error_code"], "invalid_arguments")


if __name__ == "__main__":
    unittest.main()