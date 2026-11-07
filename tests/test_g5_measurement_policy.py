"""G5-T01 statistics/policy self-tests; this module never runs product benchmarks."""

from __future__ import annotations

import hashlib
import json
import math
from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parents[1]
G5 = ROOT / "benchmarks" / "g5"
POLICY_PATH = G5 / "policy-v1.json"
MANIFEST_PATH = G5 / "workloads-v1.json"
PROFILE_PATH = G5 / "machine-profile-v1.schema.json"
VECTORS_PATH = G5 / "statistics-selftests-v1.json"
MASK64 = (1 << 64) - 1


def load_json(path: Path) -> dict:
    with path.open(encoding="utf-8") as handle:
        return json.load(handle)


def expand(session: dict) -> list[float]:
    values = []
    for run in session["runs"]:
        value = float(run["value"])
        count = run["count"]
        if not math.isfinite(value) or value <= 0 or not isinstance(count, int) or count <= 0:
            raise ValueError("samples must be positive and finite")
        values.extend([value] * count)
    if not values:
        raise ValueError("session must contain samples")
    return values


def sample_statistic(values: list[float], percentile: float) -> float:
    if percentile == 50:
        if not values or any(not math.isfinite(value) or value <= 0 for value in values):
            raise ValueError("invalid median input")
        ordered = sorted(values)
        middle = len(ordered) // 2
        if len(ordered) % 2 == 0:
            return (ordered[middle - 1] + ordered[middle]) / 2
        return ordered[middle]
    return nearest_rank(values, percentile)


def nearest_rank(values: list[float], percentile: float) -> float:
    if not values or not math.isfinite(percentile) or not 0 < percentile <= 100:
        raise ValueError("invalid nearest-rank input")
    ordered = sorted(values)
    return ordered[math.ceil(percentile * len(ordered) / 100.0) - 1]


class SplitMix64:
    def __init__(self, seed: int) -> None:
        self.state = seed & MASK64

    def next_u64(self) -> int:
        self.state = (self.state + 0x9E3779B97F4A7C15) & MASK64
        value = self.state
        value = ((value ^ (value >> 30)) * 0xBF58476D1CE4E5B9) & MASK64
        value = ((value ^ (value >> 27)) * 0x94D049BB133111EB) & MASK64
        return value ^ (value >> 31)

    def randbelow(self, bound: int) -> int:
        if bound <= 0:
            raise ValueError("bound must be positive")
        span = 1 << 64
        cutoff = span - span % bound
        while True:
            value = self.next_u64()
            if value < cutoff:
                return value % bound


def seed_for(policy_id: str, case_id: str, metric_id: str, session_id: str) -> int:
    material = "\0".join((policy_id, case_id, metric_id, session_id)).encode("utf-8")
    return int.from_bytes(hashlib.sha256(material).digest()[:8], "big")


def paired_ratio_ci(reference: list[dict], candidate: list[dict], *, policy_id: str, case_id: str, metric_id: str, quantile: float, replicates: int) -> tuple[float, float]:
    if not reference or len(reference) != len(candidate):
        raise ValueError("sessions must be non-empty and paired")
    pairs = []
    streams = []
    seen = set()
    for before, after in zip(reference, candidate):
        sid = before.get("session_id")
        if not isinstance(sid, str) or sid in seen or after.get("session_id") != sid:
            raise ValueError("session identities must be unique and paired")
        seen.add(sid)
        old_values, new_values = expand(before), expand(after)
        if len(old_values) != len(new_values):
            raise ValueError("paired sample lengths differ")
        pairs.append((old_values, new_values))
        streams.append(SplitMix64(seed_for(policy_id, case_id, metric_id, sid)))
    if replicates <= 0:
        raise ValueError("replicates must be positive")
    ratios = []
    for _ in range(replicates):
        old_sample, new_sample = [], []
        for (old_values, new_values), stream in zip(pairs, streams):
            count = len(old_values)
            for _ in range(count):
                index = stream.randbelow(count)
                old_sample.append(old_values[index])
                new_sample.append(new_values[index])
        ratios.append(sample_statistic(new_sample, quantile) / sample_statistic(old_sample, quantile))
    return nearest_rank(ratios, 2.5), nearest_rank(ratios, 97.5)


