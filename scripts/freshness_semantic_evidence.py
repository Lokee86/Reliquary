#!/usr/bin/env python3
"""Reproduce compact Step 4 evidence from existing exported inventories."""
import argparse
import hashlib
import json
from pathlib import Path
from freshness_semantic_calibrate import load, report


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--inventory", action="append", default=[])
    parser.add_argument("--output", required=True)
    args = parser.parse_args()
    results = []
    trajectories = None
    for path in [None] + args.inventory:
        rows, metadata = load(path)
        result = report(rows, metadata, [.70, .75, .80], [16, 32, 64],
                        [("linear", 10), ("steep", 25), ("fixed", 5)], True)
        if trajectories is None:
            trajectories = [
                {"threshold": c["threshold"], "curve": c["curve"], "cap": c["cap"],
                 "scenarios": c["metrics"]["controlled_attention_trajectories"]}
                for c in result["configurations"] if c["limit"] == 64]
        for config in result["configurations"]:
            config["metrics"].pop("controlled_attention_trajectories")
        result["name"] = "synthetic" if path is None else Path(path).stem
        results.append(result)
        print("calibrated", result["name"], flush=True)
    evidence = {
        "schema": 2,
        "harness_sha256": hashlib.sha256(Path(__file__).with_name(
            "freshness_semantic_calibrate.py").read_bytes()).hexdigest(),
        "exporter_sha256": hashlib.sha256(Path("examples/freshness_semantic_export.rs").read_bytes()).hexdigest(),
        "results": results, "controlled_attention_trajectories": trajectories}
    Path(args.output).write_text(json.dumps(evidence, indent=2, sort_keys=True,
                                allow_nan=False) + "\n", encoding="utf-8", newline="\n")


if __name__ == "__main__":
    main()
