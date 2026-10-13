"""N1 design oracles; these do not exercise or claim Rust runtime acceptance.

Run: python -m unittest discover -s scripts -p test_freshness_normalization_contract.py
"""
import unittest


def accepted_clock(records):
    """Records: (source key, accepted position, provenance timestamp)."""
    ordered = sorted(records, key=lambda record: record[1])
    if len({key for key, _, _ in ordered}) != len(ordered):
        raise ValueError("duplicate source")
    if [position for _, position, _ in ordered] != list(range(1, len(ordered) + 1)):
        raise ValueError("invalid accepted prefix")
    if any(not isinstance(timestamp, int) for _, _, timestamp in ordered):
        raise ValueError("missing timestamp evidence")
    return {key: position for key, position, _ in ordered}


def fragment_event(owner, fragment, represented, clock, comparisons, targets):
    """Minimal intended-contract oracle, independent of product codecs/search.

    targets: target -> (birth, provenance source keys, excluded source cohorts).
    comparisons: (target, already-qualified bonus), including repeated rows.
    """
    if not represented or any(key not in clock for key in represented):
        raise ValueError("unavailable source evidence")
    position = max(clock[key] for key in represented)
    effects = {}
    for target, bonus in comparisons:
        birth, provenance, cohorts = targets[target]
        if birth >= position or set(represented) & (provenance | cohorts):
            continue
        effects[target] = max(effects.get(target, 0), bonus)
    return (owner, "semantic-fragment", fragment), position, effects


class NormalizationDesignContract(unittest.TestCase):
    def setUp(self):
        self.clock = accepted_clock([("early", 1, 200), ("late", 2, 100)])
        self.targets = {
            "older": (0, set(), set()),
            "self-early": (0, {"early"}, set()),
            "cohort-early": (0, set(), {"early"}),
            "same-position": (2, set(), set()),
            "future": (3, set(), set()),
        }

    def event(self, fragment="f", represented=("early", "late"), comparisons=(("older", 4),)):
        return fragment_event("owner", fragment, represented, self.clock, comparisons, self.targets)

    def test_mixed_fragment_at_latest_position_only(self):
        self.assertEqual(self.event()[1:], (2, {"older": 4}))
        self.assertNotEqual(self.event()[1], self.clock["early"])

    def test_strongest_comparison_never_sum(self):
        rows = (("older", 4), ("older", 9), ("older", 4))
        self.assertEqual(self.event(comparisons=rows)[2], {"older": 9})
        self.assertEqual(self.event(comparisons=rows[::-1]), self.event(comparisons=rows))

    def test_every_represented_turn_and_cohort_excluded(self):
        rows = tuple((target, 10) for target in self.targets)
        self.assertEqual(self.event(comparisons=rows)[2], {"older": 10})

    def test_reembedding_changes_evidence_not_identity(self):
        original = self.event()
        regenerated = self.event(comparisons=(("older", 10),))
        self.assertEqual(original[0], regenerated[0])
        accepted = {original[0]: original[2]}
        accepted.setdefault(regenerated[0], regenerated[2])
        self.assertEqual(accepted[original[0]], {"older": 4})
        self.assertEqual(len(accepted), 1)

    def test_overlap_is_separate_fragment_not_multiple_turn_events(self):
        self.assertNotEqual(self.event("f")[0], self.event("g")[0])
        self.assertEqual(self.event(represented=("late", "early", "early")), self.event())

    def test_owner_identity_isolated(self):
        other = fragment_event("other", "f", ("early", "late"), self.clock, (), self.targets)
        self.assertNotEqual(other[0], self.event()[0])

    def test_timestamp_regression_preserves_accepted_clock(self):
        self.assertEqual(self.clock, {"early": 1, "late": 2})
        self.assertEqual(accepted_clock([("b", 2, -1), ("a", 1, 0)]), {"a": 1, "b": 2})

    def test_timestamp_ties_do_not_resort_source_keys(self):
        self.assertEqual(accepted_clock([("z", 1, 0), ("a", 2, 0)]), {"z": 1, "a": 2})

    def test_invalid_prefix_and_missing_timestamp_rejected(self):
        for records in ([('a', 2, 0)], [('a', 1, 0), ('b', 1, 1)],
                        [('a', 1, 0), ('a', 2, 1)], [('a', 1, None)]):
            with self.subTest(records=records), self.assertRaises(ValueError):
                accepted_clock(records)

    def test_missing_source_never_guesses_availability(self):
        for represented in ((), ("missing",), ("early", "missing")):
            with self.subTest(represented=represented), self.assertRaises(ValueError):
                self.event(represented=represented)


if __name__ == "__main__":
    unittest.main()
