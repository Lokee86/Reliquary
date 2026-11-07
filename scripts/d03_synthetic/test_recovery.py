from __future__ import annotations

import json
import os
import shutil
import subprocess
import sys
import tempfile
import unittest
import uuid
from pathlib import Path
from unittest import mock

from scripts.d03_synthetic import durability
from scripts.d03_synthetic.engine import (
    CANDIDATE, PREVIOUS, STATE_FILE, _write_state, inspect_state, stage, validate,
)
from scripts.d03_synthetic.files import D03Error, parse_json
from scripts.d03_synthetic.rehearsal import rehearse, rollback_prewrite
from scripts.d03_synthetic.validation import validate_generation


class D03RestartSafetyTests(unittest.TestCase):
    def setUp(self) -> None:
        self.roots: list[Path] = []
        self.repo = Path(__file__).resolve().parents[2]

    def root(self) -> Path:
        path = Path(tempfile.gettempdir()) / f"d03-synthetic-{uuid.uuid4().hex}"
        self.roots.append(path)
        return path

    def tearDown(self) -> None:
        temp_root = Path(tempfile.gettempdir()).resolve()
        for path in self.roots:
            if path.exists():
                self.assertEqual(path.parent.resolve(), temp_root)
                self.assertTrue(path.name.startswith("d03-synthetic-"))
                shutil.rmtree(path)

    def cli(self, *arguments: str, expected: int = 0) -> subprocess.CompletedProcess[str]:
        environment = os.environ.copy()
        environment["PYTHONDONTWRITEBYTECODE"] = "1"
        result = subprocess.run(
            [sys.executable, "-m", "scripts.d03_synthetic", *arguments],
            cwd=self.repo,
            env=environment,
            capture_output=True,
            text=True,
        )
        self.assertEqual(result.returncode, expected, result.stderr)
        return result

    def inspect_after_restart(self, root: Path) -> dict:
        result = self.cli("inspect-state", "--root", str(root))
        return json.loads(result.stdout)

    def test_subprocess_crashes_reconcile_rehearsal_before_and_after_replace(self) -> None:
        cases = (
            ("crash_before_selection_replace", 81, "StagedReadOnly", PREVIOUS),
            ("crash_after_selection_replace", 82, "CandidateFenced", CANDIDATE),
        )
        for failpoint, exit_code, phase, selected in cases:
            with self.subTest(failpoint=failpoint):
                root = self.root()
                stage(root)
                self.cli(
                    "rehearse", "--root", str(root), "--failpoint", failpoint,
                    expected=exit_code,
                )
                state = self.inspect_after_restart(root)
                self.assertEqual(state["phase"], phase)
                self.assertEqual(state["selected_generation_id"], selected)
                persisted = parse_json((root / STATE_FILE).read_bytes())
                self.assertIsNone(persisted["pending_transition"])

    def test_subprocess_crashes_reconcile_rollback_before_and_after_replace(self) -> None:
        before = self.root()
        stage(before)
        rehearse(before)
        self.cli(
            "rollback-prewrite", "--root", str(before), "--failpoint",
            "crash_before_selection_replace", expected=83,
        )
        state = self.inspect_after_restart(before)
        self.assertEqual(state["phase"], "CandidateFenced")
        self.assertEqual(state["selected_generation_id"], CANDIDATE)
        self.assertEqual(rollback_prewrite(before)["selected_generation_id"], PREVIOUS)

        after = self.root()
        stage(after)
        rehearse(after)
        self.cli(
            "rollback-prewrite", "--root", str(after), "--failpoint",
            "crash_after_selection_replace", expected=84,
        )
        state = self.inspect_after_restart(after)
        self.assertEqual(state["phase"], "PrewriteRolledBack")
        self.assertEqual(state["selected_generation_id"], PREVIOUS)

    def test_simulated_unavailable_selector_replace_keeps_route_unchanged(self) -> None:
        root = self.root()
        stage(root)
        result = self.cli(
            "rehearse", "--root", str(root), "--failpoint",
            "simulate_selector_replace_unavailable", expected=1,
        )
        error = json.loads(result.stderr)
        self.assertIn("injected selector replacement unavailability", error["error"])
        persisted = error["persisted_state"]
        self.assertEqual(persisted["phase"], "SelectionPending")
        self.assertEqual(persisted["selected_generation_id"], PREVIOUS)
        self.assertEqual(persisted["actual_selection"], PREVIOUS)
        self.assertEqual(persisted["pending_transition"]["operation"], "rehearsal")
        self.assertEqual(len(persisted["manifest_sha256"]), 64)
        recovered = self.inspect_after_restart(root)
        self.assertEqual(recovered["phase"], "StagedReadOnly")
        self.assertEqual(recovered["selected_generation_id"], PREVIOUS)
        rehearse(root)

        result = self.cli(
            "rollback-prewrite", "--root", str(root), "--failpoint",
            "simulate_selector_replace_unavailable", expected=1,
        )
        error = json.loads(result.stderr)
        self.assertIn("injected selector replacement unavailability", error["error"])
        persisted = error["persisted_state"]
        self.assertEqual(persisted["phase"], "SelectionPending")
        self.assertEqual(persisted["selected_generation_id"], CANDIDATE)
        self.assertEqual(persisted["actual_selection"], CANDIDATE)
        self.assertEqual(persisted["pending_transition"]["operation"], "rollback")
        recovered = self.inspect_after_restart(root)
        self.assertEqual(recovered["phase"], "CandidateFenced")
        self.assertEqual(recovered["selected_generation_id"], CANDIDATE)
        self.assertEqual(rollback_prewrite(root)["selected_generation_id"], PREVIOUS)

    def test_corrupt_rollback_target_does_not_claim_verified_recovery(self) -> None:
        root = self.root()
        stage(root)
        rehearse(root)
        self.cli(
            "rollback-prewrite", "--root", str(root), "--failpoint",
            "crash_after_selection_replace", expected=84,
        )
        previous = root / "generations" / PREVIOUS
        (previous / "generation-manifest.json").unlink()
        result = self.cli("inspect-state", "--root", str(root), expected=1)
        self.assertEqual(json.loads(result.stderr)["status"], "INCOMPLETE")
        pending = parse_json((root / STATE_FILE).read_bytes())
        self.assertEqual(pending["phase"], "SelectionPending")
        self.assertEqual(pending["pending_transition"]["operation"], "rollback")

    def test_directory_sync_uncertainty_after_replace_recovers_fenced(self) -> None:
        root = self.root()
        stage(root)
        original = durability.sync_directory
        sync_count = 0

        def fail_after_pointer_replace(directory: Path) -> None:
            nonlocal sync_count
            sync_count += 1
            if sync_count == 2:
                raise D03Error("simulated directory flush failure")
            original(directory)

        with mock.patch(
            "scripts.d03_synthetic.durability.sync_directory",
            side_effect=fail_after_pointer_replace,
        ):
            with self.assertRaisesRegex(D03Error, "simulated directory flush failure"):
                rehearse(root)
        recovered = inspect_state(root)
        self.assertEqual(recovered["phase"], "CandidateFenced")
        self.assertEqual(recovered["selected_generation_id"], CANDIDATE)

    def test_staged_state_and_validator_reject_missing_candidate_digest(self) -> None:
        root = self.root()
        stage(root)
        state_path = root / STATE_FILE
        state = parse_json(state_path.read_bytes())
        state["manifest_sha256"] = None
        _write_state(root, state)
        with self.assertRaisesRegex(D03Error, "candidate manifest digest"):
            validate(root)
        candidate = root / "generations" / CANDIDATE
        with self.assertRaisesRegex(D03Error, "expected manifest digest"):
            validate_generation(candidate, None)  # type: ignore[arg-type]

if __name__ == "__main__":
    unittest.main()
