# Phase 7 — Final dependency graph elimination and architecture acceptance

**Status:** Execution started; B7 remains open. Integrated Phase 6 source and bounded A6 receipts satisfy the Phase 7 execution prerequisite; the missing Phase 5 A5 full graph proof is an explicit A7 obligation. See the [execution evidence](dependency-convergence-phase7-execution-2026-10-24.md). No H7 implementation or A7 acceptance is claimed.
**Date:** 2026-10-23.
**Campaign:** [Dependency Convergence](dependency-convergence-architecture-refactor-2026-10-21.md).
**Implementation prerequisite (satisfied):** Phase 6 A6 accepted and integrated at `0495689`, with Phase 5 implementation/behavioral evidence integrated at `17514f0`. Phase 5 A5 **full architecture refresh was not accepted**; that proof is explicitly carried into Phase 7 A7, not a prerequisite to B7/H7/R7 execution. This is not a waiver: no final A7/refactor completion claim is allowed until the full graph-proof obligation is satisfied under the original architecture standard. If it cannot be proven, document the exact gap and leave A7 open. Base source cuts on the integrated Phase 6 checkpoint.
**Scope:** Convergence closure, residual dependency hard cuts, enforceable permanent source rules and integration. No new product features, general tooling redesign or storage format migration.

## Purpose

**Prove the actual production architecture converges to the adopted 34-component directed acyclic graph and remove every remaining unapproved dependency cycle/backedge.** A hand-authored DAG passing is not sufficient; assigning findings to an owner, suppressing warnings or shrinking a raw mixed test/analyzer SCC is not completion. Also demonstrate absence of owner→runtime/public upward reach-through and eliminate obsolete transition machinery. The architecture should become *simpler*, with fewer implementation backedges and no shadow owners, not merely more classified.

## Overview

The four gates below require evidence, conditional source cuts, permanent enforcement and final integration in that order. The accepted graph and behavioral contracts remain the acceptance standard.

## Inherited baseline and decision rules

- The accepted target `tools/pitlord/convergence-component-dag.json` contains 34 logical components and 163 explicitly justified permitted edges. No new allowed edge may be added simply to make current source pass.
- Phase 0's immutable before-image remains preserved; Phase 1–4 have bounded source ownership evidence; Phase 4 source snapshot and receipt live at `10cbe4b`. The integrated Phase 5 source/behavior checkpoint and accepted Phase 6 A6 source/behavior receipt are the **authoritative immediate before-image** for residual hard cuts. Phase 5 A5 full graph acceptance remains incomplete and must be discharged as part of A7, without assuming the stale analyzer snapshot is current.
- Prior source-import results (1,409 → 937 cycle-forming rows at the Phase 1–3 checkpoint) indicate movement **only in that relation set**. They do not prove complete SCC convergence; avoid misleading headline numbers.
- Arcana symbol relationships can include test-only, cfg-inactive, same-name inferred calls or immutable values. The previous file-node-only induced export is inadequate to prove symbol dependencies. **Do not fix this by relaunching full uncached exports or writing an unbounded graph-verification framework.**
- Component assignments must distinguish lower store/service logic from lower-directory public `impl Cva` extensions. Legitimate owner-qualified immutable DTO imports are separate from forbidden implementation capabilities; permitted value cycles should not be silently counted as executable callback cycles.
- Different evidence classes may be separately proven: Rust source imports from direct module resolver; high-confidence symbol calls from anchored source inspection; macro expansion / cfg variants where compilation profile matters; uncertain analyzer rows explicitly retained as uncertainty. Record coverage and do not equate a subset with an exhaustive proof.

## Invariants and acceptance policy

