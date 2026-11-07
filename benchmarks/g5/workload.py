"""Frozen G5 policy/workload validation and deterministic synthetic inputs."""
from __future__ import annotations

import hashlib
import json
import math
import struct
from pathlib import Path
from typing import Any, Mapping

MEMORY_METRIC_IDS = ("peak_private_bytes", "peak_working_set_bytes")

FROZEN_POLICY_EXPECTATIONS = (
    (("schema_id",), "reliquary-g5-measurement-policy-v1"),
    (("policy_version",), 1),
    (("status",), "frozen-before-measurement"),
    (("authority", "ticket"), "G5-T01"),
    (("authority", "spec_commit"), "b6332a8ff5396a7725c73be7643c32950e2de7e"),
    (("authority", "spec_path_at_spec_commit"), "docs/architecture/g5-performance-release-acceptance-spec-2026-11-06.md"),
    (("authority", "pg_c01_reviewed_commit"), "0f824a09"),
    (("thresholds", "unexplained_correctness_privacy_history_recovery_differences_max"), 0),
    (("thresholds", "median_latency_candidate_over_reference_max"), 1.10),
    (("thresholds", "p95_latency_candidate_over_reference_max"), 1.25),
    (("thresholds", "sustained_throughput_candidate_over_reference_min"), 0.90),
    (("thresholds", "peak_memory_candidate_over_reference_max"), 1.15),
    (("thresholds", "peak_memory_metrics"), ["peak_private_bytes", "peak_working_set_bytes"]),
    (("thresholds", "report_both_memory_metrics_when_available"), True),
    (("thresholds", "p99"), "diagnostic_only"),
    (("thresholds", "structural_guards"), [
        "No new whole-file copy/materialization on ordinary bounded retrieval or per-turn operation.",
        "No provider call under an owner mutation lock.",
        "No unbounded queue or stale-owner retry loop.",
    ]),
    (("thresholds", "tighter_baseline_rule"), "Existing tighter, directly applicable accepted baselines take precedence; record the accepted baseline and scope."),
    (("comparison", "pair_key"), ["case_id", "workload_input_sha256", "session_id", "pair_index"]),
    (("comparison", "pair_definition"), "One old and one new sample for the same case, frozen input, machine profile, and session; record arm order and alternate which arm runs first."),
    (("comparison", "quantile_method", "median"), "Conventional median; average the two middle observations when the count is even."),
    (("comparison", "quantile_method", "p95_and_ci_endpoints"), "nearest-rank-v1: rank = ceil(percentile * sample_count / 100), one-based."),
    (("comparison", "bootstrap", "algorithm"), "session-stratified paired percentile bootstrap v1"),
    (("comparison", "bootstrap", "unit"), "Aligned old/new sample pairs; resample pair indexes with replacement within each of three sessions."),
    (("comparison", "bootstrap", "confidence_level_percent"), 95),
    (("comparison", "bootstrap", "interval_percentiles"), [2.5, 97.5]),
    (("comparison", "bootstrap", "replicates"), 10000),
    (("comparison", "bootstrap", "index_rng"), "SplitMix64-v1 with rejection sampling for unbiased bounded indexes."),
    (("comparison", "bootstrap", "seed"), "First 64 bits big-endian of SHA-256(UTF-8(policy_schema_id + NUL + case_id + NUL + metric_id + NUL + session_id)); separate stream per session."),
    (("comparison", "bootstrap", "latency"), "Compute p50 and p95 ratio per resample."),
    (("comparison", "bootstrap", "throughput"), "Compute p50 candidate/reference ratio per resample."),
    (("comparison", "bootstrap", "memory"), "Compute p50 ratio of per-sample process peak bytes; also report every sample and observed maximum."),
    (("comparison", "rules", "maximum_ratio_pass"), "upper 95% interval bound <= limit"),
    (("comparison", "rules", "maximum_ratio_fail"), "lower 95% interval bound > limit"),
    (("comparison", "rules", "minimum_ratio_pass"), "lower 95% interval bound >= limit"),
    (("comparison", "rules", "minimum_ratio_fail"), "upper 95% interval bound < limit"),
    (("comparison", "rules", "otherwise"), "INCONCLUSIVE"),
    (("comparison", "rules", "incomplete_evidence"), "INCOMPLETE, not a pass."),
    (("comparison", "rules", "unsupported_old_capability"), "NOT_COMPARABLE; never treat as zero cost."),
    (("comparison", "rules", "no_trustworthy_reference"), "UNBASELINED; freeze a case-specific absolute limit and rationale before release measurement and keep the G5 item open until reviewed."),
    (("comparison", "rules", "incomparable_build_or_inputs"), "INCONCLUSIVE; rerun under comparable host and input conditions without changing limits."),
    (("comparison", "rules", "measurement_window_violation"), "INCONCLUSIVE; do not accept a run with overlapping work or forbidden machine changes."),
    (("comparison", "rules", "correctness"), "Any unexplained semantic, privacy, history, or recovery difference is FAIL regardless of speed."),
    (("comparison", "near_zero_reference"), "Batch a repeatable operation until the total timed interval exceeds measured monotonic-clock resolution; record batch size and resolution. No ratio exemption."),
    (("sampling", "independent_sessions"), 3),
    (("sampling", "short_operations", "warmups_per_arm_per_session_min"), 5),
    (("sampling", "short_operations", "measured_pairs_per_session_min"), 30),
    (("sampling", "short_operations", "measured_pairs_per_session_cap"), 90),
    (("sampling", "short_operations", "interpretation"), "One pair contains one old and one new sample; this conservative per-session reading meets or exceeds the stated minimum."),
    (("sampling", "costly_open_or_maintenance", "alternating_old_new_pairs_per_session_min"), 10),
    (("sampling", "costly_open_or_maintenance", "alternating_old_new_pairs_per_session_cap"), 30),
    (("sampling", "costly_open_or_maintenance", "interpretation"), "One pair contains one old and one new sample; alternate which arm runs first across pairs and sessions."),
    (("sampling", "cap_rule"), "If the 95% interval still straddles the threshold at the explicit cap, report INCONCLUSIVE; do not extend the cap or change a threshold after results."),
    (("sampling", "outliers"), "Retain and report every sample; never silently remove or replace an outlier."),
    (("sampling", "session_rule"), "Preserve raw samples and session IDs; never pool away session boundaries."),
    (("machine_safety", "exclusive_measurement_window"), "Required for release measurements, not merely a separate worktree."),
    (("machine_safety", "no_overlap"), ["CPU-heavy build/test", "disk scan", "model inference", "another benchmark"]),
    (("machine_safety", "prohibited"), ["flush OS caches", "disable security tools", "alter global performance settings", "kill unrelated workloads"]),
    (("machine_safety", "cold_start"), "Fresh process only; no cold-OS-cache claim."),
    (("machine_safety", "build"), "Release mode with locked dependencies; record compiler, optimization/features, source and binary hashes."),
    (("machine_safety", "storage"), "Check free space before release compilation; reuse an isolated cache per incompatible runtime; serialize Cargo; do not create per-ticket target directories or delete another lane's cache."),
    (("machine_safety", "mutation"), "Run mutations on copies and recheck source/candidate custody hashes afterward."),
    (("evidence", "per_run"), ["policy and workload hashes", "case/input/session/pair/order", "machine profile", "source/binary/toolchain/features", "commands/args/exit/duration/log hashes", "raw samples and units", "correctness witness", "bootstrap seed/count/interval/classification", "resource maxima", "custody recheck"]),
    (("evidence", "privacy"), "Report hashes, IDs, and counts; never conversation text, credentials, or raw private payloads."),
    (("evidence", "runner_interface"), "benchmark --workloads <manifest> --old <binary> --new <binary> --cohort <approved-manifest> --output <directory>"),
    (("evidence", "memory_metric_record", "required_fields"), ["metric_id", "comparability_status", "sample_sufficiency_status", "units", "subject_process_scope", "sample_window", "reference_arm", "candidate_arm", "unavailable_reason"]),
    (("evidence", "memory_metric_record", "arm_fields"), ["collector_name", "collector_version", "collector_api_or_source", "collector_semantics", "raw_samples"]),
    (("evidence", "memory_metric_record", "comparability_status_values"), ["COMPARABLE", "INCOMPLETE", "NOT_COMPARABLE", "UNBASELINED"]),
    (("evidence", "memory_metric_record", "sample_sufficiency_status_values"), ["SUFFICIENT", "INCOMPLETE"]),
    (("machine_profile_schema",), "machine-profile-v1.schema.json"),
    (("statistics_selftests",), "statistics-selftests-v1.json"),
    (("memory_measurement_contract", "applies_to_workload_ids"), ["G5-01", "G5-02", "G5-04", "G5-07", "G5-08"]),
    (("memory_measurement_contract", "comparison", "independent_metrics"), True),
    (("memory_measurement_contract", "comparison", "threshold_key"), "thresholds.peak_memory_candidate_over_reference_max"),
    (("memory_measurement_contract", "comparison", "statistic"), "For each metric independently, compute the paired-bootstrap ratio of p50 per-sample process peak bytes; retain every raw sample and report the observed maximum."),
    (("memory_measurement_contract", "comparison", "both_metrics_required_for_memory_gate_pass"), True),
    (("memory_measurement_contract", "comparison", "never_substitute_one_metric_for_the_other"), True),
)

