# Freshness P1 — grouped-origin propagation verification (2026-10-03)

Parent: [Documentation index](INDEX.md). Prerequisite: [checksum-frozen P0](freshness-performance-baseline-2026-10-03.md). Governing [performance-hardening plan](freshness-performance-hardening-plan-2026-10-03.md).

## Purpose

Record P1 source ownership, independent correctness results, measured exploratory speedups and remaining release gates for behavior-preserving grouped-origin multi-source propagation. This record is not itself evidence that P1 has passed every release gate.

## Overview

**Status: P1 grouped-origin implementation accepted against the frozen P0 graph-propagation baseline, with final project-wide release gates reserved for P6.** Working branch `perf/freshness-hardening` is based on accepted P0 commit `2705670`. P1 edits `src/freshness/propagation.rs` only; accepted event records, score policy, persistence and graph proofs are unchanged. No P2 global scheduling policy, dense graph-index persistence or P4 storage optimization has been adopted.

Per event, filter excluded roots, sort/deduplicate within each immutable originating `Option<CommunityId>` and perform one multi-source strongest-path walk per origin group. Merge distinct groups by physical Memory maximum only. An unknown origin forms its own group with the existing outside-cost rule. This preserves different roots' origin-relative edge costs, excluded bridges, positive-event traversal through any stored Freshness state, and the strongest candidate for each recipient. The implementation uses an ordered sparse strength frontier and in-memory per-group best maps. P0 and matched small synthetic measurements do not justify an additional copied dense graph index before real-corpus comparison.

Intra-event parallel frontier workers were removed from this path because P0 measured synchronization overhead; the existing independent-event batch API retains its bounded parallel worker pool. `PropagationStats.worker_count=1` truthfully reports serial per-event evaluation even if the caller passes a larger cap. This is an internal implementation policy, not a change to event ordering, persisted receipts or logical effects. Any later coarse-grained single-event threading remains P2 work requiring separate evidence.

## Correctness verification

- The production `src/freshness/propagation.rs` and policy were imported directly into a standalone Rust test wrapper along with the *exact* `tests/freshness_propagation_oracle.rs` module. `rustc --edition=2024 -O --test` built it; **14/14** production-unit, policy and independent-oracle tests passed in 2.75 seconds. This bypassed unrelated external dependency compilation, not any test assertion.
- After one external Lore dependency compilation timeout, retry of `cargo test --locked --test freshness_propagation_oracle -j 2` against the **modified Reliquary crate** succeeded: **4/4** expanded independent oracle tests passed in 10.84 seconds.
- The eight targeted integration groups (`freshness_f1_reference`, `freshness_f7_acceptance`, `freshness_f7_recovery`, `freshness_owner_contract`, `freshness_r2_contract`, `freshness_reconcile_contract`, `freshness_recovery_contract`, `freshness_propagation_oracle`) passed with **46/46** tests and exit 0. This covers accepted-use receipt identity and replay, multiple Dream roots, exclusions, fixed and nondefault policies, worker/batch invariance, reopened accepted effects, recovery and reconciliation.
- Source formatting and `git diff --check` succeeded before the focused runs. Full repository/CLI final-source regression and P6 integration acceptance are separate gates.

## Exploratory matched algorithm comparison

Actual original P0 production propagation and current P1 candidate production propagation modules were imported into the same small optimized standalone Rust benchmark harness, differing only in the source paths. Both were built with the same `rustc --edition=2024 -O` toolchain, used the same deterministic 2,048-node/8,224-edge graph and were measured in alternating ABBA order over four original and four candidate process executions; each event run included 25 inner trials and each batch run 15. Source-path-normalized harness equivalence was asserted. Numbers below are medians of process-run inner medians, in microseconds.

| Workload | Original | Grouped candidate | Directional speedup |
| --- | ---: | ---: | ---: |
| +50 Dream: 8 roots in one Community | 2539.5 | 169.5 | 14.98x |
| +50 Dream: 8 roots in different Communities | 3042.0 | 1354.5 | 2.25x |
| +25 accepted use: 1 root | 114.5 | 50.5 | 2.27x |
| +25 batch: 64 independent events, 1 worker | 6920.5 | 3217.5 | 2.15x |
| +25 batch: 64 independent events, 4 workers | 2178.0 | 1037.5 | 2.10x |
| +25 batch: 64 independent events, 8 workers | 1760.0 | 887.5 | 1.98x |

