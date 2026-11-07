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
from scripts.d03_synthetic.engine import STATE_FILE, stage
from scripts.d03_synthetic.files import D03Error, parse_json
from scripts.d03_synthetic.validation import validate_generation


class D03IntegrityFailureTests(unittest.TestCase):
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

    def test_missing_manifest_is_structured_and_reparse_path_is_rejected_before_read(self) -> None:
        root = self.root()
        stage(root)
        manifest = root / "generations" / "d03-synthetic-candidate" / "generation-manifest.json"
        manifest.unlink()
        result = self.cli("validate", "--root", str(root), expected=1)
        self.assertEqual(json.loads(result.stderr)["status"], "INCOMPLETE")

        root = self.root()
        stage(root)
        candidate = root / "generations" / "d03-synthetic-candidate"
        alias = root / "generations" / "candidate-alias"
        try:
            os.symlink(candidate, alias, target_is_directory=True)
        except OSError as error:
            self.skipTest(f"directory reparse fixture unavailable: {error}")
        digest = parse_json((root / STATE_FILE).read_bytes())["manifest_sha256"]
        with mock.patch("scripts.d03_synthetic.validation.read_bytes") as reader:
            with self.assertRaisesRegex(D03Error, "reparse point"):
                validate_generation(alias, digest)
            reader.assert_not_called()

    def test_unreadable_state_has_separate_structured_inspection_step(self) -> None:
        root = self.root()
        stage(root)
        (root / STATE_FILE).write_text("{bad json", encoding="utf-8")
        failure = self.cli("validate", "--root", str(root), expected=1)
        envelope = json.loads(failure.stderr)
        self.assertEqual(envelope["status"], "INCOMPLETE")
        self.assertNotIn("persisted_state", envelope)
        self.assertIn("Run inspect-state separately", envelope["recovery_hint"])

        inspection = self.cli("inspect-state", "--root", str(root), expected=1)
        second = json.loads(inspection.stderr)
        self.assertEqual(second["status"], "INCOMPLETE")
        self.assertNotIn("persisted_state", second)
        self.assertIn("inspection failed closed", second["recovery_hint"])
        self.assertNotIn("Traceback", inspection.stderr)

    def test_unsupported_directory_sync_fails_closed(self) -> None:
        directory = Path(tempfile.gettempdir())
        if os.name == "nt":
            patcher = mock.patch(
                "scripts.d03_synthetic.durability._sync_windows_directory",
                side_effect=OSError("unsupported"),
            )
        else:
            patcher = mock.patch(
                "scripts.d03_synthetic.durability.os.fsync",
                side_effect=OSError("unsupported"),
            )
        with patcher:
            with self.assertRaisesRegex(D03Error, "durability sync is unavailable"):
                durability.sync_directory(directory)


if __name__ == "__main__":
    unittest.main()
