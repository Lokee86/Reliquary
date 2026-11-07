"""Synthetic-only G5 runner preflight and statistical conformance tests."""
from __future__ import annotations

import math
from pathlib import Path
from typing import Any, Mapping

from .evidence import (
    assess_sample_sufficiency,
    classify_memory_gate,
    validate_memory_metric_records,
)
from .statistics import (
    SplitMix64,
    bootstrap_ratio_interval,
    classify_interval,
    paired_sessions,
)
from .workload import (
    ContractError,
    MEMORY_METRIC_IDS,
    REQUIRED_HISTORICAL_BLOCKERS,
    generated_population_item,
    iter_generated_population,
    read_json,
    sha256_file,
    validate_frozen_policy,
    validate_g5_contract,
)

ROOT = Path(__file__).resolve().parents[2]
DEFAULT_WORKLOADS = ROOT / "benchmarks" / "g5" / "workloads-v1.json"
DEFAULT_POLICY = ROOT / "benchmarks" / "g5" / "policy-v1.json"
DEFAULT_SELFTESTS = ROOT / "benchmarks" / "g5" / "statistics-selftests-v1.json"


def verify_statistics_selftests(
    policy: Mapping[str, Any],
    selftests: Mapping[str, Any],
) -> list[str]:
    """Run the frozen T01 statistical vectors; these are tests, not measurements."""
    errors = validate_frozen_policy(policy)
    if errors:
        return errors
    expected_words = ["e220a8397b1dcdaf", "6e789e6aa1b965f4", "06c45d188009454f"]
    rng = SplitMix64(0)
    actual_words = [f"{rng.next_u64():016x}" for _ in range(3)]
    if actual_words != expected_words:
        errors.append("SplitMix64-v1 seed-zero self-test failed")

    config = policy.get("comparison", {}).get("bootstrap", {})
    if selftests.get("schema_id") != "reliquary-g5-statistics-selftests-v1":
        errors.append("unexpected statistical self-test schema")
    if selftests.get("vectors_version") != 1:
        errors.append("statistical self-test vector version must be 1")
    if not str(config.get("index_rng", "")).startswith("SplitMix64-v1"):
        errors.append("policy bootstrap RNG differs from SplitMix64-v1")
    schema_id = policy.get("schema_id", "")
    replicates = selftests.get("bootstrap_replicates", config.get("replicates", 0))
    if replicates != config.get("replicates"):
        errors.append("statistical self-test replicate count differs from frozen policy")
    interval_percentiles = tuple(config.get("interval_percentiles", [2.5, 97.5]))

    for case in selftests.get("cases", []):
        case_id = case.get("id", "<unknown>")
        try:
            paired = paired_sessions(case.get("reference", []), case.get("candidate", []))
            metric_id = case.get("metric", "")
            if not isinstance(metric_id, str):
                errors.append(f"{case_id}: metric must be a string")
                continue
            if metric_id.startswith("throughput-"):
                statistic = "throughput"
            elif metric_id in MEMORY_METRIC_IDS or metric_id.startswith("memory-"):
                statistic = "memory"
            elif metric_id.startswith("latency-"):
                statistic = "latency"
            else:
                errors.append(f"{case_id}: unsupported self-test metric {metric_id}")
                continue
            interval = bootstrap_ratio_interval(
                paired,
                policy_schema_id=schema_id,
                case_id=case_id,
                metric_id=case.get("metric", ""),
                statistic=statistic,
                percentile=float(case.get("quantile", 50)),
                replicates=int(replicates),
                interval_percentiles=(float(interval_percentiles[0]), float(interval_percentiles[1])),
            )
            expected = case.get("expected_interval")
            if expected is not None and len(expected) != 2:
                errors.append(f"{case_id}: frozen confidence interval must contain two endpoints")
                continue
            if expected is not None and any(
                not math.isclose(actual, float(target), rel_tol=1e-12, abs_tol=1e-12)
                for actual, target in zip(interval, expected)
            ):
                errors.append(f"{case_id}: interval {interval} differs from frozen {expected}")
            limit = float(case.get("limit"))
            decision = classify_interval(interval, case.get("direction"), limit)
            if decision != case.get("expected_decision"):
                errors.append(f"{case_id}: expected {case.get('expected_decision')}, got {decision} ({interval})")
            if case.get("straddle") is True and not (interval[0] <= limit <= interval[1]):
                errors.append(f"{case_id}: expected interval to straddle {limit}, got {interval}")
        except (TypeError, ValueError, IndexError) as exc:
            errors.append(f"{case_id}: {exc}")
    if not selftests.get("cases"):
        errors.append("no frozen statistical self-test cases were found")
    return errors


