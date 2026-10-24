# Phase 6 — Public façade and API dependency convergence

**Status:** Source implemented; acceptance pending the [execution receipt](dependency-convergence-phase6-execution-2026-10-23.md).
**Date:** 2026-10-23.
**Campaign:** [Dependency Convergence](dependency-convergence-architecture-refactor-2026-10-21.md).
**Execution prerequisite (satisfied for B6):** Phase 5 implementation integrated into `main` at `9a3678f`, final evidence recorded at `17514f0`, with source/consumer handoff and passing Rust/CLI/corpus/documentation checks. Phase 5 **A5 is explicitly not fully accepted** because the full architecture refresh produced no snapshot. That evidence gap is **not an implementation blocker for Phase 6 B6–A6**; carry it as an open, named graph-proof obligation into Phase 7 A7. Do not claim A5 acceptance, invent an architecture snapshot, waive the final DAG requirement, or repeatedly run the stalled analyzer. Phase 6 must establish its own actual source-edge evidence and gates.
**Scope:** Correction-only public boundary migration. No new APIs, features, storage formats, providers, or semantics.

## Purpose

Remove remaining upward dependencies on the public `Cva`/`Phylactery` façade from owner stores, domain services, Project Environment and Runtime internals; put public extensions and reexports in the correct upper composition layer; remove obsolete internal forwarding. Keep supported standalone/offline, mounted Gateway/Instance and CLI consumer behavior. **This phase is about actual removal of dependency edges, not merely assigning their debt to Phase 6.**

## Overview

The [execution receipt](dependency-convergence-phase6-execution-2026-10-23.md) tracks source implementation, expanded bounded children, verification and any unaccepted gates. This specification remains the acceptance authority.

## Frozen evidence and known seams

- Phase 4 integrated at `10cbe4b`: `project_environment/lifecycle.rs` was moved to `src/facade/cva_project_environment.rs`; validated project scopes now belong below the façade. Do **not** duplicate or undo this cut.
- Phase 5's current specification defines 45 tasks and an A5 handoff containing surviving `Cva`/`Phylactery` consumers, `lib.rs` reexports, and the final runtime execution seams. **At B6, re-inventory from integrated Phase 5 HEAD, not today's 309-file/runtime snapshot.**
- `tools/pitlord/convergence-phase6-perception-facade-import-closure.json` identifies **82 classified current source-import edges** from public-consumer Perception façade extensions to Perception implementation. These are a **starting cut register**, not proof that all 82 remain at B6, nor permission to waive their removal.
- Source-confirmed examples include `src/perception/entity_audit.rs` and `entity_reconciliation.rs` using macro-generated `impl Cva`/`impl Phylactery`; `src/archive/archive_search.rs` declares `impl Cva`; Retrieval also hosts public `Cva` extensions in lower-looking paths.
- Existing public `src/facade/**` and `src/lib.rs` exports, `ConfiguredRuntime` offline methods, Gateway instance API, CLI and external consumers are behavior contracts. Inventory before deleting or moving signatures.
- The source-component map is 34 logical components; directory names alone do not prove ownership. Immutable identities/value DTOs must remain owner-qualified, not be scattered into generic `shared` modules to conceal coupling.
- The retained Arcana graph has analyzer/cfg uncertainty; the induced file-node export has no complete symbol-call proof. Use direct source/callsite ownership checks, a consistent production-only before/after edge view and bounded negative fixtures. Never count edge disposition as edge elimination.

## Invariants

1. Owners, History, Import, Maintenance, Project Environment, semantic services and storage **do not depend upward on public product `Cva`/`Phylactery` composition or Gateway presentation**.
2. `Cva`/`Phylactery` remain supported public standalone entrypoints where currently contracted; this is **not** a mandate to remove the types.
3. Public extension implementations live in the façade/public-consumer tier, delegating directly to narrow accepted domain/owner contracts. No retained lower-layer façade callback or bridging stack.
4. Runtime hard cuts completed in Phase 5 remain intact; do not introduce a façade middleman between Gateway and owner execution.
5. Preserve REL/PHY durable semantics, Error/result behavior, privacy authorization, snapshot ordering, public method effects, source format and existing CLI behavior.
6. No generic `FacadeService`, additional runtime, duplicate store, blanket compatibility layer or extensive artificial test multiplication. Hard-cut internal callers and immediately delete obsolete paths.
7. Phase 6 owns public surface/extension placement. Phase 7 owns final cross-phase graph elimination and permanent policy ratchet, not deferred Phase 6 implementation.