def classify(lower: float, upper: float, direction: str, limit: float) -> str:
    if lower > upper or not math.isfinite(lower) or not math.isfinite(upper):
        raise ValueError("invalid interval")
    if direction == "maximum":
        if upper <= limit:
            return "PASS"
        if lower > limit:
            return "FAIL"
    elif direction == "minimum":
        if lower >= limit:
            return "PASS"
        if upper < limit:
            return "FAIL"
    else:
        raise ValueError("invalid direction")
    return "INCONCLUSIVE"


def memory_metric_comparability(
    reference_samples: list[float],
    candidate_samples: list[float],
    *,
    collector_semantics_match: bool,
    old_capability_supported: bool,
    trustworthy_reference: bool,
) -> str:
    """Classify metric comparability only, not release sample sufficiency."""
    if not old_capability_supported:
        return "NOT_COMPARABLE"
    if not trustworthy_reference:
        return "UNBASELINED"
    if (
        not collector_semantics_match
        or not reference_samples
        or not candidate_samples
        or len(reference_samples) != len(candidate_samples)
    ):
        return "INCOMPLETE"
    if any(not math.isfinite(value) or value <= 0 for value in reference_samples + candidate_samples):
        return "INCOMPLETE"
    return "COMPARABLE"


class G5PolicyTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.policy = load_json(POLICY_PATH)
        cls.manifest = load_json(MANIFEST_PATH)
        cls.profile = load_json(PROFILE_PATH)
        cls.vectors = load_json(VECTORS_PATH)

    def test_case_matrix_and_synthetic_only_scope(self) -> None:
        expected = [f"G5-{n:02d}" for n in range(1, 10)]
        self.assertEqual(self.manifest["case_ids"], expected)
        self.assertEqual([case["id"] for case in self.manifest["cases"]], expected)
        self.assertFalse(self.manifest["scope"]["live_provider_calls"])
        self.assertFalse(self.manifest["scope"]["private_source_corpus"])
        self.assertFalse(self.manifest["scope"]["measurement_results"])
        self.assertFalse(self.manifest["scope"]["hermes"])

    def test_source_controlled_g4_aliases_and_model_inputs(self) -> None:
        cohort = self.manifest["synthetic_g4_cohort"]
        self.assertEqual(cohort["cohort_id"], "g4-native-rel21-corpus-v1")
        self.assertEqual(cohort["seed"], "g4-rel21-v1")
        self.assertEqual(
            [owner["alias"] for owner in cohort["owners"]],
            ["rel-lantern", "rel-cascade", "phy-reader-a", "phy-reader-b"],
        )
        probe = self.manifest["synthetic_probe_data"]
        self.assertEqual(probe["vector_profile"]["dimensions"], 4)
        for record in probe["records"]:
            self.assertEqual(len(record["vector_bits"]), 4)
        self.assertTrue(all(item["id"] for item in probe["model_fixtures"]))

    def test_workload_dimensions_and_correctness_are_explicit(self) -> None:
        for case in self.manifest["cases"]:
            with self.subTest(case=case["id"]):
                self.assertTrue(case["measurements"])
                self.assertTrue(case["checks"])
        self.assertEqual(self.manifest["cases"][8]["comparison"], "native rehearsal outside G5-T03; UNBASELINED until its absolute limit is reviewed.")

    def test_g5_08_uses_fixed_population_and_public_retrieval_defaults(self) -> None:
        population = self.manifest["population_generator"]
        self.assertEqual(population["sizes"], [1000, 10000, 50000])
        self.assertEqual(population["retrieval_candidate_limit"], 30)
        self.assertEqual(population["retrieval_result_limit"], 10)
        self.assertFalse(population["accuracy_claim"])

    def test_thresholds_match_g5_spec_without_relaxation(self) -> None:
        thresholds = self.policy["thresholds"]
        self.assertEqual(thresholds["unexplained_correctness_privacy_history_recovery_differences_max"], 0)
        self.assertEqual(thresholds["median_latency_candidate_over_reference_max"], 1.10)
        self.assertEqual(thresholds["p95_latency_candidate_over_reference_max"], 1.25)
        self.assertEqual(thresholds["sustained_throughput_candidate_over_reference_min"], 0.90)
        self.assertEqual(thresholds["peak_memory_candidate_over_reference_max"], 1.15)
        self.assertEqual(thresholds["p99"], "diagnostic_only")

    def test_memory_measurements_are_distinct_and_mapped_per_case(self) -> None:
        contract = self.policy["memory_measurement_contract"]
        expected_metrics = ["peak_private_bytes", "peak_working_set_bytes"]
        self.assertEqual(self.policy["thresholds"]["peak_memory_metrics"], expected_metrics)
        self.assertEqual(contract["applies_to_workload_ids"], ["G5-01", "G5-02", "G5-04", "G5-07", "G5-08"])
        self.assertTrue(contract["comparison"]["both_metrics_required_for_memory_gate_pass"])
        self.assertTrue(contract["comparison"]["never_substitute_one_metric_for_the_other"])
        for metric_id in expected_metrics:
            with self.subTest(metric=metric_id):
                spec = contract["metrics"][metric_id]
                self.assertEqual(spec["workload_measurement"], metric_id)
                self.assertEqual(spec["units"], "bytes")
        for case in self.manifest["cases"]:
            if case["id"] in contract["applies_to_workload_ids"]:
                with self.subTest(case=case["id"]):
                    self.assertIn("peak_private_bytes", case["measurements"])
                    self.assertIn("peak_working_set_bytes", case["measurements"])
                    self.assertNotIn("peak_memory_bytes", case["measurements"])

    def test_memory_metric_comparability_is_distinct_from_sample_sufficiency(self) -> None:
        comparable_inputs = dict(collector_semantics_match=True, old_capability_supported=True, trustworthy_reference=True)
        self.assertEqual(memory_metric_comparability([100.0], [110.0], **comparable_inputs), "COMPARABLE")
        self.assertEqual(memory_metric_comparability([100.0], [], **comparable_inputs), "INCOMPLETE")
        self.assertEqual(memory_metric_comparability([100.0], [110.0], collector_semantics_match=False, old_capability_supported=True, trustworthy_reference=True), "INCOMPLETE")
        self.assertEqual(memory_metric_comparability([100.0], [110.0, 111.0], **comparable_inputs), "INCOMPLETE")
        self.assertEqual(memory_metric_comparability([100.0], [110.0], collector_semantics_match=True, old_capability_supported=False, trustworthy_reference=True), "NOT_COMPARABLE")
        self.assertEqual(memory_metric_comparability([100.0], [110.0], collector_semantics_match=True, old_capability_supported=True, trustworthy_reference=False), "UNBASELINED")
        contract = self.policy["memory_measurement_contract"]
        self.assertIn("INCOMPLETE", contract["comparability"]["missing_or_incompatible_collector_or_sample"])
        self.assertIn("do not", contract["comparability"]["missing_or_incompatible_collector_or_sample"])
        self.assertIn("NOT_COMPARABLE", contract["comparability"]["unsupported_old_product_capability"])
        self.assertIn("UNBASELINED", contract["comparability"]["no_trustworthy_reference"])
        gate = contract["comparability"]["case_memory_gate"]
        self.assertIn("FAIL if either", gate)
        self.assertIn("INCONCLUSIVE", gate)
        self.assertIn("PASS only when both", gate)

    def test_memory_metric_report_records_provenance_and_separate_sample_sufficiency(self) -> None:
        record = self.policy["evidence"]["memory_metric_record"]
        self.assertEqual(record["location"], "per-run result evidence.memory_metrics[]")
        self.assertEqual(record["record_key"], ["case_id", "workload_input_sha256", "session_id", "pair_index", "metric_id"])
        self.assertIn("comparability_status", record["required_fields"])
        self.assertIn("sample_sufficiency_status", record["required_fields"])
        self.assertIn("COMPARABLE", record["comparability_status_values"])
        self.assertIn("SUFFICIENT", record["sample_sufficiency_status_values"])
        self.assertIn("INCOMPLETE", record["sample_sufficiency_status_values"])
        self.assertIn("collector_api_or_source", record["arm_fields"])
        self.assertIn("collector_name", record["arm_fields"])
        self.assertIn("collector_version", record["arm_fields"])
        self.assertIn("collector_semantics", record["arm_fields"])
        self.assertIn("COMPARABLE alone is not sufficient", record["sample_sufficiency"])
        self.assertIn("measured-pair", record["sample_sufficiency"])
        self.assertGreaterEqual(self.policy["sampling"]["short_operations"]["measured_pairs_per_session_min"], 30)
        self.assertEqual(self.policy["sampling"]["independent_sessions"], 3)

    def test_sample_minima_caps_and_sessions_are_explicit(self) -> None:
        sampling = self.policy["sampling"]
        self.assertEqual(sampling["independent_sessions"], 3)
        short = sampling["short_operations"]
        self.assertGreaterEqual(short["warmups_per_arm_per_session_min"], 5)
        self.assertGreaterEqual(short["measured_pairs_per_session_min"], 30)
        self.assertGreaterEqual(short["measured_pairs_per_session_cap"], short["measured_pairs_per_session_min"])
        costly = sampling["costly_open_or_maintenance"]
        self.assertGreaterEqual(costly["alternating_old_new_pairs_per_session_min"], 10)
        self.assertGreaterEqual(costly["alternating_old_new_pairs_per_session_cap"], costly["alternating_old_new_pairs_per_session_min"])

    def test_missing_or_unsupported_baselines_do_not_pass(self) -> None:
        rules = self.policy["comparison"]["rules"]
        self.assertIn("NOT_COMPARABLE", rules["unsupported_old_capability"])
        self.assertIn("UNBASELINED", rules["no_trustworthy_reference"])
        self.assertIn("INCONCLUSIVE", rules["incomparable_build_or_inputs"])
        self.assertEqual(self.manifest["paired_comparison"]["g5_t03_cases"], [f"G5-{n:02d}" for n in range(1, 9)])

    def test_machine_profile_requires_host_build_inputs_custody_and_exclusivity(self) -> None:
        schema = self.profile
        self.assertFalse(schema["additionalProperties"])
        self.assertTrue({"machine", "builds", "inputs", "conditions", "custody", "measurement_eligibility"} <= set(schema["required"]))
        conditions = schema["properties"]["conditions"]
        for key in ["exclusive_window", "overlapping_compilation", "overlapping_benchmark", "overlapping_disk_scan", "overlapping_model_inference", "security_tools_changed", "global_performance_settings_changed", "unrelated_workloads_killed"]:
            self.assertIn(key, conditions["required"])

    def test_splitmix64_reference_words(self) -> None:
        stream = SplitMix64(0)
        actual = [f"{stream.next_u64():016x}" for _ in range(3)]
        self.assertEqual(actual, self.vectors["splitmix64_seed_zero_first_words_hex"])

    def test_paired_bootstrap_vectors(self) -> None:
        for case in self.vectors["cases"]:
            with self.subTest(case=case["id"]):
                lower, upper = paired_ratio_ci(
                    case["reference"],
                    case["candidate"],
                    policy_id=self.policy["schema_id"],
                    case_id=case["id"],
                    metric_id=case["metric"],
                    quantile=case["quantile"],
                    replicates=self.vectors["bootstrap_replicates"],
                )
                self.assertEqual(classify(lower, upper, case["direction"], case["limit"]), case["expected_decision"])
                if case.get("expected_interval") is not None:
                    self.assertAlmostEqual(lower, case["expected_interval"][0])
                    self.assertAlmostEqual(upper, case["expected_interval"][1])
                if case.get("straddle"):
                    self.assertLessEqual(lower, case["limit"])
                    self.assertGreater(upper, case["limit"])

    def test_unpaired_and_nonpositive_samples_are_rejected(self) -> None:
        with self.assertRaises(ValueError):
            paired_ratio_ci(
                [{"session_id": "s1", "runs": [{"value": 1, "count": 2}]}],
                [{"session_id": "s1", "runs": [{"value": 1, "count": 1}]}],
                policy_id=self.policy["schema_id"], case_id="bad", metric_id="latency-p50",
                quantile=50, replicates=10,
            )
        with self.assertRaises(ValueError):
            expand({"runs": [{"value": 0, "count": 1}]})

    def test_even_sample_median_uses_midpoint(self) -> None:
        self.assertEqual(sample_statistic([1, 2, 3, 4], 50), 2.5)
        self.assertEqual(sample_statistic([1, 2, 3, 4], 95), 4)

    def test_threshold_equality_and_interval_straddling(self) -> None:
        self.assertEqual(classify(1.10, 1.10, "maximum", 1.10), "PASS")
        self.assertEqual(classify(0.90, 0.90, "minimum", 0.90), "PASS")
        self.assertEqual(classify(0.90, 1.10, "maximum", 1.10), "PASS")
        self.assertEqual(classify(1.10, 1.20, "maximum", 1.10), "INCONCLUSIVE")
        self.assertEqual(classify(0.80, 0.89, "minimum", 0.90), "FAIL")


if __name__ == "__main__":
    unittest.main()
