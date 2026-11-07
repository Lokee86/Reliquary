from __future__ import annotations

import json
import os
import shutil
import tempfile
import unittest
import uuid
from pathlib import Path

from scripts.d03_synthetic.engine import (
    CANDIDATE,
    PREVIOUS,
    STATE_FILE,
    _write_state,
    inspect_state,
    stage,
    validate,
)
from scripts.d03_synthetic.files import D03Error, canonical_bytes, parse_json, sha256
from scripts.d03_synthetic.rehearsal import rehearse, rollback_prewrite


class D03SyntheticHarnessTests(unittest.TestCase):
    def setUp(self) -> None:
        self.roots: list[Path] = []

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

    def _manifest(self, root: Path) -> tuple[Path, dict]:
        path = root / "generations" / CANDIDATE / "generation-manifest.json"
        return path, parse_json(path.read_bytes())

    def _tamper_manifest(self, root: Path, mutate) -> None:
        path, manifest = self._manifest(root)
        mutate(manifest)
        raw = canonical_bytes(manifest) + b"\n"
        path.write_bytes(raw)
        state_path = root / STATE_FILE
        state = parse_json(state_path.read_bytes())
        state["manifest_sha256"] = sha256(canonical_bytes(manifest))
        _write_state(root, state)

    def test_stage_builds_six_synthetic_owners_and_validates(self) -> None:
        root = self.root()
        staged = stage(root)
        self.assertTrue(staged["simulation_only"])
        self.assertEqual(staged["phase"], "StagedReadOnly")
        self.assertEqual(staged["selected_generation_id"], PREVIOUS)
        report = validate(root)
        self.assertEqual(report["status"], "INCOMPLETE")
        self.assertEqual(report["evidence_scope"], "synthetic_fixture")
        self.assertEqual(report["owner_count"], "6")
        self.assertFalse(report["real_owner_open_performed"])
        _, manifest = self._manifest(root)
        self.assertEqual(len(manifest["owners"]), 6)
        self.assertEqual(sum(row["owner_kind"] == "REL" for row in manifest["owners"]), 3)
        self.assertEqual(sum(row["owner_kind"] == "PHY" for row in manifest["owners"]), 3)
        self.assertEqual(manifest["d02_report"]["evidence_scope"], "synthetic_fixture")

    def test_stage_is_create_only_and_refuses_non_temp_roots(self) -> None:
        root = self.root()
        stage(root)
        with self.assertRaises(D03Error):
            stage(root)
        outside = Path(__file__).resolve().parent / "d03-synthetic-not-temp"
        with self.assertRaises(D03Error):
            stage(outside)
        self.assertFalse(outside.exists())

    def test_stage_faults_leave_old_selection_and_no_candidate_route(self) -> None:
        for failpoint in ("before_sync", "after_sync"):
            with self.subTest(failpoint=failpoint):
                root = self.root()
                with self.assertRaises(D03Error):
                    stage(root, failpoint)
                state = inspect_state(root)
                self.assertEqual(state["phase"], "Unverified")
                self.assertEqual(state["selected_generation_id"], PREVIOUS)
                self.assertFalse((root / "generations" / CANDIDATE).exists())

    def test_successful_rehearsal_stays_fenced_until_prewrite_rollback(self) -> None:
        root = self.root()
        stage(root)
        before = validate(root)
        rehearsed = rehearse(root)
        self.assertEqual(rehearsed["phase"], "Rehearsed")
        self.assertEqual(rehearsed["selected_generation_id"], CANDIDATE)
        self.assertEqual(rehearsed["writer_gate"], "closed")
        self.assertFalse(rehearsed["write_boundary_crossed"])
        self.assertFalse(rehearsed["activation_authorized"])
        self.assertEqual(rehearsed["pending_host_outcome"], "held")
        self.assertEqual(validate(root)["manifest_sha256"], before["manifest_sha256"])
        rolled_back = rollback_prewrite(root)
        self.assertEqual(rolled_back["phase"], "PrewriteRolledBack")
        self.assertEqual(rolled_back["selected_generation_id"], PREVIOUS)
        self.assertEqual(rolled_back["writer_gate"], "closed")

    def test_rehearsal_failures_leave_old_or_fenced_candidate(self) -> None:
        old_root = self.root()
        stage(old_root)
        with self.assertRaises(D03Error):
            rehearse(old_root, "before_selection_replace")
        old_state = inspect_state(old_root)
        self.assertEqual(old_state["phase"], "RehearsalFailedOldActive")
        self.assertEqual(old_state["selected_generation_id"], PREVIOUS)

        for failpoint in ("after_selection_replace", "health_check"):
            with self.subTest(failpoint=failpoint):
                root = self.root()
                stage(root)
                with self.assertRaises(D03Error):
                    rehearse(root, failpoint)
                state = inspect_state(root)
                self.assertEqual(state["phase"], "CandidateFenced")
                self.assertEqual(state["selected_generation_id"], CANDIDATE)
                self.assertEqual(state["writer_gate"], "closed")
                self.assertFalse(state["write_boundary_crossed"])

    def test_rollback_refuses_after_accepted_new_write(self) -> None:
        root = self.root()
        stage(root)
        state_path = root / STATE_FILE
        state = parse_json(state_path.read_bytes())
        state["write_boundary_crossed"] = True
        state["accepted_new_writes"] = "1"
        _write_state(root, state)
        with self.assertRaisesRegex(D03Error, "after an accepted new-generation write"):
            rollback_prewrite(root)

    def test_manifest_rejects_escaped_path_duplicate_id_wrong_scope_and_stale_digest(self) -> None:
        mutations = (
            lambda m: m["owners"][0].__setitem__("path", "../outside.fake.json"),
            lambda m: m["owners"][1].__setitem__("owner_uuid", m["owners"][0]["owner_uuid"]),
            lambda m: m["owners"][0].__setitem__("scope", "unspecified"),
            lambda m: m["runtime"].__setitem__("sha256", "0" * 64),
            lambda m: m["d02_report"].__setitem__("sha256", "0" * 64),
        )
        for mutate in mutations:
            with self.subTest(mutate=mutate):
                root = self.root()
                stage(root)
                self._tamper_manifest(root, mutate)
                with self.assertRaises(D03Error):
                    validate(root)

    def test_manifest_rejects_missing_corrupt_extra_and_mixed_generation_files(self) -> None:
        actions = (
            lambda root: next((root / "generations" / CANDIDATE / "owners").glob("*.rel.fake.json")).unlink(),
            lambda root: (root / "generations" / CANDIDATE / "runtime" / "runtime.fake").write_bytes(b"corrupt"),
            lambda root: (root / "generations" / CANDIDATE / "owners" / "extra.fake.json").write_bytes(b"extra"),
            lambda root: shutil.copyfile(
                next((root / "generations" / PREVIOUS / "owners").glob("*.rel.fake.json")),
                next((root / "generations" / CANDIDATE / "owners").glob("*.rel.fake.json")),
            ),
        )
        for action in actions:
            with self.subTest(action=action):
                root = self.root()
                stage(root)
                action(root)
                with self.assertRaises(D03Error):
                    validate(root)

    def test_hardlink_alias_is_rejected(self) -> None:
        root = self.root()
        stage(root)
        owner_files = sorted((root / "generations" / CANDIDATE / "owners").glob("*.fake.json"))
        target, alias = owner_files[0], owner_files[1]
        alias.unlink()
        try:
            os.link(target, alias)
        except OSError as error:
            self.skipTest(f"hard links unavailable on this temp volume: {error}")
        with self.assertRaisesRegex(D03Error, "alias"):
            validate(root)

    def test_rehearsal_refuses_unproven_writer_stop_or_unheld_host_outcome(self) -> None:
        for field, value in (("writers_stopped", False), ("pending_host_outcome", "unknown")):
            with self.subTest(field=field):
                root = self.root()
                stage(root)
                state_path = root / STATE_FILE
                state = parse_json(state_path.read_bytes())
                state[field] = value
                _write_state(root, state)
                with self.assertRaises(D03Error):
                    rehearse(root)


if __name__ == "__main__":
    unittest.main()