## Ownership target

```text
Host / CLI / supported public methods (Cva, Phylactery)
                    |
                    v
Gateway / Instance / public offline composition
        |                    |
        v                    v
owner execution          lower domain service
        \                    /
         v                  v
             owner contracts
                   |
                   v
              owner stores
                   |
                   v
               Container
```

A public convenience method may orchestrate accepted downward capabilities, but a lower owner module may neither import its implementation nor call back through it. A file under `src/perception/` that implements `Cva` is a public-consumer responsibility even if it currently shares lower-level source location; moving that extension alone does **not** move Perception's durable logic.

## Implementation plan — 35 bounded tasks, 5 acceptance gates

**Granularity contract:** Each numbered ticket has one named source family, concrete migration and protecting evidence. The residual census ticket only verifies additional domains; any actual newly discovered independent migration must be written as a bounded child before starting it, with a recorded denominator change. Gates do not count as implementation tickets; passing checks do not substitute for deleting forbidden dependencies.

### P6.0 — Integrated Phase 5 handoff and final cut register (B6; 4 tasks)

- **P6.0.1 — Integrated baseline and trace.** Pin Phase 5 implementation/receipt commits (`9a3678f`, `17514f0`), format, public exports, available source-guard outputs and supported host/CLI/offline behavior. Record incomplete A5 full architecture refresh as an explicit limitation; do not block B6 on it.
- **P6.0.2 — Exact extension census.** Enumerate production `impl Cva`, `impl Phylactery`, public façade constructors, reexports and current external/CLI callers; separate test-only from production.
- **P6.0.3 — Lower-to-public reverse-edge register.** Reconcile Phase 6's 82 Perception source rows against integrated Phase 5 HEAD, inspect Archive/Retrieval/Graph/Ego/Freshness/History/Project/Import candidate calls, classify owner vs composition per file; attach actual witnesses.
- **P6.0.4 — API preservation manifest.** Name public signatures, serialized IDs, error contracts, documented entrypoints and deprecations that truly have consumers; distinguish obsolete *private* forwarding from supported surface.

**B6 exit:** Every scheduled public boundary cut has actual file/symbol/consumer owner; exact Phase 5 contracts frozen. No unknown side-work hidden in the 35 named tickets.

### P6.1 — Public façade placement (F6; 17 bounded implementation tickets)

Each ticket owns one public-method family or tightly coupled macro family, its caller update, the underlying domain-service extraction if required, and deletion of obsolete registration. The B6 census confirms exact remaining imports; a moved implementation is accepted only when the old lower→facade dependency actually disappears.

