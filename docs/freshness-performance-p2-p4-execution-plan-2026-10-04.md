# Freshness Performance P2–P4 Completion Plan — 2026-10-04

Parent: [Documentation index](INDEX.md).

## Purpose

Finish the remaining Freshness performance work through P4 without reviving the speculative machinery removed from the roadmap.

This plan records the completed execution. It began from accepted P1 commit `cdb0716412cba07fba61a820b88139e9bccfa19a`, the then-uncommitted P2 delta in `C:\!bin\workspace\Reliquary-freshness-performance`, and the accepted P0 measurement evidence. Stop after P4 acceptance. P5/P6, Ego delivery, F8 semantic reinforcement and historical-import simulation are outside this plan.

The governing rule is simple: **measure first, retain only machinery that pays for itself**.

## Overview

P0 is accepted and identified the useful performance facts:

- grouped multi-root propagation was worth fixing and became P1;
- independent batches of 16 and 64 accepted-use events show consistent propagation-stage benefit from event-level parallelism;
- batch-4 parallelism is workload-dependent and is not worth a topology predictor;
- accepted-use graph enumeration/adjacency construction measured only about 0.003–0.007 ms on the local operation path;
- accepted-use propagation measured sub-millisecond there;
- accepted-use effect/postimage preparation and canonical receipt ingest/replay measured milliseconds to more than 100 ms as history grew;
- append/sync was comparatively small;
- historical Dream preparation measured milliseconds on the small fixture but large-history scaling is still unknown.

Execution checkpoint 2026-10-04: P1 and fixed-rule P2 are accepted; P3 is deferred/no change; P4 is accepted with one fixed, rebuildable negative receipt membership hint after profiling identified repeated normal-path durable scans as dominant. Both preappend and ingest projections remain intact. The [P2 report](freshness-performance-p2-verification-2026-10-03.md) and [P4 report](freshness-performance-p4-verification-2026-10-04.md) freeze evidence and limits. This execution is complete and stops here; P5/P6, F8 and Ego delivery remain outside scope.

## Non-negotiable invariants

No work in P2–P4 may change:

- score range, principals, hop costs, decay, Dormant traversal or positive-path termination;
- independent accepted-use logical events;
- first-cycle Dream identity, roots, exclusions, graph proof or accepted historical cut;
- owner UUID, policy version, accepted turn/event ordering or retry identity;
- immutable accepted effects;
- exact old-retry collision/refusal behavior;
- reopen/replay, repack, recovery or divergent-reconciliation semantics;
- fail-before-append validation guarantees;
- bounded recent-receipt residency: 128 receipts / 256 KiB;
- storage format unless a separate measured migration specification is explicitly approved.

Do not introduce a daemon, general scheduler, cache service, compatibility façade, new durable authority or unbounded in-memory index.

## P2 — finish conservative event-batch parallelism

### P2.0 Freeze the implementation boundary

Before editing, confirm:

- branch/worktree: `perf/freshness-hardening`;
- HEAD is accepted P1;
- no other agent owns `src/freshness/propagation.rs` or `src/runtime_inference/freshness_access.rs`;
- no conflicting Cargo benchmark job is running against the same target;
- current P2 evidence and A/A noise archives remain untouched.

Do not delete evidence until P2 is accepted.

### P2.1 Delete the adaptive planner

Primary file: `src/freshness/propagation.rs`.

Delete:

- `bounded_two_hop_work`;
- graph-size thresholds;
- Community-membership scheduling conditions;
- scan/probe budgets;
- useful-work thresholds;
- estimator-specific constants;
- estimator-specific tests.

Reduce worker planning to one rule:

1. zero events -> zero workers;
2. 1–15 events -> one worker;
3. 16+ events -> one bounded event-level pool;
4. worker count = min(requested cap, available parallelism, event count, 4), with a minimum of one once the batch is non-empty.

A caller asking for more workers does not override the 16-event threshold.

Single-event propagation remains P1 serial grouped-origin traversal. No parallel frontier traversal returns.

### P2.2 Keep the counterless production path only if it stays deep

The crate-private candidate-only batch path may remain because accepted-use does not consume propagation statistics.

It must share:

- the same event validation;
- the same fixed worker planner;
- the same grouped-origin traversal;
- the same output ordering;
- the same worker failure/join behavior.

