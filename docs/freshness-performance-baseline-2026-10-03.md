# Freshness Performance P0 — measured baseline (2026-10-03)

Parent: [Documentation index](INDEX.md).

## Purpose

Freeze the first optimized P0 measurements for [Freshness performance hardening](freshness-performance-hardening-plan-2026-10-03.md) before any P1 production-algorithm change. Distinguish end-to-end deterministic **local fixture** operations from prospective propagation on real REL topologies and from unmeasured live-host behavior. Use measured hot paths, rather than the old artificial eight-root mixed benchmark, to decide optimization order.

## Overview

**Status: P0 baseline frozen and accepted as the evidence gate for isolated P1 optimization.** This is a scoped optimized baseline, not deployment performance acceptance: historical real-corpus operation replay and live-host telemetry remain unavailable and belong to later P3/P6 validation. No production propagation algorithm was changed during P0. The verified production Freshness algorithm remains unchanged. All four release-profile propagation runs and the actual deterministic local accepted-use/Dream run exited 0. Their raw evidence is preserved. This baseline does **not** claim that the existing 14-day, 28-day or Ellis archives contain historical accepted-use/first-Dream proof, or that a synthetic fixture measures inference-provider or host latency.

The most important finding is **two different bottlenecks**: expensive eight-root Dream propagation on large graph topology and expensive canonical Freshness postimage/replay work on the actual, small local accepted-use path. Four or more workers help independent +25 event batches on large graphs; they do not consistently improve one +50 event and can worsen full-operation latency when graph computation is cheap. Investigate the whole-operation costs before choosing a universal worker policy.

## Source custody and reproducibility

Core HEAD before P0: `20e1e9ab0513b47b1e6726e45f340d3db11c4bc9`, branch `perf/freshness-hardening`, worktree `C:\!bin\workspace\Reliquary-freshness-performance`. P0 adds only opt-in trace instrumentation, benchmark examples, an expanded independent oracle and documentation; P1's replacement algorithm has **not** been implemented. The detached pristine core checkout remains at `C:\!bin\workspace\Reliquary-freshness-baseline`.

Rust `1.97.1 (8bab26f4f 2026-07-14)`; 16 logical processors; Cargo.lock SHA-256 `5362fb09abc0995f99bb33462eede0666f9721f734f8f29cef54ab944170dd85`. Baseline uses Cargo `--release --locked`, feature `freshness-performance-trace`, unchanged propagation policy, and matching optimized compilation for the diagnostic work. This is a **single-host, five-repetition directional baseline**; nearest-rank p95/p99 from five runs is the maximum, not reliable production tail-latency evidence. No parallel builds were deliberately run during timed collection.

Immutable portable evidence bundle: repository-relative `benchmarks/freshness-p0-2026-10-03.zip`, containing 18 raw measurements/aggregates/manifests plus a `MANIFEST.json` with per-file SHA-256 and byte counts. Archive SHA-256 `6a9babd20d701bb07a6ce721803a5a1c71ba72364bc93ffb9db3b8fe5be39a77`. `scripts/freshness_p0_freeze.py` reconstructs it and verifies every archived checksum. The archive does not contain authoritative REL source data or user transcript content. Raw evidence directory, local to the user's workstation: `C:\Users\archa\AppData\Local\Temp\reliquary-freshness-perf-baseline\freshness-evidence`. Files: `p0-propagation-{synthetic,14d,28d,ellis}.json`; `p0-operation.stdout.jsonl`; `p0-operation.stderr.jsonl`; `p0-operation-summary.json`; `p0-comparator-selftest.json`; `p0-measurement-manifest.json`. Script `scripts/freshness_p0_report.py` parses separate nested trace scopes without summing parent and child stages; `scripts/freshness_p0_evidence_summary.py` reproduces this report's selected propagation figures and input SHA-256s. The manifest records evidence hashes and exact toolchain/lock/profile features.

The baseline executable is `release/examples/freshness_performance_baseline.exe`; the full-operation executable is `release/examples/freshness_operation_baseline.exe`. Both were compiled in the isolated target outside `C:\!bin`. The selected propagation run uses `--repetitions 5`, worker counts 1/2/4/8, the scenario matrix documented below and mandatory `--fixture ... --sha256 ...` for real RELs. The operation run requires `RELIQUARY_FRESHNESS_TRACE=1` and deliberately creates/deletes a disposable REL.

## Authoritative real-fixture custody

All three real inputs were SHA-256 checked before use and copied to disposable writable fixture paths. The executable checked each copy and rechecked the original afterward; `source_hash_unchanged=true` for all three. No source archive was mutated.

