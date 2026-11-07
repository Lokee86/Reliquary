import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from synthetic import (
    SyntheticFrameError,
    UnsupportedSyntheticFamily,
    _frame_bytes_unchecked_for_tests,
    build_frame,
    parse_frame,
)


class SyntheticFrameTests(unittest.TestCase):
    def test_known_synthetic_families_round_trip(self):
        examples = (
            (
                "synthetic.archive_turn.v1",
                {"conversation_id": "fixture-c", "node_id": "fixture-n", "text": "synthetic text"},
            ),
            (
                "synthetic.memory_revision.v1",
                {
                    "memory_id": "fixture-m",
                    "revision": "1",
                    "title": "fixture title",
                    "body": "fixture body",
                },
            ),
        )
        for family, payload in examples:
            with self.subTest(family=family):
                self.assertEqual(parse_frame(build_frame(family, payload)).payload, payload)

    def test_build_is_deterministic_and_test_only(self):
        payload = {"conversation_id": "fixture-c", "node_id": "fixture-n", "text": "synthetic"}
        first = build_frame("synthetic.archive_turn.v1", payload)
        self.assertEqual(first, build_frame("synthetic.archive_turn.v1", payload))
        self.assertTrue(first.startswith(b"D01SYN01"))
        with self.assertRaises(SyntheticFrameError):
            parse_frame(b"not-a-test-frame")

    def test_unknown_family_is_rejected_even_with_valid_digest(self):
        raw = _frame_bytes_unchecked_for_tests(
            "synthetic.unlisted_family.v1", b'{"value":"synthetic"}'
        )
        with self.assertRaises(UnsupportedSyntheticFamily):
            parse_frame(raw)

    def test_truncation_trailing_bytes_and_digest_mismatch_reject(self):
        valid = build_frame(
            "synthetic.archive_turn.v1",
            {"conversation_id": "c", "node_id": "n", "text": "fixture"},
        )
        with self.assertRaises(SyntheticFrameError):
            parse_frame(valid[:-1])
        with self.assertRaises(SyntheticFrameError):
            parse_frame(valid + b"x")
        corrupted = bytearray(valid)
        corrupted[-1] ^= 0x01
        with self.assertRaises(SyntheticFrameError):
            parse_frame(bytes(corrupted))

    def test_version_duplicate_keys_numbers_and_unknown_fields_reject(self):
        with self.assertRaises(SyntheticFrameError):
            parse_frame(
                _frame_bytes_unchecked_for_tests(
                    "synthetic.archive_turn.v1",
                    b'{"conversation_id":"c","node_id":"n","text":"x"}',
                    version=2,
                )
            )
        with self.assertRaises(SyntheticFrameError):
            parse_frame(
                _frame_bytes_unchecked_for_tests(
                    "synthetic.archive_turn.v1",
                    b'{"conversation_id":"c","node_id":"n","text":"x","text":"y"}',
                )
            )
        with self.assertRaises(SyntheticFrameError):
            parse_frame(
                _frame_bytes_unchecked_for_tests(
                    "synthetic.archive_turn.v1",
                    b'{"conversation_id":"c","node_id":"n","text":3}',
                )
            )
        with self.assertRaises(SyntheticFrameError):
            parse_frame(
                _frame_bytes_unchecked_for_tests(
                    "synthetic.archive_turn.v1",
                    b'{"conversation_id":"c","node_id":"n","text":"x","future":"x"}',
                )
            )


if __name__ == "__main__":
    unittest.main()
