"""Independent sampling-sufficiency and memory-evidence checks for G5."""
from __future__ import annotations

import math
from typing import Any, Mapping, Sequence

from .workload import ContractError, MEMORY_METRIC_IDS, validate_frozen_policy

COMPARABILITY = {"COMPARABLE", "INCOMPLETE", "NOT_COMPARABLE", "UNBASELINED"}
SAMPLE_SUFFICIENCY = {"SUFFICIENT", "INCOMPLETE"}
MEMORY_PAIR_FIELDS = ("case_id", "workload_input_sha256", "session_id", "pair_index")


def _memory_pair_key(record: Mapping[str, Any]) -> tuple[Any, ...]:
    return tuple(record.get(field) for field in MEMORY_PAIR_FIELDS)


def _valid_memory_pair_key(record: Mapping[str, Any]) -> bool:
    case_id, input_hash, session_id, pair_index = _memory_pair_key(record)
    return (
        isinstance(case_id, str)
        and case_id.startswith("G5-")
        and isinstance(input_hash, str)
        and len(input_hash) == 64
        and all(char in "0123456789abcdef" for char in input_hash)
        and isinstance(session_id, str)
        and bool(session_id)
        and not isinstance(pair_index, bool)
        and isinstance(pair_index, int)
        and pair_index >= 0
    )


def assess_sample_sufficiency(
    sessions: Sequence[Mapping[str, Any]],
    policy: Mapping[str, Any],
    workload_class: str,
) -> tuple[str, list[str]]:
    """Keep sample counts separate from semantic memory comparability.

    Zero-based consecutive session_index and pair_index values are this runner's
    evidence convention. T01 requires recording arm order but does not prescribe
    zero-based indices.
    """
    policy_errors = validate_frozen_policy(policy)
    if policy_errors:
        raise ContractError("; ".join(policy_errors))
    sampling = policy.get("sampling", {})
    if workload_class == "short_operations":
        requirement = sampling.get("short_operations", {})
        min_pairs = requirement.get("measured_pairs_per_session_min", 0)
        max_pairs = requirement.get("measured_pairs_per_session_cap", 0)
        min_warmups = requirement.get("warmups_per_arm_per_session_min", 0)
    elif workload_class == "costly_open_or_maintenance":
        requirement = sampling.get("costly_open_or_maintenance", {})
        min_pairs = requirement.get("alternating_old_new_pairs_per_session_min", 0)
        max_pairs = requirement.get("alternating_old_new_pairs_per_session_cap", 0)
        min_warmups = requirement.get("warmups_per_arm_per_session_min", 0)
    else:
        raise ContractError("unknown workload class")

    required_sessions = sampling.get("independent_sessions", 3)
    reasons: list[str] = []
    valid_sessions = []
    expected_first_arm: str | None = None
    for row in sessions:
        if isinstance(row, Mapping):
            valid_sessions.append(row)
        else:
            reasons.append("each session record must be an object")
    session_ids = [row.get("session_id") for row in valid_sessions]
    unique_ids = [item for item in session_ids if isinstance(item, str) and item]
    if (
        len(valid_sessions) != required_sessions
        or len(unique_ids) != required_sessions
        or len(set(unique_ids)) != required_sessions
    ):
        reasons.append(f"requires exactly {required_sessions} distinct independent sessions")

    for session_position, row in enumerate(valid_sessions):
        session_id = row.get("session_id", "<unknown>")
        session_index = row.get("session_index")
        if (
            isinstance(session_index, bool)
            or not isinstance(session_index, int)
            or session_index != session_position
        ):
            reasons.append(
                f"{session_id}: session indices must be consecutive from zero in execution order"
            )
        if min_warmups > 0:
            warmups = row.get("warmups_per_arm", {})
            if not isinstance(warmups, Mapping):
                reasons.append(f"{session_id}: warmup counts are missing")
            elif any(
                isinstance(warmups.get(arm), bool)
                or not isinstance(warmups.get(arm), int)
                or warmups[arm] < min_warmups
                for arm in ("reference", "candidate")
            ):
                reasons.append(f"{session_id}: each arm requires at least {min_warmups} warmups")

        pairs = row.get("measured_pairs_per_arm", {})
        if not isinstance(pairs, Mapping):
            reasons.append(f"{session_id}: measured pair counts are missing")
            continue
        old_count, new_count = pairs.get("reference"), pairs.get("candidate")
        if any(isinstance(value, bool) or not isinstance(value, int) for value in (old_count, new_count)):
            reasons.append(f"{session_id}: measured pair counts must be integers")
        elif old_count != new_count:
            reasons.append(f"{session_id}: paired reference/candidate counts differ")
        elif old_count < min_pairs:
            reasons.append(f"{session_id}: fewer than {min_pairs} measured pairs")
        elif old_count > max_pairs:
            reasons.append(f"{session_id}: measured pair cap {max_pairs} exceeded")

        if (
            isinstance(old_count, int)
            and not isinstance(old_count, bool)
            and old_count == new_count
            and 0 <= old_count <= max_pairs
        ):
            pair_orders = row.get("pair_orders")
            if not isinstance(pair_orders, list) or len(pair_orders) != old_count:
                reasons.append(f"{session_id}: one recorded arm order is required for every measured pair")
                continue
            for expected_pair_index, pair_order in enumerate(pair_orders):
                if not isinstance(pair_order, Mapping):
                    reasons.append(f"{session_id}: pair order records must be objects")
                    continue
                pair_index = pair_order.get("pair_index")
                first_arm = pair_order.get("first_arm")
                second_arm = pair_order.get("second_arm")
                if (
                    isinstance(pair_index, bool)
                    or not isinstance(pair_index, int)
                    or pair_index != expected_pair_index
                ):
                    reasons.append(
                        f"{session_id}: arm order pair indices must be consecutive from zero"
                    )
                if first_arm not in {"reference", "candidate"} or second_arm not in {"reference", "candidate"} or first_arm == second_arm:
                    reasons.append(f"{session_id}: each pair must record one reference and one candidate arm")
                    continue
                if expected_first_arm is None:
                    expected_first_arm = first_arm
                elif first_arm != expected_first_arm:
                    reasons.append(
                        f"{session_id}: first arm must alternate across all pairs and session boundaries"
                    )
                expected_first_arm = "candidate" if first_arm == "reference" else "reference"

    return ("SUFFICIENT" if not reasons else "INCOMPLETE", reasons)


