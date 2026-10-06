# Freshness P6 integrated release verification

Parent: [Documentation index](INDEX.md).

## Purpose

Close the integrated local release gate in [the hardening plan](freshness-performance-hardening-plan-2026-10-03.md). Execution date is 2026-10-04; the filename preserves the plan's required report name.

## Overview

P6 is accepted for integrated local release on 2026-10-04, with the documented scheduling and microsecond retry tradeoffs below. Production source is frozen at `f0340e0ea093c10b4f7174ed54ecce4786214a85`; P6 adds collection, frozen evidence and documentation. Correctness, architecture, evidence and documentation gates passed before promotion. Deployment host/provider tails remain a separate unmeasured boundary.

## Question and method

Compare original P0 `2705670dc8375d8989c3615d893b80c685cd3e5b` with final P5 `f0340e0ea093c10b4f7174ed54ecce4786214a85`, using identical diagnostic harnesses and locked release builds with `freshness-performance-trace`. The operation harness adds normalized current/historical record output after timing; production source is untouched. All source digests, binary hashes, toolchain and lockfile are frozen in the archive manifests. Whole-operation identity compares stable Memory slots because the disposable owners/IDs are generated independently. Same-owner storage-interface and transformation tests protect exact immutable receipt/effect/proof encoding; independently generated physical REL files are not claimed byte-identical.

A shared Cargo target initially reused the P0 artifacts for the second worktree. That pair was rejected before collecting measurements. The candidate crate and examples were forced to rebuild from its actual source; the collector now rejects equal binary hashes. Preserved build logs distinguish that correction from accepted measurement.

## Corpus

Synthetic topology has 2,048 nodes and 8,224 active edges. The real REL originals are hashed before and after disposable-copy opening. Current 14-day membership is retained; stale 28-day/Ellis membership is treated as unknown, with same/across cases unavailable. No historical event-frequency or score trajectory claims are inferred.

| Fixture | Nodes / edges | SHA-256 |
| --- | ---: | --- |
| ChatGPT first 14 days | 1,084 / 7,029 | `49751a62dd8644800d19976a0696a69366dc1e4f8d7c39bb4b69fbd7a8a5d18b` |
| ChatGPT first 28 days | 2,810 / 22,464 | `8f4a2bf871d7f5368edde93954a2260a29c86131b28475b323a7b6c70f8c51e4` |
| Ellis pre-relationship | 674 / 4,901 | `3efd9c666aa992686abd8aac9b77cfc7dc2a0e15d6fb26ec01f8666db175e476` |

## Correctness results

Fresh final-source gates passed: root format; locked trace-enabled tests/examples check; 821 library and 61 integration tests; CLI format/check and 19 tests. Zero failed/ignored tests; doc-tests passed. The full suite includes independent propagation oracle, negative bridges/exclusions, first-Dream proof/rewiring, same-turn/out-of-order acceptance, evicted retries, history, repack, reclamation, divergent reconciliation and crash-prefix recovery. Archive import/reopen production surfaces were unchanged; no new Archive smoke was required.

## Performance results

Matched topology matrix: 208 rows, 33 samples per scenario/requested cap. All exact reference effects matched. Two batch-4/current-14-day rows lose propagation time under the deliberately serial P2 threshold; complete receipts are evaluated separately.

