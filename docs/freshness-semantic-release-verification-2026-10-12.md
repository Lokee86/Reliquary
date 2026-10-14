# F8 semantic Freshness release verification — 2026-10-12

Parent: [Documentation index](INDEX.md).

## Purpose

Record Step 12 implementation closure, standalone live F8 verification, synthetic search/event measurements and supported release scope.

## Overview

**Status:** Historical Step 12 evidence remains valid for F8 mechanics, but its opt-in activation policy is superseded by the intrinsic-F8 hard cut after C12. Live F8 is owner-local, local-only and always part of REL Freshness when eligible vector evidence exists; there is no enable/disable API. Historical reconstruction remains a separate explicit offline operation.

## Related docs

- [Linear implementation plan](freshness-semantic-historical-linear-implementation-plan-2026-10-11.md)
- [Frozen v1 constants and transitions](freshness-semantic-calibration-2026-10-11.md)
- [Live semantic contract](freshness-live-semantic-policy-contract.md)
- [API](api.md), [storage](storage-format.md), [architecture](architecture.md), [current limitations](current-limitations.md)

## Implemented closure

- Historical Step 12 used `CVAFSA01` prospective activation. That switch is now removed. Current F8 is intrinsic; retained pre-release `CVAFSA01` records are compatibility watermarks only, while historical checkpoint cuts prevent pre-checkpoint double credit.
- Archive remains the source/work authority. No queue or separate ledger is added. The bounded coordinator rediscovers uncompleted eligible accepted nodes, distinguishes pending from completed-empty evidence, and pins effects at their accepted cut. Both REL vector workers perform embedding outside owner locks, then commit through their existing owner admission/mutation boundary. No ingestion-time error is discarded.
- Births must strictly precede the source, while admission may equal it. Asynchronous/new source births cannot qualify, independently of later lifecycle changes; exact provenance/cohort checks remain additional guards. Invalid vectors are excluded before the usable limit. The adapter streams target rows and retains only top-64 selection rather than a full candidate/vector population. Pinned target evidence commits current immutable body/vector bindings and row bytes, not just effect IDs.
- Runtime background status includes pending semantic work and does not call the owner quiescent while it remains. Accepted receipts resolve before vector work; retry after cache eviction/reopen/profile rotation never recredits. Direct effects remain non-propagating and independent of +25 use/+50 Dream.

## Verification

The semantic-filtered suite passes 69 tests, including real accepted ingestion, asynchronous-birth exclusion, an actual runtime worker, activation/cancellation/reopen, conflicting activation authority, body-local graph-neighbor isolation, divergent reconciliation, reclaim and twice-reconciled event histories. The explicitly ignored measurement fixture is executed separately and passed. The final full root run passed 1,112 library tests (one separately executed ignored measurement fixture) and every integration/doc target. CLI tests passed all 19 tests. Root and CLI formatting/compile checks, full and changed-from-origin/main documentation policy, and refreshed architecture enforcement passed.

| Gate | Executed command / result |
| --- | --- |
| Root formatting | `cargo fmt --all -- --check` — passed |
| Root compile | `cargo check --locked --tests -j 2` — passed |
| Root regression | `cargo test --locked -j 2` — 1,112 library tests plus all integration/doc targets passed |
| Focused semantic | `cargo test --locked --lib semantic -j 2` — 69 passed, one measurement ignored here and executed separately |
| CLI formatting | `cargo fmt --manifest-path cli/Cargo.toml -- --check` — passed |
| CLI compile | `cargo check --manifest-path cli/Cargo.toml --locked -j 2` — passed |
| CLI regression | `cargo test --manifest-path cli/Cargo.toml --locked -j 2` — 19 passed |
| Documentation | Both required documentation-policy modes — passed |
| Architecture | `python scripts/check_architecture.py --refresh` — passed with compatible local tools |
| Whitespace | `git diff --check` — passed |

One full-run attempt exposed an existing Dream concurrency fixture that assumed scheduler overlap during a 40 ms sleep. Its tracking endpoint now waits up to five seconds for observed overlap without weakening the peak-concurrency assertion; all nine focused Dream tests and the full rerun passed. The first architecture attempt completed Lexicon scanning but failed because the newest Lexicon emitted `dependency_index_id`, which the installed Arcana reader did not support. The successful refresh used `lexicon-arcana/bin/lexicon.exe` with `arcana/target/release/arcana.exe`; no source facts or policies were suppressed.

Builds use `target/semantic-step11-20261012`, with dev/test debug information disabled and two compilation jobs. Cold dependencies were rebuilt; timeouts/cancelled preparatory attempts are not counted as test passes. Existing unrelated unused/dead-code and Windows linker warnings remain.

## Measurements and bounds

Executed command: `cargo test --locked --lib semantic_release_measurement -j 2 -- --ignored --nocapture`.

Synthetic fixture: 128 compatible target bodies, 140 accepted post-activation turns, two-dimensional deterministic unit vectors, at most 64 local effects per event. One unoptimized workstation sample:

| Operation | Result |
| --- | ---: |
| 32 exact searches | 84.445 ms total (2.639 ms/search; 48,506 target evaluations/s) |
| 140 durable event completions, including generation publication | 3,483.156 ms total (40.19 events/s) |
| Reopen | 222.248 ms |
| Resident receipts | 29 entries / 261,783 bytes |

The fixture asserts output cardinality 64, recent residency <=128 entries and <=262,144 bytes, exact evicted retry before/after reopen, and no extra file bytes on retry. These are reproducible correctness/measurement commands, not optimized production SLOs, percentile latency, provider/mutex tails, large-corpus throughput or semantic-quality evidence. Archive generation construction currently rebuilds its selected matrix; labelled relevance remains unavailable for policy calibration, but F8 activation is no longer conditional.

## Documentation impact

- Inspected: repository instructions, canonical architecture/invariants/storage/API/limitations/roadmap and frozen F8 contracts.
- Updated: shipped API/storage/limitations, source/runtime routing, policy/module status, linear plan, index and this release record.
- Not affected: historical reconstruction, PHY Freshness, Dream/use scoring principals, existing semantic Memory versions, and CLI command vocabulary.
- Compliance check: full and changed-from-origin/main documentation policy plus `git diff --check` passed after final edits.
- Known documentation gaps: production latency/relevance labels are unavailable; no claim of either is made.

## Architecture impact

- Historical note: Step 12 added the pre-release activation record; current code retains it only as a compatibility watermark. Frozen v1 policy/ownership is otherwise unchanged.
- Ownership or boundary impact: Freshness retains receipt/scoring authority, Archive retains accepted identity, and existing vector workers coordinate provider I/O outside owner locks.
- Pitlord enforcement impact: refreshed Lexicon/Arcana evidence, policy validation and Pitlord enforcement passed; the coverage guard passed 28 fixtures and classified 112 public APIs. Existing ownership/static policies are unchanged.
- Other verification impact: executed runtime, durable transformation and bounded-residency fixtures plus full root/CLI gates.
- Known architectural gaps: no historical importer, PHY clock, in-place policy upgrade or disabled interval semantics.

## Notes

Current F8 is the intrinsic live local lane. It still grants no permission to backfill old turns, fabricate historical activity cuts, traverse other owners, or propagate live similarity over Graph/Community; historical reconstruction remains a separate explicit offline operation.
