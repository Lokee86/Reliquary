"""Check the reviewed D0 inventory and later enforce the concrete legacy hard cut.

Read-only by default. --write-baseline is an explicit reviewer operation, not an
automatic approval of changed callers. Lexical references are conservative
candidates, not resolved Rust calls; final deletion requires compiler/runtime
and external locked consumer acceptance as described in the deletion manifest.
"""
import argparse
from collections import Counter
from functools import lru_cache
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys

from check_multiplexing_phase3_ownership import LEXICAL, production_source

BASE = "999fbf7d85c9f892a1bf979293588033bcc1484b"
DEFAULT_BASELINE = "docs/multiplexing-p5d-source-inventory-2026-10-08.json"
PREFIX = "src/runtime_inference/runtime_host"
FORBIDDEN = re.compile(r"\b(?:ReliquaryRuntimeHost|OwnerExecution|RuntimeMemoryProfiles|LegacyManagedSessionState)\b")
DELETE = {
    "dependency_graph", "dream", "dream_owner", "files", "insomnia", "interaction", "knowledge_memory",
    "knowledge_relation", "lifecycle", "memory_provenance", "mount", "owner_access",
    "owner_archive", "owner_execution", "perception", "perception_api",
    "perception_flow", "perception_owner", "perception_queue",
    "perception_reconciliation", "phylactery", "project_environment",
    "semantic_knowledge", "semantic_provenance", "session", "topology", "vectors",
}
SPLIT = {
    "archive_search": "Archive search result/default contract; delete old host methods",
    "knowledge": "knowledge DTOs; delete old host mutations",
    "memory_search": "bounded retrieval DTOs/scoring helpers; delete mixed-profile host methods",
    "semantic_access": "semantic owner DTOs; delete ambient host selection",
    "semantic_echo": "Echo DTOs; delete host wrapper",
    "status": "background status DTO; delete old host sampling, retain independent eligibility tests",
}
RETAIN = {
    "instances": "verified instance authorization and instance-local selection",
    "routes": "shared endpoint route configuration",
    "owner_operation_journal": "REL attribution/publication operational journal and decoder",
    "owner_publication": "canonical recoverable REL-to-PHY publication coordinator",
    "owner_publication_codec": "frozen publication encoding/decoding/verification",
}
MAINTENANCE = {"reconciliation": "closed-owner reconciliation and reports; delete old host entry point"}


def git(repo, *args):
    return subprocess.run(["git", "-C", str(repo), *args], check=True,
                          capture_output=True, text=True).stdout.strip()


def files(repo):
    # rg discovers new/untracked Rust source as well as tracked source. Ignored
    # build caches are excluded. The explicit roots bound the audit surface.
    roots = [name for name in ("src", "src-tauri", "cli", "examples", "tests") if (repo / name).exists()]
    result = subprocess.run(["rg", "--files", "-g", "*.rs", *roots],
                            cwd=repo, check=True, capture_output=True, text=True)
    return {Path(name).as_posix(): (repo / name).read_text(encoding="utf-8")
            for name in sorted(result.stdout.splitlines())}


@lru_cache(maxsize=2048)
def masked(text):
    return LEXICAL.sub(lambda m: re.sub(r"[^\n]", " ", m.group()), text)


def hits(sources, pattern):
    rows = []
    regex = re.compile(pattern)
    for path, text in sorted(sources.items()):
        code = masked(text)
        for number, line in enumerate(code.splitlines(), 1):
            if regex.search(line):
                rows.append({"file": path, "line": number,
                             "source": text.splitlines()[number - 1].strip()})
    return rows


