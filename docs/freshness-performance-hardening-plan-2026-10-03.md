# Freshness Performance Hardening — Ego Foundation (2026-10-03)

Parent: [Documentation index](INDEX.md). This is a performance-only follow-up to the [verified Freshness core](freshness-core-release-verification-2026-10-03.md) and a prerequisite for deploying [Ego Memory-Web synthesis](ego-web-synthesis-plan.md) at scale. Governing semantics: [ADR 0039](decisions/0039-unified-deterministic-memory-freshness.md), [Freshness numerical and event policy](memory-staleness-plan.md), and [completed core execution plan](freshness-completion-execution-plan-2026-10-03.md).

**Status:** P0 measurement foundation is accepted; P1 grouped-origin propagation is implemented and accepted; P2 fixed conservative event-batch scheduling is accepted; P3 current-graph caching is deferred because P0 measured graph enumeration/adjacency construction at only ~0.003–0.007 ms on the local operation path; P4 is accepted after repeated absent-identity durable scans were measured as dominant and replaced by a fixed owner-local negative membership hint, with exact positive fallback and both projections retained. The [P4 report](freshness-performance-p4-verification-2026-10-04.md) freezes the local matched evidence. The narrower P2–P4 execution is complete; P5/P6 remain outside that authorization. Final whole-operation and deployed-host verification remain outstanding. Starting implementation baseline: `feat/deterministic-memory-freshness` at `20e1e9ab` (verified, pushed on 2026-10-03). Scope excludes F8/S1–S6 semantic activity relevance and imported-history simulation; those remain deferred.

## Purpose

Remove measured Freshness bottlenecks before Ego depends on it routinely. Treat each optimization phase as a hypothesis that must survive P0 evidence rather than machinery that must be implemented. Optimize the *whole accepted-use and initial-Dream settlement operations*, not an isolated inner-loop benchmark. Preserve every accepted event, historical proof, physical effect, current/historical score, and authorized REL scope.

## Overview

The verified core passed 814 library unit tests, 54 integration tests, 19 CLI tests, architecture checks and corpus calibration. Its original propagation calibration is diagnostic, not a production workload trace: it runs 64 synthetic eight-root events per repetition, alternating +25/+50, using a test-profile binary. Actual accepted-use reinforcement records **one independent +25 event per consumed Memory**; initial Dream settlement records **one +50 event with potentially multiple pre-existing roots**. These must be benchmarked separately. The 28-day and Ellis fixtures had stale Community snapshots, so their recorded events paid outside-Community costs.

A 2026-10-03 optimized, standalone Rust diagnostic imported the production Freshness propagation and policy source; an experimental grouped-origin algorithm preserved the exact candidate maps in every exercised case. On the 2,048-node/8,224-edge synthetic graph, a same-Community eight-root +50 event examined 7,806 edges in the existing implementation versus 1,482 when grouped, taking median 2,464 versus 227 microseconds. One-root +50 measured 457 versus 233 microseconds, showing that fewer temporary candidate collections/sorts also matter. Four workers *within a single event* were slower in every diagnostic case. Processing 16 independent +25 single-root events with four event-level workers improved one current-Community synthetic batch from 2,570 to 1,236 microseconds, while a cheaper no-Community batch was approximately unchanged (430 versus 435 microseconds). These are **synthetic diagnosis**, not a claimed release speedup, real-corpus acceptance, or evidence that all root distributions benefit equally.

Existing test-profile corpus calibration recorded 14-day (1,084 nodes/7,029 edges), 28-day (2,810/22,464) and Ellis (674/4,901) Webs. Median distinct reach was 978, 1,283 and 573 Memories respectively for the artificial mixed events; high-percentile reach approached the entire Web. Parallel batch speedup was workload-dependent, including regression for every tested worker count on 28-day. Never use these figures as historical score trajectories or as evidence of normal production event frequency.

## 1. Hard boundaries and invariants

