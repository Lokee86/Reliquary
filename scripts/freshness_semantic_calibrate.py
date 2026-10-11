#!/usr/bin/env python3
"""Deterministic F8 semantic calibration/oracle harness.

Input JSONL records may contain:
  {"source":[...], "targets":[{"id":"...", "vector":[...], "label":true|false}]}
The default fixture is synthetic and requires no model inference or repository data.
"""
from __future__ import annotations
import argparse, json, math, statistics
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable

@dataclass(frozen=True)
class Curve:
    name: str
    threshold: float
    scale: float
    offset: float = 0.0
    cap: int = 100
    def bonus(self, cosine: float) -> int:
        if not math.isfinite(cosine) or cosine < self.threshold:
            return 0
        return max(0, min(self.cap, int(math.floor((cosine - self.threshold) * self.scale + self.offset + 1e-12))))

def cosine(a: list[float], b: list[float]) -> float | None:
    if len(a) != len(b) or not a:
        return None
    aa = sum(x*x for x in a); bb = sum(x*x for x in b)
    if not (math.isfinite(aa) and math.isfinite(bb)) or aa <= 0 or bb <= 0:
        return None
    value = sum(x*y for x,y in zip(a,b)) / math.sqrt(aa*bb)
    return value if math.isfinite(value) and -1.0 <= value <= 1.0 else None

def fixture() -> list[dict]:
    return [
        {"source":[1.0,0.0,0.0], "targets":[
            {"id":"near","vector":[.99,.1,0],"label":True},
            {"id":"mid","vector":[.8,.6,0],"label":True},
            {"id":"far","vector":[0,1,0],"label":False},
            {"id":"invalid","vector":[0,0,0],"label":False},
        ]},
        {"source":[0.0,1.0,0.0], "targets":[
            {"id":"near-2","vector":[.1,.99,0],"label":True},
            {"id":"false-close","vector":[.7,.714,0],"label":False},
            {"id":"wrong-dim","vector":[1,0],"label":False},
        ]},
    ]

def load(path: str | None) -> list[dict]:
    if not path: return fixture()
    rows = []
    for line in Path(path).read_text(encoding="utf-8").splitlines():
        if line.strip(): rows.append(json.loads(line))
    return rows

def evaluate(rows: list[dict], curve: Curve, limit: int) -> dict:
    total = valid = qualified = 0
    nearest = []
    selected = []
    false_selected = 0
    for row in rows:
        scored = []
        for target in row["targets"]:
            total += 1
            score = cosine(row["source"], target["vector"])
            if score is None: continue
            valid += 1
            bonus = curve.bonus(score)
            if bonus > 0: qualified += 1
            scored.append((score, target["id"], bool(target.get("label", False)), bonus))
        scored.sort(key=lambda x: (-x[0], x[1].encode("utf-8")))
        nearest.append(len(scored))
        usable = [x for x in scored if x[3] > 0][:limit]
        selected.extend((x[1], x[0], x[3]) for x in usable)
        false_selected += sum(1 for x in usable if not x[2])
    true_total = sum(bool(t.get("label", False)) for r in rows for t in r["targets"])
    true_selected = sum(1 for sid,_,_ in selected if any(
        t["id"] == sid and t.get("label", False) for r in rows for t in r["targets"]))
    return {
        "targets": total, "finite_compatible_vectors": valid,
        "compatible_vector_coverage": (valid / total if total else 0.0),
        "above_threshold_matches": qualified,
        "nearest_neighbor_count_mean": (statistics.fmean(nearest) if nearest else 0.0),
        "nearest_neighbor_count_max": (max(nearest) if nearest else 0),
        "selected_candidates": len(selected), "usable_limit": limit,
        "false_match_rate": (false_selected / len(selected) if selected else 0.0),
        "labeled_recall": (true_selected / true_total if true_total else 0.0),
        "score_trajectory": [bonus for _,_,bonus in selected],
    }

def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--input", help="JSONL corpus; omitted uses deterministic synthetic fixture")
    parser.add_argument("--output", help="write JSON report; omitted prints it")
    parser.add_argument("--thresholds", default="0.70,0.75,0.80")
    parser.add_argument("--curves", default="linear,steep")
    parser.add_argument("--limit", type=int, default=8)
    args = parser.parse_args()
    if args.limit <= 0: parser.error("--limit must be positive")
    rows = load(args.input)
    curves = []
    for threshold in (float(x) for x in args.thresholds.split(",")):
        if not 0 <= threshold <= 1: parser.error("thresholds must be in [0,1]")
        for kind in args.curves.split(","):
            if kind == "linear": curves.append(Curve(f"linear@{threshold:g}", threshold, 100))
            elif kind == "steep": curves.append(Curve(f"steep@{threshold:g}", threshold, 200))
            else: parser.error(f"unknown curve: {kind}")
    report = {"schema": 1, "fixture_rows": len(rows), "configurations": [
        {"curve": c.name, "threshold": c.threshold, "bonus_scale": c.scale,
         "candidate_limit": args.limit, "metrics": evaluate(rows, c, args.limit)}
        for c in curves]}
    output = json.dumps(report, indent=2, sort_keys=True) + "\n"
    if args.output: Path(args.output).write_text(output, encoding="utf-8")
    else: print(output, end="")
if __name__ == "__main__":
    main()
