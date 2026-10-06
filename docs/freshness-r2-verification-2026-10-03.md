# Freshness R2 event-contract implementation and verification — 2026-10-03

## Purpose

Record the R2 durable-event implementation changes, exact producer boundaries, and directly collected verification evidence. This is not an R3–R7 completion report. The R2 runtime results below are from actual executed tests.

## Overview

Repository: `C:\!bin\workspace\Reliquary`. Branch: `feat/deterministic-memory-freshness`. Changes remain on the shared, uncommitted Freshness worktree; do not commit unrelated concurrent phases from this checkpoint.

### Source changes

- `src/freshness_storage.rs` now validates the immutable producer kind, source identity, origin owner, policy version, accepted original source cut, pinned memory-graph/Community relationship, born event sources, positive per-target candidate bounds and producer-specific principal. The Dream event ID is the precise owner-bound `dream-initial:{owner}:{new_memory}`; accepted-use event IDs bind the owner, use identity and actual Memory source. An event's **original** accepted source cut may precede its rebased destination cut after reconciliation; local producers enforce exact accepted-cut equality before publication.
- Accepted-use batches are prevalidated and published as one durable owner record. Standalone event records validate their computed postimages before modifying the in-memory projection. On-disk reconciliation encoding now also validates preserved provenance and postimages **before** append. Logical receipt equivalence compares immutable event ID, accepted event cut, original source provenance, pinned proof and candidate effects; derived postimages are not part of logical identity because a subsequently accepted earlier event can alter canonical later projections.
- `src/facade/cva_freshness.rs` binds Dream event publication to a recorded pending first pass, exact accepted settlement cut, accepted graph/Community proof and pre-existing source-root set. Newly created same-publication cohort targets are excluded. This identity check runs **before** source admission to prevent an invalid event leaving a partially admitted Memory. The low-level Dream event commit method and admission write are crate-private rather than exposed as arbitrary public event producers.
- `Cva::freshness_initialize` now requires its accepted first-publication intent and matching first mutation ID. `Cva::enroll_legacy_freshness_baseline` is a distinct, explicit, one-time current-turn baseline for nonarchived settled legacy Memories lacking publication birth evidence. A legacy baseline never creates a Dream first-cycle birth, never reanchors on retry, and allocates no semantic Memory revision.

### New regression coverage

- `src/freshness_storage_tests.rs`: invalid producer/source identity, wrong accepted cut or owner, mismatched Community proof, invalid candidate principal, incorrect postimages and conflicting event reuse do not change the store projection; existing accepted-use batch corruption tests remain.
- `src/facade/cva_freshness_r2_tests.rs`: simulated pre-Freshness accepted Memory requires explicit legacy baseline and remains ineligible for fabricated new-Memory Dream events after reopen; normal first publications cannot be re-enrolled as legacy.
- `tests/freshness_r2_contract.rs`: invalid accepted-use batches leave on-disk bytes and semantic versions untouched, true receipts survive graph changes and reopen as no-op retries, and reused identities with incompatible cuts or sources are rejected.

### Verification collected

