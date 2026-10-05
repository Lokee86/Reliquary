# Freshness P5 — propagation breadth assessment (2026-10-04)

Parent: [Documentation index](INDEX.md).

## Purpose

Complete P5 of the [performance hardening plan](freshness-performance-hardening-plan-2026-10-03.md) after accepted P4 commit `c7f59dcb0856de43d400ebc105bb34fde84a3ea7`. Assess positive-event reach without changing the accepted v1 policy.

## Overview

**Research status: P5 measurement and policy disposition are accepted. Retain v1 unchanged; P6 integrated verification remains outstanding.** This is prospective graph-topology evidence, not measured production relevance or historical score trajectories. No inference was run and no authoritative fixture was modified.

All 896 events completed: 288 synthetic, 304 14-day, 152 28-day and 152 Ellis. Every origin-group reconstruction equaled the canonical candidate map; every unknown-membership event equaled an independent shortest-path arithmetic oracle. Report and raw evidence are retained.

## Question

Does nearly full-Web reinforcement arise only from artificial eight-root Dream events or stale Community membership? How do +25 single-root accepted-use events differ from +50 first-cycle Dream events?

## Method

`examples/freshness_breadth.rs` calls the existing production `propagate_with_stats` with one worker and default v1 policy. It records roots, degree of the anchor, principal, layout, membership mode, candidate-map digest, touched fraction, examined edges per unique recipient, full effect/hop histogram and separate origin-group histograms.

Select 16 evenly spaced lexical quantile anchors plus minimum/median/maximum-degree anchors, then deduplicate: 18 synthetic anchors and 19 per real fixture. For each anchor, measure +25 with one root and +50 with 1/2/4/8 roots. Clustered roots are nearest by unweighted graph distance, breaking ties by stable Memory ID; dispersed roots are evenly spaced in lexical ID order. Lexical dispersion is a deterministic topology control, not semantic diversity or a production root distribution. Clustered roots may cross Communities. No excluded cohort is fabricated: all roots are hypothetical existing Memories with no exclusions. Existing behavior/oracle suites retain exclusion coverage.

Use valid current membership when available, then repeat the same roots with all membership removed. Stale/missing snapshots remain unknown; do not regenerate Communities. The known-member experiment is consequently unavailable on 28-day and Ellis.

**Denominator:** the unique active Graph endpoint universe (including both ends), not every published MemoryStore record. Isolated/unrepresented Memories are outside this prospective projection. “Nearly full Web” means ≥90% of that graph universe. Rates are fractions of deterministic anchor scenarios, not production occurrence probabilities.

**Hop interpretation:** shortest unweighted distance from any root of the event or origin group; it need not be the path that wins under Community-dependent costs. Per-origin histograms overlap and must not be summed as unique final recipients. Their maximum-per-Memory reconstruction must equal the final canonical map. Unknown membership has a uniform −10 hop cost, so its independent oracle is exactly `max(principal - 10 * shortest_distance, 0)`, retaining positive results only.

No durable events are appended and no stored-score trajectory is inferred. These are event strengths/effects; saturation, decay, event frequency and admission timing are outside the measurement.

## Corpus and custody

| Fixture | Graph nodes / undirected edges | Snapshot | Source SHA-256 |
| --- | --- | --- | --- |
| Synthetic | 2,048 / 8,224 | Frozen synthetic membership | Deterministic generator archived |
| 14-day | 1,084 / 7,029 | Current | `49751a62dd8644800d19976a0696a69366dc1e4f8d7c39bb4b69fbd7a8a5d18b` |
| 28-day | 2,810 / 22,464 | Stale/missing, treated unknown | `8f4a2bf871d7f5368edde93954a2260a29c86131b28475b323a7b6c70f8c51e4` |
| Ellis | 674 / 4,901 | Stale/missing, treated unknown | `3efd9c666aa992686abd8aac9b77cfc7dc2a0e15d6fb26ec01f8666db175e476` |

Source paths and unchanged digest custody follow [P0 evidence](freshness-performance-baseline-2026-10-03.md). Each real input was hashed against its pinned digest, copied, copy-checked, opened only on the disposable copy and source-checked afterward. All three source checks passed.

## Results

### Scenario-stratified reach