This milestone is *behavior-preserving*. Pin the policy and accepted event version. Do not change signed score [-100,+100], accepted-turn decay/remainder, +100 birth, +25 accepted use, +50 genuine first-cycle linkage, -5 same-origin-Community and -10 other/unknown hops, positive-path termination, per-event strongest eligible physical result, independent accepted-use events, first-cycle exclusions, or Dormant traversability. In particular, **an intermediate Memory with a negative score cannot terminate traversal**; propagation evaluates positive *event strength*, not current stored score. Group origins using the **root's immutable Community membership at the accepted event's pinned snapshot**, never the current traversal node's Community or a refreshed live snapshot. All absent/stale Community membership is the existing unknown/outside-cost case.

Keep original immutable owner UUID, policy version, accepted Archive turn/event ordering, settled Dream identity, graph and Community proof, original roots and exclusions, accepted-use receipt identity, per-event effects and postimages, reopen/replay, and divergent reconciliation behavior. Existing exact old-retry checks must resolve durable accepted effects *before* recomputing a live graph. Reordering independent computation must not reorder or combine durable logical events. No new authority, journal format, daemon, general scheduler, extra activity clock, unbounded cache or compatibility façade.

Preserve owner and lock boundaries: Memory semantic revisions stay in MemoryStore; Archive owns turn cuts; Graph/Dream own graph and first-pass provenance; Freshness owns canonical scoring; Cva composes and commits. Detached Dream propagation may evaluate frozen graph inputs outside the owner mutex, with the existing proof/idempotence recheck on commit. Any reusable derived graph snapshot must be explicitly keyed, disposable, bounded and invalidated when its proof changes. Never substitute a current graph for a historical accepted cut.

Preserve the current Ego selection semantics: a full small Web is kept when it fits, hard-keeps survive selection budgets, Dormant Memories remain searchable, and passive searches, hydration, vector reads and summaries do **not** emit synthetic +25 consumption receipts. The eventual real accepted-context delivery producer belongs to later Ego/host integration, not this performance milestone.

## 2. Custody, benchmark hygiene and shared source ownership

Use an isolated performance branch/worktree **created from the verified core commit**; leave the core baseline unchanged for A/B. Inspect Git status, worktrees, active jobs and actual source before editing; do not assume a stale path or a prior session's active ownership. Isolate performance edits from the concurrent multiplexing worktree. One implementation owner at a time controls `src/freshness/propagation.rs`, `src/runtime_inference/freshness_access.rs`, `src/runtime_inference/dream_freshness.rs` and `src/facade/cva_freshness.rs`. Independent benchmark fixtures or oracle tests may be delegated after inputs/interfaces are frozen. Do not run conflicting Cargo jobs against the same target.

**Build environment:** the previously verified target outside `C:\!bin` mitigated an installed Cargo artifact reclaimer. Use an isolated target under the user's temporary directory, `CARGO_PROFILE_TEST_DEBUG=0`, bounded `-j 2` for correctness builds, and explicitly record dependency-build timeouts as infrastructure outcomes. For performance measurement compile baseline and candidate with the **same optimized profile**, Rust version, Cargo lock, feature flags, allocator and CPU/workload conditions; distinguish compilation from measured execution. Do not interpret old unoptimized test-profile measurements as comparable with new optimized runs.

Protect authoritative REL fixtures: SHA-256 before and after; copy to disposable writable locations; verify copy hashes; never mutate sources. Avoid repeatedly running multi-minute full-corpus sweeps during development: use synthetic/small fixture iterations, one controlled real baseline, focused diagnostic reruns and one final matched real acceptance sweep. Preserve machine load, profile, worker count, graph/Community freshness and repetitions alongside raw results. Benchmark samples should alternate baseline/candidate ordering or run in matched interleaved rounds rather than always measuring the candidate on a differently loaded host.

## 3. P0 — truthful full-operation baseline and trace design

### P0.1 Freeze workload shapes