def policy(path):
    stem = Path(path).stem
    suffix = stem.removeprefix("runtime_host_")
    if stem == "runtime_host":
        return {"action": "split-root", "destination": "canonical gateway root and runtime support; remove legacy host authority"}
    if suffix.startswith(("gateway", "rel_", "phy_")):
        return {"action": "retain-canonical", "destination": "Gateway/P3/P4/B concrete owner (rename only at convergence)"}
    if suffix in {"instance_tests", "route_tests", "owner_operation_journal_tests", "owner_publication_codec_tests"}:
        return {"action": "retain-canonical-tests", "destination": "instance/routes/operational journal/codec protection"}
    if suffix in DELETE:
        return {"action": "delete-legacy", "destination": "A/B/E/F canonical owner APIs; no forwarding host"}
    if suffix in SPLIT:
        return {"action": "split-common", "destination": SPLIT[suffix]}
    if suffix in RETAIN:
        return {"action": "retain-relocate", "destination": RETAIN[suffix]}
    if suffix in MAINTENANCE:
        return {"action": "retain-maintenance", "destination": MAINTENANCE[suffix]}
    if suffix == "test_support":
        return {"action": "split-test-support", "destination": "retain test_path/endpoint fixtures used by canonical tests; remove old-host wait wrappers"}
    if suffix == "tests" or suffix.endswith(("_tests", "_test_support")):
        return {"action": "replace-legacy-tests", "destination": "equivalent Gateway/REL/PHY scenarios; remove obsolete ambient fixtures"}
    raise ValueError("unclassified root module: " + path)


def block_end(code, start):
    depth = 1
    end = start
    while end < len(code) and depth:
        depth += (code[end] == "{") - (code[end] == "}")
        end += 1
    if depth:
        raise ValueError("unbalanced Rust item")
    return end


def legacy_items(text):
    code = masked(text)
    fields, methods = [], []
    struct = re.search(r"\bpub struct ReliquaryRuntimeHost\s*\{", code)
    if struct:
        end = block_end(code, struct.end())
        fields = re.findall(r"(?m)^\s*(\w+)\s*:\s*([^\n]+)", text[struct.end():end - 1])
    for impl in re.finditer(r"\bimpl(?:\s+Drop\s+for)?\s+ReliquaryRuntimeHost\s*\{", code):
        end = block_end(code, impl.end())
        for method in re.finditer(r"\b(?:(pub(?:\([^)]*\))?)\s+)?fn\s+(\w+)", code[impl.end():end - 1]):
            offset = impl.end() + method.start()
            methods.append({"name": method.group(2), "visibility": method.group(1) or "private",
                            "line": text.count("\n", 0, offset) + 1})
    return [{"name": name, "type": kind.strip().rstrip(",")} for name, kind in fields], methods


def inventory(repo):
    sources = files(repo)
    modules = {}
    declarations = {}
    references = {}
    registrations_by_file = {}
    aliases = {}
    for caller, content in sorted(sources.items()):
        original = content.splitlines()
        for number, line in enumerate(masked(content).splitlines(), 1):
            row = {"file": caller, "line": number, "source": original[number - 1].strip()}
            for token in set(re.findall(r"[A-Za-z_]\w*", line)):
                references.setdefault(token, []).append(row)
        for match in re.finditer(r'#\[path\s*=\s*"(runtime_host[^"]*\.rs)"\]\s*(?:pub(?:\([^)]*\))?\s+)?mod\s+(\w+)', content):
            number = content.count("\n", 0, match.start()) + 1
            registrations_by_file.setdefault(match.group(1), []).append(
                {"file": caller, "line": number, "source": original[number - 1].strip()})
            aliases.setdefault(match.group(1), set()).add(match.group(2))
    for path, text in sorted(sources.items()):
        if path.startswith(PREFIX) and path.endswith(".rs"):
            stem = Path(path).stem
            fields, methods = legacy_items(text)
            symbols = sorted(set(re.findall(
                r"(?m)^pub(?:\([^)]*\))?\s+(?:struct|enum|trait|type)\s+([A-Z]\w*)", masked(text))))
            declarations.update({symbol: path for symbol in symbols})
            registrations = registrations_by_file.get(Path(path).name, [])
            authority_fields = {}
            for name in ("Shared", "OwnerExecution", "LegacyManagedSessionState"):
                found = re.search(r"\bstruct\s+" + name + r"\s*\{", masked(text))
                if found:
                    end = block_end(masked(text), found.end())
                    authority_fields[name] = [
                        {"name": field, "type": kind.strip().rstrip(",")}
                        for field, kind in re.findall(r"(?m)^\s*(?:pub(?:\([^)]*\))?\s+)?(\w+)\s*:\s*([^\n]+)", text[found.end():end - 1])]
            module_aliases = sorted(aliases.get(Path(path).name, set()))
            modules[path] = {
                **policy(path), "sha256": hashlib.sha256(text.encode()).hexdigest(),
                "registrations": registrations, "types": symbols,
                "module_aliases": module_aliases,
                "authority_fields": authority_fields,
                "legacy_fields": fields, "legacy_methods": methods,
                "test_functions": re.findall(r"#\[test\]\s*fn\s+(\w+)", masked(text)),
            }
    symbols = {}
    for symbol, owner in sorted(declarations.items()):
        symbols[symbol] = {"definition": owner, "references": references.get(symbol, [])}
    # Capture all old-host references, including fixtures and module-root uses.
    # Calls by name are conservative: receiver resolution belongs to compiler.
    method_names = sorted({m["name"] for v in modules.values() for m in v["legacy_methods"]})
    method_refs = candidate_calls(sources, method_names)
    return {
        "schema_version": 1, "base": BASE, "rust_files": len(sources),
        "modules": modules, "symbols": symbols, "method_references": method_refs,
        "module_references": {alias: references.get(alias, [])
                              for alias in sorted({a for m in modules.values() for a in m["module_aliases"]})},
        "root_references": hits(sources, r"\b(?:runtime_host|ReliquaryRuntimeHost|OwnerExecution|LegacyManagedSessionState)\b"),
        "cli_legacy_references": hits({p: s for p, s in sources.items() if p.startswith("cli/")},
                                     FORBIDDEN.pattern),
    }