The only difference from the public stats path is counter collection. Do not create separate production and diagnostic algorithms.

### P2.3 Replace tests, do not layer more tests on the rejected planner

Remove estimator-specific tests and retain interface-level tests for:

- event counts 0, 1, 15, 16 and 17;
- caller caps 0/1/2/4/8/64;
- injected hardware parallelism 0/1/2/4/8;
- production max of four workers;
- exact candidate equality between serial, stats and counterless paths;
- stable original event order after parallel completion;
- invalid policy/principal rejected before worker creation;
- empty/all-excluded/disconnected events;
- worker panic/failure joins all workers and returns `WorkerFailure`;
- fixed-seed oracle equality for both serial-selected and parallel-selected batches.

Do not expose worker planning publicly just for tests. Test-only/internal observation is sufficient.

### P2.4 Verification

Run once on the simplified final source:

- `cargo fmt --all -- --check`;
- focused propagation unit tests;
- `tests/freshness_propagation_oracle.rs`;
- Freshness receipt/recovery/reconciliation integration tests;
- root `cargo check --locked -j2`;
- root `cargo test --locked -j2`;
- CLI check/tests if the changed public surface reaches CLI compilation;
- documentation validation;
- architecture validation.

Do not rerun the large performance matrix while another Cargo/rustc/Lexicon/architecture job is active.

### P2.5 Performance acceptance

Use actual P1 commit binaries and final P2 binaries, same release profile/features/toolchain/lockfile, interleaved on a quiet host.

Required measurements:

- accepted batch 1;
- accepted batch 4;
- accepted batch 16;
- accepted batch 64;
- synthetic + 14-day + 28-day + Ellis topology fixtures;
- production-relevant worker caps 1/2/4.

Acceptance:

- all candidate maps are exactly equal;
- batches 1 and 4 execute serially;
- batches 16 and 64 use the bounded pool when >1 worker is available;
- 16/64 retain material propagation-stage benefit on representative large topologies;
- no repeatable material complete-operation regression attributable to the changed 16+ scheduling mode;
- unchanged Dream/single-event p95 noise is not used to tune the scheduler.

The old 14-day batch-4 parallel win is intentionally abandoned.

### P2.6 Evidence cleanup and commit

After P2 passes:

Keep permanently:

- final P2 verification report;
- final P2 release evidence archive;
- reusable strict P1/P2 comparison harness if it remains generally useful;
- generic P0/P4 Freshness tracing.

Remove or move out of the active repo unless needed to reproduce a claim:

- partial/blocked P2 runs;
- duplicate A/A archives once their summarized noise evidence is preserved;
- adaptive-planner microbenchmark code;
- estimator-only analysis scripts;
- preacceptance archives superseded by final evidence.

Do not delete the accepted P0/P1 evidence.

Commit P2 separately on top of P1. The P2 commit must not contain P4 production changes.

**P2 exit:** fixed threshold implemented, correctness green, final matched evidence frozen, exploratory scheduler machinery pruned.

## P3 — explicit no-op/deferred checkpoint

P3 is not an implementation phase in this execution plan.

### P3.1 Record the disposition

Update the roadmap/verification docs to state:

- current accepted-use graph materialization is not a measured bottleneck;
- no graph cache or version-keyed reusable projection is being introduced;
- no cache invalidation/reopen/reconciliation machinery is authorized;
- historical Dream graph preparation remains a future measurement question.

No production source changes are required.

### P3.2 Optional historical Dream measurement

Only if a realistic large-history disposable fixture already exists cheaply, run one measurement of:

- committed-root scan;
- historical graph snapshot scan;
- chunks/bytes visited;
- total Dream prepare/commit latency.

Do not build a new expensive corpus merely to satisfy P3.

If historical graph preparation is not dominant, close P3 as deferred/no change. If it is unexpectedly dominant, record a separate follow-up hypothesis; **do not implement it before P4**.

**P3 exit:** documented no-op/defer decision, no current-graph caching code.

## P4 — accepted-use postimage/history hot path

### P4.0 Preserve and refine the useful instrumentation

Keep:

- Cargo feature `freshness-performance-trace`;
- `src/freshness/trace.rs`;
- `examples/freshness_operation_baseline.rs`;
- checksum-backed baseline/report scripts that remain reusable.