Extend `examples/freshness_calibration.rs` or add a separate deterministic diagnostic without breaking its existing fixture, hash or output contracts. Define at least these scenario families: accepted-use single-root +25; batches of 1, 4, 16 and 64 **separately identifiable** single-root +25 events; first-cycle +50 with 1, 2, 4 and 8 roots distributed within one Community, across Communities and all-unknown membership; excluded cohort roots/bridges; cold/open and warm/reused graph; current and pinned historical Community/graph snapshots. Sample actual Dream root-count and accepted-use batch-size distributions from authorized runtime telemetry or deterministic scenario fixtures; do not invent a production distribution when no observable source exists.

Report each operation's root count, unique origin groups, graph nodes/edges/active relation kinds, Community snapshot validity, original and grouped distinct reach, propagation candidate expansions, examined edges, maximum live frontier, temporary allocations/peak resident bytes where measurable and all 1/2/4/8-worker results. Distinguish individual event latency from full batch latency and event throughput.

### P0.2 Profile the complete accepted-use and Dream paths

Instrument separately, without adding per-edge logging in the hot path: exact-retry lookup, direct graph materialization, Community proof pinning, root collection, duplicate-root representative resolution, durable historical graph scanning, propagation, effect postimage preparation, receipt validation, Container append/sync, due refresh, runtime mutex ownership/wait and reopen where applicable. Capture cold versus warm paths. A historical graph scan and an event with identical topology are separate dimensions: historical provenance must never be approximated by a current graph.

For historical scans, record bytes and chunks visited, graph mutations decoded, Community snapshots encountered, version-filtered records and time spent in `initial_dream_committed_roots` and `initial_dream_graph_snapshot`. For accepted use, record the cost of `graph_relations` enumeration, adjacency construction and proof lookup. Measure outside the critical section for offline diagnostic calculations but retain separate end-to-end timings inside the real orchestrator.

### P0.3 Baseline acceptance

Produce `docs/freshness-performance-baseline-2026-10-03.md` with precise commands, fixture hashes, optimized build identity, per-scenario p50/p95/p99, peak memory and stage timings, reproducible JSON/CSV data locations, known missing production distributions and the top three measured bottlenecks. Include a pinned before/after comparison harness and the unchanged core correctness gates. No optimization phase is called successful without this baseline. Missing inference-provider or real host latency must be labeled missing, never approximated as end-to-end production latency.

## 4. P1 — canonical grouped-origin, multi-source propagation

**Primary source:** `src/freshness/propagation.rs`. Retain its narrow public event interfaces unless a measured need justifies change.

### P1.1 Replace redundant search internally

Canonicalize surviving roots: filter exclusions, sort/deduplicate, and group by `Option<CommunityId>` from the frozen accepted membership map. For each unique origin group, seed **all group roots** at the event principal into one strongest-first multi-source traversal. Track the greatest event strength per physical Memory **within that origin group**, and merge groups by physical Memory maximum at the end. Different origins must not collapse, even if their paths overlap, because the costs depend on the originating root's Community. Unknown origins may collapse with other unknown origins: every hop in that group has the same outside cost.

Use a deterministic dense Memory-ID-to-index projection and bounded integer-strength buckets (subject to P0 allocation results) instead of repeatedly allocating sorted candidate vectors and tree-map frontier entries. Build the index once per frozen graph, if profitable; if dense projection costs exceed savings on tiny graphs, retain a narrow efficient small-graph path. Preserve undirected active Memory-relation semantics and existing cross-kind neighbor deduplication. Disallow bridge and recipient inclusion for event-excluded Memories. Scores exactly equal to zero never enqueue a positive propagation candidate. Avoid traversal tied to current Memory score/dormancy.

Prove the equivalence: all roots in the same immutable origin group share identical edge-cost functions; the maximum-remainder path from *any* root is the multi-source maximum. Merging origin groups by physical maximum is identical to the original per-root search's output. Do not change root credit, event principal, duplicate suppression or receipt identity.

### P1.2 Independent correctness oracle

