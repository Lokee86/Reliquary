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
from synthetic import build_frame


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

    def test_verify_rejects_receipt_bound_to_a_different_map(self):
        data = _fixture("map-binding-check")
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
            receipt["coverage_map_sha256"] = "0" * 64
            receipt_path.write_text(json.dumps(receipt), encoding="utf-8")
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
        self.assertEqual(verify.returncode, 6)
        diagnostic = json.loads(verify.stderr)
        self.assertEqual(diagnostic["error_code"], "target_mismatch")
        self.assertFalse(diagnostic["payload_emitted"])


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