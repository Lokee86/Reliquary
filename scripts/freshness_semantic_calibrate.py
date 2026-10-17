#!/usr/bin/env python3
"""Offline F8 oracle. JSONL per-turn rows or schema-2 retained-vector inventory.
No production policy, inference, or owner-file writes. Optional timing is diagnostic.
"""
from __future__ import annotations
import argparse
import hashlib
import heapq
import json
import math
import statistics
import time
from dataclasses import dataclass
from pathlib import Path


@dataclass(frozen=True)
class Curve:
    name: str
    threshold: float
    cap: int = 10

    def __post_init__(self):
        if not math.isfinite(self.threshold) or not -1 <= self.threshold < 1:
            raise ValueError("threshold must be finite in [-1,1)")
        if not 1 <= self.cap <= 100 or self.name not in ("linear", "steep", "fixed"):
            raise ValueError("invalid curve/cap")

    def bonus(self, similarity: float) -> int:
        if not math.isfinite(similarity) or not -1 <= similarity <= 1:
            raise ValueError("invalid similarity")
        if similarity < self.threshold:
            return 0
        if self.name == "fixed":
            return self.cap
        # Inclusive qualifying floor=1; endpoint=cap. Binary64, no epsilon.
        scale = (self.cap - 1) / (1 - self.threshold)
        return min(self.cap, 1 + math.floor((similarity - self.threshold) * scale))


def normalize(vector):
    if not vector or any(not math.isfinite(x) for x in vector):
        return None
    norm = math.sqrt(sum(float(x) * float(x) for x in vector))
    if not math.isfinite(norm) or norm == 0:
        return None
    return [float(x) / norm for x in vector]


def cosine(a, b):
    if len(a) != len(b):
        return None
    a, b = normalize(a), normalize(b)
    if a is None or b is None:
        return None
    return max(-1.0, min(1.0, sum(x*y for x, y in zip(a, b))))


def eligible(target):
    # Explicit fixtures may model failures; retained inventory is prospective.
    return not any(target.get(key, False) for key in (
        "wrong_owner", "self_source", "future", "archived", "unadmitted",
        "incompatible_profile", "invalid_generation", "obsolete_body"))


def rank(row):
    fragments = row.get("sources", [row.get("source")])
    matches = {}
    attempts = invalid = filtered = 0
    for target in row["targets"]:
        if target["id"] == row.get("exclude_id") or not eligible(target):
            filtered += 1
            continue
        best = None
        for fragment in fragments:
            attempts += 1
            score = cosine(fragment, target["vector"])
            if score is None:
                invalid += 1
                continue
            best = score if best is None else max(best, score)
        if best is None:
            continue
        identity = target["id"]
        label = target.get("label")  # Missing is unknown, never false.
        previous = matches.get(identity)
        if previous is not None and previous[1] != label:
            raise ValueError("conflicting labels for one target in a turn")
        if previous is None or best > previous[0]:
            matches[identity] = (best, label)
    return matches, {"cosine_evaluations": attempts, "invalid_pairs": invalid,
                     "eligibility_filtered_rows": filtered}


def select(matches, curve, limit):
    # heapq.nsmallest keeps O(limit) selection scratch, filters before cap.
    qualified = ((identity, score, label) for identity, (score, label) in matches.items()
                 if score >= curve.threshold)
    return heapq.nsmallest(limit, qualified, key=lambda x: (-x[1], x[0].encode("utf-8")))