| Fixture / scenario / cap | P0 p50 / p95 / p99 ms | Final p50 / p95 / p99 ms | Median speedup | Edges per unique recipient P0 / final | Origin groups |
| --- | ---: | ---: | ---: | ---: | ---: |
| synthetic / dream_roots_8_same / 1 | 2.419 / 2.702 / 3.679 | 0.183 / 0.226 / 0.290 | 13.22x | 52.65 / 8.05 | 1 |
| synthetic / dream_roots_8_unknown / 1 | 1.231 / 1.380 / 1.696 | 0.083 / 0.100 / 0.101 | 14.76x | 43.21 / 8.06 | 1 |
| synthetic / accepted_batch_64 / 4 | 2.290 / 2.869 / 2.946 | 1.173 / 1.502 / 1.628 | 1.95x | 8.05 / 8.05 | 1 |
| synthetic / accepted_batch_4 / 4 | 0.352 / 0.468 / 0.555 | 0.203 / 0.244 / 0.249 | 1.73x | 8.06 / 8.06 | 1 |
| first14d / dream_roots_8_same / 1 | 43.868 / 54.240 / 57.573 | 2.334 / 2.553 / 2.691 | 18.80x | 103.92 / 13.02 | 1 |
| first14d / dream_roots_8_unknown / 1 | 22.844 / 25.579 / 25.898 | 1.297 / 1.756 / 1.872 | 17.62x | 93.96 / 13.03 | 1 |
| first14d / accepted_batch_64 / 4 | 9.038 / 12.499 / 12.621 | 5.412 / 6.431 / 6.478 | 1.67x | 14.32 / 14.32 | 1 |
| first14d / accepted_batch_4 / 4 | 1.014 / 1.198 / 1.624 | 1.404 / 1.549 / 1.927 | 0.72x | 14.58 / 14.58 | 1 |
| first28d / dream_roots_8_unknown / 1 | 47.471 / 53.156 / 65.074 | 4.550 / 4.958 / 4.977 | 10.43x | 89.06 / 16.00 | 1 |
| first28d / accepted_batch_64 / 4 | 3.365 / 4.112 / 4.606 | 1.726 / 2.308 / 2.510 | 1.95x | 18.71 / 18.71 | 1 |
| first28d / accepted_batch_4 / 4 | 0.419 / 0.528 / 0.538 | 0.251 / 0.280 / 0.295 | 1.67x | 19.34 / 19.34 | 1 |
| ellis / dream_roots_8_unknown / 1 | 19.549 / 20.935 / 22.912 | 0.855 / 0.900 / 1.013 | 22.87x | 113.59 / 14.58 | 1 |
| ellis / accepted_batch_64 / 4 | 2.845 / 3.876 / 4.464 | 1.367 / 1.588 / 1.792 | 2.08x | 18.11 / 18.11 | 1 |
| ellis / accepted_batch_4 / 4 | 0.488 / 0.647 / 0.967 | 0.250 / 0.280 / 0.434 | 1.95x | 17.47 / 17.47 | 1 |

Complete operation stages below are milliseconds. Receipt rows time three distinct receipts, with one +25 event per listed Memory in each receipt; they are not one event or per-receipt latency. All worker caps and individual raw values remain archived.