1. Actual compiled production implementation/capability edges obey allowed owner direction, with zero unapproved SCCs of size >1; self-loops representing implementation recursion are reviewed, not automatically owner-cycle violations.
2. Every approved cross-component source edge has a justified permitted target or exact immutable value contract; no widening blanket allowlist to hide debt.
3. No Container/owner store→service/runtime/facade, semantic domain→Gateway/public facade, Project Environment→runtime/facade or Context Engine→Gateway/Instance upward callbacks.
4. Gateway/Instance/REL/PHY retain one owner writer, independent instance grants and established queue, stream, publication/cancellation, host delegation and privacy behavior.
5. No obsolete forwarding/duplicated state or shim remains merely for a former architecture; retain supported public contracts and external persistent marker semantics.
6. No wire-format/product feature changes. Existing development format migration rules are triggered only if a separately approved unavoidable format change occurs.
7. Architecture enforcement runs reproducibly and affordably on changed-source/CI paths, with periodic full graph verification only when source/data evidence requires it.

## Execution plan — 31 bounded tickets, 4 gates

**Granularity contract:** The 16 P7.1 tickets below each own an identifiable architectural seam with a bounded source/owner scope and accepted test family. They are **conditional hard cuts**: B7 must provide an actual violating edge witness before code is changed; a clean seam closes as a verified no-op. An independent unexpected residual cycle cannot be squeezed into a catch-all; it needs its own named child ticket and explicit denominator reconciliation before code changes. No ticket can pass by classifying a violation while leaving it in place. Gates remain separate.

Phase 7 work is bounded by **demonstrated remaining violations**, not sweeping the repository or reopening accepted owner designs. No task receives a passing status simply for classifying an edge.

### P7.0 — Integrated checkpoint and residual source evidence (B7; 4 tasks)

- **P7.0.1 — Freeze integrated source provenance.** Record accepted Phase 6 A6 and integrated Phase 5 source/behavior commits, Cargo/profile/cfg, production module coverage and hash-pinned receipts, ignored tests and known platform limits. Explicitly name the incomplete Phase 5 A5 full-graph proof as an A7 carry-forward, not a B7 execution blocker.
- **P7.0.2 — Reconcile actual component mapping.** Audit unmapped or ambiguous *reachable production* modules; assign by responsibility using owner evidence. Test-only and excluded cfg files are not production holes.
- **P7.0.3 — Build comparable SCC/edge witnesses.** Use the same 34-component map and relation policy on Phase 0/bounded intermediate/accepted A6 sources; derive imports and confirmed implementation calls; retain analyzer/cfg uncertainty explicitly.
- **P7.0.4 — Exact residual register and ownership.** For every unapproved production cycle/backedge record both concrete source endpoints, capability type, owner, planned source cut and preserving test; report accepted immutable/type-only nonviolations separately.

**B7 exit:** Complete enumerated residual implementation-edge register with traceable source/cfg coverage and no undocumented waivers. If full program call coverage is unavailable, record the bounded proof limits explicitly; do not claim exhaustive graph coverage.

### P7.1 — Residual architectural hard cuts (H7; 16 bounded tickets)

The categories are fixed architectural seams, not a request to invent a violation. Each ticket starts with the B7 source witness register for exactly its own component pair or responsibility. **Where zero source-confirmed violations remain, close the ticket as a verified no-op.** Where one ticket contains multiple independent SCC components, split it at B7 before implementation and publish the denominator change. No blanket residual catch-all.