- `cargo check --locked --tests --examples` with isolated `CARGO_TARGET_DIR=target/r2-owner-verification`: **passed** (job `job_-AwIlXX5SquFlkKLbEKEAHir`) after the R2 source and regression tests were included. An earlier shared-target compilation produced incompatible/missing external dependency artifacts and was not used as evidence.
- `cargo fmt --all`: completed (job `job_3m-NkaUq_hiZkGfheM11IKWe`) following the initial R2 edit batch. A subsequent targeted formatting pass also completed before the successful combined check.
- The first isolated unit runtime test and a cached-target retry both **timed out while compiling the external Lore dependency stack** (jobs `job_gxkQrxV-dtdbTUNtS-6pwtxz` and `job_SFYln0nobNR_WbuZy3q3bDM5`); neither reached Reliquary's test execution.
- Further focused attempts likewise timed out **before any test assertions ran**: `freshness_storage::tests` (job `job_H2I-YadQS__JkLjFR_7Bpnhz`), `freshness_r2_contract` (job `job_3eMv4N2gx1QPMYW6mxOwcnCq`) and the combined `cargo test --locked --lib freshness -j 2` (job `job_gbXtPbK_vNo7e_QptSjdFBGV`). All were still compiling external Lore/dependencies.
- Latest `cargo fmt --all -- --check` (job `job_8KPswmRCjYOC_bWAjPKW5w3I`), repository documentation policy (job `job_sstXKIy2x_wjmN-OZ5Bpnp2S`) and `git diff --check` (job `job_G0JTra6RvcXfOEukcxe0giOI`) **passed**.
- **R2 focused runtime acceptance verified (2026-10-03): 6 passed, 0 failed.** `cargo test --locked --lib freshness_storage::tests -j 2` passed **4/4** (isolated `target/r2-owner-verification`, job `job_9gn5isgdlTXZ_VLcAh22Wtxa`); `cargo test --locked --lib cva_freshness::r2_tests -j 2` passed **1/1** (same isolated target, job `job_eQao8Y3O6be8LtBZGEjFj_rh`); `cargo test --locked --test freshness_r2_contract -j 2` passed **1/1** (job `job_nNi8sj1GlTuiLJCIIBuFrOGZ`). The exact facade test module was confirmed; this was not a zero-test result.
- **Focused repair verified:** The initial storage execution (job `job_g0uPTgu08exq4a6SBMc041C0`) ran 4 tests, exposing one postimage assertion failure. The current compact durable event codec stores immutable effects, not derived postimages; ingest now computes canonical postimages from effects, while `encode_event_with_provenance` rejects a malformed caller-supplied postimage before encoding. The unchanged substantive invalid-provenance and corrupt-byte checks plus corrected pre-append postimage assertion pass in the subsequent 4/4 storage run. Earlier dependency timeouts remain historical build failures, not runtime assertion failures.
- **Post-repair combined compile:** `cargo check --locked --tests --examples -j 2` passed (job `job_2lxjx1IZgT9FeVqmvXRaz-Hd`).
- **Final hygiene checks:** `cargo fmt --all -- --check` passed after the narrow test-assertion formatting correction (job `job_bJuCJc29fo0pRPlaIRtx4R_x`); repository documentation policy passed (job `job_o2WcAaN9uPOTDlByJUI7Zw_c`) and passed again after the final status/document updates (job `job_Wv4z3WzN7rbmaMLLQiNSUySy`); `git diff --check` passed (job `job_EbcDcvZ7IVTtgEDtegLTRwnO`) and again after the document updates (job `job_V0_WhLIXe3Ycnp-mf16B4pev`).
- The focused R2 exit gate is complete. This does not assert that the full Freshness suite or R3–R7 gates are complete.
- **Final post-repair hygiene checks:** `cargo fmt --all -- --check` passed (job `job_bJuCJc29fo0pRPlaIRtx4R_x`); documentation policy passed (job `job_d4xCBvD2JBjLv98PvjuEptxO`, `documentation check: passed (library-engine)`); `git diff --check` passed (job `job_ouqUEmCWRci4JgvGCnN4Q0VO`). The Git check covers tracked changes; the worktree also contains untracked files belonging to concurrent phases.

## Related docs

- [R2 remaining-work source specification](freshness-remaining-work-2026-10-03.md)
- [Main Freshness implementation plan](generic-staleness-implementation-plan.md)
- [Memory Freshness numerical policy](memory-staleness-plan.md)
- [Architectural decision 0039](decisions/0039-unified-deterministic-memory-freshness.md)
- [R1 verification evidence](freshness-r1-verification-2026-10-03.md)

## Notes

R2's focused invalid-batch/retry/reopen exit gate has been verified with the collected 6/6 results above. R2 remains distinct from R3 crash-prefix closure, R4 performance/long-ledger verification, R5 general transformation proof preservation, R6 live context consumers and R7 full repository/CLI/architecture gates. Full Freshness regression and broader production readiness are not established by this R2-only gate. Existing shared worktree changes across these phases have not been staged or pushed in this task.