The eight-root same-Community event retained **184 identical candidate recipients**, while examined edges decreased from **9,687** to **1,482** (~6.54x fewer). Different-Community roots retain independent searches and the original edge count; their speedup chiefly avoids repeated intermediate sorting and tuple allocation. This clears P1's *initial exploratory* 2x same-origin latency/edge targets and shows no sampled synthetic median regression, but does **not** qualify a whole-crate Cargo `--release` benchmark or end-to-end durable performance.

Preserved local exploratory evidence: `benchmarks/freshness-p1-exploratory-2026-10-03.zip`, SHA-256 `adcbc3c9417d98104387819fff678d017319d7d7a2f9fbded6c9b9b028675d52`. It contains all raw alternating samples, old/new benchmark source and the independently executed exact-oracle wrapper. Original source SHA-256 `a11bacc9adb4053a865ca75dc9e3a636260f0f1acfef7d7de8b50e34ac18da57`; candidate source SHA-256 `033a2b8f1046cec7b856f4eb6a357b579819620811b019c4d4d8ed905815c492`. These source hashes apply only to this exploratory candidate checkpoint.

## Matched Cargo release benchmark and P1 acceptance

The original verified P0 release benchmark executable was copied and SHA-256 verified **before** recompiling the new production Reliquary crate with identical Rust version, Cargo lock, `--release --locked --features freshness-performance-trace` and workload selection. The new Cargo release compilation completed successfully. The original benchmark executable hash is `36d3ef47f8fb42eb4b9f497e366b4df8acc7d30903a3e8f0ff0c81522dd850d6` and the new candidate binary hash is `e74cc24ab0fa4484308c43ac522328d5b879715e615098f5e1a1356b2fe03fa8`. Baseline and candidate use the same fixtures, graph-projection digest, event roots/exclusions, policies, repetition counts and Community proof treatment. The comparator verified **matching optimized build identity** and exact per-event candidate-map digests for **all 160** selected scenario/worker comparisons (68 synthetic, 36 14-day, 28 28-day and 28 Ellis). All three authoritative original fixture hashes were reverified unchanged after their disposable-copy runs.

| Graph and event (1 requested worker) | P0 p50 ms | P1 p50 ms | Improvement |
| --- | ---: | ---: | ---: |
| Synthetic: 8-root +50 same Community | 2.150 | 0.181 | 11.88x |
| 14-day: 8-root +50 same Community | 42.047 | 2.374 | 17.71x |
| 14-day: 8-root +50 different Communities | 40.582 | 17.183 | 2.36x |
| 28-day: 8-root +50, unknown membership | 45.284 | 4.332 | 10.45x |
| Ellis: 8-root +50, unknown membership | 20.128 | 0.924 | 21.79x |
| 14-day: 64 independent +25 events | 29.320 | 19.194 | 1.53x |
| 28-day: 64 independent +25 events | 9.033 | 4.603 | 1.96x |

All **160** measured scenario/worker entries improved median propagation time. Minimum observed median speedups across each suite were **1.35x** synthetic, **1.25x** 14-day, **1.32x** 28-day and **1.56x** Ellis. **Zero** entries showed a p95 regression greater than 10%; each suite's *highest* candidate/baseline p95 ratio was respectively **0.90, 0.78, 0.75 and 0.73**. These five- or eleven-repetition sample p95 values are directional maxima, not production latency percentiles, and the full-Cargo release runs were matched on inputs/toolchain but collected sequentially rather than interleaved; the separate alternating ABBA standalone experiment provides an independent same-host directional cross-check.

The persistent evidence archive `benchmarks/freshness-p1-release-2026-10-03.zip` is SHA-256 **`024de745eeb9a9607a7972e45e9f444eb0247b666652aeb89693366f32ecfef5`**. Its 16 verified entries include original qualified P0 evidence, candidate release raw/qualified outputs and per-fixture strict comparator JSON for the full 160 comparisons, with a `MANIFEST.json` recording per-file SHA, original/candidate binary hashes, source hashes, Rust version, lock identity and scope limitations. The frozen canonical P0 bundle remains unchanged and independently verifiable.