One-root and eight-root rows are shown here; every 2/4-root row remains in archived `summary.json`. Membership “available” means valid current membership on 14-day or frozen membership on synthetic.

| Fixture / membership | Event | Roots / layout | Samples | Reach min / median / max | ≥90% reach rate | Edges per recipient median |
| --- | --- | --- | ---: | --- | ---: | ---: |
| synthetic / available | +25 | 1 / clustered | 18 | 3.1% / 3.2% / 3.6% | 0.0% | 8.05 |
| synthetic / all_unknown | +25 | 1 / clustered | 18 | 1.4% / 1.4% / 1.9% | 0.0% | 8.03 |
| synthetic / available | +50 | 1 / clustered | 18 | 4.0% / 6.1% / 9.0% | 0.0% | 8.06 |
| synthetic / all_unknown | +50 | 1 / clustered | 18 | 3.1% / 3.6% / 7.5% | 0.0% | 8.06 |
| synthetic / available | +50 | 8 / clustered | 18 | 6.1% / 7.5% / 12.1% | 0.0% | 8.06 |
| synthetic / all_unknown | +50 | 8 / clustered | 18 | 3.2% / 5.0% / 10.6% | 0.0% | 8.07 |
| synthetic / available | +50 | 8 / dispersed | 18 | 32.0% / 48.4% / 70.3% | 0.0% | 8.07 |
| synthetic / all_unknown | +50 | 8 / dispersed | 18 | 25.0% / 28.9% / 59.8% | 0.0% | 8.06 |
| 14d / available | +25 | 1 / clustered | 19 | 5.4% / 19.6% / 27.9% | 0.0% | 14.55 |
| 14d / all_unknown | +25 | 1 / clustered | 19 | 1.6% / 10.7% / 20.3% | 0.0% | 15.15 |
| 14d / available | +50 | 1 / clustered | 19 | 95.5% / 98.7% / 99.9% | 100.0% | 13.09 |
| 14d / all_unknown | +50 | 1 / clustered | 19 | 42.3% / 87.1% / 97.4% | 31.6% | 13.53 |
| 14d / available | +50 | 8 / clustered | 19 | 99.1% / 99.9% / 100.0% | 100.0% | 38.63 |
| 14d / all_unknown | +50 | 8 / clustered | 19 | 92.3% / 98.9% / 99.7% | 100.0% | 13.09 |
| 14d / available | +50 | 8 / dispersed | 19 | 99.9% / 100.0% / 100.0% | 100.0% | 77.48 |
| 14d / all_unknown | +50 | 8 / dispersed | 19 | 99.2% / 99.7% / 99.9% | 100.0% | 13.00 |
| 28d / all_unknown | +25 | 1 / clustered | 19 | 0.7% / 4.3% / 16.2% | 0.0% | 17.98 |
| 28d / all_unknown | +50 | 1 / clustered | 19 | 31.0% / 63.2% / 95.2% | 5.3% | 17.01 |
| 28d / all_unknown | +50 | 8 / clustered | 19 | 84.9% / 95.9% / 99.3% | 84.2% | 16.22 |
| 28d / all_unknown | +50 | 8 / dispersed | 19 | 98.8% / 99.5% / 99.9% | 100.0% | 16.03 |
| ellis / all_unknown | +25 | 1 / clustered | 19 | 0.4% / 15.1% / 40.2% | 0.0% | 17.07 |
| ellis / all_unknown | +50 | 1 / clustered | 19 | 1.0% / 91.1% / 99.0% | 52.6% | 15.16 |
| ellis / all_unknown | +50 | 8 / clustered | 19 | 95.0% / 98.7% / 99.7% | 100.0% | 14.69 |
| ellis / all_unknown | +50 | 8 / dispersed | 19 | 99.0% / 99.6% / 100.0% | 100.0% | 14.60 |

### Full single-root effect histograms

Counts pool event-recipient pairs over the anchors; they are neither distinct union recipients nor production frequency.

| Fixture / membership | +25 effects (effect:count) | +50 effects (effect:count) |
| --- | --- | --- |
| 14-day / current | 5:1856, 10:1025, 15:684, 20:205, 25:19 | 5:762, 10:2907, 15:4389, 20:5335, 25:3099, 30:1856, 35:1025, 40:684, 45:205, 50:19 |
| 14-day / unknown | 5:1747, 15:273, 25:19 | 10:8660, 20:5909, 30:1747, 40:273, 50:19 |
| 28-day / unknown | 5:2079, 15:328, 25:19 | 10:22844, 20:9172, 30:2079, 40:328, 50:19 |
| Ellis / unknown | 5:1697, 15:288, 25:19 | 10:3961, 20:3894, 30:1697, 40:288, 50:19 |