Normal builds must continue to compile tracing to no-ops.

Add only stage boundaries needed to separate the dominant accepted-use path. Do not add per-edge or per-record logging.

The current outer stages are too broad:

- `accepted_use.effect_postimage_preparation`;
- `accepted_use.receipt_validation_ingest`.

Split them internally enough to answer where the time goes.

### P4.1 Microprofile accepted-use preparation

Primary source: `src/freshness_storage.rs::prepare_accepted_use_batch`.

Measure separately:

- use-identity validation;
- existing-use lookup;
- per-source existing-event lookup;
- event construction + validation;
- preappend `project_with_events(&events)`;
- payload encoding;
- bytes/events/effects count.

Important current hypothesis: `prepare_accepted_use_batch` calls `project_with_events(&events)` before append solely to prove the batch can be projected canonically. This may become increasingly expensive with durable history.

Do not remove that validation yet.

### P4.2 Microprofile accepted-use ingest

Primary sources:

- `FreshnessStore::ingest_appended`;
- `FreshnessStore::ingest`, `JournalEntry::AcceptedUseBatch`;
- `FreshnessStore::project_with_events`;
- `FreshnessStore::canonical_projection`;
- `ordering::ordered_events`;
- `find_use` / `find_event`.

Measure separately:

- payload decode;
- receipt identity lookup;
- constituent event lookup/validation;
- `project_with_events(events)`;
- fast in-order projection versus fallback `canonical_projection`;
- durable scan count/bytes/chunks when `find_use` or `find_event` falls out of the recent receipt window;
- `remember` / recent-receipt maintenance.

Current source performs a preappend `project_with_events` during preparation and then another `project_with_events` after append during ingest. Treat **double projection** as the first hypothesis to confirm or falsify.

Also determine whether the growing-history cost is actually coming from fallback canonical projection, receipt lookup scans, cloning `records`, ordering/sorting, or a combination.

### P4.3 Freeze the diagnosis before optimization

Produce a short P4 baseline table for accepted-use batches 1/4/16/64 at at least three history depths:

- fresh/small;
- beyond the 128-receipt recent window;
- a substantially larger disposable history.

For every shape/depth record:

- end-to-end accepted-use time;
- preparation time;
- ingest time;
- projection time(s);
- durable receipt lookup/scan time;
- encode time;
- append/sync;
- due refresh;
- payload size;
- number of events/effects;
- history chunks/bytes visited where applicable.

Use fresh disposable RELs per comparable sample. Do not compare stages taken at different history depths as if stationary.

At this point choose exactly one dominant P4 target.

### P4.4 Optimization order

Apply these options in order. Stop as soon as the measured bottleneck is removed.

#### Option A — eliminate duplicate projection without weakening preappend validation

Use if the profile confirms that both preappend and postappend projection dominate.

Preferred shape: deepen `FreshnessStore` behind one internal prepare/apply contract rather than exposing more storage details to Cva.

A possible internal prepared object may contain the encoded payload plus validated derived projection/metadata needed to apply the exact accepted batch after durable append. The public/runtime caller should still see one accepted-use operation, not storage internals.

Requirements:

- all semantic validation still completes before append;
- preparation does not mutate the live store;
- append/sync remains the durability boundary;
- after successful append, applying the prepared result must be equivalent to decoding/replaying that exact payload;
- on append/sync failure, in-memory Freshness state remains unchanged;
- reopen from disk reproduces the same state using ordinary journal replay;
- do not persist derived postimages merely to avoid recomputation unless separately justified;
- no new storage format is required.

If a prepared object would require duplicating most ingest logic, prefer a smaller internal helper that performs one validated projection and reuses it on the immediate append path.

#### Option B — remove unnecessary full-history fallback work

Use only if profiling shows `canonical_projection` / `ordering::ordered_events` dominates despite in-order accepted-use events.

Investigate why the fast in-order condition fails. Repair the condition or data flow if the accepted batch is genuinely append-ordered.

Do not weaken handling for:

- out-of-order reconciliation;
- historical cuts;
- same-turn event ordering;
- older retries;
- recovery/reopen.

The normal in-order accepted-use path may be fast; exceptional replay paths may remain more expensive.

#### Option C — bounded receipt lookup acceleration

