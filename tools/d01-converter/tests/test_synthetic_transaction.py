import hashlib
import os
import sys
import tempfile
import unittest
from contextlib import contextmanager
from unittest.mock import patch
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))


from coverage_map import load_provisional_coverage_map
from pgc01_adapter import load_pinned_snapshot
from synthetic import SyntheticFrameError, build_frame, parse_frame
from synthetic_transaction import (
    CandidateAlreadyExists,
    SyntheticPublicationError,
    create_only_synthetic_artifact,
)


_CONTRACT_ROOT = os.environ.get("RELIQUARY_PG_C01_ROOT")


@contextmanager
def _temporary_workspace():
    manager = tempfile.TemporaryDirectory()
    try:
        yield manager
    finally:
        manager.cleanup()

def _frame(family: str, text: str) -> bytes:
    if family == "synthetic.archive_turn.v1":
        payload = {"conversation_id": "fixture-c", "node_id": "fixture-n", "text": text}
    else:
        payload = {"memory_id": "fixture-m", "revision": "1", "title": "fixture", "body": text}
    return build_frame(family, payload)


@unittest.skipUnless(_CONTRACT_ROOT, "set RELIQUARY_PG_C01_ROOT to the PG-C01 contract root")
class SyntheticCreateOnlyTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.snapshot = load_pinned_snapshot(Path(_CONTRACT_ROOT))
        tool_root = Path(__file__).resolve().parents[1]
        cls.coverage_map = load_provisional_coverage_map(
            tool_root / "codec-coverage.provisional.json", cls.snapshot
        )

    def test_create_only_writes_only_a_test_artifact_under_temporary_directory(self):
        data = _frame("synthetic.archive_turn.v1", "fixture body")
        with _temporary_workspace() as workspace:
            receipt = create_only_synthetic_artifact(
                workspace, "owner.d01synthetic", data, self.snapshot, self.coverage_map
            )
            output = Path(workspace.name) / receipt.file_name
            self.assertEqual(output.read_bytes(), data)
            self.assertEqual(parse_frame(output.read_bytes()).family, "synthetic.archive_turn.v1")
            self.assertEqual(receipt.sha256, hashlib.sha256(data).hexdigest())
            self.assertEqual(receipt.byte_count, len(data))
            self.assertEqual(receipt.state, "synthetic-test-artifact")


    def test_existing_destination_is_preserved_without_overwrite(self):
        data = _frame("synthetic.archive_turn.v1", "new fixture")
        with _temporary_workspace() as workspace:
            output = Path(workspace.name) / "candidate.d01synthetic"
            output.write_bytes(b"pre-existing-sentinel")
            with self.assertRaises(CandidateAlreadyExists):
                create_only_synthetic_artifact(
                    workspace, output.name, data, self.snapshot, self.coverage_map
                )
            self.assertEqual(output.read_bytes(), b"pre-existing-sentinel")
            self.assertEqual(list(Path(workspace.name).glob(".d01-synthetic-stage-*")), [])

    def test_racing_writers_publish_at_most_one_complete_fixture(self):
        frames = (
            _frame("synthetic.archive_turn.v1", "race alpha"),
            _frame("synthetic.memory_revision.v1", "race beta"),
        )
        with _temporary_workspace() as workspace:
            def attempt(data):
                try:
                    return create_only_synthetic_artifact(
                        workspace, "same-name.d01synthetic", data, self.snapshot, self.coverage_map
                    )
                except CandidateAlreadyExists as error:
                    return error

            with ThreadPoolExecutor(max_workers=2) as executor:
                outcomes = list(executor.map(attempt, frames))
            successes = [item for item in outcomes if not isinstance(item, Exception)]
            collisions = [item for item in outcomes if isinstance(item, CandidateAlreadyExists)]
            self.assertEqual(len(successes), 1)
            self.assertEqual(len(collisions), 1)
            output = Path(workspace.name) / "same-name.d01synthetic"
            published = output.read_bytes()
            self.assertIn(published, frames)
            self.assertEqual(parse_frame(published).family, successes[0].semantic_family)
            self.assertEqual(list(Path(workspace.name).glob(".d01-synthetic-stage-*")), [])

    def test_unclassified_input_and_unsafe_names_create_nothing(self):
        with _temporary_workspace() as workspace:
            with self.assertRaises(SyntheticFrameError):
                create_only_synthetic_artifact(
                    workspace, "source", b"CVAAFMT2", self.snapshot, self.coverage_map
                )
            good_frame = _frame("synthetic.archive_turn.v1", "valid")
            for name in ("../escape", "subdir/file", "", "."):
                with self.subTest(name=name), self.assertRaises(ValueError):
                    create_only_synthetic_artifact(workspace, name, good_frame, self.snapshot, self.coverage_map)
            self.assertEqual(list(Path(workspace.name).iterdir()), [])

    def test_no_replace_failure_does_not_fall_back_to_overwrite(self):
        data = _frame("synthetic.archive_turn.v1", "fixture")
        with _temporary_workspace() as workspace:
            output = Path(workspace.name) / "candidate"
            with patch("synthetic_transaction.os.link", side_effect=OSError("link unsupported")):
                with self.assertRaises(SyntheticPublicationError):
                    create_only_synthetic_artifact(workspace, output.name, data, self.snapshot, self.coverage_map)
            self.assertFalse(output.exists())
            self.assertEqual(list(Path(workspace.name).glob(".d01-synthetic-stage-*")), [])


if __name__ == "__main__":
    unittest.main()