- **P7.1.01 — Container→semantic backedges.** **Scope:** `src/container/**` and physical owner guard; remove any confirmed upward callback, preserving inspect/repair and writer contracts. **Evidence:** exact violating callsite or negative guard.
- **P7.1.02 — Owner-store→orchestrator backedges.** **Scope:** registered store owner files and `owner-boundaries.json`; hard-cut runtime/facade reach-through only, preserving explicit immutable DTO contracts. **Evidence:** owner fixture and source callers.
- **P7.1.03 — Archive↔Memory implementation cycle.** **Scope:** Archive/Memory owner contracts and any B7 witness; separate immutable IDs from implementation calls. **Evidence:** exact cycle witness and owner tests.
- **P7.1.04 — Graph↔Perception implementation cycle.** **Scope:** Graph, transitional Relationship/Perception; remove cross-owner capability backreach, preserve Entity IDs. **Evidence:** bounded graph/perception tests.
- **P7.1.05 — Freshness↔Dream owner cycle.** **Scope:** freshness/dream stored state and owner-local proof; remove real cross-owner service callbacks if present. **Evidence:** current freshness/Dream acceptance tests.
- **P7.1.06 — Insomnia↔vectors/owner lifecycle.** **Scope:** completion codecs, profile/vector owner facts, lifecycle; retire real upward callbacks rather than immutable tags. **Evidence:** codec/worker tests and exact witnesses.
- **P7.1.07 — Ego or remaining semantic-owner backedges.** **Scope:** B7 Ego owner and any other exact semantic SCC not covered above. **Cut:** only source-confirmed cross-owner service calls; record each independent SCC as a child before implementation. **Evidence:** owner contract and affected semantic tests.
- **P7.1.08 — History→Import service callback.** **Scope:** `src/history/**`, import epoch store/model/codec; keep read-only proof contracts, remove only a confirmed callback to Import orchestration. **Evidence:** history guard and source trace.
- **P7.1.09 — Import→Maintenance and reconciliation callback.** **Scope:** Import settlement/coordination and Maintenance; remove surviving implementation cycles without reopening Phase 3 format. **Evidence:** import completion and maintenance/repack tests.
- **P7.1.10 — Project Environment→runtime/facade.** **Scope:** accepted `src/project_environment/**`; no return of Phase 4's `Cva` dependency or unvalidated backend root. **Evidence:** scope/owner and Git/Lore tests.
- **P7.1.11 — External Source→Gateway/domain callback.** **Scope:** `src/external_source/**`, Gateway external adapter; preserve host transcript authority without lower→host reach-through. **Evidence:** existing external gateway tests.
- **P7.1.12 — Runtime↔domain execution backedges.** **Scope:** Phase 5 `RelRuntime`/`PhyRuntime`, worker and gateway registries; eliminate actual owner-service→runtime cycles while preserving one mounted writer. **Evidence:** source witness, worker/multiplexing tests.
- **P7.1.13 — Provider/configured-runtime backedges.** **Scope:** `ConfiguredRuntime`, provider ports, lower profile/codec types; remove implementation direction inversion, preserve offline use. **Evidence:** endpoint/serialization tests.
- **P7.1.14 — Public facade→lower callback cycles.** **Scope:** Phase 6 `src/facade/**` and public extension consumers; no lower module depends on public product composition. **Evidence:** public API tests and reverse-edge checker.
- **P7.1.15 — Context Engine↔Gateway backedges.** **Scope:** context evidence/compaction and live Gateway callers; preserve current compaction semantics, remove context→runtime implementation references. **Evidence:** context checkpoint tests.
- **P7.1.16 — Obsolete internal adapters after final cuts.** **Scope:** exact B7 list of dead private forwarders, shadow owner selection/queue and old host aliases; delete only after last consumer migrated, leave supported public API. **Evidence:** deleted symbol and consumer register.

**H7 exit:** Each category has exact eliminated edge witnesses or proved absence. Any remaining unapproved implementation SCC blocks the gate even if its ticket was nominally completed.

### P7.2 — Permanent architecture ratchets (R7; 5 tasks)

- **P7.2.1 — Exact file/component ownership.** Lock mapping of compiled reachable production files; reject unregistered new production files and impossible/unknown owner classifications.
- **P7.2.2 — Source dependency policy.** Enforce approved owner directions/immutable DTO contract exceptions with grounded source endpoints; negative fixtures for forbidden upward imports, owner capability leakage and public extension placement.
- **P7.2.3 — Confirmed calls and uncertainty policy.** Validate real symbol-level capability calls where supported; classify macros/cfg/analyzer-only edges, preserve uncertain source references and fail acceptance on material unresolved unknowns rather than whitelisting them.
- **P7.2.4 — CI performance and deterministic receipts.** Make fast changed-source checks the default and a bounded full-source integration gate available; hash tool versions, cfg, map, inputs and evidence, no expensive all-node export loop.
- **P7.2.5 — Guard regression negatives.** Inject/revert representative cross-component illegal imports, callbacks, public extensions, cyclic component edges and stale exception entries; verify non-zero failures.

**R7 exit:** Reintroducing the eliminated coupling fails automated checks, while allowed immutable values and existing public API remain valid.

