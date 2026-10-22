#!/usr/bin/env python3
"""Progressive convergence gate; Pitlord owns dependency evaluation/fingerprints.

Capture is an explicit maintainer action, never part of the ordinary CI check.
The original policy remains unbaselined in check_architecture.py.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import re
import subprocess
import sys
from pathlib import Path

from check_architecture import ROOT, WORKSPACE, arcana_candidates, executable, resolve
from check_component_dag import validate_file as validate_component_target

DIRECTORY = ROOT / "tools/pitlord"
POLICY = DIRECTORY / "convergence.json"
BASELINE = DIRECTORY / "convergence-baseline.json"
DEBT = DIRECTORY / "convergence-debt.json"
EVIDENCE = DIRECTORY / "convergence-graph.json"
CATEGORIES = {
    "durable ownership", "read dependency", "mutation dependency", "construction/init",
    "orchestration callback", "shared type", "configuration", "test-only",
    "obsolete compatibility",
}


def read(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def write(path: Path, value: dict) -> None:
    path.write_text(json.dumps(value, indent=2) + "\n", encoding="utf-8")


def source_manifest(root: Path) -> dict[str, str]:
    return {path.relative_to(root).as_posix(): hashlib.sha256(path.read_bytes()).hexdigest()
            for path in sorted((root / "src").rglob("*.rs"))}


def command_json(command: list[str], accepted: tuple[int, ...] = (0,)) -> dict:
    result = subprocess.run(command, cwd=ROOT, capture_output=True, text=True, encoding="utf-8", timeout=300)
    if result.returncode not in accepted:
        raise RuntimeError(f"{command[0]} failed ({result.returncode}): {result.stderr or result.stdout}")
    return json.loads(result.stdout)


def node_identity(node: dict | None) -> str:
    if node is None:
        return ""
    if node.get("identity"):
        return "identity:" + node["identity"]
    if node.get("key"):
        return "key:" + node["key"]
    parts = [node["kind"], node["path"], node["name"]]
    if node.get("span"):
        parts.extend(str(node["span"][key]) for key in ("start_line", "start_column", "end_line", "end_column"))
    return "fallback:" + "\0".join(parts)


def fingerprint(rule: str, evidence: dict) -> str:
    # Pitlord evidence fingerprint v1, used only to associate classifications
    # with engine findings; no independent dependency evaluation here.
    parts = [rule, evidence["issue"], node_identity(evidence["source"]),
             evidence.get("relation", ""), node_identity(evidence.get("target")),
             ",".join(sorted(evidence.get("areas", []))),
             evidence.get("source_area", ""), evidence.get("target_area", "")]
    return "sha256:" + hashlib.sha256("\0".join(parts).encode()).hexdigest()


def findings(report: dict) -> dict[str, tuple[str, dict]]:
    if report.get("schema") != "pitlord.report.v1":
        raise ValueError("unsupported Pitlord report schema")
    result = {}
    for diagnostic in report.get("diagnostics") or []:
        for evidence in diagnostic["evidence"]:
            key = fingerprint(diagnostic["rule_id"], evidence)
            if key in result:
                raise ValueError(f"duplicate Pitlord evidence fingerprint: {key}")
            result[key] = (diagnostic["rule_id"], evidence)
    return result


def validate_target() -> None:
    policy = read(POLICY)
    areas = policy["areas"]
    canonical = read(DIRECTORY / "semantic.json")["areas"]
    if areas != canonical:
        raise ValueError("convergence ownership must match the canonical Pitlord areas")
    names = {area["id"] for area in areas}
    allowed = read(DIRECTORY / "convergence-target.json")["allowed"]
    if set(allowed) != names:
        raise ValueError("target matrix must classify every area")
    rules = {}
    rule_ids = set()
    for rule in policy["rules"]:
        if len(rule.get("from_areas", [])) != 1 or rule["id"] in rule_ids:
            raise ValueError("direction rules must have one source area and unique IDs")
        source = rule["from_areas"][0]
        if source in rules:
            raise ValueError(f"duplicate direction rule for {source}")
        rules[source] = rule
        rule_ids.add(rule["id"])
    relations = {"imports", "depends-on", "calls", "references", "implements"}
    for area, targets in allowed.items():
        if not set(targets) <= names or area not in targets:
            raise ValueError(f"invalid target direction for {area}")
        denied = names - set(targets)
        if denied and set(rules[area]["to_areas"]) != denied:
            raise ValueError(f"policy and allowed target matrix differ for {area}")
        if denied and (rules[area]["type"] != "forbid_dependency" or set(rules[area]["relations"]) != relations):
            raise ValueError(f"incomplete dependency evidence policy for {area}")
    if set(rules) != {area for area in names if names - set(allowed[area])}:
        raise ValueError("unexpected or missing convergence direction rule")


def validate(baseline: dict, debt: dict, current: dict) -> list[str]:
    errors = []
    if baseline.get("version") != 1 or debt.get("version") != 1:
        return ["unsupported convergence baseline/debt version"]
    known = {entry["fingerprint"] for entry in baseline["entries"]}
    classified = {entry["fingerprint"] for entry in debt["entries"]}
    if len(known) != len(baseline["entries"]) or len(classified) != len(debt["entries"]):
        errors.append("duplicate convergence baseline/debt fingerprint")
    if known != classified:
        errors.append("baseline and classified debt must contain exactly the same findings")
    for entry in debt["entries"]:
        if not entry.get("categories") or not set(entry["categories"]) <= CATEGORIES:
            errors.append(f"unclassified responsibility: {entry['fingerprint']}")
        if type(entry.get("phase")) is not int or entry["phase"] not in range(1, 8) or not entry.get("disposition", "").strip():
            errors.append(f"missing removal phase/disposition: {entry['fingerprint']}")
        if entry.get("reviewed") is not True:
            errors.append(f"source review required: {entry['fingerprint']}")
        if entry.get("evidence_status") not in {"source-confirmed", "test-only", "analyzer-artifact"} or not entry.get("review_note", "").strip():
            errors.append(f"missing source evidence disposition: {entry['fingerprint']}")
    for key in sorted(set(current) - known):
        rule, evidence = current[key]
        errors.append(f"new unclassified dependency: {rule}: {evidence['source']['path']} --{evidence.get('relation')}--> {evidence['target']['path']} ({key})")
    for key in sorted(known - set(current)):
        errors.append(f"resolved/stale debt must be deleted from both ledgers: {key}")
    return errors


def validate_inventory() -> None:
    original = read(EVIDENCE)
    document = read(DIRECTORY / "convergence-boundaries.json")
    expected = {(edge["source_area"], edge["target_area"]) for edge in original["analysis"]["dependencies"]}
    edges = document["boundaries"]
    actual = {(edge["source_area"], edge["target_area"]) for edge in edges}
    if document.get("version") != 1 or document.get("baseline_commit") != original["baseline_commit"] or expected != actual or len(actual) != len(edges):
        raise ValueError("every captured domain edge must have exactly one boundary disposition")
    for edge in edges:
        if not edge.get("disposition", "").strip() or not edge.get("categories") or not set(edge["categories"]) <= CATEGORIES:
            raise ValueError(f"unclassified captured domain edge: {edge['source_area']} -> {edge['target_area']}")


def owner(path: str, areas: list[dict]) -> str:
    matches = [area["id"] for area in areas if any(path == p or path.startswith(p.rstrip("/") + "/") for p in area["paths"])]
    if len(matches) != 1:
        raise ValueError(f"expected one convergence owner for {path}: {matches}")
    return matches[0]


def disposition(source: str, target: str, source_name: str = "", target_name: str = "") -> tuple[int, str]:
    """Suggest the campaign phase from the owners and concrete edge endpoints.

    This is a routing hint for review, not a claim that the edge was read or
    approved. Captured entries deliberately remain unreviewed until a person
    records the actual responsibility and disposition.
    """
    symbols = f"{source_name} {target_name}".lower()
    if source == "container":
        if any(word in symbols for word in ("open", "scan", "recover", "repair", "truncate")):
            return 1, "Review whether inspection/recovery belongs in the physical storage contract or an explicit repair operation."
        return 1, "Review the Container edge; retain only physical framing, I/O, identity, synchronization, and global ordering there."
    if "project-environment" in {source, target}:
        return 4, "Review the Project Environment boundary; repository authority stays with Lore/Git and semantic correlation stays with Reliquary."
    if source in {"native-import", "history-rewrite"} or target in {"native-import", "history-rewrite"}:
        return 3, "Review whether this is cross-owner composition; keep import/history rules above narrow owner read/proof contracts."
    if target == "facade":
        return 6, "Review the lower-to-facade edge; move the contract to its owner and keep public composition at the top."
    if target == "runtime-inference" or source in {"chronos", "config-security", "context-engine"}:
        return 5, "Review the runtime/configuration boundary; keep orchestration above owner policy and execution contracts."
    if source == target:
        return 2, "Review whether this is a justified owner-local dependency or a responsibility that should be split."
    return 2, "Review ownership and keep only owner-local behavior or a narrow cross-owner read/proof contract."


def classify(rule: str, evidence: dict, areas: list[dict]) -> dict:
    source, target = evidence["source"], evidence["target"]
    source_area, target_area = owner(source["path"], areas), owner(target["path"], areas)
    source_name, target_name = source.get("name", ""), target.get("name", "")
    phase, action = disposition(source_area, target_area, source_name, target_name)
    relation = evidence.get("relation", "")
    source_path = source["path"].lower()
    target_path = target["path"].lower()
    source_symbol = source_name.lower()
    target_symbol = target_name.lower()
    categories: list[str] = []
    review_hint = "Confirm the responsible owner and select the edge's actual responsibility; endpoint names alone are insufficient."

    # Test scope is the only category inferred from location/name alone.
    if ("test" in Path(source_path).stem or "test" in source_symbol
            or source_path.startswith(("tests/", "cli/tests/"))):
        categories = ["test-only"]
        review_hint = "Confirm this edge is exercised only by tests and is not compiled into production code."
    elif source_area == "container":
        if any(word in f"{source_symbol} {target_symbol}" for word in ("open", "scan", "recover", "repair", "truncate")):
            categories = ["construction/init"]
            review_hint = "Confirm whether this is physical open/inspection/recovery or semantic replay delegated through a callback."
        elif any(word in target_symbol for word in ("append", "write", "allocate", "sync")):
            categories = ["mutation dependency"]
            review_hint = "Confirm the target is a physical storage mutation; semantic admission and reachability remain owner responsibilities."
        elif any(word in target_symbol for word in ("read", "chunk", "version", "identity")):
            categories = ["read dependency"]
            review_hint = "Confirm the target is a physical storage read/query and does not interpret semantic reachability."
    elif source_area == "config-security" or target_area == "config-security":
        if any(word in f"{source_symbol} {target_symbol} {source_path} {target_path}"
               for word in ("config", "credential", "secret", "key", "route", "provider")):
            categories = ["configuration"]
    elif target_area == "runtime-inference" and any(
        word in f"{target_symbol} {target_path}" for word in ("gateway", "runtime_host", "worker", "route", "endpoint")
    ):
        categories = ["orchestration callback"]
        review_hint = "Confirm this edge is a callback into runtime orchestration rather than a provider-neutral execution contract."
    elif relation in {"calls", "observed-calls", "routes-to"}:
        names = f"{source_symbol} {target_symbol}"
        if any(word in names for word in ("open", "create", "initialize", "rebuild", "replay", "construct", "from_bytes")):
            categories = ["construction/init"]
            review_hint = "Confirm this call constructs, opens, or replays owner state."
        elif any(word in names for word in ("publish", "append", "write", "put", "set_", "update", "delete", "remove", "tombstone", "commit", "complete", "mutate")):
            categories = ["mutation dependency"]
            review_hint = "Confirm this call performs an authoritative mutation and identify the owner of that mutation."
        elif any(word in names for word in ("read", "get", "find", "lookup", "load", "query", "search", "inspect", "validate", "prove", "snapshot", "current", "latest")):
            categories = ["read dependency"]
            review_hint = "Confirm this is a read/proof dependency and identify the authoritative owner of the returned fact."
        elif source_area in {"facade", "native-import", "history-rewrite", "runtime-inference", "project-environment"}:
            review_hint = "This may be cross-owner composition; confirm which domain owns the rule and whether the call mutates state."
    elif relation in {"references", "implements"} and target_name[:1].isupper():
        categories = ["shared type"]
        review_hint = "Confirm the referenced type is a dependency-neutral contract or belongs with its semantic owner."
    elif relation in {"imports", "depends-on"}:
        review_hint = "Classify the module dependency from its actual use: shared contract, read, mutation, construction, or composition."

    if "runtimehost" in target_symbol or "runtime_host" in target_path:
        # The historical name is a retirement concern only when a concrete
        # compatibility/forwarding symbol is the target; file naming alone is
        # not enough to assert obsolete compatibility.
        if any(word in target_symbol for word in ("runtimehost", "routes")):
            review_hint += " Check whether the RuntimeHost-named surface is still a supported Gateway/Instance contract or obsolete compatibility."
    return {"fingerprint": fingerprint(rule, evidence), "reviewed": False, "source_area": source_area,
            "target_area": target_area, "categories": sorted(set(categories)), "review_hint": review_hint, "phase": phase,
            "source": source["path"], "source_name": source.get("name", ""),
            "target": target["path"], "target_name": target.get("name", ""),
            "relation": relation, "disposition": action}


def capture(pitlord: Path, arcana: Path, report: dict) -> None:
    areas = read(POLICY)["areas"]
    staged = ROOT / ".verification-tmp/convergence-baseline.json"
    staged.parent.mkdir(exist_ok=True)
    subprocess.run([str(pitlord), "baseline", "--repo", str(ROOT), "--policy", str(POLICY),
                    "--arcana", str(arcana), "--output", str(staged), "--timeout", "4m"], check=True, cwd=ROOT)
    current = findings(report)
    classified = [classify(rule, evidence, areas) for _, (rule, evidence) in sorted(current.items())]
    analysis = command_json([str(pitlord), "analyze", "--repo", str(ROOT), "--policy", str(POLICY),
                             "--arcana", str(arcana), "--scope", "src", "--relations", "imports,depends-on,calls,references,implements", "--format", "json"])
    snapshot_id = (ROOT / ".arcana/CURRENT").read_text().strip()
    # Ask Pitlord for every cross-area relationship, including permitted
    # downward edges. This keeps field/local dataflow out of the boundary
    # inventory while retaining the exact concrete module/symbol endpoints.
    full_policy = {"version": 1, "areas": areas, "rules": [
        {"id": "inventory-" + area["id"], "type": "forbid_dependency",
         "from_areas": [area["id"]], "to_areas": [other["id"] for other in areas if other != area],
         "relations": ["imports", "depends-on", "calls", "references", "implements"]}
        for area in areas]}
    inventory_policy = staged.parent / "convergence-inventory-policy.json"
    write(inventory_policy, full_policy)
    complete = command_json([str(pitlord), "check", "--repo", str(ROOT), "--policy", str(inventory_policy),
                             "--arcana", str(arcana), "--format", "json", "--timeout", "4m"], (0, 1))
    edges = []
    public_crossings = {}
    source_lines = {}
    for _, evidence in findings(complete).values():
        source, target = evidence["source"], evidence["target"]
        edges.append({"source": source["path"], "source_name": source["name"],
                      "target": target["path"], "target_name": target["name"], "relation": evidence["relation"]})
        source_area, target_area = owner(source["path"], areas), owner(target["path"], areas)
        if source_area == target_area or source["kind"] not in {"function", "method", "constructor"} or not source.get("span"):
            continue
        if source_area not in {"facade", "runtime-inference", "native-import", "history-rewrite", "project-environment"}:
            continue
        if source["path"] not in source_lines:
            source_lines[source["path"]] = (ROOT / source["path"]).read_text(encoding="utf-8").splitlines()
        lines = source_lines[source["path"]]
        line = source["span"]["start_line"]
        header = "\n".join(lines[line - 1:line + 3])
        if not re.search(r"\bpub\s+(?:async\s+)?fn\b", header):
            continue
        key = (source["path"], line, source["name"])
        public_crossings.setdefault(key, set()).add(target_area)
    head = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
    source_hashes = source_manifest(ROOT)
    if source_hashes != source_manifest(ROOT / ".lexicon/repo/source"):
        raise ValueError("authored Rust changed during graph analysis; refresh evidence before capture")
    documents = [(BASELINE, read(staged)), (DEBT, {"version": 1, "entries": classified}),
                 (EVIDENCE, {"version": 1, "baseline_commit": head, "arcana_snapshot": snapshot_id,
                             "source_sha256": source_hashes, "analysis": analysis,
                             "public_entrypoints": [{"path": path, "line": line, "name": name,
                                                     "dependency_areas": sorted(targets)}
                                                    for (path, line, name), targets in sorted(public_crossings.items())],
                             "module_edges": sorted(edges, key=lambda e: tuple(e.values()))})]
    published = []
    try:
        for path, document in documents:
            candidate = staged.parent / path.name
            write(candidate, document)
        for path, _ in documents:
            (staged.parent / path.name).replace(path)
            published.append(path)
    except BaseException:
        for path in published:
            path.unlink(missing_ok=True)
        raise
    print(f"Captured {len(current)} provisional findings, {len(edges)} concrete module edges; source-review classifications before accepting.")


def self_test() -> None:
    entry = {"fingerprint": "known"}
    baseline = {"version": 1, "entries": [entry]}
    record = {**entry, "reviewed": True, "categories": ["shared type"], "phase": 2,
              "evidence_status": "source-confirmed", "review_note": "imports the owner contract",
              "disposition": "move to owner"}
    debt = {"version": 1, "entries": [record]}
    evidence = {"issue": "dependency", "source": {"kind": "module", "path": "a", "name": "a"},
                "target": {"kind": "module", "path": "b", "name": "b"}, "relation": "imports"}
    current = {"known": ("rule", evidence)}
    assert not validate(baseline, debt, current)
    assert validate(baseline, debt, {**current, "new-edge-in-existing-SCC": ("rule", evidence)})
    assert validate(baseline, debt, {})
    assert validate(baseline, {"version": 1, "entries": []}, current)
    assert validate(baseline, {"version": 1, "entries": [{**record, "categories": ["unknown"]}]}, current)
    assert validate(baseline, {"version": 1, "entries": [{**record, "phase": 0}]}, current)
    assert validate(baseline, {"version": 1, "entries": [{**record, "phase": True}]}, current)
    assert validate(baseline, {"version": 1, "entries": [{**record, "disposition": "  "}]}, current)
    assert validate({"version": 1, "entries": [entry, entry]}, debt, current)
    assert validate(baseline, {"version": 1, "entries": [{**record, "reviewed": False}]}, current)
    duplicate_report = {"schema": "pitlord.report.v1", "diagnostics": [
        {"rule_id": "rule", "evidence": [evidence, evidence]},
    ]}
    try:
        findings(duplicate_report)
    except ValueError as error:
        assert "duplicate Pitlord evidence fingerprint" in str(error)
    else:
        raise AssertionError("duplicate current evidence must fail closed")
    assert node_identity({"identity": "x"}) == "identity:x"
    assert not findings({"schema": "pitlord.report.v1", "diagnostics": None})
    print("Convergence guard: 13 positive/negative fixtures passed")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--capture", action="store_true", help="initial capture only; creates provisional unreviewed debt and refuses overwrite")
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--check-target", action="store_true", help="validate the logical target DAG without scanning source")
    args = parser.parse_args()
    self_test()
    count, edges = validate_component_target()
    if args.self_test or args.check_target:
        print(f"Logical target DAG: {count} components, {edges} justified edges, no upward reach or cycles")
        return 0
    validate_target()
    pitlord = resolve("PITLORD", "pitlord", [WORKSPACE / "pitlord/bin" / executable("pitlord")])
    arcana = resolve("ARCANA", "arcana", arcana_candidates())
    report = command_json([str(pitlord), "check", "--repo", str(ROOT), "--policy", str(POLICY),
                           "--arcana", str(arcana), "--format", "json", "--timeout", "4m"], (0, 1))
    if args.capture:
        if BASELINE.exists() or DEBT.exists() or EVIDENCE.exists():
            raise ValueError("capture already exists; retire resolved ledger entries explicitly instead of resetting campaign debt")
        capture(pitlord, arcana, report)
    else:
        original = read(EVIDENCE)
        current_snapshot = (ROOT / ".arcana/CURRENT").read_text().strip()
        if current_snapshot == original["arcana_snapshot"] and source_manifest(ROOT) != original["source_sha256"]:
            raise ValueError("Rust source changed with the original graph still active; run check_architecture.py --refresh")
        if source_manifest(ROOT) != source_manifest(ROOT / ".lexicon/repo/source"):
            raise ValueError("Rust source differs from the indexed graph; run check_architecture.py --refresh after source edits settle")
        validate_inventory()
    errors = validate(read(BASELINE), read(DEBT), findings(report))
    if errors:
        print("\n".join(errors[:20]), file=sys.stderr)
        if len(errors) > 20:
            print(f"... {len(errors)} total convergence errors; inspect the classified debt ledger", file=sys.stderr)
        return 1
    # The engine remains the final authority for baseline evaluation.
    subprocess.run([str(pitlord), "check", "--repo", str(ROOT), "--policy", str(POLICY),
                    "--baseline", str(BASELINE), "--arcana", str(arcana), "--timeout", "4m"], cwd=ROOT, check=True)
    print(f"Convergence guard: {len(findings(report))} classified existing findings; no new or stale debt")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, ValueError, KeyError, RuntimeError, subprocess.SubprocessError) as error:
        print(f"convergence check failed: {error}", file=sys.stderr)
        raise SystemExit(1)
