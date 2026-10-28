# Freshness service extraction baseline

Captured before F2 on 2026-10-28 for the service extraction specified in [Freshness service crate extraction](freshness-service-crate-extraction-spec-2026-10-28.md).

## Purpose

Freeze the source and behavioral evidence before extraction.

## Overview

This capture records the pre-move service boundary and representative contract tests.

## Repository state

- HEAD: `0cfba254b57525888cb76c676d85eabfd9403c29`.
- The specification names baseline `7579b892`; checked-out HEAD differs. This capture describes the actual checkout.
- Initial `git status --short`: only `M docs/graph-crate-extraction-evidence-2026-10-26.md`. This unrelated edit is preserved.
- No baseline commands modified tracked files.

## Ownership and public surface

- Root `src/lib.rs` declares `#[path = "freshness/mod.rs"] pub mod freshness`, re-exports `CommunityMembership`, `FreshnessBoundaryIndex`, Store-owned `FreshnessError`/`FreshnessRecord`/`FreshnessState`, `MemoryAdjacency`, `PropagationError`, `PropagationEvent`, and `propagate`; it separately declares/re-exports the root `freshness_due` module. It aliases `reliquary-freshness-store` as `freshness_storage` and converts root `MemoryId` values to Store `FreshnessMemoryId` at the owner boundary.
- `crates/reliquary-freshness/src/lib.rs` is a placeholder with no API or dependencies.
- `crates/reliquary-freshness-store/src/lib.rs` owns policy, score, wire policy and storage; exported values include `FreshnessRecord`, `FreshnessPolicy`, replay/receipt, semantic/import, and rewrite-baseline APIs. Store is the durable owner.
- Root `Cargo.toml` directly depends on `reliquary-freshness-store`, `reliquary-graph`, `reliquary-memory-core`, and `sha2`; these are the relevant currently available edges for the service extraction. Service and Store dependency manifests were inspected; Store must not depend on the service.

## Current service sources and consumers

- Algorithms still in root: `src/freshness/{propagation,semantic_policy,historical_planner,historical_propagation,historical_scorer,historical_semantic,historical_dream,score}.rs` and `src/freshness_due.rs`.
- Owner adapters to preserve include `src/runtime_inference/{freshness_access,dream_freshness}.rs`; `src/facade/cva_freshness*.rs`, `cva_historical_freshness.rs`, `cva_historical_import.rs`, `cva_semantic_{live,source,fragment}.rs`; root Dream journal and `freshness_trace`.
- Root public type exports and public Cva method signatures are current source-compatibility constraints. `src/freshness/trace.rs` remains optional root instrumentation; `src/freshness_dream_journal.rs` remains the Dream Store adapter.

## Existing baseline test evidence

The standalone policy oracle `tests/freshness_f1_reference.rs` was compiled with its documented `rustc --edition=2024 --test` command to `target/freshness_f1_reference.exe` and run: **17 passed, 0 failed**. This freezes admission/decay boundaries and remainders, event roots and source exclusion, multi-root strongest-path reduction, cycles, Community hop costs/boundaries, and no reinforcement from passive reads. The oracle is independent of production and is not evidence that the placeholder service crate is implemented.

Representative exact outputs needed for relocation are pinned by existing tests rather than copied into a second test harness: production propagation tests in `src/freshness/propagation.rs`; F8 policy and digest tests in `src/freshness/semantic_policy.rs`; timeline, score, event identity and import-propagation tests in the corresponding `historical_*_tests.rs`; due crossing tests in `src/freshness/score.rs` and root `freshness_due` tests; receipt and Dream contracts in root freshness owner/runtime/facade suites. F2/F3/F4 should compare these existing assertions at the extracted service boundary; no duplicate baseline implementation is introduced.

## Public re-export decision

No change is required at F1. Preserve the existing root `freshness` module path and root-level type/function re-exports by re-exporting canonical service crate values at the final hard cut. Preserve Store-owned receipt and persisted value types from the Store; do not add adapter functions or a second canonical type.

## Documentation impact

- Inspected: extraction specification, root `AGENTS.md`, root `src/lib.rs`, root and service manifests, Store exports, source modules and existing baseline tests.
- Updated: this F1 evidence record.
- Not affected: unrelated Graph extraction evidence edit; Freshness Store sources and persisted contracts.
- Compliance check: recorded actual HEAD/status and ran only the documented standalone oracle.
- Known documentation gaps: this is a baseline capture only; current subsystem maps should be updated after implementation reaches F6.

## Related docs

- [Freshness service extraction specification](freshness-service-crate-extraction-spec-2026-10-28.md)
- [Freshness subsystem](subsystems/freshness.md)

## Notes

The capture does not establish final extraction acceptance.