### P7.3 — Whole integration and disposition (A7; 6 tasks)

- **P7.3.1 — Full source-graph acceptance and Phase 5 A5 proof closure.** At one final checkpoint, resolve the inherited Phase 5 A5 architecture-refresh evidence gap with sufficient reproducible current source/cfg/symbol proof (without repeating the stalled analyzer blindly). Report actual component count, production coverage, authorized directed edges, SCCs, residual violations, ambiguous calls and before/after evidence; do not substitute target-DAG validation. If material graph proof remains unavailable, leave A7 and the overall refactor unaccepted.
- **P7.3.2 — Full locked build/test acceptance.** Run required formatter/build/CI configuration and one integrated library/CLI/integration regression pass; log pass/fail/ignored and exact scope, use stored receipts for unchanged corpus.
- **P7.3.3 — Data and behavioral invariants.** Verify unchanged wire format/corpus and scoped privacy, queue, stream, host/project semantics; only focused replay for genuinely changed owners.
- **P7.3.4 — Documentation and roadmap reconciliation.** Update canonical architecture, module/maintainer map, invariants, subsystem docs and phase summaries; remove stale descriptions but retain decisions and accepted historical receipts.
- **P7.3.5 — Worktree/branch integration audit.** Reconcile every active refactor worktree/hunk, integrate or explicitly discard with evidence, delete temporary generated verification machinery, commit final implementation; confirm no hidden unmerged accepted work.
- **P7.3.6 — Formal closeout and Phase 8 prohibition.** Publish final measured architecture outcome and remaining independently scoped roadmap work. No "bounded closure" substituted for original graph contract; if an acceptance condition cannot be proven, mark it open and name exact evidence needed rather than claiming the refactor complete.

**A7 exit:** Source-proven allowed architectural DAG, tested accepted behavior, permanent ratchets, canonical docs and integrated committed HEAD. Any new feature work remains on the normal roadmap.

**Denominator:** P7.0 4 + P7.1 16 + P7.2 5 + P7.3 6 = **31 bounded tickets**; gates **B7, H7, R7, A7 = 4**. Baseline preparation is committed at `89de02d`; no gate is accepted and no H7 hard cut is implemented. B7 binds each conditional ticket to actual source witnesses or a documented zero-finding disposition; it does not license vague implementation bundles.

## Sequencing and cost

```text
Phase 5 integrated source + accepted Phase 6 A6
                                    |
                                    v
                           B7 residual register
                                    |
                                    v
                          H7 actual hard cuts
                                    |
                                    v
                           R7 permanent guards
                                    |
                                    v
                       A7 final full acceptance
```

Run direct source inspection and focused guard tests during each cut. Recalculate a comparable production graph **once at B7 and once at A7** (unless source changes invalidate a named intermediate proof). Do not rerun full cargo, Arcana or retained-corpus tests after every edit. Avoid parallel Cargo builds on shared target artifacts. Source ownership, `lib.rs`, policy manifests and generated index have one integration owner at a time. Snapshot/checkpoint names are immutable for comparisons; derived receipt content is committed with final changes.

## Explicit failure states

Unverified symbol calls are not automatically violations, but unresolved **material** reachability cannot be called complete. Known Phase 5/6 architecture violations cannot be waived as future work. Source-graph gaps and analyzer ambiguity are not invitations to move the acceptance standard. If final proof is obstructed by tooling, either use concrete manual source/cfg evidence sufficient for that claim or record the exact unproven part and leave A7 open. This refactor ends at Phase 7; separate future features do not create a Phase 8.

## Related docs

- [Campaign](dependency-convergence-architecture-refactor-2026-10-21.md)
- [Execution evidence](dependency-convergence-phase7-execution-2026-10-24.md)
- [Architecture](architecture.md)

## Notes

Execution evidence preserves the original final architecture acceptance requirements. Accepted Phase 6 A6 and integrated Phase 5 source/behavior evidence authorize B7/H7/R7 work; Phase 5's incomplete full graph proof is an A7 closure obligation, not a circular prerequisite. No source evidence or prerequisite correction waives that proof.