**P1 acceptance:** the independent 14-test standalone suite, expanded 4-test Cargo oracle, **46/46** targeted Freshness integration regression tests, matched optimized full-Cargo 160-case exact-output comparisons, exploratory same-Community `>=2x` latency/edge reduction, and no sampled p95 regression gate have passed. There was one initial external Lore compilation timeout before the successful cached-build retry; that timeout was not treated as passing test evidence.

**Remaining outside P1:** P2 adaptive scheduling, P3 versioned graph reuse, P4 canonical effect-postimage and ingest costs, P5 scoring breadth and P6 final whole-repository/host performance release gates. Current P1 changes have demonstrated *propagation-stage* acceleration only: P0's actual small-owner operation had much larger postimage and event-ingest costs, so no overall host throughput increase is claimed. P6 should repeat targeted benchmarks under an operationally representative workload with reliable tail sample sizes, allocation peaks and whole-operation validation.

## Independent acceptance recheck — 2026-10-03, 18:12 PDT

Current production source remains SHA-256 `033a2b8f1046cec7b856f4eb6a357b579819620811b019c4d4d8ed905815c492`. Luna independently reviewed fixed-origin grouping, unknown membership, exclusions, stronger-arrival dominance and batch ordering without finding a candidate-map correctness gap. Single-event `worker_count` is per-event traversal concurrency, not total batch concurrency; the empty/all-excluded fast path reports zero workers.

Fresh final-source verification completed with exit 0: `cargo fmt --all -- --check`; `cargo check --locked -j2`; `cargo test --locked -j2` (814 library tests, 57 integration tests, zero ignored, doc-tests passed); and the eight focused Freshness integration groups (46/46). Commands used `CARGO_TARGET_DIR=C:\Users\archa\AppData\Local\Temp\reliquary-r3-verification`, test/dev debug info disabled. Structural and changed-from-origin/main documentation checks passed. No production source changed during this recheck.

Luna verified all 16 archive payload checksums, current/original production source hashes and all 160 workload/effect-map pairs. The retained candidate executable independently matches `e74cc24ab0fa4484308c43ac522328d5b879715e615098f5e1a1356b2fe03fa8`. The original executable was unavailable in a bounded follow-up inventory; its checksum remains recorded collection-time evidence, not a newly reverified binary. Direct recomputation from archived raw results confirms 160 median improvements (minimum 1.2549x), zero >10% p95 regressions and synthetic same-origin edge reduction 9,687 to 1,482 (6.5364x), with 11.8834x median propagation improvement. No new timed measurements were collected.

P1 acceptance remains valid within its propagation-stage scope. Whole-operation and deployed-host release acceptance, adaptive scheduling and later phases remain outstanding. Changes remain uncommitted on `perf/freshness-hardening` as the execution plan reserves commit/push for final P6 gates.

## Documentation and architecture impact

Documentation impact: inspected the frozen P0 report, execution plan and current propagation contracts; updated the P1 report, plan, API, architecture, behavioral contracts, limitations, coverage and maintainer routing. Storage format and policy are unaffected. Change-impact documentation validation passed. Full live-host performance evidence remains a P6 gap.

Architecture impact: no new standard, owner, graph cache or persistence boundary. Freshness retains traversal ownership; independent events retain their existing bounded worker pool. The source remains within the existing Pitlord Freshness owner. Whole-project release verification remains P6 work.

## Related docs

- [Frozen P0 benchmark and evidence](freshness-performance-baseline-2026-10-03.md)
- [Performance hardening execution specification](freshness-performance-hardening-plan-2026-10-03.md)
- [Current Freshness implementation](architecture.md)
- [Freshness public APIs](api.md)
- [Behavioral contracts and independent oracle](behavioral-contracts.md)

## Notes

Keep the original P0 ZIP and core branch immutable. Neither this grouped implementation nor the exploration changes F8 historical/semantic reinforcement, accepted-use semantics, durable scoring or the original P0 evidence. The matched standalone results are directional and do not substitute for final source, real-corpus or live Ego integration evidence.