Retain and expand `tests/freshness_propagation_oracle.rs`: compare against its independent exhaustive simple-path reference, **not merely against old production output**. Cover all four-node undirected graphs, every supported origin/missing-Community partition, all relevant excluded-root/bridge combinations, duplicate and unsorted roots, multi-origin convergence, cycles, re-entry, empty/disconnected graphs, default and coherent nondefault pinned policies, +25/+50 and worker settings. Add deterministic larger randomized differential cases with fixed seeds and independent-event batches; assert exact per-event candidate maps and array ordering. Include adversarial same-origin roots where a weak earlier path is superseded by a stronger one.

Run R2–R6 regression coverage after this phase and show identical replay/postimage results on accepted receipts and first-cycle recovery, not merely standalone propagation equality.

### P1.3 Performance gate

In matching optimized synthetic eight-root/same-origin +50 benchmarks, target at least **2x lower median propagation latency and 2x fewer examined edges**; these are a provisional minimum supported by the prior diagnostic, not a universal promise. Require no material regression in single-root, separated-origin and unknown-Community scenarios; investigate any repeatable >10% p95 loss before promotion. Report reach and full candidate-map equality; never manufacture a speedup by dropping low-strength recipients. If dense representation loses on a small graph, select the measured smaller path rather than forcing one implementation universally.

## 5. P2 — conservative event-batch parallelism

**Primary source:** `src/freshness/propagation.rs`; consumer: `src/runtime_inference/freshness_access.rs`.

P2 is deliberately simple. P1 owns the single-event propagation algorithm. P2 only decides whether a batch of **independent accepted-use events** is large enough to justify one bounded event-level worker pool.

### P2.1 Fixed rule

- One event is always serial P1 grouped-origin propagation.
- Batches with fewer than **16 events** are serial.
- Batches with **16 or more events** may use one bounded event-level pool.
- The production pool is capped at **4 workers**, further limited by caller cap, available parallelism and event count.
- No topology classifier, Community-based dispatch rule, two-hop estimator, adaptive predictor, nested pool, frontier parallelism, work stealing or persisted scheduler state.
- Preserve event identity and input ordering; concurrency may change wall-clock completion order only.
- The counterless production path may remain if it is exactly equivalent to the stats path except for diagnostic accounting.

The fixed 16-event threshold intentionally gives up the measured batch-4 win on the 14-day graph. That tradeoff is accepted to keep the scheduler predictable and maintainable. P1 evidence showed 16- and 64-event batches are the first measured sizes with consistent parallel benefit across the tested topologies.

### P2.2 Acceptance

Verify exact candidate-map equality, stable event ordering, worker-cap behavior, failure/join semantics and serial behavior below 16. Run matched release measurements primarily at production-relevant 1/2/4-worker caps. P2 is accepted when 16/64-event batches retain material benefit on representative large topologies and the fixed rule introduces no repeatable material regression in the complete accepted-use operation. Do not tune production code against p95 noise in unchanged Dream/single-event paths.

## 6. P3 — deferred graph-preparation hypothesis

**No P3 implementation is currently authorized.**

P0 measured accepted-use graph enumeration and adjacency construction at roughly **0.003–0.007 ms**, while accepted-use postimage/history processing measured in the **multi-millisecond to >100 ms** range. Building current-graph caches, graph-version invalidation machinery or reusable projections to save microseconds is not justified by the evidence.

Keep current graph preparation as-is unless a later full-operation measurement shows it has become material after larger bottlenecks are removed.

Historical Dream preparation remains a separate measurement question. P0 observed historical root/graph scans in the millisecond range on a small local fixture, but did not establish large-history scaling. Before any historical cache, combined scan or derived projection is implemented, first benchmark a realistic large accepted history. If that measurement shows a material bottleneck, write a focused follow-up specification preserving the exact accepted graph cut/proof. Otherwise P3 closes as **deferred/no change**.

## 7. P4 — accepted-use postimage/history hot path

P4 is the next active optimization phase after P2 because P0 identified this as the dominant observed full-operation cost. On the local operation baseline, propagation was sub-millisecond while effect postimage preparation and canonical receipt ingest/replay consumed milliseconds to hundreds of milliseconds as history grew.