| Stage | P0 p50 / p95 / p99 ms | Final p50 / p95 / p99 ms | Median speedup |
| --- | ---: | ---: | ---: |
| cold_rel_create | 2.581900 / 8.960400 / 8.960400 | 2.681300 / 16.873500 / 16.873500 | 0.96x |
| initial_dream_1_root_prepare_evaluate_commit | 134.748200 / 139.017500 / 139.017500 | 119.481800 / 124.034600 / 124.034600 | 1.13x |
| initial_dream_2_root_prepare_evaluate_commit | 135.535700 / 140.449100 / 140.449100 | 122.815900 / 126.176800 / 126.176800 | 1.10x |
| initial_dream_4_root_prepare_evaluate_commit | 153.383800 / 159.809800 / 159.809800 | 136.380900 / 143.571800 / 143.571800 | 1.12x |
| initial_dream_8_root_prepare_evaluate_commit | 156.016400 / 163.913000 / 163.913000 | 143.561000 / 145.838700 / 145.838700 | 1.09x |
| accepted_use_cold_single_memory_receipt | 21.028400 / 23.139900 / 23.139900 | 11.998000 / 24.103100 / 24.103100 | 1.75x |
| accepted_use_warm_exact_retry | 0.015400 / 0.019700 / 0.019700 | 0.014400 / 0.023400 / 0.023400 | 1.07x |
| accepted_use_receipts_1_memories_workers_1 | 50.482200 / 52.068300 / 52.068300 | 23.141700 / 35.650300 / 35.650300 | 2.18x |
| accepted_use_receipts_1_memories_workers_2 | 50.435500 / 53.104900 / 53.104900 | 23.381100 / 25.575600 / 25.575600 | 2.16x |
| accepted_use_receipts_1_memories_workers_4 | 50.663800 / 53.973800 / 53.973800 | 23.069600 / 25.054800 / 25.054800 | 2.20x |
| accepted_use_receipts_1_memories_workers_8 | 50.831500 / 51.648100 / 51.648100 | 23.347600 / 27.288500 / 27.288500 | 2.18x |
| accepted_use_receipts_4_memories_workers_1 | 83.781100 / 88.742500 / 88.742500 | 23.555100 / 24.695100 / 24.695100 | 3.56x |
| accepted_use_receipts_4_memories_workers_2 | 83.275700 / 86.420500 / 86.420500 | 24.302400 / 25.955700 / 25.955700 | 3.43x |
| accepted_use_receipts_4_memories_workers_4 | 85.935600 / 93.037600 / 93.037600 | 24.385000 / 25.718600 / 25.718600 | 3.52x |
| accepted_use_receipts_4_memories_workers_8 | 85.798700 / 90.761100 / 90.761100 | 24.634500 / 28.445400 / 28.445400 | 3.48x |
| accepted_use_receipts_16_memories_workers_1 | 218.394700 / 259.171700 / 259.171700 | 25.367200 / 29.354900 / 29.354900 | 8.61x |
| accepted_use_receipts_16_memories_workers_2 | 216.371900 / 224.843400 / 224.843400 | 27.194300 / 28.417200 / 28.417200 | 7.96x |
| accepted_use_receipts_16_memories_workers_4 | 278.366300 / 289.125500 / 289.125500 | 86.778600 / 91.357900 / 91.357900 | 3.21x |
| accepted_use_receipts_16_memories_workers_8 | 314.020900 / 334.415600 / 334.415600 | 118.953000 / 123.093200 / 123.093200 | 2.64x |
| accepted_use_receipts_64_memories_workers_1 | 860.660700 / 899.812500 / 899.812500 | 124.915300 / 141.417200 / 141.417200 | 6.89x |
| accepted_use_receipts_64_memories_workers_2 | 937.577100 / 947.717900 / 947.717900 | 192.590800 / 203.680100 / 203.680100 | 4.87x |
| accepted_use_receipts_64_memories_workers_4 | 962.834600 / 978.148800 / 978.148800 | 201.331900 / 228.634900 / 228.634900 | 4.78x |
| accepted_use_receipts_64_memories_workers_8 | 1025.804600 / 1054.485600 / 1054.485600 | 257.798800 / 279.159700 / 279.159700 | 3.98x |
| reopen | 425.429000 / 453.820000 / 453.820000 | 433.345500 / 455.100000 / 455.100000 | 0.98x |
| accepted_use_reopen_exact_retry | 4.050300 / 4.096200 / 4.096200 | 4.052700 / 4.119100 / 4.119100 | 1.00x |
| initial_dream_1_root_reopen_retry | 0.002700 / 0.003300 / 0.003300 | 0.002500 / 0.003400 / 0.003400 | 1.08x |
| initial_dream_2_root_reopen_retry | 0.001300 / 0.001600 / 0.001600 | 0.001300 / 0.009900 / 0.009900 | 1.00x |
| initial_dream_4_root_reopen_retry | 0.001000 / 0.001600 / 0.001600 | 0.001100 / 0.002400 / 0.002400 | 0.91x |
| initial_dream_8_root_reopen_retry | 0.001100 / 0.001800 / 0.001800 | 0.001000 / 0.001300 / 0.001300 | 1.10x |

Stationary history shapes use 64 published Memories, no active Memory graph, same-turn source-only effects, saturation at 100, and seed construction outside timed samples. Every pair matches payload bytes, accepted event count, current/historical records; each executable verifies current records after reopen.

| Depth / batch / stage | P0 p50 / p95 / p99 ms | Final p50 / p95 / p99 ms | Median speedup |
| --- | ---: | ---: | ---: |
| 160 / 1 / history_accepted_use | 13.081200 / 14.298100 / 14.298100 | 0.447600 / 0.540700 / 0.540700 | 29.23x |
| 160 / 1 / history_exact_retry | 0.013900 / 0.015700 / 0.015700 | 0.016200 / 0.017700 / 0.017700 | 0.86x |
| 160 / 64 / history_accepted_use | 240.904800 / 252.837400 / 252.837400 | 1.844800 / 1.900700 / 1.900700 | 130.59x |
| 160 / 64 / history_exact_retry | 0.210600 / 0.215900 / 0.215900 | 0.209900 / 0.227500 / 0.227500 | 1.00x |
| 1024 / 1 / history_accepted_use | 26.884200 / 29.310900 / 29.310900 | 0.486300 / 0.599500 / 0.599500 | 55.28x |
| 1024 / 1 / history_exact_retry | 0.014800 / 0.017700 / 0.017700 | 0.019500 / 0.019700 / 0.019700 | 0.76x |
| 1024 / 64 / history_accepted_use | 572.446300 / 584.585600 / 584.585600 | 1.870700 / 1.877000 / 1.877000 | 306.01x |
| 1024 / 64 / history_exact_retry | 0.208600 / 0.214800 / 0.214800 | 0.215200 / 0.239400 / 0.239400 | 0.97x |