Under unknown membership these same counts give the exact hop histogram: +25 effects 25/15/5 correspond to hops 0/1/2; +50 effects 50/40/30/20/10 correspond to hops 0/1/2/3/4. In current 14-day single-root +50, shortest-hop counts are 0:19, 1:273, 2:1747, 3:5909, 4:8660, 5:3458, 6:215. Full per-event, per-origin and synthetic histograms are in the raw archive.

### Effect strength and root distribution

| Fixture / membership | Single-root +25 recipients at ≤10 | Single-root +50 recipients at ≥30 | Eight dispersed roots +50 recipients at ≥30 |
| --- | ---: | ---: | ---: |
| 14-day / current | 76.0% | 18.7% | 76.9% |
| 14-day / unknown | 85.7% | 12.3% | 53.4% |
| 28-day / unknown | 85.7% | 7.0% | 31.1% |
| Ellis / unknown | 84.7% | 20.3% | 71.6% |

These percentages pool event-recipient counts within the named shape. Broad single-root +50 reach often contains weak effects, but broad eight-root reach cannot be dismissed as exclusively weak: dispersed roots materially raise strong-effect coverage. The 14-day eight-root clustered cases use 1–4 known origin groups; dispersed cases use 4–8. Synthetic clustered cases use 1–2, dispersed cases eight. These are observed group counts, not assumptions that a spatial cluster stays within one Community.

## Interpretation and policy disposition

1. **Ordinary +25 reinforcement is narrower than Dream.** No sampled +25 event reached ≥90%. Native fixture medians were 19.6% on current 14-day, 4.3% on unknown 28-day and 15.1% on unknown Ellis. Much of that reach receives only +5/+10. This does not prove repeated accepted-use events cannot accumulate broad scores.
2. **Large Dream reach is not solely an eight-root artifact.** Current 14-day single-root +50 reaches ≥90% in all 19 scenarios, with a median 98.7%. Unknown Ellis has median 91.1%, though a disconnected/low-degree anchor is much narrower. Actual root placement and connectivity matter.
3. **Stale membership does not artificially increase reach under this policy.** On the same 14-day topology/roots, removing membership raises same-origin hop costs to −10 and reduces median +50 single-root reach from 98.7% to 87.1%. Across different corpora this paired effect cannot be inferred from their medians alone.
4. **Multiple dispersed roots increase both breadth and strong-effect coverage.** This is expected under v1's strongest-positive-path rule and independent immutable origin costs. Synthetic topology stays much narrower than the real corpora, demonstrating that graph structure matters.
5. **Retain v1; make no v2 change in this milestone.** The breadth risk is real topology evidence, but the available data cannot establish whether the reached Memories are semantically inappropriate or whether these root shapes routinely occur. Introducing truncation or a new cutoff would alter accepted effects without semantic-quality evidence. This is a decision to preserve accepted behavior, not a claim that v1's selectivity is proven suitable for Ego.

A future policy review should test whether semantically unrelated Memories receive high effects in genuine first-cycle settlements and whether those effects degrade Ego selection after real event frequency/decay. If demonstrated, separately specify v2 with explicit replay/version/compatibility implications. P5 creates no such implementation requirement and does not block P6.

## Testable follow-up hypotheses

| Hypothesis | Evidence needed to resolve it |
| --- | --- |
| Frequent real multi-root Dream settlement gives high reinforcement to unrelated areas | Authorized actual root-count/origin distributions plus semantic labels for strong recipients |
| Repeated small +25 events flatten relevance despite their narrower individual reach | Genuine accepted-use/admission/turn evidence or clearly detached synthetic histories with stated schedules |
| Valid Communities improve semantic locality despite larger geometric reach | Paired semantic recipient evaluation on the same topology, keeping root/effect policy pinned |
| Eight-root prospective benchmarks overstate normal Dream breadth | Actual first-cycle root distribution; no inference-based corpus reprocessing is required by P5 |

