"""Regression checks for clock-neutral Archive backing frontiers."""
from collections import defaultdict, Counter
import hashlib
import unittest

from f03_verify_prefix_availability import Attachment, cut_rows, file_gate, turn_references


class AvailabilityTests(unittest.TestCase):
    def test_same_head_late_binding_changes_prefix_availability(self):
        content = hashlib.sha256(b"body").digest()
        length = (4).to_bytes(8, "little")
        legacy = Attachment(b"", content, length, b"file", b"text/plain")
        attachment = Attachment(legacy.legacy_id(), content, length, b"file", b"text/plain")
        frontiers = {point: (40 + point, point, str(point)) for point in range(5)}

        def project(binding_cut):
            deltas = defaultdict(Counter)
            identity, backing, binding = file_gate(
                attachment, 1, None, (binding_cut, content, "same-path"), 4, deltas)
            self.assertIsNone(identity)
            self.assertEqual(backing, (1, binding_cut - 1))
            self.assertEqual(binding, binding_cut)
            return list(cut_rows(frontiers, deltas, 4, 1))

        first, second = project(2), project(3)
        self.assertEqual(first[-1], second[-1])
        self.assertEqual(first[2]["archive_backing_gate"], "satisfied")
        self.assertEqual(second[2]["archive_backing_gate"], "blocked")

    def test_project_derived_id_fails_before_binding_even_with_inline_content(self):
        content = hashlib.sha256(b"body").digest()
        length = (4).to_bytes(8, "little")
        provisional = Attachment(b"", content, length, b"file", b"")
        attachment = Attachment(provisional.project_id("path"), content, length, b"file", b"")
        identity, backing, _ = file_gate(
            attachment, 1, 0, (3, content, "path"), 3, defaultdict(Counter))
        self.assertEqual(identity, (1, 2))
        self.assertIsNone(backing)

    def test_grouped_clock_gap_keeps_previous_complete_frontier(self):
        frontiers = {0: (44, 1, "head-zero"), 3: (97, 2, "head-three")}
        rows = list(cut_rows(frontiers, defaultdict(Counter), 3, 1))
        self.assertEqual([row["physical_frontier_end"] for row in rows], ["44", "44", "44", "97"])
        self.assertEqual([row["physical_frame_count"] for row in rows], ["1", "1", "1", "2"])

    def test_malformed_turn_attachment_count_fails(self):
        wire = b"CVATUR03" + (1).to_bytes(8, "little") + b"\0" * 40
        for field in (b"node", b"conversation", b"", b"user", b""):
            wire += len(field).to_bytes(4, "little") + field
        wire += (1).to_bytes(4, "little")  # The one claimed attachment is absent.
        with self.assertRaises(ValueError):
            turn_references(wire)


if __name__ == "__main__":
    unittest.main()