def candidate_calls(sources, names):
    result = {name: [] for name in names}
    if not names:
        return result
    regex = re.compile(r"(?:\.|::)\s*(" + "|".join(re.escape(n) for n in names) + r")\s*(?:\(|::<)")
    for path, text in sorted(sources.items()):
        original = text.splitlines()
        for match in regex.finditer(masked(text)):
            number = text.count("\n", 0, match.start(1)) + 1
            result[match.group(1)].append(
                {"file": path, "line": number, "source": original[number - 1].strip()})
    return result


def external(repo, method_names=()):
    sources = files(repo)
    imports = hits(sources, r"\breliquary_memory\b")
    return {"head": git(repo, "rev-parse", "HEAD"),
            "dirty": bool(git(repo, "status", "--porcelain")),
            "rust_files": len(sources), "import_files": sorted({r["file"] for r in imports}),
            "import_references": imports,
            "method_references": candidate_calls(sources, method_names),
            "legacy_references": hits(sources, FORBIDDEN.pattern),
            "shared_type_references": hits(sources, r"\b(?:ReliquaryRuntimeHostError|ReliquaryRuntimeRoutes|RuntimeSemanticOwnerKind|RuntimeBackgroundStatus|RelReconciliationReport|VisibleMemorySearchResult|RuntimeEchoSource)\b"),
            "manifest": (repo / "src-tauri/Cargo.toml").read_text(encoding="utf-8")}


def hard_cut(repo, warlocks):
    violations = []
    for path, text in files(repo).items():
        # Test and example consumers must migrate too; cfg(test) bodies in
        # production modules are excluded here but full-build acceptance covers them.
        code = masked(text) if ("tests" in path or path.startswith("examples/")) else production_source(text)
        for number, line in enumerate(code.splitlines(), 1):
            if FORBIDDEN.search(line):
                violations.append({"file": path, "line": number})
    for consumer in warlocks:
        evidence = external(consumer)
        if evidence["legacy_references"]:
            violations.append({"consumer": str(consumer), "legacy_references": evidence["legacy_references"]})
    if not warlocks:
        violations.append({"gate": "explicit upgraded Warlock checkout required"})
    return violations