| Fixture | Source bytes | SHA-256 | Current Community? |
| --- | ---: | --- | --- |
| chatgpt-first14d | 19,692,418 | `49751a62dd8644800d19976a0696a69366dc1e4f8d7c39bb4b69fbd7a8a5d18b` | Yes |
| chatgpt-first28d | 49,882,422 | `8f4a2bf871d7f5368edde93954a2260a29c86131b28475b323a7b6c70f8c51e4` | No; unknown fallback |
| rhelm-david-r-ellis-pre-relationship | 11,830,382 | `3efd9c666aa992686abd8aac9b77cfc7dc2a0e15d6fb26ec01f8666db175e476` | No; unknown fallback |

Authoritative source paths: `C:\!bin\workspace\reliquary-fixtures\local\authoritative\<fixture>\project.prj.rel`. The prospective topologies have 1,084/7,029, 2,810/22,464 and 674/4,901 graph nodes/undirected active edges. Restoring Communities artificially in the stale fixtures would be a different experiment and is **not** included in their acceptance numbers.

## Prospective optimized propagation measurements

Times below are **median milliseconds for the pure propagation operation** and do not include graph open/pin, accepted-use postimage preparation, append/sync or Dream graph publication. Five repetitions per selected real scenario; current source and policy are identical across requested worker counts. The accepted-use family consists of **separate** +25 single-root events; the Dream family is **one** +50 multi-root event. Each tested worker result was checked against serial candidate maps. All synthetic and real JSONs retain per-event identity, roots, exclusions, immutable candidate-map digest, worker statistics and raw latency samples.

| Topology / scenario | 1 worker | 4 workers | 8 workers | Recipients |
| --- | ---: | ---: | ---: | ---: |
| Synthetic 2,048 / 8,224, 16 independent +25 events | 1.720 | 0.822 | 0.776 | 1,058 event-recipient pairs |
| Synthetic, one +50 event / 8 roots / same origin | 2.279 | 2.618 | 2.768 | 184 |
| 14-day, 16 independent +25 events | 8.747 | 2.639 | 2.106 | 3,297 event-recipient pairs |
| 14-day, one +50 event / 8 same-Community roots | 43.475 | 41.565 | 39.617 | 1,079 of 1,084 |
| 14-day, one +50 event / 8 across-Community roots | 44.302 | 40.789 | 40.454 | 1,084 of 1,084 |
| 14-day, one +50 event / 8 unknown roots | 21.834 | 23.600 | 23.565 | 1,078 of 1,084 |
| 28-day, 16 independent +25 events | 2.582 | 1.013 | 0.906 | 2,334 event-recipient pairs |
| 28-day, one +50 event / 8 unknown roots | 45.518 | 49.676 | 48.227 | 2,807 of 2,810 |
| Ellis, 16 independent +25 events | 2.371 | 1.037 | 0.816 | 1,937 event-recipient pairs |
| Ellis, one +50 event / 8 unknown roots | 19.521 | 21.109 | 21.240 | 672 of 674 |

Recipients in a batch count individual event-recipient pairs, **not** a deduplicated union over the entire batch. Real-root selection is deterministic/lexical, not sampled production frequency. The complete synthetic JSON additionally covers independent batches 1/4/16/64, multi-root Dream sizes 1/2/4/8 across same/across/unknown origin groups and excluded root/bridge cases at workers 1/2/4/8. Focused real JSON deliberately measures representative size-1 and size-16 accepted batches plus eight-root Dream; a full cross-product of each real corpus is unnecessary for the P1 hot-path hypothesis and remains a P6 acceptance expansion.

Real p95 examples at one/four workers: 14-day accepted batch 16, 9.277/2.959 ms; 14-day same-origin Dream, 45.496/41.879 ms; 28-day eight-root Dream, 46.843/53.247 ms. The synthetic same-origin eight-root case degraded from 2.279 to 2.618 ms with four workers. These indicate per-event parallel frontier overhead; an event-level batch can still benefit substantially.

Raw file SHA-256: synthetic `c4c25c847485e45a8c4f13cb43c2f717b54ad4ff8947adb77b6694bf9593ed7b`; 14-day `27b35d78e9fd634f11c426eeff7c4e76cc4f331b6f13921101f4c9ca49cfe52d`; 28-day `ed18e0ab7ecffac7c609886309561bdaa309be43c969a299e9c95e1ef0edb477`; Ellis `6054e0c9a5afbadddbc7323bb6fc92633bc4916f852e18772c0e77cc0c3838e3`.

## Actual local operation trace