def validate_memory_metric_records(
    records: Sequence[Mapping[str, Any]],
    policy: Mapping[str, Any],
) -> list[str]:
    """Validate both distinct metrics, including each arm's collector provenance."""
    policy_errors = validate_frozen_policy(policy)
    if policy_errors:
        return [f"frozen policy mismatch: {error}" for error in policy_errors]
    errors: list[str] = []
    by_id: dict[str, Mapping[str, Any]] = {};
    expected = set(MEMORY_METRIC_IDS)
    for record in records:
        metric_id = record.get("metric_id")
        if metric_id not in expected:
            errors.append(f"unknown memory metric: {metric_id}")
            continue
        if metric_id in by_id:
            errors.append(f"duplicate memory metric: {metric_id}")
        by_id[metric_id] = record
    if set(by_id) != expected:
        errors.append("both peak_private_bytes and peak_working_set_bytes must be recorded independently")

    required = policy.get("evidence", {}).get("memory_metric_record", {}).get("required_fields", [])
    pair_keys: dict[str, tuple[Any, ...]] = {}
    for metric_id in expected.intersection(by_id):
        record = by_id[metric_id]
        missing = [field for field in required if field not in record]
        if missing:
            errors.append(f"{metric_id}: missing fields {', '.join(missing)}")
        case_id = record.get("case_id")
        input_hash = record.get("workload_input_sha256")
        session_id = record.get("session_id")
        pair_index = record.get("pair_index")
        pair_keys[metric_id] = _memory_pair_key(record)
        if not isinstance(case_id, str) or not case_id.startswith("G5-"):
            errors.append(f"{metric_id}: case_id is required")
        if (
            not isinstance(input_hash, str)
            or len(input_hash) != 64
            or any(char not in "0123456789abcdef" for char in input_hash)
        ):
            errors.append(f"{metric_id}: workload_input_sha256 must be a lowercase SHA-256")
        if not isinstance(session_id, str) or not session_id:
            errors.append(f"{metric_id}: session_id is required")
        if isinstance(pair_index, bool) or not isinstance(pair_index, int) or pair_index < 0:
            errors.append(f"{metric_id}: pair_index must be a non-negative integer")

        comparability = record.get("comparability_status")
        sufficiency = record.get("sample_sufficiency_status")
        if comparability not in COMPARABILITY:
            errors.append(f"{metric_id}: invalid comparability status")
        if sufficiency not in SAMPLE_SUFFICIENCY:
            errors.append(f"{metric_id}: invalid sample-sufficiency status")
        if record.get("units") != "bytes":
            errors.append(f"{metric_id}: memory units must be bytes")
        if not isinstance(record.get("subject_process_scope"), str) or not record.get("subject_process_scope").strip():
            errors.append(f"{metric_id}: subject process scope is required")
        if not record.get("sample_window"):
            errors.append(f"{metric_id}: exact sample window is required")

        reason = record.get("unavailable_reason")
        if comparability == "COMPARABLE":
            if reason is not None:
                errors.append(f"{metric_id}: COMPARABLE evidence must not carry an unavailable reason")
            for arm_name in ("reference_arm", "candidate_arm"):
                arm = record.get(arm_name)
                if not isinstance(arm, Mapping):
                    errors.append(f"{metric_id}: {arm_name} collector evidence is required")
                    continue
                arm_required = (
                    "collector_name",
                    "collector_version",
                    "collector_api_or_source",
                    "collector_semantics",
                    "raw_samples",
                )
                if any(not arm.get(key) for key in arm_required):
                    errors.append(f"{metric_id}: {arm_name} requires collector provenance and raw samples")
                samples = arm.get("raw_samples")
                if not isinstance(samples, list) or not samples:
                    errors.append(f"{metric_id}: {arm_name} raw samples are required")
                elif any(
                    isinstance(value, bool)
                    or not isinstance(value, (int, float))
                    or not math.isfinite(float(value))
                    or float(value) <= 0
                    for value in samples
                ):
                    errors.append(f"{metric_id}: {arm_name} samples must be finite positive byte counts")
            ref_arm, new_arm = record.get("reference_arm"), record.get("candidate_arm")
            if isinstance(ref_arm, Mapping) and isinstance(new_arm, Mapping):
                old_samples, new_samples = ref_arm.get("raw_samples"), new_arm.get("raw_samples")
                if isinstance(old_samples, list) and isinstance(new_samples, list) and len(old_samples) != len(new_samples):
                    errors.append(f"{metric_id}: paired memory sample counts differ")
                old_semantics = ref_arm.get("collector_semantics")
                new_semantics = new_arm.get("collector_semantics")
                if old_semantics and new_semantics and old_semantics != new_semantics:
                    errors.append(f"{metric_id}: collector semantics are not documented as equivalent")
        elif not isinstance(reason, str) or not reason.strip():
            errors.append(f"{metric_id}: unavailable_reason is required for {comparability}")

    if len(pair_keys) == len(expected):
        first_key = next(iter(pair_keys.values()))
        if any(key != first_key for key in pair_keys.values()):
            errors.append(
                "peak_private_bytes and peak_working_set_bytes must share an identical "
                "case/input/session/pair key"
            )
    return errors


