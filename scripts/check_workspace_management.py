#!/usr/bin/env python3
"""Keep workspace management a concrete immutable composition surface."""
from pathlib import Path
import importlib.util
import re
import sys

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("p4_coverage", ROOT / "scripts/check_multiplexing_phase4_coverage.py")
p4 = importlib.util.module_from_spec(spec)
spec.loader.exec_module(p4)

def check(source):
    source = p4.masked(source)
    errors = []
    forbidden = {"Cva", "Reliquary", "Phylactery", "InteractionRuntime", "ReliquaryRuntimeHost", "RelJournal", "SessionState",
                 "ConversationRegistry", "ConversationCoordinator", "StreamHub", "CapabilityRouter"}
    identifiers = set(re.findall(r"\b[A-Za-z_]\w*\b", source))
    errors += [f"management cannot own or expose {name}" for name in sorted(forbidden & identifiers)]
    if re.search(r"\bGatewayRuntime\s*::\s*(?:new|new_with_host)\s*\(", source):
        errors.append("management cannot construct another gateway")
    compact = re.sub(r"\s+", "", source)
    for field in (".cva", ".archive(", ".container", ".rel_executions", ".phy_executions", ".conversation.journal("):
        if field in compact:
            errors.append(f"management cannot inspect owner internals: {field}")
    # The only owner-handle closure is pure delivery. No validation closure may
    # grow a source reader, mutator, callback or returned handle.
    for name in ("with_rel", "with_phy"):
        for match in re.finditer(rf"\.{name}\(", compact):
            if not compact[match.start():].startswith(f".{name}(token,|_|Ok(value))"):
                errors.append(f"management {name} closure must be delivery-only")
    return errors

def self_test():
    fixtures = 0
    def expect(value):
        nonlocal fixtures
        fixtures += 1
        assert value
    for owner in ("Cva","Reliquary","Phylactery","InteractionRuntime","ReliquaryRuntimeHost","RelJournal","SessionState",
                  "ConversationRegistry","ConversationCoordinator","StreamHub","CapabilityRouter"):
        expect(check(f"use crate::{owner} as Alias;"))
    expect(check("GatewayRuntime :: new ();"))
    expect(check("GatewayRuntime::new_with_host();"))
    for source in ("runtime.cva.memory()", "runtime . archive ()", "gateway.rel_executions.read()",
                   "gateway.phy_executions.read()", "gateway.conversation.journal(owner)",
                   "runtime.container"):
        expect(check(source))
    for method in ("with_rel","with_phy"):
        expect(check(f"gateway.{method}(token, |owner| Ok(owner));"))
        expect(not check(f"gateway.{method}(token, |_| Ok(value))"))
    expect(not check('// Cva::new()\nlet note = "RelJournal";'))
    expect(not check("gateway.conversation_summaries_for(instance, owner);"))
    print(f"Workspace management guard: {fixtures} negative/positive fixtures passed")

def main():
    if "--self-test" in sys.argv:
        self_test()
    errors = []
    for path in sorted((ROOT / "src/runtime_inference").glob("workspace_management*.rs")):
        if path.stem.endswith("_tests"):
            continue
        errors += [f"{path.name}: {error}" for error in check(path.read_text(encoding="utf-8"))]
    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print("Workspace management guard: no raw owners, store reads, journal or second gateway")
    return 0

if __name__ == "__main__":
    raise SystemExit(main())