def self_test():
    assert FORBIDDEN.search("use crate::ReliquaryRuntimeHost as H;")
    assert FORBIDDEN.search("OwnerExecution::start(runtime)")
    assert not FORBIDDEN.search("ReliquaryRuntimeHostError")
    assert not FORBIDDEN.search("ReliquaryRuntimeRoutes")
    assert not FORBIDDEN.search("GatewayRuntime::new()")
    assert not FORBIDDEN.search(masked('// ReliquaryRuntimeHost\nlet s = "OwnerExecution";'))
    assert not FORBIDDEN.search(masked('let s = r#"ReliquaryRuntimeHost"#;'))
    fields, methods = legacy_items("pub struct ReliquaryRuntimeHost {\n field: Option<String>,\n}\nimpl ReliquaryRuntimeHost {\n pub fn new() { let s = \"}\"; }\n fn hidden() {}\n}\nimpl Drop for ReliquaryRuntimeHost { fn drop(&mut self) {} }")
    assert [f["name"] for f in fields] == ["field"]
    assert [m["name"] for m in methods] == ["new", "hidden", "drop"]
    assert policy(PREFIX + "_owner_publication.rs")["action"] == "retain-relocate"
    assert policy(PREFIX + "_gateway.rs")["action"] == "retain-canonical"
    assert policy(PREFIX + "_owner_execution.rs")["action"] == "delete-legacy"
    try:
        policy(PREFIX + "_unreviewed.rs")
    except ValueError:
        pass
    else:
        raise AssertionError("new module accepted without classification")
    assert policy(PREFIX + "_test_support.rs")["action"] == "split-test-support"
    assert policy(PREFIX + "_tests.rs")["action"] == "replace-legacy-tests"
    calls = candidate_calls({"src/test.rs": "host.\n run(\n); Host::run(); // host.run()\n"}, ["run"])
    assert len(calls["run"]) == 2
    assert [r["line"] for r in calls["run"]] == [2, 3]
    print("D0 scanner: 17 lexical, extraction, classification and multiline-call fixtures passed")


def format_inventory(value, level=0):
    """Keep leaf evidence rows on one line; share alias references once."""
    indent = "  " * level
    if isinstance(value, dict):
        if all(not isinstance(v, (dict, list)) for v in value.values()):
            return json.dumps(value, ensure_ascii=False)
        rows = ["  " * (level + 1) + json.dumps(k) + ": " + format_inventory(v, level + 1)
                for k, v in value.items()]
        return "{\n" + ",\n".join(rows) + "\n" + indent + "}"
    if isinstance(value, list):
        if all(not isinstance(v, (dict, list)) for v in value):
            return json.dumps(value, ensure_ascii=False)
        rows = ["  " * (level + 1) + format_inventory(v, level + 1) for v in value]
        return "[\n" + ",\n".join(rows) + "\n" + indent + "]"
    return json.dumps(value, ensure_ascii=False)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--baseline", default=DEFAULT_BASELINE)
    parser.add_argument("--write-baseline", action="store_true")
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--hard-cut", action="store_true")
    parser.add_argument("--warlock", type=Path, action="append", default=[])
    args = parser.parse_args()
    if args.self_test:
        self_test()
    if args.hard_cut:
        violations = hard_cut(args.repo, args.warlock)
        print(json.dumps({"hard_cut_violations": violations}, indent=2))
        return int(bool(violations))
    current = inventory(args.repo)
    current["consumers"] = {p.name: external(p, sorted(current["method_references"])) for p in args.warlock}
    baseline = args.repo / args.baseline
    if args.write_baseline:
        rendered = format_inventory(current)
        assert json.loads(rendered) == current
        baseline.write_text(rendered + "\n", encoding="utf-8")
    if args.check:
        expected = json.loads(baseline.read_text(encoding="utf-8"))
        # External current heads/imports are checked only when explicitly supplied.
        if not args.warlock:
            current["consumers"] = expected.get("consumers", {})
        if current != expected:
            print("D0 inventory drift: rerun source/caller review before updating the baseline", file=sys.stderr)
            return 1
    print(json.dumps({"base": current["base"], "rust_files": current["rust_files"],
                      "modules": len(current["modules"]),
                      "actions": dict(Counter(m["action"] for m in current["modules"].values())),
                      "legacy_methods": sum(len(m["legacy_methods"]) for m in current["modules"].values()),
                      "legacy_fields": sum(len(m["legacy_fields"]) for m in current["modules"].values()),
                      "cli_legacy_references": len(current["cli_legacy_references"]),
                      "consumers": {k: {"head": v["head"], "dirty": v["dirty"],
                                       "legacy_references": len(v["legacy_references"])}
                                    for k, v in current["consumers"].items()}}, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