def classify_memory_gate(records: Sequence[Mapping[str, Any]]) -> str:
    """Fail if either sufficient comparable metric fails; pass requires both."""
    by_id = {record.get("metric_id"): record for record in records}
    if set(by_id) != set(MEMORY_METRIC_IDS) or len(records) != 2:
        return "INCOMPLETE"
    if any(not _valid_memory_pair_key(by_id[metric_id]) for metric_id in MEMORY_METRIC_IDS):
        return "INCOMPLETE"
    keys = [_memory_pair_key(by_id[metric_id]) for metric_id in MEMORY_METRIC_IDS]
    if keys[0] != keys[1]:
        return "INCOMPLETE"
    for metric_id in MEMORY_METRIC_IDS:
        record = by_id[metric_id]
        if (
            record.get("comparability_status") == "COMPARABLE"
            and record.get("sample_sufficiency_status") == "SUFFICIENT"
            and record.get("decision") == "FAIL"
        ):
            return "FAIL"
    if all(
        by_id[metric_id].get("comparability_status") == "COMPARABLE"
        and by_id[metric_id].get("sample_sufficiency_status") == "SUFFICIENT"
        and by_id[metric_id].get("decision") == "PASS"
        for metric_id in MEMORY_METRIC_IDS
    ):
        return "PASS"
    if all(
        by_id[metric_id].get("comparability_status") == "COMPARABLE"
        and by_id[metric_id].get("sample_sufficiency_status") == "SUFFICIENT"
        for metric_id in MEMORY_METRIC_IDS
    ):
        return "INCONCLUSIVE"
    return "INCOMPLETE"