## Retained artifacts and reproduction

Archive: `benchmarks/freshness-p5-breadth-2026-10-04.zip`, SHA-256 `a7f2931224f6ab5123a41abaf53cb3b033341d80fa3d5e29945916228e9f3a59`. It contains four raw JSON datasets, all-shape summary, exact diagnostic/collector source and a per-entry SHA-256/size manifest. Every payload entry was rehashed after packing.

Production parent: `c7f59dcb0856de43d400ebc105bb34fde84a3ea7`. The archive's `source_commit` is this parent plus the archived uncommitted diagnostic source, not a claim that the parent contains the new example. Final phase commit contains this report and those sources. Diagnostic binary SHA-256: `49307c1a83ddafe0f9949477e292a13167f9ac009eb7ce5a9a70a33816ee8e80`; Rust diagnostic source SHA-256: `b1872425d5c0641cd20c4fc6e8ea63798dde0c0dd82eb25df45223673fcb6309`.

Commands from the performance worktree, with `CARGO_TARGET_DIR=%TEMP%\reliquary-freshness-perf-baseline`:

```text
cargo build --release --locked -j2 --example freshness_breadth
python scripts/freshness_breadth_report.py --binary %TEMP%\reliquary-freshness-perf-baseline\release\examples\freshness_breadth.exe
```

The first release library build took 4m19s; final diagnostic rebuild took 8.77s. Collection completed in 13.1s. Concurrent compilation existed on another worktree; no elapsed-time performance results are inferred. Collection duration is operational evidence only.

## Verification

Final example build and all four diagnostic runs exited 0. All 896 origin-group comparisons, 600 unknown-mode BFS comparisons, histogram-count checks and fixture-custody checks passed. Archive checksums passed. Full root tests passed: 821 library tests and 61 integration tests, zero failures/ignored, plus doc-tests. The new example compiled in both release and the ordinary test build. Format, example check, both documentation gates and whitespace checks complete the final verification. CLI/architecture refresh is not required: no CLI, public library surface, production source or ownership boundary changed.

Correctness uses the warmed target outside the checkout, with dev/test debug disabled and bounded jobs:

```text
cargo test --locked -j2
cargo fmt --all -- --check
cargo check --locked -j2 --example freshness_breadth
python ../engineering-standards/tools/docs_policy/check.py --repo .
python ../engineering-standards/tools/docs_policy/check.py --repo . --changed-from origin/main
git diff --check
```

The first documentation gate detected an accidental truncated replacement of the large development guide. Its full HEAD content was restored before retaining the additive P5 section and running the final gates. No production source was changed.

## Limitations

No production event-frequency distribution, semantic relevance labels, history-based stored-score trajectories, source-only-versus-full-Web semantic evaluation, rebuilt Communities on stale fixtures, live Ego output-quality evaluation, allocation peak or latency measurement. Anchor selection is deterministic coverage, not random statistical sampling. No current-membership conclusion can be claimed for 28-day or Ellis. The graph denominator excludes isolated/unrepresented Memories; raw endpoint coverage must not be advertised as every published Memory. Root exclusions were empty prospective existing-Memory events; this does not replace accepted historical first-cycle proofs or their tests.

## Documentation impact

- Inspected: AGENTS, engineering documentation standards/procedure, hardening/P2–P4/P0/P4 evidence, development, coverage, maintainer routing, roadmap and limitations.
- Updated: this research report, P5 status in hardening, index, diagnostic commands/routing/coverage and current breadth limitation.
- Not affected: API, architecture, accepted event policy, storage format, source ownership and behavioral contracts.
- Compliance check: full root tests, release/example check, format, structural and changed-from-origin/main documentation gates, and whitespace checks.
- Known documentation gaps: actual event frequencies and semantic-quality/trajectory evidence remain unavailable.

## Related docs

- [Hardening plan](freshness-performance-hardening-plan-2026-10-03.md)
- [P0 evidence](freshness-performance-baseline-2026-10-03.md)
- [P4 verification](freshness-performance-p4-verification-2026-10-04.md)
- [Development](development.md)
- [Current limitations](current-limitations.md)

## Notes

P5 ends here. No policy/production-source change, inference processing, semantic reinforcement, imported-history simulation, P6 execution, merge or worktree cleanup is included.
