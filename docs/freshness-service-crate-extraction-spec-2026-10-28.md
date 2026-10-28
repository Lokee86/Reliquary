# Freshness service crate extraction — implementation specification

**Status:** Specified; not implemented (2026-10-28).
**Repository / branch:** `Reliquary-crate-first` · `refactor/crate-first-restart`
**Baseline:** `7579b892` (Chronos correction committed).
**Target:** `crates/reliquary-freshness/` — currently a placeholder, to become the authoritative deterministic Freshness **service** crate.
**Scope:** Behavior-preserving, staged hard cut. This is **not** Freshness Store extraction, a policy change, or historical-format recovery. Preserve the unrelated uncommitted `docs/graph-crate-extraction-evidence-2026-10-26.md` edit.

## Purpose

Extract the deterministic Freshness service from the product root into a compiled library with a small, coherent public interface. The goal is local changes with local compile/test impact: reinforcement/propagation, semantic scoring and historical calculation belong together and depend **down** on their already-extracted data owners. A Freshness feature must not require modifying Container, Memory Core, Graph, the durable Freshness Store, or unrelated semantic services.

## Overview

The lower `reliquary-freshness-store` is already implemented, owns admitted `FreshnessRecord`/`FreshnessPolicy`, event receipts, journal/replay, semantic/import settlement and rewrite baselines, and has only Container as its internal Reliquary dependency. **Do not extract it again or move its persisted logic upward.**

The target `crates/reliquary-freshness/src/lib.rs` currently exports no implementation. Its service logic still compiles in root `src/freshness/`: `propagation.rs` (event frontier, strongest-path reduction, bounded workers), `semantic_policy.rs` (cosine/bonus/reduction), `historical_{planner,propagation,scorer,semantic,dream}.rs` (offline deterministic reconstruction) and `score.rs` (rebuildable boundary index). `src/freshness_due.rs` contains the volatile due dispatcher. `src/freshness/policy.rs` merely re-exports lower-store types.

The implementation already has usable narrow pure inputs: `MemoryAdjacency`, `CommunityMembership`, events, availability and policy. Yet `propagation.rs`, `historical_*.rs` and scheduling still import `crate::MemoryId`, `crate::CommunityId` or root `GraphRelation*` types, making them impossible to extract by merely moving files. Root runtime/facade callers supply validated owner cuts, select source/target Memories and persist the resulting store effects. Preserve that division.

The completed outcome is **one** Freshness service implementation with direct lower-crate dependencies and root adapters performing only actual cross-owner authorization, I/O, orchestration and result publication. No new model call, journal, runtime lane, graph index, generalized identity package or compatibility forwarding layer.

### Developer outcomes

- Modify propagation or semantic numerical policy inside `reliquary-freshness` and run focused crate tests without rebuilding an unrelated source owner to test the algorithm.
- Keep Memory publication, accepted-use and first-cycle Dream semantics stable; callers supply prevalidated snapshots, while the service evaluates deterministic effects.
- Keep offline historical planning and score production pure: no new Container access or authoritative reconstruction policy inside the service.
- Keep the due dispatcher rebuildable and bounded, with notifications triggered by existing owner activity, not a new activity source.
- Delete the root duplicate implementation; keep public product paths and persisted bytes unchanged.

## Governing ownership and contracts