G5_09_BLOCKERS = ("G5-09_NATIVE_REHEARSAL", "G5-09_ABSOLUTE_LIMIT_REVIEW")


def preflight_result(policy: Mapping[str, Any], workloads: Mapping[str, Any]) -> dict[str, Any]:
    """Return per-case no-measurement gates without opening binaries or cohort data."""
    validate_g5_contract(policy, workloads)
    historical_reason = "historical source admission and independent parity have not authorized comparative measurements"
    g5_09_reason = (
        "G5-09 requires its own native rehearsal and a reviewed case-specific absolute limit; "
        "it remains UNBASELINED and is not part of the G5-T03 historical cohort comparison."
    )
    case_results = []
    for case_id in workloads["case_ids"]:
        if case_id == "G5-09":
            case_results.append(
                {
                    "case_id": case_id,
                    "status": "UNBASELINED",
                    "measurement_results": False,
                    "blockers": list(G5_09_BLOCKERS),
                    "reason": g5_09_reason,
                }
            )
        else:
            case_results.append(
                {
                    "case_id": case_id,
                    "status": "INCOMPLETE",
                    "measurement_results": False,
                    "blockers": list(REQUIRED_HISTORICAL_BLOCKERS),
                    "reason": historical_reason,
                }
            )
    blockers = list(
        dict.fromkeys(
            blocker
            for item in case_results
            for blocker in item["blockers"]
        )
    )
    return {
        "schema_id": "reliquary-g5-runner-preflight-v1",
        "measurement_status": "INCOMPLETE",
        "measurement_results": False,
        "comparison_performed": False,
        "private_inputs_read": False,
        "historical_cohort_accessed": False,
        "cohort_manifest_opened": False,
        "old_binary_opened": False,
        "new_binary_opened": False,
        "adapter_launch_allowed": False,
        "adapter_processes_started": False,
        "output_created": False,
        "blockers": blockers,
        "per_case_results": case_results,
        "reason": (
            "No measurements were taken. Historical G5-T03 cases remain gated by F03-V04 and D02-T04; "
            "G5-09 remains separately UNBASELINED pending its native rehearsal and absolute-limit review."
        ),
    }


def run_synthetic_selftests(
    policy_path: str | Path = DEFAULT_POLICY,
    workloads_path: str | Path = DEFAULT_WORKLOADS,
    selftests_path: str | Path = DEFAULT_SELFTESTS,
) -> dict[str, Any]:
    policy = read_json(policy_path)
    workloads = read_json(workloads_path)
    selftests = read_json(selftests_path)
    validate_g5_contract(policy, workloads)
    stat_errors = verify_statistics_selftests(policy, selftests)
    if stat_errors:
        raise ContractError("; ".join(stat_errors))
    return {
        "schema_id": "reliquary-g5-runner-selftest-v1",
        "status": "PASS",
        "synthetic_only": True,
        "measurement_results": False,
        "comparison_performed": False,
        "private_inputs_read": False,
        "provider_calls": False,
        "hermes": False,
        "memory_metric_ids": list(MEMORY_METRIC_IDS),
        "policy_sha256": sha256_file(policy_path),
        "workload_manifest_sha256": sha256_file(workloads_path),
        "statistical_vectors_checked": len(selftests["cases"]),
    }