## Regression investigation and disposition

The initial matrix preserves two propagation flags, four complete-operation flags and three historical retry flags. No samples or outliers were removed to improve ratios. Additional matched/A/A runs were sequential under the same guards.

The current 14-day batch-4 propagation p95 rose from 1.198 to 1.549 ms at cap4 and from 1.147 to 1.500 ms at cap8. This is the already accepted P2 below-16 serial tradeoff. The complete four-memory/cap4 receipt row improved from 93.038 to 25.719 ms at p95. No scheduler retuning is retained.

The next tables use microseconds. Matched operation/history rows have nine additional samples per variant; A/A rows have six samples per side from the identical candidate binary.

| Family / stage | P0 or A p50 / p95 / p99 us | Final or B p50 / p95 / p99 us | p95 ratio |
| --- | ---: | ---: | ---: |
| followup / matched / cold_rel_create | 2613.70 / 3439.50 / 3439.50 | 2734.70 / 3099.40 / 3099.40 | 0.901 |
| followup / matched / accepted_use_warm_exact_retry | 12.70 / 15.50 / 15.50 | 14.80 / 33.10 / 33.10 | 2.135 |
| followup / matched / initial_dream_2_root_reopen_retry | 1.20 / 1.40 / 1.40 | 1.30 / 1.40 / 1.40 | 1.000 |
| followup / matched / initial_dream_4_root_reopen_retry | 1.10 / 1.10 / 1.10 | 1.10 / 1.10 / 1.10 | 1.000 |
| followup / aa / cold_rel_create | 2581.40 / 3683.30 / 3683.30 | 2769.05 / 3863.10 / 3863.10 | 1.049 |
| followup / aa / accepted_use_warm_exact_retry | 15.00 / 33.70 / 33.70 | 15.00 / 29.30 / 29.30 | 0.869 |
| followup / aa / initial_dream_2_root_reopen_retry | 1.35 / 1.70 / 1.70 | 1.30 / 1.50 / 1.50 | 0.882 |
| followup / aa / initial_dream_4_root_reopen_retry | 1.15 / 1.30 / 1.30 | 1.10 / 1.20 / 1.20 | 0.923 |
| history-followup / matched / depth160 / batch1 / history_exact_retry | 14.00 / 16.70 / 16.70 | 18.90 / 24.40 / 24.40 | 1.461 |
| history-followup / matched / depth1024 / batch1 / history_exact_retry | 16.30 / 17.90 / 17.90 | 16.80 / 25.00 / 25.00 | 1.397 |
| history-followup / matched / depth1024 / batch64 / history_exact_retry | 230.80 / 376.70 / 376.70 | 224.30 / 229.90 / 229.90 | 0.610 |
| history-followup / trace-off / depth160 / batch1 / history_exact_retry | 13.70 / 20.90 / 20.90 | 12.80 / 28.50 / 28.50 | 1.364 |
| history-followup / aa / depth160 / batch1 / history_exact_retry | 16.55 / 17.00 / 17.00 | 16.50 / 17.60 / 17.60 | 1.035 |

Cold-create and settled-Dream retry flags did not reproduce: matched follow-up p95 ratios were 0.901 for creation and 1.000 for both 2/4-root settled retries. Warm accepted-use retry p95 increased again (15.5 to 33.1 us); its median increased by 2.1 us. Identical-candidate A/A medians were both 15.0 us and maxima 33.7/29.3 us. These controls demonstrate variable sample maxima; they do not erase the matched flag.

Historical depth160/batch1 retry median increased by 4.9 us and p95 by 7.7 us; depth1024/batch1 median increased by 0.5 us and p95 by 7.1 us. Depth1024/batch64 did not reproduce its flag (p95 376.7 to 229.9 us). Disabling collection at depth160/batch1 reversed the median difference (13.7 to 12.8 us), while the sample maximum still rose from 20.9 to 28.5 us. This supports an optional-observation cost but does not establish default-build or deployment tails: both executables still have the diagnostic feature compiled in.

