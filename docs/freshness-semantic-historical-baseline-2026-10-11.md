# F8 + historical reconstruction baseline — 2026-10-11

Parent: [Documentation index](INDEX.md).

## Purpose

Record Step 1's source/test baseline for the ordered F8 and historical reconstruction implementation. This note freezes the current branch before implementation.

## Overview

**Status:** Step 1 complete. The isolated baseline passed 28 Freshness library tests; no F8 semantic reinforcement implementation exists at this revision.

### Branch and baseline

- Branch: `feat/freshness-semantic-historical-linear`
- Base: local `main` at `a32d3357be9b6adb732f799bf28fc0b953a5e667` (commit adding the linear plan).
- Worktree: `C:\!bin\workspace\Reliquary-freshness-semantic`.
- The original `main` worktree had one unrelated uncommitted edit to `docs/multiplexing-implementation-plan.md`; it was left untouched. The step branch began clean.
- Full baseline command: `cargo test --locked -j 2`. Workspace process exited `4294967295` during dependency compilation after 174 seconds, with no compiler diagnostic or tests run.
- First focused attempt, `cargo test --locked freshness_storage:: -j 2 -- --nocapture`, exited 101 during dependency compilation because `tonic-prost-build` could not find `prost_build`/`tonic_build`; no tests ran in that attempt.
- Passing focused command: `cargo test --locked --lib freshness:: -j 2 -- --nocapture`, with `CARGO_TARGET_DIR=C:\Users\archa\AppData\Local\Temp\reliquary-r3-verification` and `CARGO_PROFILE_TEST_DEBUG=0`. Exit 0; 28 passed, 0 failed, 1,057 filtered out. The 28 include Freshness score/policy/propagation unit tests and Freshness facade R2/recovery tests. Build emitted existing dead-code warnings only.

### Current F8-relevant seams

- **Freshness score and policy:** `src/freshness/score.rs`, `src/freshness/policy.rs`, `src/freshness/mod.rs`. `FreshnessRecord` already owns bounded score, REL admission/accounting anchors, lazy decay, and local `reinforce_with_policy`; lifecycle boundaries are derived.
- **Freshness durable owner/journal:** `src/freshness_storage.rs` plus `src/freshness/freshness_storage_{membership,ordering,rebuild,stream}.rs`. Existing records, event identities/proofs, accepted-use receipts, replay, and bounded retry handling live here.
- **Cva composition and reconciliation:** `src/facade/cva_freshness.rs`, `src/facade/cva_freshness_due.rs`, `src/facade/cva_reconcile_freshness.rs`; Dream publication integration is in `src/facade/cva_memory_publish.rs` and `src/runtime_inference/dream_freshness.rs`.
- **Accepted-turn seam:** `src/facade/cva_turn_ingest.rs` delegates to `src/archive/turn_ingest_store.rs`. The accepted ingest result is a node plus attachments; `ingest_turn_with_receipt` advances existing Freshness scheduling from the REL activity count. It does not supply semantic fragment vectors or produce an F8 event. Fragment/vector generation is separate (`src/facade/cva_vector_generations.rs`, `src/retrieval/archive_vector_store.rs`). The authoritative point for genuine accepted-fragment evidence therefore remains a Step 6 design question.
- **Memory vector/search surface:** `src/facade/cva_memory_vectors.rs`, `src/retrieval/memory_vector_store.rs`, `src/retrieval/memory_retrieval_vectors.rs`, `src/retrieval/compatibility_vector.rs`, and `src/retrieval/semantic_search.rs`. Memory vectors bind immutable current body IDs to profile-specific packed rows; semantic retrieval is compatibility/profile and current-generation gated. Archive fragment vectors have separate Archive ownership and mappings.
- **Likely affected tests:** existing score unit tests in `src/freshness/score.rs`; storage tests `src/freshness_storage_tests.rs`; facade Freshness R2, Dream-history, and recovery modules; runtime accepted-use/Dream tests; and integration contracts `tests/freshness_{f1_reference,f7_acceptance,f7_recovery,owner_contract,propagation_oracle,r2_contract,reconcile_contract,recovery_contract}.rs` plus `tests/rel_activity_clock_contract.rs`. Search/vector compatibility regressions belong with the existing retrieval and vector tests. No focused F8 correctness fixture or F8 producer/storage implementation was found.

### Policy and scope evidence

ADR 0039 explicitly labels semantic activity reinforcement accepted design but not implemented. Its frozen constraints are owner-REL accepted turns, compatible existing Memory vectors, once-per-Memory credit using strongest fragment match, exclusion of newly created turn Memories, and direct local effects only. The initial cosine 0.75 threshold and bonus curve remain uncalibrated. Historical import is a distinct later path and must not be introduced into live processing.

### Verification commands

- Full root baseline: `cargo test --locked -j 2` — incomplete, process exit `4294967295` while compiling dependencies.
- Focused Freshness baseline: `cargo test --locked freshness_storage:: -j 2 -- --nocapture` — exit 101 during dependency compilation (E0463 missing `prost_build`/`tonic_build`); no tests executed.
- Step 12 release gate adds required formatting, checks, CLI and documentation/architecture gates. They are not substituted by this baseline.
- No Rust source or runtime behavior was changed.

## Related docs

- [Linear F8 and historical implementation plan](freshness-semantic-historical-linear-implementation-plan-2026-10-11.md)
- [Unified Freshness policy](memory-staleness-plan.md)
- [ADR 0039 — Unified deterministic Memory Freshness](decisions/0039-unified-deterministic-memory-freshness.md)
- [Freshness core release verification](freshness-core-release-verification-2026-10-03.md)
- [Maintainer map](maintainer-map.md)

## Notes

The prior Freshness release verification used Rust 1.97.1, `CARGO_PROFILE_TEST_DEBUG=0`, `-j 2`, and isolated target directory `C:\Users\archa\AppData\Local\Temp\reliquary-r3-verification`; reusing that target configuration avoided the initial dependency build issue.

## Documentation impact

- Inspected: `AGENTS.md`, architecture, invariants, storage format, maintainer map, current limitations, roadmap, ADR 0039, Freshness core verification, linear plan, Freshness score/storage/facade, accepted-turn ingestion, retrieval/vector modules, and existing Freshness tests/contracts.
- Updated: this baseline record and the Step 1 progress marker in the linear plan.
- Not affected: Rust source, storage formats, runtime behavior, APIs, and shared engineering standards.
- Compliance check: pending after documentation edits.
- Known documentation gaps: accepted-turn semantic evidence ownership remains intentionally unresolved for Step 2/6.