class ContractError(ValueError):
    """A frozen G5 policy or workload contract is malformed."""


def read_json(path: str | Path) -> dict[str, Any]:
    value = json.loads(Path(path).read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise ContractError(f"{path}: expected a JSON object")
    return value


def sha256_file(path: str | Path) -> str:
    digest = hashlib.sha256()
    with Path(path).open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _require(condition: bool, message: str) -> None:
    if not condition:
        raise ContractError(message)


def _f32_is_finite(bit_string: str) -> bool:
    if len(bit_string) != 8 or any(c not in "0123456789abcdef" for c in bit_string):
        return False
    return math.isfinite(struct.unpack("<f", bytes.fromhex(bit_string))[0])




_MISSING_POLICY_VALUE = object()


def _value_at_path(policy: Mapping[str, Any], path: tuple[str, ...]) -> Any:
    value: Any = policy
    for key in path:
        if not isinstance(value, Mapping) or key not in value:
            return _MISSING_POLICY_VALUE
        value = value[key]
    return value


def _same_json_value(actual: Any, expected: Any) -> bool:
    if type(actual) is not type(expected):
        return False
    if isinstance(expected, list):
        return len(actual) == len(expected) and all(
            _same_json_value(actual_value, expected_value)
            for actual_value, expected_value in zip(actual, expected)
        )
    return actual == expected


def validate_frozen_policy(policy: Mapping[str, Any]) -> list[str]:
    """Pin T01's released policy version and every runner-relevant limit/contract."""
    errors: list[str] = []
    for path, expected in FROZEN_POLICY_EXPECTATIONS:
        actual = _value_at_path(policy, path)
        if actual is _MISSING_POLICY_VALUE or not _same_json_value(actual, expected):
            errors.append(f"frozen policy mismatch at {'.'.join(path)}")
    return errors

def validate_g5_contract(policy: Mapping[str, Any], workloads: Mapping[str, Any]) -> None:
    """Check T01's frozen metrics and synthetic workload semantics."""
    policy_errors = validate_frozen_policy(policy)
    _require(not policy_errors, "; ".join(policy_errors))
    _require(policy.get("schema_id") == "reliquary-g5-measurement-policy-v1", "unexpected policy schema")
    _require(policy.get("status") == "frozen-before-measurement", "measurement policy is not frozen")
    _require(policy.get("authority", {}).get("ticket") == "G5-T01", "policy authority must be G5-T01")

    memory = policy.get("memory_measurement_contract", {})
    metric_map = memory.get("metrics", {})
    _require(tuple(sorted(metric_map)) == tuple(sorted(MEMORY_METRIC_IDS)), "both independent memory metrics are required")
    _require(memory.get("comparison", {}).get("independent_metrics") is True, "memory metrics must remain independent")
    _require(
        memory.get("comparison", {}).get("never_substitute_one_metric_for_the_other") is True,
        "memory metric substitution must be forbidden",
    )
    _require(
        policy.get("thresholds", {}).get("peak_memory_metrics") == list(MEMORY_METRIC_IDS),
        "policy threshold list must name both memory metrics",
    )
    metric_record = policy.get("evidence", {}).get("memory_metric_record", {})
    _require(
        metric_record.get("comparability_status_values") == ["COMPARABLE", "INCOMPLETE", "NOT_COMPARABLE", "UNBASELINED"],
        "memory comparability statuses changed without a policy version",
    )
    _require(
        metric_record.get("sample_sufficiency_status_values") == ["SUFFICIENT", "INCOMPLETE"],
        "memory sample-sufficiency statuses changed without a policy version",
    )
    _require(
        "sample_sufficiency_status" in metric_record.get("required_fields", []),
        "sample sufficiency must be recorded separately",
    )

    _require(workloads.get("schema_id") == "reliquary-g5-workloads-v1", "unexpected workload schema")
    _require(workloads.get("measurement_status") == "NOT_STARTED", "workload manifest must remain unmeasured in this slice")
    scope = workloads.get("scope", {})
    _require(scope.get("live_provider_calls") is False, "live provider calls are outside this runner")
    _require(scope.get("private_source_corpus") is False, "private corpus is outside this runner")
    _require(scope.get("measurement_results") is False, "workload manifest must not claim results")
    _require(scope.get("hermes") is False, "Hermes work is outside this runner")
    _require(workloads.get("case_ids") == [f"G5-{number:02d}" for number in range(1, 10)], "workload case IDs differ")

    cohort = workloads.get("synthetic_g4_cohort", {})
    _require(cohort.get("fresh_disposable_cohort_per_run") is True, "synthetic owner cohort must be disposable per run")
    _require(cohort.get("owner_binaries_committed") is False, "synthetic owner binaries must not be committed")
    shape = cohort.get("minimum_shape", {})
    _require(shape.get("conversations") == 12 and shape.get("accepted_turns") == 40, "synthetic cohort minimum shape changed")
    _require(shape.get("branch_forks") == 3 and shape.get("episodes_per_rel") == 1, "synthetic cohort semantic shape changed")
    source_scenarios = cohort.get("source_scenarios", [])
    _require(len(source_scenarios) >= 6, "synthetic source scenarios are incomplete")
    scenario_by_id = {item.get("id"): item for item in source_scenarios}
    _require(len(scenario_by_id) == len(source_scenarios), "synthetic source scenario IDs must be unique")

    probes = workloads.get("synthetic_probe_data", {})
    profile = probes.get("vector_profile", {})
    dimensions = profile.get("dimensions")
    _require(isinstance(dimensions, int) and not isinstance(dimensions, bool) and dimensions > 0, "synthetic vector dimensions are invalid")
    _require(profile.get("provider_calls") is False, "synthetic probes must not call a provider")
    records = probes.get("records", [])
    aliases = [record.get("alias") for record in records]
    _require(len(aliases) == len(set(aliases)) and all(isinstance(alias, str) for alias in aliases), "synthetic record aliases must be unique")
    owners = {owner.get("alias") for owner in cohort.get("owners", [])}
    for record in records:
        _require(record.get("owner") in owners, f"{record.get('alias')}: unknown synthetic owner")
        scenario = scenario_by_id.get(record.get("source"))
        _require(scenario is not None, f"{record.get('alias')}: unknown synthetic source scenario")
        expected_owner = scenario.get("memory_route") or scenario.get("owner")
        _require(record.get("owner") == expected_owner, f"{record.get('alias')}: source is routed to the wrong owner")
        vectors = record.get("vector_bits")
        _require(
            isinstance(vectors, list)
            and len(vectors) == dimensions
            and all(isinstance(value, str) and _f32_is_finite(value) for value in vectors),
            f"{record.get('alias')}: invalid synthetic f32 vector",
        )

    alias_set = set(aliases)
    query_orders = {}
    for query in probes.get("queries", []):
        expected = query.get("expected_alias_order")
        _require(isinstance(expected, list) and all(alias in alias_set for alias in expected), "query expectation references an unknown alias")
        query_orders[query.get("id")] = expected
    required_orders = {
        "q-lantern-lexical": ["memory-lantern-release"],
        "q-afternoons-vector": ["memory-afternoons"],
        "q-lantern-hybrid": ["memory-lantern-release"],
        "q-mira-agentic": ["memory-mira-mention"],
    }
    for query_id, expected in required_orders.items():
        _require(query_orders.get(query_id) == expected, f"{query_id}: frozen synthetic result order changed")
    freshness = next((item for item in probes.get("model_fixtures", []) if item.get("id") == "freshness-settlement-v1"), {})
    _require(freshness.get("expected_accepted_use_receipts") == 1, "synthetic freshness fixture must require one accepted-use receipt")
    _require(freshness.get("read_only_retrieval_grants_no_credit") is True, "read-only retrieval must not grant freshness credit")

    historical = workloads.get("historical_input_policy", {})
    _require(historical.get("measurement_ticket") == "G5-T03", "historical cohort gate must remain assigned to G5-T03")
    _require("F03-V04" in historical.get("hard_predecessors", []), "F03 source admission must gate authentic comparisons")
    _require("D02-T04" in historical.get("hard_predecessors", []), "D02 parity acceptance must gate authentic comparisons")


def generated_population_item(workloads: Mapping[str, Any], index: int) -> dict[str, Any]:
    """Return one deterministic synthetic population row without writing a corpus."""
    if isinstance(index, bool) or not isinstance(index, int) or index < 0:
        raise ValueError("population index must be a non-negative integer")
    generator = workloads.get("population_generator", {})
    seed = generator.get("seed")
    if not isinstance(seed, str) or not seed:
        raise ContractError("population generator seed is missing")
    profile_vectors = []
    for record in workloads.get("synthetic_probe_data", {}).get("records", []):
        vector = record.get("vector_bits")
        if isinstance(vector, list) and len(vector) == 4 and vector.count("3f800000") == 1 and vector.count("00000000") == 3:
            if vector not in profile_vectors:
                profile_vectors.append(vector)
    if len(profile_vectors) != 4:
        raise ContractError("four frozen one-hot vectors are required")
    memory_id = hashlib.sha256((seed + chr(0) + str(index)).encode("utf-8")).digest()[:16].hex()
    return {
        "memory_id": memory_id,
        "body": f"Synthetic memory {index:06d} cohort-{index % 16:02d}",
        "vector_bits": list(profile_vectors[index % 4]),
    }


def iter_generated_population(workloads: Mapping[str, Any], size: int):
    allowed = workloads.get("population_generator", {}).get("sizes", [])
    if isinstance(size, bool) or size not in allowed:
        raise ValueError(f"population size must be one of {allowed}")
    for index in range(size):
        yield generated_population_item(workloads, index)