First split the measured cost into its actual components: effect/postimage construction, canonical accepted-history lookup/replay, due refresh, payload serialization, append/sync and exact retry lookup. Optimize only the component demonstrated to dominate. Do not assume the journal format or persistence layer is the problem before that microprofile exists.

Retain bounded receipt residency (128 recent receipts with a conservative 256-KiB ceiling), bounded sorted history runs, immutable accepted effects, original accepted clock cuts, corrupt-prefix refusal and fail-before-append guarantees. Old evicted retries may still cost a linear durable accepted-prefix scan; any rebuildable lookup index requires separate measured justification and bounded-memory/recovery tests.

Prioritize regular accepted-use over one-time Dream settlement, recovery/reconciliation and owner reopen. A measured null result ends the attempted optimization rather than triggering new machinery.

**Gate:** matched complete accepted-use operations show a material improvement in the measured dominant stage with unchanged canonical effects/replay/reopen behavior and no material end-to-end regression. Historical Dream preparation may be profiled alongside this work, but is not grounds for speculative caching.

## 8. P5 — relevance breadth assessment, *not* a policy change

Measure real fixture graph distance and positive-event reach by scenario, root distribution, Community snapshot health, principal and number of distinct roots. Report the full score-effect histogram by hop/origin, touched-memory fraction, edges examined per unique recipient, and rate of nearly full-Web reach. Compare +25 single-root versus +50 multi-root and both current and unknown Community membership, clearly marking topology-only prospective measurements. Use detached synthetic accepted-event histories if a trajectory experiment is required; real historical trajectories are unavailable from current fixture provenance.

Ask whether large reach is *expected under the accepted policy* or an artifact of artificial eight-root benchmarks and stale Community snapshots. Keep current v1 policy unchanged. If breadth is genuinely too indiscriminate for Ego, write a separate proposal with evidence, explicit v2 scoring/propagation policy, compatibility and replay implications. Do not introduce unversioned truncation, top-K pruning, minimum freshness thresholds or recipient blocking into this performance branch; those change event semantics. Existing dormant and full-small-Web inclusion guarantees remain intact.

**Gate:** a breadth report with scenario-stratified plots/tables, testable hypotheses and a documented decision to retain v1 or separately spec v2. P5 may conclude “no policy change”; it cannot block P6 solely because a future v2 idea exists.

## 9. P6 — integrated verification and measured release

### Required correctness matrix

Compare original baseline and candidate on independent propagation oracle, same-turn multi-root and cross-Community paths, negative recipient bridging, exclusions, duplicate chains, original first-pass Dream roots, later graph rewiring, accepted multi-source use, exact evicted retries, out-of-order/same-turn events and historical cuts. Verify canonical scores and effect maps **bit-for-bit** before and after reopen, repack, reclamation and divergent reconciliation, and on crash-prefix/retry fixtures. Pinned owner/policy/proof validation and invalid-preappend atomicity must remain intact. Do not require identical physical append ordering for genuinely different acceptance arrival schedules; require identical canonical logical projections under the same accepted source events.

Run `cargo fmt --all -- --check`, `cargo check --locked --tests --examples -j 2`, `cargo test --locked -j 2`, focused propagation/Freshness storage/owner/reconciliation/recovery tests, and CLI format/check/tests if affected. Run documentation structural and changed-from checks plus `python scripts/check_architecture.py --refresh` with an actually working supported Lexicon adapter; record any timeout separately from test failures. If Archive import/reopen surfaces change, run the `archive_roundtrip` smoke. Do not treat the prior core's passing tests as passing evidence for a modified candidate: collect actual final-source results.

### Required performance matrix and success condition

