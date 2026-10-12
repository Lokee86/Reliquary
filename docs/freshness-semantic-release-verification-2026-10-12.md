# F8 semantic Freshness release verification — 2026-10-12

Parent: [Documentation index](INDEX.md).

## Purpose

Record the standalone Step 12 release gate for live semantic Freshness (F8), including test evidence, bounded-receipt evidence, measurements, and explicit release blockers.

## Overview

**Status:** Step 12 blocked; live F8 is not released. Historical reconstruction remains untouched.

The durable semantic policy, candidate adapter, accepted-source proof, event codec, replay, recovery, and transformation fixtures are green. The runtime activation gate is not green because accepted-turn ingestion invokes semantic processing before vector evidence exists and discards the result; no post-vector-publication coordinator retries the accepted source/cohort. Enabling F8 would make reinforcement timing-dependent and could lose valid semantic events.

## Related docs

- [Linear implementation plan](freshness-semantic-historical-linear-implementation-plan-2026-10-11.md)
- [Semantic policy module](freshness-semantic-policy-module-2026-10-11.md)
- [Live semantic policy/API contract](freshness-live-semantic-policy-contract.md)
- [Current limitations](current-limitations.md)

## Verification evidence

- `cargo test --locked --lib semantic -j 2`: 64 tests passed on the Step 11 isolated target.
- `cargo test --locked --lib freshness -j 2`: 65 tests passed on the Step 11 isolated target, including bounded receipt eviction/reopen coverage.
- `cargo test --locked -j 2`: 1,107 library tests and all integration/doc-test targets passed.
- `cargo fmt --all -- --check`: passed.
- `cargo check --locked --tests -j 2`: passed.
- `git diff --check`: passed.
- Full and changed-from-`origin/main` documentation policy checks: passed before this record was added; they are rerun after the final indexed version.
- Architecture refresh: coverage guard passed (28 fixtures / 112 public APIs), but Lexicon analysis timed out before Arcana/Pitlord enforcement; this remains an explicit incomplete gate.
- A fresh Step 12 target rebuild timed out during dependency compilation without reaching tests; the prior isolated target remains the passing Rust evidence.

## Measurements and bounds

The passing Freshness suite includes the long-owner reopen/receipt-cache-turnover test and the semantic transformation matrix. These demonstrate bounded receipt retention, exact duplicate resolution after cache eviction, and byte-preserving reclamation/reconciliation. No production throughput claim is made: the live coordinator is not shipped, so event/search throughput must be measured after the readiness seam is wired.

## Required follow-up

Add an explicit post-vector-publication semantic readiness/cohort coordinator. It must retain or rediscover accepted source nodes, wait for complete genuine fragment/vector evidence and the complete newly-created Memory cohort, propagate errors instead of swallowing them, and invoke the existing read-only candidate adapter followed by the durable semantic acceptor exactly once. Add an end-to-end accepted-turn test proving a real vector-ready turn creates one local event, excludes its source cohort, and leaves neighbors unchanged. Rerun this release gate, including throughput/receipt measurements, before marking Step 12 complete.

## Notes

No public API, storage format, CLI behavior, current-limitation claim, or historical-import behavior was changed by this blocked release gate. The record is intentionally explicit that correctness fixtures passing is insufficient to release a live producer whose readiness coordinator is absent.
