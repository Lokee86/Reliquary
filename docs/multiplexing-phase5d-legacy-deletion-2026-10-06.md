# Multiplexing P5-D — final legacy-host hard cut and convergence

Parent: [P5/6 parallel plan](multiplexing-phase5-parallel-execution-plan-2026-10-06.md)  
Status: **D0 inventory is complete on `feat/multiplexing-p5d-inventory` from N0 `999fbf7`; see [deletion manifest](multiplexing-p5d-deletion-manifest-2026-10-06.md). D1–D4 remain blocked on accepted C closure plus E/F convergence. G must prove any framework-neutral public-runtime contracts are integrated or independent of the legacy host; completion of unavailable host-specific ACP/Hermes adapters is not a deletion blocker.**

## Purpose

Remove the obsolete ReliquaryRuntimeHost singleton/ambient execution path after all legitimate internal and external callers have migrated to the canonical gateway/instance system. Preserve semantic data, accepted durability and actual external contracts. Do not keep a mirrored host as a compatibility facade.

## Overview

The merged P3/P4 runtime exists alongside a separately instantiated old ReliquaryRuntimeHost. Runtime_host.rs still wires old modules, mixed profile/PHY fields and legacy managed selection. Old modules include runtime_host_owner_execution.rs, runtime_host_session.rs, runtime_host_mount.rs, runtime_host_insomnia.rs, runtime_host_dream*.rs, runtime_host_perception*.rs, runtime_host_vectors.rs, runtime_host_phylactery.rs and multiple legacy Archive/semantic/project helpers. New gateway modules have already split REL/PHY execution, personal routing, journal, host routing, subscriptions and streams.

No file is deleted merely because its name starts with runtime_host_: gateway.rs uses the same prefix and final reused semantic helpers may earn their own module. Determine deletion through reverse callers and ownership, not filename globbing. Some shared error/report/query types currently live at the legacy root; move surviving types to the canonical module instead of keeping an old host-shaped API.

## Preconditions (hard blockers)

1. A's exact owner/operation migration matrix accepted and negative authorization tests green.
2. B's worker/capability matrix accepted; no canonical worker depends on old Shared or ambient state.
3. E's transport-neutral runtime surface accepted; no host integration still requires legacy session/runtime authority.
4. F's workspace-management surface accepted for all management calls required by migrated consumers; no raw mutable store escape remains.
5. G's framework-neutral import/external-source contracts are integrated where they intersect the public runtime; unavailable external adapters are explicitly recorded rather than hidden.
6. C's migrated Warlock actually builds/tests pinned to accepted A+B+E+F plus applicable G integration, with one Arcana identity and no old-host imports. CLI checked or explicitly unaffected.
7. P6-H failure fixtures and baseline runner available.
8. Readable REL/PHY formats and authenticated external contracts identified; no undocumented assumptions of public compatibility.

If any real consumer remains, stop deletion and route it to its owning lane. Do not add a compatibility wrapper.

## Implementation steps

### D0 — reverse dependency and deletion manifest
Run Rust/source searches plus actual import inventory, not only Arcana graph. For each old root module, list in-repo callers, Warlock/CLI calls, tests, on-disk data touchpoints and final destination. Separately classify:
- delete old host only;
- keep and relocate common reusable data/report type;
- retain narrowly scoped standalone maintenance functionality with no mutable gateway duplicate;
- maintain required on-disk decoder (not a legacy runtime).

Create docs/multiplexing-p5d-deletion-manifest-2026-10-06.md with no-unaccounted-callers gate. Use structural graph only if current; previous Lexicon refresh timed out.

### D1 — root/canonical interface merge
Integrate A and B source into one Reliquary convergence branch; wire modules and public exports exactly once. Expose only narrow gateway/instance operations needed by C. Rename/relocate legacy-host-named common errors if they would otherwise force the old type shape to survive. No forwarding aliases between two runtime hosts. Test canonical GatewayHostAdapter and lifecycle while old host is still present but never used by migrated consumer.

### D2 — remove legacy runtime and old-only helpers
Delete ReliquaryRuntimeHost struct, constructor/public exports, global active REL/PHY, mutable phylactery and mixed profiles, global managed session and old worker/router/mount duplication. Delete old-only files and forwarding modules after verifying no retained Rust caller. Relocate genuine reusable domain queries into their semantic owner without changing storage authority.

Do not delete P3 coordinator/journal, P4 sequencer/stream, final RelRuntime/PhyRuntime, any real persisted reader, or data invariants. Preserve one owner for each live REL/PHY and one final mount fence.

### D3 — test/guard hard cut
Replace legacy-host characterization that encodes ambient behavior with equivalent GatewayRuntime/InstanceRuntime tests; delete only truly obsolete fixtures. Add negative build/source guards for old host imports and duplicate owner paths. Confirm no second journal writer, branch coordinator, personal worker set or raw app-managed InteractionRuntime writable session.

### D4 — docs and release handoff
Update architecture, api, behavioral-contracts, maintainer-map, documentation-coverage, current-limitations, roadmap and ADR 0037 singleton status, retaining history/rationale. Publish accepted deletion manifest and full test results to P6 final gate.

## Integration tests

- two principals, one REL, separate PHYs and branch selections;
- two instances, same principal/PHY, one personal worker and one durable queue;
- old public host symbols fail to compile/use in migrated consumers, new API passes;
- shutdown/reopen host debt/revocation/stream epoch behavior;
- private PHY bytes never appear in REL broad distribution;
- interruption, Insomnia routed publication and Archive idempotent finalization survive deletion;
- Warlock upgraded pinned build + CLI locked tests succeed.

## Acceptance gates

No reachable old runtime authority, no old exports/imports in real consumers, no compatibility alias, no unsupported format removal, one canonical REL/PHY worker set. Required locked compile, fmt, library/integration/tests/examples, CLI, docs checks and migration guards must pass or carry explicit new regression blockers. P6 gives the final stress/performance/release verdict, not D alone.

## Out of scope

Changing runtime behavior or model semantics to make deletion convenient, rebuilding stores, speculative optimization, and forcing external public compatibility absent proven dependency.

## Documentation and architecture impact

D owns canonical current-state and ADR updates only after source removal. Architectural rule: delete obsolete internal shapes instead of preserving both; keep a compatibility reader only for concrete persisted bytes or documented external data.

## Related docs

- [Parallel execution](multiplexing-phase5-parallel-execution-plan-2026-10-06.md)
- [Phase 0B G6 consumer inventory](multiplexing-phase0b-consumer-verification.md)
- [P3/P4 main integration](multiplexing-p34-main-integration-2026-10-06.md)

## Notes

A clean build cannot prove that every out-of-tree caller migrated. Verify Warlock/CLI directly; preserve concrete persisted formats.