`freshness_operation_baseline` creates a disposable 68-Memory REL with 64 pre-existing Memories and 15 active Memory graph edges following the four Dream publication events. It publishes four Dream source Memories, drives actual first-pass Dream relation publication and **pinned prepare/evaluate/commit**, and exercises actual receipt validation, +25 effect application, append/sync, due refresh, reopen and exact retry. There is no live inference provider, large archived historical graph or deployed host mutex in this fixture. Trace measurements are inclusive by stage; nested stage timings must never be summed into their parent's totals.

Initial recorded outer Dream fixture times (one sample per scenario, **not** p95): one/two/four/eight roots 125.057/128.144/136.274/151.785 ms. The previous trace did not split one-time fixture relationship publication from true settlement; do not label these numbers pure runtime settlement latency. The optional fixture-publication trace extension required an expensive release rebuild that was cancelled without changing any accepted production algorithm. Its extra instrumentation was reverted exactly to the previously compiled and tested diagnostic source. Thus the existing outer Dream timings **still include fixture publication**; publication and full live provider/host costs remain a separate high-fidelity follow-up, not misattributed to pinned settlement.

Local accepted use recorded one cold single-Memory receipt in 20.407 ms, exact same-ID warm retry in 0.047 ms, owner reopen in 439.625 ms and an exact retry after reopen in 3.925 ms. Three **distinct** receipts for each batch shape produced the following *total* batch timings; they are not per-receipt latency percentiles:

| Three receipt operations, each containing | 1 worker | 4 workers | 8 workers |
| --- | ---: | ---: | ---: |
| 1 Memory | 46.748 ms | 45.500 ms | 48.801 ms |
| 4 Memories | 77.225 ms | 80.155 ms | 81.032 ms |
| 16 Memories | 189.313 ms | 257.089 ms | 294.738 ms |
| 64 Memories | 777.911 ms | 894.991 ms | 964.810 ms |

Stage-level samples across the 49 distinct receipt operations show median/p95 **effect postimage preparation** 9.889/156.309 ms; **receipt validation/ingest** 15.145/157.993 ms; **Container append/sync** 0.509/0.741 ms; **propagation** 0.263/0.556 ms; **graph enumeration and adjacency construction** 0.003/0.007 ms each. Exact retry lookup for these growing-history receipts had median 1.587 ms. These are sequence-wide stage distributions across different receipt sizes and a growing journal, not stationary per-shape p95 or independently representative real-corpus distributions.

For four Dream preparations, median/p95 of the pinned historical-graph preparation wrapper was 6.291/6.707 ms, including an individual historical graph scan of 1.360/1.495 ms; committed-root scan was 1.324/1.737 ms. Dream final receipt/postimage/append/sync/due-refresh wrapper had median/p95 21.121/26.467 ms; its pure propagation was median/p95 0.019/0.040 ms. Historical scans visited roughly 500 chunks for this local fixture and are expected to grow with accepted history; a 500-chunk fixture cannot establish large-corpus history-scan scaling.

Four additional freshly constructed local RELs completed the **identical optimized diagnostic executable**, giving five independent run-level samples per fixed shape (while the 49 internal per-operation samples remain heterogeneous by batch size/journal growth). Across five runs, p50/p95 for the complete eight-root Dream fixture including relation publication were **170.675/177.721 ms**; one-root fixture **139.713/151.854 ms**. For three distinct 16-Memory receipts, total batch p50/p95 was **223.068/232.487 ms serial** versus **290.023/381.309 ms with four workers**. For three 64-Memory receipts, total batch p50/p95 was **912.826/989.644 ms serial** versus **1,014.756/1,062.736 ms with four workers**. These demonstrate that accepted-use batch parallelism is not beneficial in this small, growing-history *complete* operation even though the larger-graph pure-propagation benchmarks favor it. Reproduce all selected stage medians, per-shape p50/p95/p99 and ten raw output/trace SHA-256s from `p0-operation-aggregate.json` using `scripts/freshness_p0_aggregate.py`.

Across those five runs, pooled stage p50: accepted-use effect postimage preparation **12.678 ms**, receipt validation/ingest **19.517 ms**, propagation **0.271 ms** and append/sync **0.540 ms**. Dream commit receipt/postimage/append/sync/due-refresh wrapper median **22.065 ms**, historical-graph snapshot wrapper **6.291 ms**, and pure propagation **0.018 ms**. These pooled values mix shapes and journal sizes; do not infer stationary operation latency from them.