- **P6.1.01 — Perception audit entrypoint.** **Source:** `src/perception/entity_audit.rs` and current macro registration. **Cut:** public `impl Cva/Phylactery` to `src/facade/`, audit computation remains lower. **Evidence:** exact method references; audit tests; removed old registration.
- **P6.1.02 — Perception reconciliation entrypoint.** **Source:** `src/perception/entity_reconciliation.rs`. **Cut:** relocate public macro methods, retain reconciliation policy; migrate callsites and delete old link. **Evidence:** reconciliation tests and reverse-edge diff.
- **P6.1.03 — Perception principal entrypoint.** **Source:** `src/perception/entity_principal.rs`. **Cut:** inspect macro-generated/public binding and move only its public owner methods, keeping identity rules Perception-owned. **Evidence:** principal/privacy behavior tests.
- **P6.1.04 — Resolution batch façade.** **Source:** `src/perception/entity_resolution_batch.rs`. **Cut:** extract public composition only where the B6 symbol census proves it; keep lower batch policy. **Evidence:** exact binding and batch tests.
- **P6.1.05 — Resolution prepare façade.** **Source:** `src/perception/entity_resolution_prepare.rs`. **Cut:** migrate any public method extension, retain owner preparation logic. **Evidence:** references, compile and resolution tests.
- **P6.1.06 — Resolution commit and owner binding.** **Source:** `src/perception/entity_resolution_commit.rs`, `entity_resolution_owner.rs`. **Cut:** single atomic public commit/owner call boundary; no duplicate mutation coordinator. **Evidence:** commit/retry and owner tests.
- **P6.1.07 — Resolution processor façade.** **Source:** `src/perception/entity_resolution_processor.rs`. **Cut:** ensure processor calls lower owner contracts rather than the public product facade; move public glue if present. **Evidence:** processor test and source edge trace.
- **P6.1.08 — Reconciliation prepare.** **Source:** `src/perception/entity_reconciliation_prepare.rs`. **Cut:** separate public orchestration from lower candidate preparation. **Evidence:** caller trace and existing reconciliation tests.
- **P6.1.09 — Reconciliation commit/owner boundary.** **Source:** `src/perception/entity_reconciliation_commit.rs`, `entity_reconciliation_owner.rs`. **Cut:** public commit routing up, mutation authority below, no callback. **Evidence:** transaction/reconciliation tests.
- **P6.1.10 — Perception candidate exposure.** **Source:** `src/perception/entity_candidates.rs`. **Cut:** classify reexports and public-consumer dependencies; remove genuine facade reach-through only, preserve domain-owned candidate logic. **Evidence:** exact symbol witness and source guard.
- **P6.1.11 — Archive search public method.** **Source:** `src/archive/archive_search.rs`. **Cut:** move `impl Cva` up, retain Archive search/read implementation below and preserve search errors. **Evidence:** archive search tests.
- **P6.1.12 — Retrieval conversation search.** **Source:** `src/retrieval/conversation_search.rs`. **Cut:** move `Cva` public entrypoint, leave retrieval policy below. **Evidence:** conversation search tests.
- **P6.1.13 — Retrieval lexical search.** **Source:** `src/retrieval/lexical_search.rs`. **Cut:** review and extract only facade-dependent public glue; preserve lexical policy. **Evidence:** lexical search tests.
- **P6.1.14 — Retrieval semantic and memory search.** **Source:** `src/retrieval/semantic_search.rs`, `memory_search.rs`. **Cut:** related public query/owner admission binding moved above retrieval; leave independent ranking/selection below. **Evidence:** search semantics and permission tests.
- **P6.1.15 — Retrieval search and ranking.** **Source:** `src/retrieval/search.rs`, `search_rank.rs`. **Cut:** remove remaining real public-composition dependency without changing scoring/ranking. **Evidence:** search rank fixtures.
- **P6.1.16 — Residual named-domain extensions.** **Source:** B6 exact census of other areas (Dream/Freshness/Ego/Graph/Import/History). **Cut:** inspect each source-confirmed candidate, and **if multiple independent cuts remain, create explicit child tickets before execution**; if none, record zero and the evidence. Not permission for an unbounded cross-domain rewrite. **Evidence:** complete census with signed-off nonviolations.
- **P6.1.17 — Retire 82-edge Perception deferral register.** **Source:** `convergence-phase6-perception-facade-import-closure.json`, production component map and moved macro registrations. **Cut:** reconcile each original edge to absent, or to a genuinely nonviolating owner-qualified value. **Evidence:** exact before/after import witnesses; zero deferred implementation backedges.

**F6 exit:** All independently named Perception/Archive/Retrieval public cuts are finished; every original Perception deferral row has an evidence-backed disposition, and no discovered extra domain is hidden in the residual ticket.

### P6.2 — Public composition and supported consumers (C6; 6 tasks)

- **P6.2.1 — `Cva` composition and construction.** Make public construction/open/replay invoke accepted owner-lifecycle and owner read/proof contracts without lower owner calls to `Cva`.
- **P6.2.2 — `Phylactery` composition.** Preserve PHY privacy and identity; remove lower service callbacks through `Phylactery`.
- **P6.2.3 — Public project/environment composition.** Verify Phase 4's `cva_project_environment` stays upper and only passes validated scopes/accepted facts; no Path/repository bypass.
- **P6.2.4 — Gateway and offline public consumers.** Verify Phase 5's mounted execution and `ConfiguredRuntime` offline calls use canonical owners; remove leftover private facade-through-runtime invocation.
- **P6.2.5 — API/reexports and CLI.** Rehome exports in `src/lib.rs` without breaking supported imports/signatures; delete only proven obsolete internal aliases or forwarding.
- **P6.2.6 — Retire transitory façade/host shims.** Remove actual dead wrappers, compatibility names and unused module files after consumer updates; report exact preserved public exceptions.

