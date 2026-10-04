# Freshness P2 verification — fixed event-batch scheduling

Parent: [Documentation index](INDEX.md).

## Purpose

Freeze acceptance of P2 on accepted P1 `cdb0716412cba07fba61a820b88139e9bccfa19a`, following the [P2–P4 execution plan](freshness-performance-p2-p4-execution-plan-2026-10-04.md).

## Overview

P2 is accepted on 2026-10-04. No topology estimator remains. Empty batches select zero workers; 1–15 events select one; 16+ events select min(caller cap, available parallelism, event count, 4), with nonempty caps/hardware values floored to one. Each event keeps P1 serial grouped-origin traversal. The counterless crate-private path shares validation, scheduling, traversal, joins and original-order assembly with the public statistics path.

Worker failure joins every handle and returns WorkerFailure. Policy and every principal validate before scheduling. Scores, event/proof identity, durable effects, storage, retry and historical ordering are unchanged.

## Correctness verification

Final simplified source passed:

- `cargo fmt --all -- --check`.
- `cargo test --locked -j2 --lib freshness::propagation`: 12/12.
- Eight focused Freshness/oracle integration groups: 50/50, including eight independent oracle tests.
- `cargo check --locked -j2`.
- `cargo test --locked -j2`: 818 library tests, 61 integration tests, zero failed/ignored; doc-tests passed.
- CLI locked check/tests: 19/19.
- Documentation structural and changed-from-origin/main checks.
- `python scripts/check_architecture.py --refresh`: fresh snapshot, policy validation and Pitlord exit 0.

Correctness commands used the warmed `reliquary-r3-verification` target with dev/test debug info disabled. An initial cold-target dependency build was cancelled and is not passing evidence. The first architecture scan used a debug Rust adapter and was cancelled; the successful retry used the prepared `reliquary-freshness-architecture-adapters` configuration. Pitlord reported six rules, 50,285 source nodes and zero relationships; this is the configured gate, not proof of arbitrary runtime relationships.

## Matched release evidence

`cargo build --release --locked --features freshness-performance-trace --example freshness_performance_baseline --example freshness_operation_baseline -j2` completed with exit 0 in 3m57s. Frozen accepted P1 executables and final P2 executables use the same release profile, feature, lockfile and Rust toolchain. The strict harness ran five alternating fresh-process rounds with eleven propagation repetitions per row on synthetic, 14-day, 28-day and Ellis fixtures. Source fixture hashes, graph projections and all 208 event-map comparisons matched. Compiler/indexer snapshots guarded every sample.

Final archive: `benchmarks/freshness-p2-fixed-final-2026-10-04.zip`, SHA-256 `1d06f11e10ff22f1676998a384099cb41212cfd09474725b4dc537331d23822f`. The archive preserves raw samples, comparisons, binary/source hashes, exact workload metadata and checksummed payloads.

Candidate median propagation speedup over its own serial selection:

| Topology | Batch 16 / 2 workers | Batch 16 / 4 workers | Batch 64 / 2 workers | Batch 64 / 4 workers |
| --- | ---: | ---: | ---: | ---: |
| Synthetic | 1.322x | 1.870x | 1.792x | 2.892x |
| 14-day | 1.817x | 3.048x | 1.912x | 3.546x |
| 28-day | 1.363x | 1.907x | 1.806x | 2.927x |
| Ellis | 1.383x | 1.974x | 1.556x | 2.668x |

Batch modes follow the exact internal planner covered by injected hardware/cap boundary tests. Public per-event statistics remain serial-frontier counts, and the Cargo benchmark does not expose total pool concurrency. This reporting limit is explicit rather than relabeling requested caps as observed workers. Four-event execution stays serial, intentionally abandoning the old 14-day parallel win.

For complete accepted-use operation samples at batch 16/64 and production-relevant caps 1/2/4, candidate/baseline medians ranged from approximately 1.034 to 0.961. No >5% median regression appeared in those six operation rows. Each operation row times three distinct receipts and has five observations; sampled p95 is a maximum, not a production tail estimate. P2 retains material parallel propagation benefit; it does not claim a complete-operation speedup.

One isolated propagation-stage flag remained: 28-day batch 16 at cap 4 was 14.3% slower in median and 20.4% slower in sampled p95 than P1. Both versions select four workers there, and the candidate still measured 1.907x over its serial path. This is disclosed as a stage-level result, not dismissed by a global noise allowance. The accepted gate is retained parallel benefit plus no material complete-operation regression; no universal P1/P2 propagation no-regression claim is made.

## Noise and evidence disposition

Earlier identical-binary A/A controls demonstrated substantial tail noise: typical across-row p95 differences were 3.77–8.55%, with individual unchanged rows far above 10%. Their checksum-backed combined summary remains at `benchmarks/freshness-p2-r1-noise-summary-2026-10-04.json`. Unchanged Dream/single-event tail spikes are not scheduler tuning evidence. The final fixed-rule run is the acceptance evidence under the newer P2–P4 plan; old adaptive runs are superseded.

Partial/adaptive/duplicate control archives and estimator-only scripts are moved out of the active repository into temporary P2 custody. Accepted P0/P1 evidence remains untouched. The strict comparison harness and generic tracing remain.

## P3 disposition

P3 is closed as a deferred/no-change checkpoint. P0 measured current graph enumeration/adjacency at about 0.003–0.007 ms in the local operation path; that does not justify a graph cache, reusable version projection or invalidation service. Historical Dream preparation remains a future measurement question. No expensive historical corpus was built and no P3 production code was added. P4 remains the next optimization target.

## Documentation impact

- Inspected: execution plan, P0/P1/noise evidence and current API/architecture contracts.
- Updated: P2 report, current scheduling contracts, maintainer routing and phase status.
- Not affected: score policy, accepted-use identity, storage format and Ego delivery.
- Compliance check: structural/change-impact validation passed; final report checked before commit.
- Known documentation gaps: real host event distributions and production-tail evidence remain outside this milestone.

## Architecture impact

- Standards added or changed: none.
- Ownership or boundary impact: Freshness retains one internal traversal/scheduling owner; no new service or authority.
- Pitlord enforcement impact: existing policy unchanged; refresh/check passed with the scope above.
- Other verification impact: exact independent oracle, failure joins, receipt/recovery/reconciliation and CLI gates passed.
- Known architectural gaps: static relationship output was empty; lifecycle guarantees rely on behavioral tests.

## Related docs

- [Execution plan](freshness-performance-p2-p4-execution-plan-2026-10-04.md)
- [P1 verification](freshness-performance-p1-verification-2026-10-03.md)
- [Performance hardening roadmap](freshness-performance-hardening-plan-2026-10-03.md)

## Notes

Stop scope remains P4 acceptance or measured null result. P5/P6, F8 and Ego delivery are outside this execution.
