# Freshness R4 verification — 2026-10-03

Parent: [Documentation index](INDEX.md).

## Purpose

Record canonical history, accepted-prefix scanning and finite receipt lookup evidence.

## Overview

The previous per-event repeated history scan was replaced with one accepted-prefix scan, bounded sorted runs and pairwise merges. Owner reopen validates receipts through a disposable disk lookup and performs one final canonical replay. No lifetime event/receipt ID table is retained in RAM.

## Verified contracts

The ordinary root suite and focused `cargo test --locked freshness_storage:: -j 2 -- --nocapture` exercise mixed Dream/use forward, reverse and permuted arrivals, an independent integer decay/saturation oracle, same-turn event bounds, corrupt/truncated prefixes, unsupported journal magic, cross-run duplicate/conflict handling and spill cleanup. Public future-turn/graph cuts and blank event bounds are rejected without owner writes. Owner retry resolves original durable effects before live graph traversal.

`canonical_history_scaling_measurement` covers 128, 512, 4,096 and 20,000 prevalidated reverse-arrival batches, serial expected scores, bounded cache rollover/rebuild and evicted exact/conflicting retries. `long_owner_reopen_keeps_finite_receipts_and_exact_evicted_retry` builds a native Cva owner with 20,000 prevalidated accepted-use batches, reopens twice, checks current/historical values, exact old retry, conflict rejection and byte purity. Fixture construction appends prevalidated batches directly; it is not a benchmark of 20,000 repeated public producer calls.

## Evidence and repairs

Final frozen-source job `job_KEDYwixNQjbm3N-JxjBYGT8u` passed root fmt/check, 814 unit tests, all 54 integration tests (zero ignored), and all 10 focused storage tests. The focused invocation took 46.271 seconds. Canonical replay/oldest retry in ms: 128 = 5/1; 512 = 27/8; 4,096 = 215/51; 20,000 = 1,498/264. Final native owner reopen 22,154/21,659 ms, evicted retry 239/244 ms, historical query 120/120 ms; 128 entries/130,432 accounted bytes in both runs. Job `job_7FbxxlOr4LWVm5nZAFxdZ7Pa` passed the 20,000-receipt owner test (28,068/25,536 ms reopen, 277/286 ms evicted retry, 139/137 ms historical query; 128 resident entries, 130,432 accounted bytes) but failed one existing provenance negative after an edit restored an older codec tail. A truncated read was mistakenly written back; the cached codec tail was restored, its stricter exact Community/graph proof equality reinstated, and comprehensive checks rerun. That earlier failed suite is not acceptance evidence.

## Bounds and limits

The recent cache is bounded by 128 entries and conservative 256-KiB receipt accounting. Sorting run buffers use 256 KiB; pairwise merges retain a bounded number of decoded records. One exceptionally large accepted event payload is indivisible. Scratch disk use grows with history and is rebuildable; it is removed after successful/failed in-process operations, while interrupted processes may leave disposable directories. Cold reopen stores receipts on disk, so the implementation exchanges temporary I/O for bounded RAM. An old exact retry still costs a linear accepted-prefix scan after reopen; repeated old misses/late appends are not constant time. No checkpoint/retention scheme is claimed.

## Related docs

- [Completion execution plan](freshness-completion-execution-plan-2026-10-03.md)
- [Core verification](freshness-core-release-verification-2026-10-03.md)
- [Development](development.md)

## Notes

F8/S1–S6 are excluded by the user's execution scope. Measurements are diagnostic, not release-build latency guarantees. Exact frozen source identity and final repository gates are recorded in the core report.