def score_trajectory(events, initial=100, through=None):
    score = initial
    accounted = 0
    points = []
    for turn, bonus in sorted(events):
        if turn < accounted or not 1 <= bonus <= 100:
            raise ValueError("invalid score event")
        steps = (turn - accounted) // 10
        score = max(-100, score - steps)
        accounted += steps * 10
        score = min(100, score + bonus)
        points.append({"turn": turn, "score": score, "accounted_turn": accounted})
    if through is not None:
        if through < accounted:
            raise ValueError("tail cut regression")
        score = max(-100, score - (through - accounted) // 10)
    return {"events": points, "tail_score": score}


def percentile(values, p):
    if not values:
        return None
    return sorted(values)[max(0, math.ceil(len(values)*p/100)-1)]


def fixture():
    def vector(c):
        return [c, math.sqrt(1-c*c), 0.0]
    targets = [{"id": f"boundary-{i:02}", "vector": vector(c), "label": label}
               for i, (c, label) in enumerate([
                   (.6999, False), (.70, False), (.7499, False),
                   (.75, True), (.7501, True), (.80, True),
                   (.85, True), (.90, True), (.95, True), (1., True)])]
    targets += [{"id": "zero", "vector": [0., 0., 0.]},
                {"id": "dimension", "vector": [1., 0.]},
                {"id": "future", "vector": vector(1.), "future": True},
                {"id": "self", "vector": vector(1.), "self_source": True},
                {"id": "foreign", "vector": vector(1.), "wrong_owner": True}]
    return [{"source": [1., 0., 0.], "targets": targets},
            {"sources": [[1., 0., 0.]]*8, "targets": targets},
            {"source": [1., 0., 0.], "targets": [
                {"id": f"dense-{i:04}", "vector": vector(.7501+i/2048)}
                for i in range(512)]}]


def load(path):
    if path is None:
        return fixture(), {"kind": "synthetic", "labels": "controlled oracle"}
    raw = Path(path).read_bytes()
    metadata = {"input_sha256": hashlib.sha256(raw).hexdigest()}
    if raw.lstrip().startswith(b"{") and b'"populations"' in raw:
        inventory = json.loads(raw)
        rows = []
        coverage = []
        for population in inventory["populations"]:
            targets = population["target_vectors"]
            for source in population["sources"]:
                rows.append({"source": source["vector"], "targets": targets,
                             "profile": population["profile"],
                             "exclude_id": source.get("memory_id")})
            coverage.append({
                "profile": population["profile"],
                "dimensions": population["dimensions"],
                "lifecycle_eligible_memories": population["lifecycle_eligible_memories"],
                "compatible_target_bindings": len(targets),
                "archive_fragment_rows": population["archive_fragment_rows"],
                "sampled_queries": len(population["sources"]),
                "source_kind": population["source_kind"]})
        metadata.update({"kind": "retained-vector-neighborhood-only",
                         "fixture_sha256": inventory["fixture_sha256"],
                         "coverage": coverage, "labels": "unavailable"})
        return rows, metadata
    return [json.loads(x) for x in raw.splitlines() if x.strip()], metadata


def evaluate(ranked, curve, limit):
    counts, selected_counts, similarities, events = [], [], [], {}
    true_total = labeled_selected = false_selected = true_selected = 0
    for position, (matches, _) in enumerate(ranked, 1):
        counts.append(sum(score >= curve.threshold for score, _ in matches.values()))
        similarities.extend(score for score, _ in matches.values())
        true_total += sum(label is True for _, label in matches.values())
        chosen = select(matches, curve, limit)
        selected_counts.append(len(chosen))
        for identity, score, label in chosen:
            labeled_selected += label is not None
            true_selected += label is True
            false_selected += label is False
            events.setdefault(identity, []).append((position*100+12, curve.bonus(score)))
    selected = sum(selected_counts)
    return {
        "neighbors_above_threshold": {"p50": percentile(counts, 50),
            "p95": percentile(counts, 95), "max": max(counts, default=0)},
        "selected": selected,
        "truncated_turns": sum(n > limit for n in counts),
        "labeled_selected": labeled_selected,
        "false_match_rate": false_selected/labeled_selected if labeled_selected else None,
        "labeled_recall": true_selected/true_total if true_total else None,
        "similarity": {"p50": percentile(similarities, 50),
                       "p95": percentile(similarities, 95)},
        "trajectory_tail_distribution": {
            "min": min((score_trajectory(e, through=len(ranked)*100+212)["tail_score"]
                        for e in events.values()), default=None),
            "max": max((score_trajectory(e, through=len(ranked)*100+212)["tail_score"]
                        for e in events.values()), default=None)},
        "controlled_attention_trajectories": [
            {"cosine": c, "initial": initial, "interval": interval,
             **score_trajectory([(12+i*interval, curve.bonus(c)) for i in range(6)],
                                initial, 12+6*interval)}
            for c in (curve.threshold, (curve.threshold+1)/2, 1.)
            for initial in (-100, 100) for interval in (10, 100, 1000)]}


def report(rows, metadata, thresholds, limits, curves, timing=False):
    start = time.perf_counter_ns()
    ranked = [rank(row) for row in rows]
    elapsed = time.perf_counter_ns()-start
    total_rows = sum(len(row["targets"]) for row in rows)
    valid_rows = sum(len(matches) for matches, _ in ranked)
    result = {"schema": 2, "evidence": metadata, "source_rows": len(rows),
              "target_rows": total_rows, "valid_unique_target_rows": valid_rows,
              "work": {key: sum(stats[key] for _, stats in ranked)
                       for key in ("cosine_evaluations", "invalid_pairs",
                                   "eligibility_filtered_rows")},
              "configurations": [
                  {"threshold": threshold, "curve": name, "cap": cap, "limit": limit,
                   "metrics": evaluate(ranked, Curve(name, threshold, cap), limit)}
                  for threshold in thresholds for name, cap in curves for limit in limits]}
    if timing:
        result["diagnostic_python_search_ns"] = elapsed
        result["cosines_per_second"] = result["work"]["cosine_evaluations"]*1e9/max(1,elapsed)
    return result


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--input")
    parser.add_argument("--output")
    parser.add_argument("--thresholds", default="0.70,0.75,0.80")
    parser.add_argument("--curves", default="linear,steep,fixed")
    parser.add_argument("--limits", default="16,32,64")
    parser.add_argument("--limit", type=int)
    parser.add_argument("--timing", action="store_true")
    args = parser.parse_args()
    try:
        thresholds = [float(x) for x in args.thresholds.split(",")]
        limits = [args.limit] if args.limit is not None else [int(x) for x in args.limits.split(",")]
        if any(x <= 0 for x in limits):
            raise ValueError("limits must be positive")
        caps = {"linear": 10, "steep": 25, "fixed": 5}
        curves = [(name, caps[name]) for name in args.curves.split(",")]
        for threshold in thresholds:
            for name, cap in curves:
                Curve(name, threshold, cap)
        rows, metadata = load(args.input)
        output = json.dumps(report(rows, metadata, thresholds, limits, curves, args.timing),
                            indent=2, sort_keys=True, allow_nan=False) + "\n"
    except (ValueError, KeyError) as error:
        parser.error(str(error))
    if args.output:
        Path(args.output).write_text(output, encoding="utf-8", newline="\n")
    else:
        print(output, end="")


if __name__ == "__main__":
    main()