Use only if evicted `find_use`/`find_event` durable scans dominate normal accepted-use or exact-retry workloads.

Any lookup aid must be:

- derived/rebuildable;
- bounded;
- owner-local;
- invalidated/rebuilt on reopen/repack/reconciliation as required;
- not a new durable authority.

Do not create an unbounded in-memory receipt map. The existing 128/256-KiB bound remains the default semantic/resource contract unless separately approved.

If scans only affect rare old retries rather than normal accepted use, do not optimize them in P4.

### P4.5 Correctness gates

Whichever P4 option is retained must preserve:

- preappend rejection of invalid owner/use ID/turn/source/event/proof/effects;
- no partial in-memory mutation on failed append/sync;
- exact retry returns no duplicate reinforcement;
- conflicting same use/event identity rejects;
- event ordering and postimages bit-for-bit;
- current records before/after reopen;
- historical `record_at` / `record_at_cut`;
- receipt eviction behavior;
- rebuild/repack/reconciliation;
- crash-prefix recovery;
- bounded recent receipt memory;
- Dream event ingest behavior, which shares storage machinery.

Add focused tests at the internal FreshnessStore interface. Do not preserve slow implementation-specific tests if the implementation is replaced; retain behavior tests.

### P4.6 Performance acceptance

Compare final P4 candidate against the accepted post-P2 baseline with the same trace feature and disposable fixture generator.

P4 passes only if:

- the identified dominant accepted-use stage materially improves at medium/large history;
- complete accepted-use latency improves for at least one representative normal batch shape;
- no repeatable material regression appears on small history or exact retry;
- append/sync cost is not misreported as a CPU optimization;
- memory remains bounded;
- candidate effects and final Freshness records are identical.

Do not require Dream to improve unless P4 changed shared storage code that affects Dream. If it does, rerun focused Dream event/recovery tests and ensure no material regression.

### P4.7 Instrumentation disposition and commit

Retain generic stage tracing after P4. It is cheap in normal builds because the feature-off path is a no-op and it remains useful for later integrated verification.

Keep only stable stage names that correspond to durable conceptual boundaries. Remove temporary one-off counters or trace labels used solely to distinguish an abandoned hypothesis.

Freeze a P4 verification report with:

- baseline/candidate commits;
- exact commands;
- trace/binary/source hashes;
- stage table;
- diagnosis;
- retained optimization;
- rejected/null-result ideas;
- correctness results;
- remaining limitations.

Commit P4 separately.

**P4 exit:** accepted-use dominant cost is either materially reduced with preserved semantics, or P4 records a measured null result and stops without adding machinery.

## Required commit topology

Keep phase boundaries explicit:

1. accepted P1: existing commit `cdb0716...`;
2. P2: simplified fixed-threshold implementation + P2 docs/final evidence cleanup;
3. P3: documentation-only no-op/defer checkpoint, which may be folded into P2 documentation if no separate commit adds clarity;
4. P4: profiling instrumentation refinements + one justified hot-path optimization + P4 verification.

Do not mix P4 production changes into P2.

## Files expected to remain after completion

Production:

- `src/freshness/propagation.rs` — P1 traversal + simple P2 batch scheduling;
- `src/runtime_inference/freshness_access.rs` — accepted-use orchestration;
- `src/freshness_storage.rs` and its focused modules — P4 owner if storage projection/replay is the measured target.

Permanent diagnostics:

- `src/freshness/trace.rs`;
- `examples/freshness_operation_baseline.rs`;
- `examples/freshness_performance_baseline.rs` if still useful for P1/P2 regression;
- minimal reusable aggregation/comparison scripts;
- final checksum-backed evidence archives and verification reports.

Temporary P2 adaptive-estimator tooling should not survive merely because it already exists.

## Related docs

- [Freshness performance hardening plan](freshness-performance-hardening-plan-2026-10-03.md)
- [P0 baseline](freshness-performance-baseline-2026-10-03.md)
- [P1 verification](freshness-performance-p1-verification-2026-10-03.md)
- [P2 verification](freshness-performance-p2-verification-2026-10-03.md)
- [Freshness core release verification](freshness-core-release-verification-2026-10-03.md)

## Notes

Stop after P4 acceptance/null-result documentation.

Do not begin P5 breadth/policy work or P6 integrated release work under this plan.