**C6 exit:** Supported APIs remain behaviorally equivalent; no real owner/service→facade callback survives.

### P6.3 — Source conformance and regression (D6; 5 tasks)

- **P6.3.1 — Component ownership map delta.** Update exact moved/deleted files in `production-components.json` and owner/source guards; don't remap unrelated repo paths.
- **P6.3.2 — Actual Phase 6 edge elimination.** Compare same-map production import/call witnesses before/after; count eliminated, retained immutable, analyzer-uncertain and still violating edges separately.
- **P6.3.3 — Negative architecture ratchet.** Forbid new lower→public dependencies, enforce public-extension placement and fail closed on unknown reverse edges; keep precise bounded exceptions only when unavoidable and source-proven.
- **P6.3.4 — Focused behavior preservation.** Existing Perception/Archive/Retrieval/Cva/PHY and configured/runtime/CLI/API integration tests for changed seams; no new feature suite.
- **P6.3.5 — Cross-domain integration gate.** Run one consolidated focused set once after cuts stabilize; retain actual failed/ignored/timeouts and resolve introduced regressions.

**D6 exit:** All registered Phase 6 reverse-edge violations eliminated, source rules enforce nonrecurrence and supported behavior preserved. Do not accept solely from the target DAG being acyclic.

### P6.4 — Acceptance and Phase 7 handoff (A6; 3 tasks)

- **P6.4.1 — Final build/regression/corpus gate.** At single integrated checkpoint run required formatter, locked builds/tests, affected corpus and current-format policy; reuse unchanged-input verified receipts.
- **P6.4.2 — Canonical docs/API maintenance.** Update `architecture.md`, `api.md`, maintainer map and touched subsystem documents with exact new ownership; run documentation policy once.
- **P6.4.3 — Integration/handoff.** Commit all Phase 6 code, migrate-forward independent worktree changes, enumerate accepted API, exact remaining non-Phase-6 component cycles, source provenance and remaining uncertainties for Phase 7.

**A6 exit:** Accepted, integrated source+test+API receipt on main; no Phase 6 ownership debt silently reclassified to Phase 7.

**Denominator:** P6.0 4 + P6.1 17 + P6.2 6 + P6.3 5 + P6.4 3 = **35 named tickets**, with **5 gates** (B6, F6, C6, D6, A6). The original specification began with none implemented or accepted; current task/gate disposition and added bounded children are recorded in the execution receipt. B6 verifies that the integrated Phase 5 source still matches the named cuts; genuine newly discovered independent work requires an explicit child ticket before implementation, not an open-ended task. `lib.rs`, macro registries and public exports have one integration owner.

## Verification budget and stop conditions

Prefer directly inspecting concrete source edges and focused existing tests per cut. Run one broad locked regression only at A6; no overlapping Cargo builds on one target dir, repeated whole Arcana exports or expensive rescan loops. Use identical file→34-component mapping/cfg and relation selection before/after, and show witnesses for every retained violating SCC. If symbol analyzer coverage is incomplete, label exact evidence incomplete; **never turn unproven claims into passed gates**. Stop only for an actual new wire/public contract change requiring authorization, an ownership ambiguity blocking a safe cut, or an observed regression; record source and next owner.

## Non-goals and Phase 7 boundary

This phase does not add new inference routing, Ego/context behavior, Observation, new REL/PHY APIs, Cloud sync, or format migration. Phase 7 should receive the architecture after Phase 6's *real* public cuts, not be asked to finish those cuts by renaming their debt.

## Related docs

- [Execution receipt](dependency-convergence-phase6-execution-2026-10-23.md)
- [Architecture](architecture.md)
- [Public API](api.md)

## Notes

Source implementation and acceptance are tracked separately; source moves do not establish dependency elimination.