| Source of authority | Required invariant |
| --- | --- |
| [Crate-first architecture](crate-first-architecture.md), [layout](crate-first-layout-spec-2026-10-25.md), [crate map](architecture/crate-map.json) | Service depends only downward; `reliquary-freshness-store` cannot depend back on service. No root-product dependency or cycles. |
| [Freshness subsystem](subsystems/freshness.md), [ADR 0039](decisions/0039-unified-deterministic-memory-freshness.md) | Signed score -100…+100, new +100, accepted-use +25, first-cycle Dream +50, -1/10 accepted REL turns; same-origin hop 5, outside hop 10; strongest-path and one event/Memory application. |
| [Live F8 semantic contract](freshness-live-semantic-policy-contract.md) | Intrinsic local-only semantic effects, cosine threshold 0.75, bonus 1…10, top-64, exact receipt identity, cohort exclusion, no reinforcement from passive reads or re-embedding. |
| [Historical production disposition](freshness-historical-production-disposition-2026-10-13.md), [historical manifest policy](freshness-historical-manifest-policy.md) | Explicit offline/exclusive REL reconstruction, authenticated provenance, accepted Archive activity order, pinned owner/graph/Community/vector evidence; no mounted or PHY feature added. |
| [Freshness Store extraction](freshness-store-crate-extraction-spec-2026-10-26.md), [evidence](freshness-store-crate-extraction-evidence-2026-10-26.md) | Store validates/applies submitted receipts and effects, owns canonical record/policy/wire identities, and must remain unchanged unless a narrow existing-contract defect is proven. |
| [Current limitations](current-limitations.md#crate-first-recovery-boundary) | Promoted `CVAFRS07` semantic owner recovery and the authoritative six-file corpus check remain separate, **unclaimed** acceptance gates. Do not add legacy readers or conflate passing synthetic tests with corpus admission. |

## Target architecture and exact source crosswalk

| Current location | Final owner / action |
| --- | --- |
| `src/freshness/propagation.rs` and its inline tests | Move to `crates/reliquary-freshness/src/propagation.rs`. Own `PropagationEvent`, `PropagationOutcome/Stats/Error`, adjacency/membership type shapes, single- and batched-event frontiers and worker policy. |
| `src/freshness/semantic_policy.rs` and its inline tests | Move to service. Own `SemanticPolicy`, `SemanticCurve`, candidate/target/effect value models, cosine/profile validation, deterministic local reduction and canonical digest **calculation**. Reuse version/cap constants owned by Store; retain digest goldens. |
| `src/freshness/historical_planner.rs` with tests | Move the **pure** timeline/availability planning, `SourceKey`, `HistoricalTimeline`, `PlannerError`; Archive activity sourcing remains with `Cva::historical_freshness_timeline`. |
| `src/freshness/historical_propagation.rs` with tests | Move import-only pinned-graph propagation, eligibility/authorized-bridge handling, event worker sharing and error types. Does **not** acquire owner grants. |
| `src/freshness/historical_scorer.rs` with tests | Move chronological event application `score_historical_records` and `HistoricalScoreEvent/Error`; calculations consume the Store's canonical `FreshnessRecord` and policy, never write a Store. |
| `src/freshness/historical_semantic.rs` with tests | Move source/vector evidence evaluation, event planning and hash identity; supplied source/target evidence must already be authenticated by the root's historical manifest/Import coordinator. |
| `src/freshness/historical_dream.rs` with tests | Move pure first-cycle event planning and ID generation; use Graph's canonical `GraphRelationKind/Origin` types; Dream/root still author first-pass proof and canonical duplicate representatives. |
| `src/freshness/score.rs` | Move only the **service-owned** `FreshnessBoundaryIndex` and `next_state_boundary{_with_policy}`, with their current tests. Replace root Memory IDs with the canonical Memory Core ID, and import record/score math from Store. Do not duplicate `FreshnessRecord`, `FreshnessState` or settlement arithmetic. |
| `src/freshness_due.rs` | Move `FreshnessDueDispatcher`, `FreshnessDue`, `FreshnessReversal`, `FreshnessRefresh` into a service module (e.g. `freshness_due`). Pure, volatile index; no persistence or owner activity source. |
| `src/freshness/policy.rs`, `src/freshness/mod.rs` | Retire the duplicated module declarations and one-line policy proxy. Re-export the canonical Store value types from the service's narrow `lib.rs` and make root `freshness` refer directly to the service crate. |
| `src/freshness/trace.rs` | **Remain root**, currently `freshness_trace`: optional operation-level timing/logging. The service must not depend upward on it or create an unnecessary telemetry trait. |
| `src/freshness_dream_journal.rs` | **Remain root Dream Store adapter**, including FirstPass proof and receipt conversions; moving it would violate Dream ownership. |
| `src/facade/cva_freshness*.rs`, `cva_historical_freshness.rs`, `cva_historical_import.rs`, `cva_semantic_{live,source,fragment}.rs` | **Remain root** as owner-authorized preparation, accepted-turn checking, admission, receipt/Container appends, historical freeze/commit, due notification buffer and conversions. Do not move `impl Cva` into the service. |
| `src/runtime_inference/{freshness_access,dream_freshness}.rs` | **Remain root** for runtime lock release/reacquire, Graph/Community snapshot collection, worker scheduling and accepted-result commit. Call the extracted service for pure evaluation. |
| `crates/reliquary-freshness-store/**` | **Unchanged durable owner**; no new store, wire format, birth logic or event-replay path. |

### Dependency and identity decision

`reliquary-freshness` may directly depend on **`reliquary-freshness-store`** (score, pinned policies, event math), **`reliquary-memory-core`** (the existing `MemoryId([u8;32])`), and **`reliquary-graph`** (the existing `CommunityId([u8;32])`, `GraphRelationKind/Origin`). These edges are already permitted by `crate-map.json`. Use `sha2` only where current semantic/historical hash production uses it. Do **not** import Container directly without demonstrated use; no Archive, Dream, Insomnia, Retrieval, Runtime, root product, Switchboard or vector provider dependency is needed for this pure engine.

The service's `MemoryId` and `CommunityId` must be the **existing canonical lower-owner types**, not freshly invented wrappers or copies. Keep `FreshnessMemoryId` solely at the already-established **store boundary**, with conversions only where accepted store evidence is prepared/consumed. Do not convert entire graph snapshots into a second ID representation merely for extraction. Verify collection ordering, hashes and event identity before deleting root modules.

The lower service **receives** materialized, authorized Memory adjacency and Community membership; importing Graph's type declarations does **not** authorize it to fetch Graph state or mutate relationships. It takes no broad `Cva`, owner mutable handle, service locator, boxed callbacks, or provider endpoint. Authorization and transaction ownership stay with callers.

### Public API cut

- Preserve the root's existing public `freshness::propagate*`, `freshness::semantic_policy::*`, `freshness::historical_*::*`, `FreshnessBoundaryIndex`, `FreshnessDue*`, `FreshnessReversal`, `FreshnessPolicy` and score/value exports wherever they are public today. Use direct re-exports/one crate alias, **not** forwarding functions or duplicate types.
- Root `src/lib.rs` can re-export the service module as `freshness`; avoid two authoritative `mod freshness` implementations. Keep existing product Cva method signatures and error mappings.
- Service exports only actual consumed value models and operations. Internal frontier variants, worker joins, policy helpers and index internals stay private/`pub(crate)`; promote a previously root-private batch method only as needed by current root callers.
- Historical public values such as `HistoricalDreamLink` retain semantic fields and accepted bytes/IDs, without a new serialized contract.
- Do not add a public versioned facade or compatibility API for the displaced root module.

## Implementation strategy — bounded hard cut

**Declare one temporary red migration window** from the first actual source move until all direct root consumers are rewired. No dual implementation, stub module with copied algorithms, temporary forwarding crate or generated call inventory. Work in these verifiable slices:

1. **F1 — Freeze contracts and boundaries.** Record HEAD/status, root `src/lib.rs` exports, actual `Cargo.toml` dependencies, service/store API shapes, touched consumers and current baseline tests. Freeze representative output for one-root and multi-root propagation (including Community boundary), F8 threshold/digest/top-64, historic timeline/score identities, due crossings, and Dream/use receipts. Preserve the Graph evidence edit. Determine whether any existing public re-export needs adjustment before moving it.
2. **F2 — Pure numerical engine.** Implement `reliquary-freshness` dependencies/exports and move `semantic_policy.rs`, `propagation.rs` and their tests **once**. Use Memory Core and Graph canonical IDs and the existing Store policy/score values. Validate exactly the same contribution maps, strongest-path ties, worker bounds and semantic digests. At this point root may temporarily be red; do not build an adapter to conceal that.
3. **F3 — Historical pure engine.** Move `historical_{planner,propagation,scorer,semantic,dream}.rs` and tests directly. Preserve planner input/error types, historical H1 authorization assumptions, event IDs, sorted event order, no-link/unsupported coverage, score remainder and import-only propagation rules. Historical Freeze/Manifest/Import orchestration stays at root. Confirm service has no IO or imports of product/Archive/Dream modules.
4. **F4 — Derived due scheduling.** Move boundary math/index from `score.rs` and volatile `freshness_due.rs`; preserve carry handling, exact boundary turns, deterministic ordering and bounded crossing/reversal drains. Leave `Cva::freshness_refresh_due`, accepted-activity triggering and notification buffers at root. Do not store a new notification queue or choose a clock inside the service.
5. **F5 — Direct consumer hard cut and deletion.** Rewire `runtime_inference/freshness_access.rs`, `dream_freshness.rs`, `facade/cva_freshness*.rs`, semantic F8, historical Import/Manifest, Dream and the public re-exports to the compiled crate. Remove the displaced root files/module declarations; keep `freshness_trace` and Dream journal owner. Check that no `src/freshness` algorithm survives and that all users link to the same canonical public types. Fix only necessary call-site type or visibility changes.
6. **F6 — Integration and close.** Run focused, workspace, root, separately locked CLI and documentation checks below. Verify Cargo dependency direction and old-path absence with ordinary source search/`cargo metadata`, not a new architecture analyzer. Update the current Freshness subsystem code map, architecture/maintainer maps and directly affected API/behavioral docs. Record evidence truthfully. Commit **only** the service extraction and directly owned documentation after passing gates; retain unrelated Graph work.

Keep the group order, but subdivide F2/F3 into tiny coherent compilation cuts if needed. Do not label a library extracted merely because its `Cargo.toml` and placeholder `lib.rs` exist. Maintain stable user-facing behavior while allowing an intermediate red workspace only inside the explicitly declared migration window.

## Verification and acceptance

### Focused lower-crate tests

- `cargo test -p reliquary-freshness --locked --offline -j 1`: migrated tests must cover score/due boundary math, nondefault policy, saturation/carry/overflow, deterministic traversal across original Community boundaries, strongest-path and cycles, exclusions, disabled self-reinforcement, stable worker count and batch order.
- F8: exact cosine validation, profile/generation rejection, fixed threshold, +1…+10 monotone cap, top-64 eligibility, strongest target reduction and pinned digest/identity goldens. Local semantic effects must never propagate.
- Historical H1–H8 pure tests: Archive-clock ordering supplied explicitly, availability and source-evidence failures, cohort exclusions, authorized future bridge vs ineligible recipient, event-ID byte equality, no-link/duplicate-proof precedence, deterministic per-recipient chronological settlement, policy rejection, exact identity under reordering.
- Volatile due/notification contract: `FreshnessBoundaryIndex` and `FreshnessDueDispatcher` reopen/rebuild from Store records, same-turn ties, missed boundaries, finite per-activity processing and reversal order; no durable notification schema.
- Compare frozen expected outputs before/after source relocation. Test at the **service** public interface, not by copying tests into both crates.

### Root consumer and durability tests

- Accepted-use handoff: passive search/internal retrieval produces **zero reinforcement**; only confirmed consumed Memory creates +25, complete strongest-path recipient set and one idempotent durable receipt; exact retry after reopen.
- Dream first cycle: +50 only for accepted initial Dream relation, frozen cohort/Graph/Community proof, zero-link settlement, duplicate representative, failure after prepare before commit, and repeat/reopen; no extra publication or spontaneous birth.
- Intrinsic F8: source fragment/cohort eligibility, prior-birth rule, top-64, profile rotation, completed-empty vs pending, no double credit, production worker and root source/readiness semantics.
- Historical Import/CVA: retained manifest, H1 source/graph/clock authorization, sparse event replay, exact native checkpoint, existing Memory preservation, disposal/reopen/prefix recovery and idempotence; no new mutable Historical Freshness API.
- Due engine: accepted REL turn alone advances derived boundaries, notifications remain bounded and survive rebuild, no scoring mutation from queries, REL/PHY isolation and lazy remainder.
- Root-facing `pub use` paths and CLI calls remain source-compatible; no new owner-level method or serialized field.

### Final integration gate

Run, record exit codes and report pre-existing exceptions explicitly:

```text
cargo test -p reliquary-freshness --locked --offline -j 1
cargo test -p reliquary-freshness-store --locked --offline -j 1
cargo check --workspace --all-targets --locked --offline -j 1
cargo test --lib --locked --offline -j 1
cargo test --manifest-path cli/Cargo.toml --locked --offline -j 1
cargo fmt --all -- --check
ddocs check --indexes --links
```

Also run focused root `freshness`, `semantic`, `historical`, `dream` and `agentic` suites and existing performance/receipt regression fixtures; do not expand test count mechanically. Use `rg`/Cargo metadata to show the final allowed library edges and absence of root implementation copies; use Arcana only if a fresh, affordable index exists. If architecture docs tooling is unavailable or graph evidence is stale, say so rather than generating replacement machinery or claiming those gates passed.

**Must not be called complete until:** a non-placeholder service owns all specified algorithms; the root's duplicate algorithms are deleted; the Store remains the sole durable Freshness owner; root consumers are integrated; current-format bytes/policy digests/event identities and error behavior match baseline; all applicable final gates are green; documentation/source maps reflect reality; no accidental public dependency on the root product exists. CVAFRS07 six-file corpus acceptance is **outside this extraction** and cannot be inferred from ordinary tests.

## Out of scope

No Freshness v2 calibration, threshold/principal/hop change, broader semantic discovery, new model/provider route, Graph cache, new lower store, persistent due queue, PHY Freshness activity clock, Ego policy changes, general historic mounted operations, reworked Dream duplicate policy, vector generation redesign, or new runtime workers. No external-format migration or compatibility reader, and no broad facade refactor to make unrelated Root modules vanish.

## Related docs

- [Freshness subsystem](subsystems/freshness.md)
- [Crate-first architecture](crate-first-architecture.md) and [physical crate map](architecture/crate-map.json)
- [Freshness Store specification](freshness-store-crate-extraction-spec-2026-10-26.md) and [evidence](freshness-store-crate-extraction-evidence-2026-10-26.md)
- [Live F8 semantic policy](freshness-live-semantic-policy-contract.md)
- [Historical Freshness disposition](freshness-historical-production-disposition-2026-10-13.md)
- [Behavioral contracts](behavioral-contracts.md), [Current limitations](current-limitations.md)

## Notes

This specification is an implementation target, not an acceptance report. The planned six slices are dependency stages, not six simultaneous independent tasks. No new storage format or public Memory-ID conversion layer is authorized. User-visible eligibility and score behavior must remain unchanged throughout the final integrated product.
