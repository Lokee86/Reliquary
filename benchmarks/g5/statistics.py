"""Deterministic, policy-versioned statistics for synthetic G5 runner checks.

This module never reads product data or starts a benchmark process.
"""
from __future__ import annotations

import hashlib
import math
from dataclasses import dataclass
from typing import Mapping, Sequence

MASK64 = (1 << 64) - 1
SPLITMIX_GAMMA = 0x9E3779B97F4A7C15
SPLITMIX_MUL1 = 0xBF58476D1CE4E5B9
SPLITMIX_MUL2 = 0x94D049BB133111EB


@dataclass
class SplitMix64:
    state: int

    def next_u64(self) -> int:
        self.state = (self.state + SPLITMIX_GAMMA) & MASK64
        value = self.state
        value = ((value ^ (value >> 30)) * SPLITMIX_MUL1) & MASK64
        value = ((value ^ (value >> 27)) * SPLITMIX_MUL2) & MASK64
        return (value ^ (value >> 31)) & MASK64

    def bounded(self, bound: int) -> int:
        """Return an unbiased integer in [0, bound) using rejection sampling."""
        if bound <= 0 or bound > (1 << 64):
            raise ValueError("bound must be in 1..=2^64")
        if bound == (1 << 64):
            return self.next_u64()
        threshold = (1 << 64) % bound
        while True:
            value = self.next_u64()
            if value >= threshold:
                return value % bound


def session_seed(policy_schema_id: str, case_id: str, metric_id: str, session_id: str) -> int:
    material = "\0".join((policy_schema_id, case_id, metric_id, session_id)).encode("utf-8")
    return int.from_bytes(hashlib.sha256(material).digest()[:8], "big")


def median(values: Sequence[float]) -> float:
    if not values:
        raise ValueError("median requires at least one value")
    ordered = sorted(values)
    middle = len(ordered) // 2
    if len(ordered) % 2:
        return float(ordered[middle])
    return float((ordered[middle - 1] + ordered[middle]) / 2)


def nearest_rank(values: Sequence[float], percentile: float) -> float:
    """Nearest-rank percentile with a one-based ceil(percentile * n / 100) rank."""
    if not values or not 0 < percentile <= 100:
        raise ValueError("nearest_rank requires values and percentile in (0, 100]")
    ordered = sorted(values)
    rank = math.ceil(percentile * len(ordered) / 100)
    return float(ordered[rank - 1])


def expand_runs(session_rows: Sequence[Mapping[str, object]]) -> dict[str, list[float]]:
    expanded: dict[str, list[float]] = {}
    for row in session_rows:
        session_id = row.get("session_id")
        runs = row.get("runs")
        if not isinstance(session_id, str) or not session_id or session_id in expanded:
            raise ValueError("session IDs must be unique non-empty strings")
        if not isinstance(runs, list) or not runs:
            raise ValueError(f"{session_id}: runs must be a non-empty list")
        values: list[float] = []
        for run in runs:
            if not isinstance(run, dict):
                raise ValueError(f"{session_id}: each run must be an object")
            value = run.get("value")
            count = run.get("count")
            if isinstance(value, bool) or not isinstance(value, (int, float)):
                raise ValueError(f"{session_id}: run value must be numeric")
            if not math.isfinite(float(value)) or float(value) <= 0:
                raise ValueError(f"{session_id}: run value must be finite and positive")
            if isinstance(count, bool) or not isinstance(count, int) or count <= 0:
                raise ValueError(f"{session_id}: run count must be a positive integer")
            values.extend([float(value)] * count)
        expanded[session_id] = values
    return expanded


def paired_sessions(
    reference_rows: Sequence[Mapping[str, object]],
    candidate_rows: Sequence[Mapping[str, object]],
) -> dict[str, tuple[list[float], list[float]]]:
    reference = expand_runs(reference_rows)
    candidate = expand_runs(candidate_rows)
    if reference.keys() != candidate.keys():
        raise ValueError("reference and candidate session IDs must match exactly")
    paired: dict[str, tuple[list[float], list[float]]] = {}
    for session_id in reference:
        old = reference[session_id]
        new = candidate[session_id]
        if len(old) != len(new):
            raise ValueError(f"{session_id}: reference and candidate pair counts differ")
        paired[session_id] = (old, new)
    return paired


def bootstrap_ratio_interval(
    paired: Mapping[str, tuple[Sequence[float], Sequence[float]]],
    *,
    policy_schema_id: str,
    case_id: str,
    metric_id: str,
    statistic: str,
    percentile: float = 50,
    replicates: int = 10000,
    interval_percentiles: tuple[float, float] = (2.5, 97.5),
) -> tuple[float, float]:
    """Session-stratified paired percentile bootstrap of a candidate/reference ratio."""
    if not paired or replicates <= 0:
        raise ValueError("paired sessions and a positive replicate count are required")
    if statistic not in {"latency", "throughput", "memory"}:
        raise ValueError("unsupported bootstrap statistic")
    session_ids = sorted(paired)
    if len(session_ids) != 3:
        raise ValueError("the frozen policy requires exactly three independent sessions")
    for session_id in session_ids:
        old, new = paired[session_id]
        if not old or len(old) != len(new):
            raise ValueError(f"{session_id}: paired non-empty samples are required")
        if any(not math.isfinite(float(v)) or float(v) <= 0 for v in (*old, *new)):
            raise ValueError(f"{session_id}: ratio samples must be finite and positive")

    rngs = {
        session_id: SplitMix64(session_seed(policy_schema_id, case_id, metric_id, session_id))
        for session_id in session_ids
    }
    ratios: list[float] = []
    for _ in range(replicates):
        reference_sample: list[float] = []
        candidate_sample: list[float] = []
        for session_id in session_ids:
            old, new = paired[session_id]
            rng = rngs[session_id]
            for _ in range(len(old)):
                index = rng.bounded(len(old))
                reference_sample.append(float(old[index]))
                candidate_sample.append(float(new[index]))

        if statistic in {"throughput", "memory"}:
            ratio = median(candidate_sample) / median(reference_sample)
        else:
            candidate_value = median(candidate_sample) if percentile == 50 else nearest_rank(candidate_sample, percentile)
            reference_value = median(reference_sample) if percentile == 50 else nearest_rank(reference_sample, percentile)
            ratio = candidate_value / reference_value
        if not math.isfinite(ratio) or ratio <= 0:
            raise ValueError("bootstrap produced an invalid ratio")
        ratios.append(ratio)

    low, high = interval_percentiles
    if not 0 < low < high <= 100:
        raise ValueError("invalid confidence interval percentiles")
    return (
        nearest_rank(ratios, low),
        nearest_rank(ratios, high),
    )


def classify_interval(interval: tuple[float, float], direction: str, limit: float) -> str:
    lower, upper = interval
    if lower > upper or not math.isfinite(lower) or not math.isfinite(upper):
        raise ValueError("confidence interval must be finite and ordered")
    if not math.isfinite(limit) or limit <= 0:
        raise ValueError("limit must be finite and positive")
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
        raise ValueError("direction must be maximum or minimum")
    return "INCONCLUSIVE"
