#!/usr/bin/env python3
"""Validate Reliquary's accepted *logical* component DAG; no Arcana scan required.

Pitlord remains authoritative for live Rust source-edge capture and reviewed
Phase 0 debt. This checker does not infer source ownership from coarse paths.
"""
from __future__ import annotations

import argparse
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
TARGET = ROOT / "tools/pitlord/convergence-component-dag.json"
KINDS = {"foundation", "physical", "store", "composition", "capability",
         "service", "integration", "orchestration", "consumer"}
MODES = {"read", "write", "construct", "invoke"}


def validate(target: dict) -> list[str]:
    errors = []
    if target.get("version") != 1 or target.get("direction") != "consumer -> dependency":
        errors.append("incorrect target version or edge direction")
    nodes = target.get("components")
    if not isinstance(nodes, list) or not nodes:
        return errors + ["components must be a nonempty array"]
    by_id = {}
    for node in nodes:
        key = node.get("id")
        if not isinstance(key, str) or not key or key in by_id:
            errors.append(f"duplicate/invalid component id: {key}")
            continue
        by_id[key] = node
        if type(node.get("rank")) is not int or node["rank"] < 0:
            errors.append(f"invalid dependency order: {key}")
        if type(node.get("phase")) is not int or node["phase"] not in range(1, 8):
            errors.append(f"invalid phase: {key}")
        if node.get("kind") not in KINDS or not str(node.get("responsibility", "")).strip():
            errors.append(f"missing kind or ownership contract: {key}")
    graph = {key: [] for key in by_id}
    for key, node in by_id.items():
        seen = set()
        for edge in node.get("depends_on", []):
            dest = edge.get("to")
            if dest in seen:
                errors.append(f"duplicate permitted edge: {key} -> {dest}")
            seen.add(dest)
            if dest not in by_id:
                errors.append(f"unknown dependency: {key} -> {dest}")
                continue
            graph[key].append(dest)
            if edge.get("mode") not in MODES or not str(edge.get("why", "")).strip():
                errors.append(f"unjustified edge: {key} -> {dest}")
            if key == dest or node["rank"] <= by_id[dest]["rank"]:
                errors.append(f"upward/peer dependency forbidden even when acyclic: {key} -> {dest}")
            if key == "history" and edge.get("mode") != "read":
                errors.append(f"History must be read-only: {key} -> {dest}")
            if node["kind"] in {"foundation", "physical", "store"} and by_id[dest]["kind"] not in {"foundation", "physical"}:
                errors.append(f"lower-owner backreach: {key} -> {dest}")

    visited, visiting = set(), set()

    def visit(key: str) -> None:
        if key in visiting:
            errors.append(f"directed cycle at {key}")
            return
        if key in visited:
            return
        visiting.add(key)
        for dependency in graph[key]:
            visit(dependency)
        visiting.remove(key)
        visited.add(key)

    for key in by_id:
        visit(key)
    return errors


def validate_file() -> tuple[int, int]:
    data = json.loads(TARGET.read_text(encoding="utf-8"))
    errors = validate(data)
    if errors:
        raise ValueError("invalid target DAG: " + "; ".join(errors[:15]))
    return len(data["components"]), sum(len(n["depends_on"]) for n in data["components"])


def self_test() -> None:
    target = json.loads(TARGET.read_text(encoding="utf-8"))
    assert not validate(target)
    def copy():
        return json.loads(json.dumps(target))
    upward = copy()
    next(x for x in upward["components"] if x["id"] == "memory-store")["depends_on"].append(
        {"to": "retrieval", "mode": "read", "why": "forbidden backreach"})
    assert any("upward/peer" in e for e in validate(upward))
    cycle = copy()
    next(x for x in cycle["components"] if x["id"] == "pure-codec")["depends_on"].append(
        {"to": "container", "mode": "read", "why": "forbidden cycle"})
    assert any("directed cycle" in e for e in validate(cycle))
    unknown = copy()
    next(x for x in unknown["components"] if x["id"] == "retrieval")["depends_on"].append(
        {"to": "imaginary-service", "mode": "read", "why": "unknown"})
    assert any("unknown dependency" in e for e in validate(unknown))
    peer = copy()
    next(x for x in peer["components"] if x["id"] == "dream")["depends_on"].append(
        {"to": "insomnia", "mode": "read", "why": "forbidden peer"})
    assert any("upward/peer" in e for e in validate(peer))
    missing_reason = copy()
    next(x for x in missing_reason["components"] if x["id"] == "retrieval")["depends_on"][0]["why"] = ""
    assert any("unjustified edge" in e for e in validate(missing_reason))
    history_write = copy()
    next(x for x in history_write["components"] if x["id"] == "history")["depends_on"][0]["mode"] = "write"
    assert any("History must be read-only" in e for e in validate(history_write))


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--self-test", action="store_true", help="also run negative invariant fixtures")
    args = parser.parse_args()
    count, edges = validate_file()
    if args.self_test:
        self_test()
    print(f"Logical DAG valid: {count} components, {edges} allowlisted edges, no cycles/upward dependencies")
    if args.self_test:
        print("Negative DAG fixtures passed: upward, cycle, unknown, peer, missing reason, History write")


if __name__ == "__main__":
    main()