P6 accepts these explicit microsecond retry costs with retained exact idempotence, bounded hint/cache residency and opt-in observability. The dominant regular receipt paths materially improved on both connected and deep-history fixtures. No universal retry speedup or blanket no-regression claim is made. The fixed batch-4 propagation tradeoff and slower durable-root-scan stage remain visible. New hint/cache/scheduler machinery is not introduced at this gate.



## Representative stages and memory

The eight-root Dream and first of three 64-memory/four-worker receipts below retain their exact scope positions. All times are milliseconds; stage parents are inclusive. P0 lacks the later fine-grained lookup/projection counters, so absent rows are unavailable rather than zero. Full per-scope p50/p95/p99 and raw arrays are in `measurement/trace-stage-summary.json`.

| Scope / stage | P0 p50 / p95 / p99 ms | Final p50 / p95 / p99 ms |
| --- | ---: | ---: |
| 9 / dream.duplicate_root_resolution | 0.014200 / 0.032800 / 0.032800 | 0.012200 / 0.018900 / 0.018900 |
| 9 / dream.durable_historical_graph_scan | 1.683200 / 1.740600 / 1.740600 | 6.204100 / 7.123800 / 7.123800 |
| 9 / dream.historical_graph_snapshot | 6.407900 / 9.201300 / 9.201300 | 5.756400 / 6.515200 / 6.515200 |
| 9 / dream.initial_dream_committed_roots | 1.606100 / 1.657900 / 1.657900 | 1.593800 / 1.678600 / 1.678600 |
| 9 / dream.initial_dream_graph_snapshot | 1.577400 / 1.725100 / 1.725100 | 1.610600 / 1.633100 / 1.633100 |
| 9 / dream.root_collection_persistence | 0.403900 / 0.490800 / 0.490800 | 0.388900 / 0.496400 / 0.496400 |
| 10 / dream.propagation | 0.033000 / 0.052100 / 0.052100 | 0.007400 / 0.025500 / 0.025500 |
| 11 / dream.receipt_validation_postimage_append_sync_due_refresh | 27.514500 / 37.338800 / 37.338800 | 18.576800 / 20.199100 / 20.199100 |
| 56 / accepted_use.adjacency_construction | 0.003400 / 0.004000 / 0.004000 | 0.002800 / 0.002900 / 0.002900 |
| 56 / accepted_use.community_membership_materialization | 0.000100 / 0.000100 / 0.000100 | 0.000000 / 0.000100 / 0.000100 |
| 56 / accepted_use.community_proof_pinning | 0.000100 / 0.000200 / 0.000200 | 0.000100 / 0.000200 / 0.000200 |
| 56 / accepted_use.container_append_sync | 0.778700 / 2.636800 / 2.636800 | 0.588100 / 0.899100 / 0.899100 |
| 56 / accepted_use.direct_graph_materialization | 0.002500 / 0.010300 / 0.010300 | 0.002400 / 0.002800 / 0.002800 |
| 56 / accepted_use.due_refresh | 0.049900 / 0.060100 / 0.060100 | 0.043100 / 0.048300 / 0.048300 |
| 56 / accepted_use.effect_postimage_preparation | 152.661500 / 158.546700 / 158.546700 | 31.646600 / 59.677800 / 59.677800 |
| 56 / accepted_use.exact_retry_lookup | 1.896900 / 2.116700 / 2.116700 | 0.001400 / 0.002000 / 0.002000 |
| 56 / accepted_use.propagation | 0.388700 / 0.456600 / 0.456600 | 0.355100 / 0.526300 / 0.526300 |
| 56 / accepted_use.receipt_validation | 0.000300 / 0.000600 / 0.000600 | 0.000200 / 0.000300 / 0.000300 |
| 56 / accepted_use.receipt_validation_ingest | 159.526200 / 169.068900 / 169.068900 | 33.619600 / 39.945700 / 39.945700 |
| 56 / accepted_use.root_collection | 0.004600 / 0.006400 / 0.006400 | 0.004800 / 0.005100 / 0.005100 |

Current graph enumeration and adjacency remain a few microseconds on this 12-node/15-edge, 68-Memory full-operation fixture. The representative receipt preparation median fell from 152.7 to 31.6 ms and ingest from 159.5 to 33.6 ms. Canonical projections/history scans still dominate the retained connected-graph path. The durable Dream-root scan label increased from 1.68 to 6.20 ms, while the separate graph-snapshot label fell from 6.41 to 5.76 ms and complete Dream settlement improved. This negative stage measurement is retained; historical caching remains unimplemented.