The full-operation raw stdout SHA-256 on the initial trace was `7504e6a7c60ca592645a453e0793fe6262915d073ffefe3126ddfd77ba2b913b`; original trace stderr SHA-256 `e72133beb8e3fc0abf39ce326788a5196badfb14634f1ba0a3791cedb19160ea`. If the diagnostic trace is rerun after instrumentation, record the new hashes in the manifest and supersede these initial full-operation numbers with a clearly labeled separate run; never silently combine distinct instrumented builds.

## Evidence-driven bottleneck order

1. **Normal accepted-use postimages and canonical ingest/replay:** on the actual small REL, milliseconds to hundreds of milliseconds versus sub-millisecond propagation. This is the primary *observed* full-operation cost and increases as more events accumulate. Distinguish the cost of canonical historical event processing from effect-map generation in a P4 focused microprofile before redesigning persistence.
2. **Multi-root Dream graph traversal on large real topology:** an eight-root prospective event spends approximately 20–46 ms and can touch nearly every Memory, even in a sparse global graph. Group roots by immutable originating Community; test exact independent oracle equivalence before claiming speedup.
3. **First-cycle Dream preparation and commit:** historical root/graph scans and canonical commit already cost measurable milliseconds on a very small fixture. Measure publication separately; investigate historically pinned graph scan and postimage/replay growth on larger disposable histories before introducing any cache.

These bottlenecks operate at different stages and fixture sizes; the ranking is an **optimization priority list**, not a measured cross-fixture comparison of end-to-end production total cost. Cheap single-root accepted batches warrant serial execution; independent larger batches have a justified separate parallel execution strategy.

## Verification and limits

The opt-in feature-enabled Rust library/tests/examples suite completed successfully in the integration worktree before the fixture-only diagnostic timing change. The separate independent oracle worktree passed four expanded propagation oracle tests; the integration feature-enabled run also passed all four. The standalone comparator's **synthetic-versus-itself structural self-test exited 0**, proving that its input matrix and candidate-map checks can accept identical evidence. It is *not* a legitimate before/after improvement comparison. Current raw propagation output omits embedded `build_metadata`; the external immutable manifest carries the toolchain/profile/lock identity. A true P1 comparator performance-qualification result must use matching embedded build identities or an explicitly verified manifest adapter; do not interpret the current self-test's unqualified build field as an accepted matched-build comparison.

Not measured: production frequencies of accepted-use batch sizes or Dream root distributions; actual host lock contention; live provider/Insomnia/Dream inference latency; time-positioned real-corpus Freshness histories; process/allocator peak memory; complete real-corpus receipt/postimage/persistence end-to-end; all real-fixture root/workload combinations. These must not be invented or silently substituted with the available prospective topology runs. Stale real Community snapshots properly fall back to unknown membership; they cannot be read as measurements with valid current Communities.

**P0 disposition: accepted for P1 experiments and implementation.** The original optimized trace executable completed five independently constructed local RELs without error; the extra fixture-publication instrumentation was reverted after its release rebuild was cancelled, returning to the previously compiled/tested diagnostic implementation. The earlier integrated feature-enabled Rust suite and the independent expanded four-test oracle passed on that implementation. The post-revert source passed `cargo fmt --all -- --check`, `git diff --check`, full documentation validation and changed-from-HEAD validation. The propagation benchmark passed in optimized release form on synthetic and all three hash-verified real graphs, with per-event candidate equality checks; the comparison script passed structural self-comparison (not a candidate speedup test). The bundled evidence's 18 raw files passed archive-entry SHA-256 verification. This accepts the *measurement prerequisite* for P1; it does not certify P1 performance, optimize any production logic or claim live-host production latency. The cancelled optional extra release rebuild was **not** claimed as passing test evidence. P1 must rerun its own final-source checks and matched comparator, using this frozen baseline. Missing larger historical full-operation tests, actual allocation peaks, host mutex contention and real production event-frequency distributions remain explicit P3/P6 follow-ups, not fabricated P0 samples.

## Related docs

- [Freshness performance hardening plan](freshness-performance-hardening-plan-2026-10-03.md)
- [Freshness core release verification](freshness-core-release-verification-2026-10-03.md)
- [Freshness R4 history verification](freshness-r4-verification-2026-10-03.md)
- [Freshness R6 scheduling and consumer verification](freshness-r6-verification-2026-10-03.md)
- [Ego Memory-Web synthesis plan](ego-web-synthesis-plan.md)

## Notes

F8 semantic reinforcement and imported-history simulation remain deferred. No scoring-policy change, nonpositive-recipient pruning, event collapse, new durable graph authority, or speculative cache is authorized by this baseline. Final P1 decisions must reflect the separation between expensive independent-event batches and one potentially expensive multi-root Dream event.
