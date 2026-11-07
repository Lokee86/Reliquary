from __future__ import annotations

import copy
import hashlib
import sys
import unittest
from pathlib import Path
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

from benchmarks.g5.runner import (
    MEMORY_METRIC_IDS,
    assess_sample_sufficiency,
    classify_memory_gate,
    preflight_result,
    read_json,
    run_synthetic_selftests,
    validate_g5_contract,
    validate_memory_metric_records,
    generated_population_item,
    verify_statistics_selftests,
    validate_frozen_policy,
)
from benchmarks.g5.statistics import (
    SplitMix64,
    bootstrap_ratio_interval,
    median,
    nearest_rank,
    paired_sessions,
)
from benchmarks.g5.workload import ContractError

POLICY = ROOT / "benchmarks" / "g5" / "policy-v1.json"
WORKLOADS = ROOT / "benchmarks" / "g5" / "workloads-v1.json"
SELFTESTS = ROOT / "benchmarks" / "g5" / "statistics-selftests-v1.json"


def session_rows(pair_counts: list[int], include_warmups: bool = False) -> list[dict]:
    """Build explicit, globally alternating synthetic pair-order evidence."""
    expected_first = "reference"
    sessions = []
    for index, pair_count in enumerate(pair_counts, start=1):
        pair_orders = []
        for pair_index in range(pair_count):
            second = "candidate" if expected_first == "reference" else "reference"
            pair_orders.append(
                {
                    "pair_index": pair_index,
                    "first_arm": expected_first,
                    "second_arm": second,
                }
            )
            expected_first = second
        row = {
            "session_id": f"s{index}",
            "session_index": index - 1,
            "measured_pairs_per_arm": {"reference": pair_count, "candidate": pair_count},
            "pair_orders": pair_orders,
        }
        if include_warmups:
            row["warmups_per_arm"] = {"reference": 5, "candidate": 5}
        sessions.append(row)
    return sessions


def metric_record(metric_id: str, comparability: str, sufficiency: str, reason: str | None = None) -> dict:
    record = {
        "case_id": "G5-01",
        "workload_input_sha256": "a" * 64,
        "session_id": "synthetic-session-1",
        "pair_index": 0,
        "metric_id": metric_id,
        "comparability_status": comparability,
        "sample_sufficiency_status": sufficiency,
        "units": "bytes",
        "subject_process_scope": "synthetic test adapter process only",
        "sample_window": {"phase": "one synthetic test operation"},
        "reference_arm": None,
        "candidate_arm": None,
        "unavailable_reason": reason,
    }
    if comparability == "COMPARABLE":
        collector = {
            "collector_name": "synthetic-test-collector",
            "collector_version": "1",
            "collector_api_or_source": "unit-test fixture",
            "collector_semantics": "same synthetic per-operation peak bytes",
            "raw_samples": [100, 120],
        }
        record["reference_arm"] = dict(collector)
        record["candidate_arm"] = dict(collector, raw_samples=[101, 121])
    return record