The full-operation fixture is a disposable local REL with unknown membership, fixed same-turn acceptance, real durable receipts and first-Dream prepare/evaluate/commit. It excludes provider latency and real host contention. A caller cap of one beat four on its cheap 16/64-event graph; the fixed scheduler ceiling is not a claim of universally optimal concurrency.

| Process family | P0 maximum working set MiB | Final maximum working set MiB |
| --- | ---: | ---: |
| propagation | 192.180 | 85.562 |
| operation | 10.340 | 10.156 |
| history | 8.379 | 8.648 |

Windows process peak working set includes fixture construction, reopen, serialization and output, not just propagation or storage allocations. It is not allocator peak heap, which remains unavailable. The fixed additional membership state is 8,192 bytes plus flag/alignment; recent receipt bounds remain 128 entries / 256 KiB. Positive or incomplete hints and saturation safely retain exact scans; old evicted retries and historical queries remain linear.

Raw propagation aggregates include batch event throughput, examined edges per event, exact recipient counts and origin groups. Per-event stats do not report batch concurrency: `effective_workers` is intentionally null in batch rows, and requested workers are caps. Scheduler behavior is source/test-backed; the collector does not manufacture observed worker counts.


## Frozen evidence

Archive: `benchmarks/freshness-p6-release-2026-10-04.zip`, SHA-256 `3bcd52d4ce01845f8f8dd1846ab4d1f937439cb353de388096422878666c35cd`. All 1821 payload entries are indexed by bytes and SHA-256 in `ARCHIVE-MANIFEST.json` and rehashed after packing. Executables are represented by hashes/build logs rather than included as opaque binaries.

`BUILD-SOURCES.json` and `BUILD-VALIDATION.json` bind original/final commits, all compiled production files, identical harnesses and the recorded line-ending exception. `sources/baseline` and `sources/candidate` preserve complete Rust source and Cargo manifest/lock snapshots. The two root-level Rust harness files include the post-timing identity output. `reproduction/scripts` preserves collector, guard and strict comparator source.

`measurement/aggregate-*-baseline.json` and `aggregate-*-candidate.json` contain raw latency/event samples, effects digests, edges, recipient counts, origin groups, graph identities and throughput. `comparison-*.json`, `operation-raw.json`, `operation-summary.json`, `history-raw.json`, `history-summary.json`, `trace-stage-summary.json`, `history-stage-raw.json`, `peak-working-set.json` and `MEASUREMENT-MANIFEST.json` retain the full matrix, scopes, quantiles, host observations and file provenance. Each process's stdout/stderr remains independently available.

`followup` and `history-followup` contain flagged-shape matched/A/A/collection-disabled raw samples, identities, summaries and guard observations. No contaminated attempts are treated as accepted timing. Guard observations disclose the indexing wait; unrelated OS load and between-snapshot activity are not ruled out. `CORRECTNESS.json`, `ARCHITECTURE-FINAL.json`, `COMMAND-OUTCOMES.json` and their logs retain actual exit-0 gates; final documentation outcomes are recorded alongside them.

P6 matrix job `job_dmZZ56E9ieh3JYV-tqFYLYzj`, operation investigation `job_VPKCvxSdaS49ZFQTQ4j0fbjB` and historical investigation `job_D4EERjEhQRCk0NS_JhbWLm6I` all exited 0. Frozen P4 diagnosis remains supplementary evidence; this P6 matrix independently compares original P0 with final production source.

## Build custody and commands

Host: Windows build 26200, AMD64, 16 logical processors; compiler/linker/indexer snapshots before and after each sample do not exclude unrelated OS load or activity between snapshots. Nine operation and three history processes per shape make p95/p99 sample maxima. Propagation uses 33 repetitions per scenario/cap; its sample p99 is also the maximum.

Toolchain: `rustc 1.97.1 (8bab26f4f 2026-07-14)`. Both locks have SHA-256 `5362fb09abc0995f99bb33462eede0666f9721f734f8f29cef54ab944170dd85`. No inherited release-profile/Rust compiler overrides were present. Production changes since P0 are the nine Freshness source files listed in the manifest; the P6 commit adds collection/evidence/documentation only. All 689 final Rust source files matched the frozen build, with one recorded CRLF/LF-only difference in the trace module.