Run optimized, matched, repeatable P0 and final candidate tests on the synthetic graph plus the three real corpus topology fixtures; include current/stale Community versions as they actually exist and explicitly separate artificially rebuilt experimental communities if used. Measure p50/p95/p99 full operation and per-stage latency, total batch throughput, edges/recipient, root-group distribution, peak heap or explicitly unavailable measurements, graph snapshot/pin and historical scan cost. Capture an actual accepted-use receipt path and actual prepared/committed first-Dream settlement through deterministic local fixtures, not only the pure propagation function.

**Success means** exact numerical/event equivalence, retained bounded-memory/recovery properties, demonstrated material improvement on the expensive measured representative cases, and no repeatable >10% p95 end-to-end regression on any routinely occurring supported event family without an explicitly documented tradeoff. P1's 2x synthetic same-origin diagnostic target is necessary but not sufficient; final promotion requires **measured full-operation improvement** on at least one major production-relevant bottleneck identified at P0. If P0 identifies different costs, prioritize measured hot paths over speculative changes. State remaining absolute p95, peak-memory cost and workload conditions; do not promise a universal speedup or fixed millisecond ceiling without deployment SLO evidence.

Publish `docs/freshness-performance-verification-2026-10-03.md` with source digest/commit, test commands, job exits, raw baseline/final data references, fixture hashes, corrections made, null/negative measurements, remaining issues and final acceptance disposition. Reconcile current architecture, performance limitations, maintainer map, coverage, development and implementation docs to verified behavior. Commit and push **only after** final gates; do not merge into main or prune concurrent worktrees as a side effect of this milestone.

## 10. Execution handoff and decisions

| Stage | Must exist before starting | Deliverable | Acceptance owner |
| --- | --- | --- | --- |
| P0 | Verified core commit; clean isolated worktree | Full-operation baseline, runnable diagnostic and workload matrix | Performance evidence |
| P1 | P0 measurements and preserved oracle | Grouped-origin implementation and exact oracle comparison | Propagation |
| P2 | Accepted P1 + P0/P1 batch measurements | Fixed `<16 serial / >=16 bounded pool` rule and no-regression evidence | Propagation + accepted-use |
| P3 | P0 graph-preparation measurements | Deferred/no change unless later measurement establishes a material graph-preparation bottleneck | Measurement only |
| P4 | P0 accepted-use stage breakdown + accepted P2 | Measured postimage/history hot-path optimization only | Freshness persistence + Cva |
| P5 | Scenario-stratified baseline/final traces | Evidence-driven v1 breadth disposition or separately proposed v2 | Policy review |
| P6 | All retained source changes frozen | Final gate, report, clean commit/PR boundary | Integrated release |

P0 can develop independent oracle fixtures while a single implementation owner examines P1's internal representation, but **do not edit P1 shared production files until the P0 baseline is frozen**. After P2, proceed directly to P4's measured accepted-use hot path; P3 requires no implementation unless later evidence reopens it. P5 remains measurement/policy review only. Persist reports per completed gate; the milestone is not complete when only inner-loop tests or a synthetically faster implementation exist.

## Related docs

- [P2–P4 completion execution plan](freshness-performance-p2-p4-execution-plan-2026-10-04.md)
- [Freshness core release verification](freshness-core-release-verification-2026-10-03.md)
- [Freshness R4 historical replay verification](freshness-r4-verification-2026-10-03.md)
- [Freshness R6 scheduling/consumer verification](freshness-r6-verification-2026-10-03.md)
- [ADR 0039 — unified deterministic Memory Freshness](decisions/0039-unified-deterministic-memory-freshness.md)
- [Ego Memory-Web summary plan](ego-web-synthesis-plan.md)
- [Repository development/fixture commands](development.md)

## Notes

This plan does not implement or activate Ego, external accepted-context delivery, F8 semantic-vector reinforcement or historical import simulation. The performance prerequisite ends at truthful, optimized and verified Freshness core interfaces; live Ego delivery is a distinct integration gate. No fixture has historical accepted-use/Dream admission evidence sufficient to infer production score trajectories. The new standalone diagnostic used the real production propagation and policy source but only synthetic topology and a limited scenario set; P0 and P6 must independently establish real-corpus and complete-operation performance.