class G5RunnerContractTests(unittest.TestCase):
    def setUp(self) -> None:
        self.policy = read_json(POLICY)
        self.workloads = read_json(WORKLOADS)

    def test_quantile_methods_match_frozen_policy(self) -> None:
        self.assertEqual(median([1, 2]), 1.5)
        self.assertEqual(nearest_rank(list(range(1, 21)), 95), 19)

    def test_benchmark_preflight_blocks_before_process_or_corpus_access(self) -> None:
        result = preflight_result(self.policy, self.workloads)
        self.assertEqual(result["measurement_status"], "INCOMPLETE")
        self.assertEqual(result["blockers"], ["F03-V04", "D02-T04"])
        self.assertFalse(result["measurement_results"])
        self.assertFalse(result["adapter_processes_started"])
        self.assertFalse(result["private_inputs_read"])

    def test_frozen_manifest_is_synthetic_and_not_measured(self) -> None:
        validate_g5_contract(self.policy, self.workloads)
        result = run_synthetic_selftests(POLICY, WORKLOADS, SELFTESTS)
        self.assertEqual(result["status"], "PASS")
        self.assertFalse(result["measurement_results"])
        self.assertFalse(result["private_inputs_read"])
        self.assertFalse(result["provider_calls"])

    def test_two_memory_metrics_require_provenance_and_keep_status_axes_separate(self) -> None:
        records = [
            metric_record("peak_private_bytes", "COMPARABLE", "INCOMPLETE"),
            metric_record("peak_working_set_bytes", "UNBASELINED", "SUFFICIENT", "No trustworthy reference"),
        ]
        self.assertEqual(validate_memory_metric_records(records, self.policy), [])
        self.assertEqual(classify_memory_gate(records), "INCOMPLETE")

    def test_missing_or_duplicate_memory_metric_never_passes(self) -> None:
        first = metric_record("peak_private_bytes", "COMPARABLE", "SUFFICIENT")
        self.assertTrue(validate_memory_metric_records([first], self.policy))
        self.assertEqual(classify_memory_gate([first]), "INCOMPLETE")
        self.assertTrue(validate_memory_metric_records([first, first], self.policy))
        self.assertEqual(classify_memory_gate([first, first]), "INCOMPLETE")

    def test_memory_gate_fails_if_either_sufficient_comparable_metric_fails(self) -> None:
        records = [
            metric_record("peak_private_bytes", "COMPARABLE", "SUFFICIENT"),
            metric_record("peak_working_set_bytes", "NOT_COMPARABLE", "SUFFICIENT", "Collector semantics differ"),
        ]
        records[0]["decision"] = "FAIL"
        records[1]["decision"] = "PASS"
        self.assertEqual(classify_memory_gate(records), "FAIL")

    def test_memory_gate_pass_needs_both_comparable_and_sufficient_metrics(self) -> None:
        records = [
            metric_record("peak_private_bytes", "COMPARABLE", "SUFFICIENT"),
            metric_record("peak_working_set_bytes", "COMPARABLE", "SUFFICIENT"),
        ]
        for record in records:
            record["decision"] = "PASS"
        self.assertEqual(classify_memory_gate(records), "PASS")
        records[0]["sample_sufficiency_status"] = "INCOMPLETE"
        self.assertEqual(classify_memory_gate(records), "INCOMPLETE")

    def test_three_session_pair_and_warmup_minimums_are_separate_gate(self) -> None:
        sessions = session_rows([30, 30, 30], include_warmups=True)
        self.assertEqual(assess_sample_sufficiency(sessions, self.policy, "short_operations"), ("SUFFICIENT", []))
        sessions[2]["measured_pairs_per_arm"]["candidate"] = 29
        status, reasons = assess_sample_sufficiency(sessions, self.policy, "short_operations")
        self.assertEqual(status, "INCOMPLETE")
        self.assertTrue(any("paired reference/candidate counts differ" in reason for reason in reasons))

    def test_costly_open_cap_is_applied_without_inventing_warmup_requirement(self) -> None:
        sessions = session_rows([10, 10, 10])
        self.assertEqual(assess_sample_sufficiency(sessions, self.policy, "costly_open_or_maintenance"), ("SUFFICIENT", []))
        sessions = session_rows([31, 10, 10])
        status, reasons = assess_sample_sufficiency(sessions, self.policy, "costly_open_or_maintenance")
        self.assertEqual(status, "INCOMPLETE")
        self.assertTrue(any("cap" in reason for reason in reasons))

    def test_alternating_arm_order_is_required_and_enforced_across_sessions(self) -> None:
        sessions = session_rows([30, 30, 30], include_warmups=True)
        self.assertEqual(assess_sample_sufficiency(sessions, self.policy, "short_operations"), ("SUFFICIENT", []))

        sessions[1]["pair_orders"][0]["first_arm"] = "candidate"
        sessions[1]["pair_orders"][0]["second_arm"] = "reference"
        status, reasons = assess_sample_sufficiency(sessions, self.policy, "short_operations")
        self.assertEqual(status, "INCOMPLETE")
        self.assertTrue(any("alternate across all pairs and session boundaries" in reason for reason in reasons))

    def test_arm_order_requires_one_record_per_pair_and_consecutive_pair_indices(self) -> None:
        cases = ("missing", "count", "index", "session_index", "same_arm", "nonalternating")
        for corruption in cases:
            with self.subTest(corruption=corruption):
                sessions = session_rows([30, 30, 30], include_warmups=True)
                first_pair = sessions[0]["pair_orders"][0]
                if corruption == "missing":
                    del sessions[0]["pair_orders"]
                elif corruption == "count":
                    sessions[0]["pair_orders"].pop()
                elif corruption == "index":
                    first_pair["pair_index"] = 1
                elif corruption == "session_index":
                    sessions[0]["session_index"] = 1
                elif corruption == "same_arm":
                    first_pair["second_arm"] = first_pair["first_arm"]
                else:
                    second_pair = sessions[0]["pair_orders"][1]
                    second_pair["first_arm"] = first_pair["first_arm"]
                    second_pair["second_arm"] = first_pair["second_arm"]

                status, reasons = assess_sample_sufficiency(sessions, self.policy, "short_operations")
                self.assertEqual(status, "INCOMPLETE")
                self.assertTrue(reasons)

    def test_memory_metrics_must_share_exact_pair_key_fields(self) -> None:
        mismatches = {
            "case_id": "G5-02",
            "workload_input_sha256": "b" * 64,
            "session_id": "synthetic-session-2",
            "pair_index": 1,
        }
        for field, mismatched_value in mismatches.items():
            with self.subTest(field=field):
                records = [
                    metric_record("peak_private_bytes", "COMPARABLE", "SUFFICIENT"),
                    metric_record("peak_working_set_bytes", "COMPARABLE", "SUFFICIENT"),
                ]
                for record in records:
                    record["decision"] = "PASS"
                records[1][field] = mismatched_value

                errors = validate_memory_metric_records(records, self.policy)
                self.assertTrue(any("identical case/input/session/pair key" in error for error in errors))
                self.assertEqual(classify_memory_gate(records), "INCOMPLETE")

    def test_throughput_bootstrap_uses_ratio_of_medians(self) -> None:
        reference_values = [1, 100, 1000]
        candidate_values = [2, 50, 2000]
        reference = [
            {"session_id": f"s{index}", "runs": [{"value": value, "count": 1}]}
            for index, value in enumerate(reference_values, start=1)
        ]
        candidate = [
            {"session_id": f"s{index}", "runs": [{"value": value, "count": 1}]}
            for index, value in enumerate(candidate_values, start=1)
        ]
        interval = bootstrap_ratio_interval(
            paired_sessions(reference, candidate),
            policy_schema_id="synthetic-policy-v1",
            case_id="synthetic-throughput-ratio-discriminator",
            metric_id="throughput-p50",
            statistic="throughput",
            percentile=50,
            replicates=16,
        )

        self.assertEqual(interval, (0.5, 0.5))
        self.assertEqual(median([new / old for old, new in zip(reference_values, candidate_values)]), 2.0)
        self.assertNotEqual(interval[0], 2.0)

    def test_memory_selftest_uses_memory_statistic_branch(self) -> None:
        reference_values = [1, 100, 1000]
        candidate_values = [2, 50, 2000]
        reference = [
            {"session_id": f"s{index}", "runs": [{"value": value, "count": 1}]}
            for index, value in enumerate(reference_values, start=1)
        ]
        candidate = [
            {"session_id": f"s{index}", "runs": [{"value": value, "count": 1}]}
            for index, value in enumerate(candidate_values, start=1)
        ]
        policy = self.policy
        replicates = policy["comparison"]["bootstrap"]["replicates"]
        selftests = {
            "schema_id": "reliquary-g5-statistics-selftests-v1",
            "vectors_version": 1,
            "bootstrap_replicates": replicates,
            "cases": [
                {
                    "id": "memory-p50-versus-p95-discriminator",
                    "metric": "peak_private_bytes",
                    "quantile": 95,
                    "direction": "maximum",
                    "limit": 1.0,
                    "reference": reference,
                    "candidate": candidate,
                    "expected_interval": [0.5, 0.5],
                    "expected_decision": "PASS",
                }
            ],
        }

        self.assertEqual(verify_statistics_selftests(policy, selftests), [])

    def test_bounded_index_rejects_out_of_range_draw_and_handles_edges(self) -> None:
        with mock.patch.object(SplitMix64, "next_u64", side_effect=[0, 4]) as draw:
            self.assertEqual(SplitMix64(0).bounded(3), 1)
            self.assertEqual(draw.call_count, 2)

        with mock.patch.object(SplitMix64, "next_u64", return_value=7):
            self.assertEqual(SplitMix64(0).bounded(1 << 64), 7)

        for invalid_bound in (0, -1, (1 << 64) + 1):
            with self.subTest(bound=invalid_bound), self.assertRaises(ValueError):
                SplitMix64(0).bounded(invalid_bound)

    def test_frozen_policy_version_thresholds_and_bootstrap_are_enforced(self) -> None:
        self.assertEqual(validate_frozen_policy(self.policy), [])
        mutations = [
            (("policy_version",), 2),
            (("thresholds", "median_latency_candidate_over_reference_max"), 1.11),
            (("comparison", "bootstrap", "replicates"), 9999),
            (("comparison", "bootstrap", "unit"), "unpaired observations"),
            (("sampling", "short_operations", "measured_pairs_per_session_min"), 29),
        ]
        for path, new_value in mutations:
            with self.subTest(path=path):
                policy = copy.deepcopy(self.policy)
                target = policy
                for key in path[:-1]:
                    target = target[key]
                target[path[-1]] = new_value

                errors = validate_frozen_policy(policy)
                self.assertTrue(any(".".join(path) in error for error in errors))
                with self.assertRaises(ContractError):
                    preflight_result(policy, self.workloads)

    def test_paired_bootstrap_resamples_multiple_aligned_observations_per_session(self) -> None:
        old_values = [[10, 20, 30], [100, 200, 300], [5, 10, 15]]
        aligned_new_values = [[100, 20, 1], [300, 200, 100], [15, 10, 5]]
        rotated_new_values = [[20, 1, 100], [200, 100, 300], [10, 5, 15]]

        def rows(values: list[list[int]]) -> list[dict]:
            return [
                {
                    "session_id": f"s{session_index}",
                    "runs": [{"value": value, "count": 1} for value in session_values],
                }
                for session_index, session_values in enumerate(values, start=1)
            ]

        common = {
            "policy_schema_id": "synthetic-policy-v1",
            "case_id": "paired-multiple-observations-v1",
            "metric_id": "latency-p50",
            "statistic": "latency",
            "percentile": 50,
            "replicates": 512,
            "interval_percentiles": (2.5, 97.5),
        }
        aligned_interval = bootstrap_ratio_interval(
            paired_sessions(rows(old_values), rows(aligned_new_values)),
            **common,
        )
        rotated_interval = bootstrap_ratio_interval(
            paired_sessions(rows(old_values), rows(rotated_new_values)),
            **common,
        )

        self.assertAlmostEqual(aligned_interval[0], 1 / 3)
        self.assertEqual(aligned_interval[1], 10.0)
        self.assertEqual(rotated_interval[0], 0.5)
        self.assertAlmostEqual(rotated_interval[1], 10 / 3)
        self.assertNotEqual(aligned_interval, rotated_interval)

    def test_population_generator_is_reproducible_and_synthetic(self) -> None:
        seed = self.workloads["population_generator"]["seed"]
        expected = hashlib.sha256((seed + chr(0) + "0").encode("utf-8")).digest()[:16].hex()
        first = generated_population_item(self.workloads, 0)
        second = generated_population_item(self.workloads, 1)
        self.assertEqual(first["memory_id"], expected)
        self.assertEqual(first["body"], "Synthetic memory 000000 cohort-00")
        self.assertEqual(second["body"], "Synthetic memory 000001 cohort-01")
        self.assertEqual(first["vector_bits"], ["3f800000", "00000000", "00000000", "00000000"])
        self.assertTrue(first["body"].startswith("Synthetic"))
        self.assertTrue(self.workloads["scope"]["private_source_corpus"] is False)


if __name__ == "__main__":
    unittest.main()