| Executable | P0 SHA-256 | Final SHA-256 |
| --- | --- | --- |
| Propagation | `1b6dfda7576b2b1e655b16382db7907ad94fce828db059114247ca746c996f73` | `70e5e91edad5f9acd21a30c542d416a02937966d313c0ec03099f40ab5017507` |
| Complete operation | `a2d7a69479c42755554b2d6100921b70a3b6103761d8bd5a55218df88068e880` | `cde47754da857d628f7d6b05f551d03fb3a81022164f7b9ab9c24c97fad5a878` |

Both detached checkouts ran the same build command; the accepted candidate build was forced after rejecting artifact reuse:

```text
cargo build --release --locked -j2 --features freshness-performance-trace --example freshness_performance_baseline --example freshness_operation_baseline
python scripts/freshness_release_verification.py --out %TEMP%/reliquary-freshness-p6/measurement --rounds 3 --repetitions 11 --operation-rounds 9 --history-rounds 3
```

The archived `followup.py` and `history_followup.py` run from the repository working directory, using the frozen operation binaries. The former takes nine additional matched pairs and six identical-candidate A/A pairs. The latter takes nine matched pairs per flagged history shape, nine collection-disabled pairs at depth160/batch1, and six identical-candidate A/A pairs at that shape.

Correctness used `CARGO_TARGET_DIR=%TEMP%/reliquary-r3-verification`, `CARGO_PROFILE_DEV_DEBUG=0` and `CARGO_PROFILE_TEST_DEBUG=0`:

```text
cargo fmt --all -- --check
cargo check --locked --tests --examples --features freshness-performance-trace -j2
cargo test --locked -j2
cargo fmt --manifest-path cli/Cargo.toml -- --check
cargo check --manifest-path cli/Cargo.toml --locked -j2
cargo test --manifest-path cli/Cargo.toml --locked -j2
python scripts/check_architecture.py --refresh
python ../engineering-standards/tools/docs_policy/check.py --repo .
python ../engineering-standards/tools/docs_policy/check.py --repo . --changed-from origin/main
git diff --check
```

The six Rust/CLI commands exited 0 in job `job_Yi_SguO9fzdouf2sLwCbu8YM`. Final architecture job `job_R1LaURg9BKwvy5MYtEbitytE` exited 0 using `LEXICON_ADAPTERS=%TEMP%/reliquary-freshness-architecture-adapters`: snapshot `sha256:991380d2b16b0385b0de4c95e665cafe90aa21126522e2a16950dfa7065df1da`, six rules, 50,452 nodes, zero relationships. That is a limited configured static gate; behavioral tests substantiate runtime guarantees.

## Limitations

Deployment provider/host latency, mutex-wait tails, production event frequencies and genuine historical corpus trajectories remain unavailable. Process working set does not establish allocator peak heap. Local p95/p99 values are finite-sample quantiles, not deployment SLO evidence. v1 policy remains unchanged; P3 remains deferred/no change and P5 retains v1.

## Documentation impact

- Inspected: hardening and execution plans, frozen P0/P1/P2/P4/P5 reports, architecture, invariants, storage format, maintainer map, limitations, roadmap, development and coverage.
- Updated: this report, hardening/execution status, architecture acceptance note, limitations, maintainer routing, coverage, development and index.
- Not affected: public API, durable format, score policy, receipt/proof encoding and Ego delivery.
- Compliance check: structural, changed-from-origin/main and staged/working-tree whitespace checks passed; checks repeated after archive-hash insertion with matching archived documentation output.
- Known documentation gaps: deployment distributions and historical calibration are unavailable.

## Architecture impact

- Standards added or changed: none.
- Ownership or boundary impact: no production change; diagnostic collection observes existing interfaces.
- Pitlord enforcement impact: policy unchanged; final source refresh recorded in this report.
- Other verification impact: final behavioral suite and exact baseline/candidate diagnostics.
- Known architectural gaps: static relationship output is empty; behavioral tests substantiate runtime guarantees. Positive evicted lookup and saturated hints retain scan costs.

## Related docs

- [P4 receipt lookup verification](freshness-performance-p4-verification-2026-10-04.md)
- [P5 breadth assessment](freshness-performance-p5-breadth-2026-10-04.md)
- [Development](development.md)
- [Current limitations](current-limitations.md)

## Notes

Commit and push follow final gates. Main merge, deployment, worktree pruning, Ego and F8 are separate milestones.
