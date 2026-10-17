"""Independent checks for offline calibration decisions, not production F8 tests."""
import json
import math
import unittest
from freshness_semantic_calibrate import Curve, cosine, rank, select, score_trajectory, evaluate, fixture, report


class CalibrationOracleTests(unittest.TestCase):
    def test_inclusive_threshold_and_cap(self):
        c = Curve("linear", .75, 10)
        self.assertEqual([c.bonus(s) for s in (.749999, .75, .8, .9, 1.)], [0, 1, 2, 6, 10])

    def test_monotonic_exhaustive_grid(self):
        for name, cap in (("linear", 10), ("steep", 25), ("fixed", 5)):
            c = Curve(name, .75, cap)
            values = [c.bonus(i/10000) for i in range(7500, 10001)]
            self.assertEqual(values, sorted(values))
            self.assertEqual(values[-1], cap)
            self.assertGreaterEqual(values[0], 1)

    def test_invalid_inputs(self):
        for v in ([0, 0], [float("nan"), 1], [float("inf"), 1]):
            self.assertIsNone(cosine([1, 0], v))
        self.assertIsNone(cosine([1, 0], [1]))
        with self.assertRaises(ValueError):
            Curve("linear", float("nan"))
        with self.assertRaises(ValueError):
            Curve("linear", .75).bonus(float("nan"))

    def test_filter_before_budget_and_duplicate_fragments(self):
        row = fixture()[0]
        a, _ = rank(row)
        b, _ = rank({**row, "sources": [row["source"]]*10})
        self.assertEqual(a, b)
        chosen = select(a, Curve("linear", .75), 2)
        self.assertEqual(len(chosen), 2)
        self.assertNotIn("future", [x[0] for x in chosen])
        self.assertNotIn("self", [x[0] for x in chosen])

    def test_heap_matches_independent_full_sort(self):
        matches = {f"{i:04}": ((i % 7)/10+.3, None) for i in range(1000)}
        curve = Curve("linear", .75)
        expected = sorted([(i, s, label) for i, (s, label) in matches.items() if s >= .75],
                          key=lambda x: (-x[1], x[0]))[:64]
        self.assertEqual(select(matches, curve, 64), expected)
        self.assertEqual(select(dict(reversed(list(matches.items()))), curve, 64), expected)

    def test_unlabeled_is_unknown(self):
        metrics = evaluate([( {"a": (.9, None)}, {})], Curve("linear", .75), 64)
        self.assertIsNone(metrics["false_match_rate"])
        self.assertIsNone(metrics["labeled_recall"])
        self.assertEqual(metrics["labeled_selected"], 0)

    def test_labels_are_turn_scoped(self):
        metrics = evaluate([({"a": (.9, True)}, {}), ({"a": (.9, False)}, {})],
                           Curve("linear", .75), 64)
        self.assertEqual(metrics["false_match_rate"], .5)
        self.assertEqual(metrics["labeled_recall"], 1.)

    def test_trajectory_matches_per_turn_decay_oracle(self):
        events = [(12, 1), (113, 10), (1017, 6), (3021, 10)]
        for initial in (-100, -51, 0, 95, 100):
            value = initial
            by_turn = dict(events)
            for turn in range(1, 3031):
                if turn % 10 == 0:
                    value = max(-100, value-1)
                if turn in by_turn:
                    value = min(100, value+by_turn[turn])
            self.assertEqual(score_trajectory(events, initial, 3030)["tail_score"], value)

    def test_remainder_and_saturation(self):
        got = score_trajectory([(12, 10), (19, 10)], 100, 20)
        self.assertEqual(got["tail_score"], 99)
        self.assertEqual(got["events"][-1]["accounted_turn"], 10)
        self.assertEqual(score_trajectory([(2000, 50)], 100)["tail_score"], -50)

    def test_surrogate_self_exclusion(self):
        row = {"source": [1., 0.], "exclude_id": "self", "targets": [
            {"id": "self", "vector": [1., 0.]}, {"id": "other", "vector": [1., 0.]}]}
        matches, stats = rank(row)
        self.assertEqual(set(matches), {"other"})
        self.assertEqual(stats["eligibility_filtered_rows"], 1)

    def test_deterministic_report(self):
        args = (fixture(), {"kind": "test"}, [.7, .75, .8], [16, 32, 64], [("linear", 10)])
        self.assertEqual(json.dumps(report(*args), sort_keys=True),
                         json.dumps(report(*args), sort_keys=True))


if __name__ == "__main__":
    unittest.main()
