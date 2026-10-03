# Reliquary Memory Freshness — completion execution specification (2026-10-03)

Parent: [Documentation index](INDEX.md). Governing decisions: [ADR 0039](decisions/0039-unified-deterministic-memory-freshness.md), [numerical and event contract](memory-staleness-plan.md), [F1–F8 implementation specification](generic-staleness-implementation-plan.md). Historical checkpoints: [remaining-work inventory](freshness-remaining-work-2026-10-03.md), [R2 verification](freshness-r2-verification-2026-10-03.md), [R3 verification](freshness-r3-verification-2026-10-03.md).

**Status:** execution-ready specification, **not** evidence that unfinished gates have passed. R1 buildability and the R2/R3 **focused** exit gates have collected passing evidence. R4–R7 have implementations and tests in source but no consolidated acceptance evidence. F8 semantic relevance reinforcement, including imported-archive bootstrap, is accepted design and **not implemented**. Do not reopen R1–R3 unless regression evidence requires it.

**Repository and starting branch:** `C:\!bin\workspace\Reliquary`, `feat/deterministic-memory-freshness`, HEAD observed `a6f580c` on 2026-10-03. At inspection this branch had 76 modified/untracked entries; another independent multiplexing worktree was compiling. Reinspect Git status at every execution handoff. Neither the existence nor the ownership of an uncommitted file is evidence of a currently active agent.

Execution update: the [core verification report](freshness-core-release-verification-2026-10-03.md) records passing R4–R7 library gates on the final source. The original prospective status and specification below are retained as the execution baseline; F8 was excluded by the user.

## Purpose

Provide a source-reviewed, ordered implementation and verification handoff that closes the remaining Freshness core release and separately defines the accepted semantic-relevance extension without reopening already verified earlier phases.

## Overview

The core milestone finishes R4 canonical history, R5 physical/history transformations, R6 scheduling and consumer contracts, and R7 repository-wide evidence. A separate F8 semantic milestone implements local turn-relevance reinforcement and provenance-based one-time import bootstrap after the core is accepted. Each step names source ownership, tests and a release gate; preserved existing code is the starting point rather than a reason to repeat prior extraction.

## 1. Completion boundaries and non-negotiable invariants

There are **two shippable milestones**, with an explicit ordering dependency:

1. **Core Freshness release (R4–R7):** deterministic, durable owner-local scoring/propagation/replay, correct physical transformations, bounded lifecycle notifications, documented integration seams, and collected repository verification/calibration. R1–R3 are retained prerequisites. Do not call the core release complete merely because the relevant files and focused test names exist.
2. **Semantic relevance extension (S1–S6 / F8):** ingestion-time local reinforcement and one-time imported-corpus bootstrap on the completed core. It has its own policy/version, production activation and acceptance evidence. An incomplete F8 must not retroactively invalidate an otherwise verified core release or be misrepresented as current behavior.

Preserve ADR 0039: one signed score per owner/Memory in [-100,+100]; new accepted first publications start at +100; extracted Memories do **not** decay until successful active Dream settlement; immediately active direct publications may admit at birth; decay is one point per ten accepted **post-admission owner-REL turns**, conserving the remainder. Genuine first-cycle Dream connections create one +50 multi-root event to pre-existing information. An **accepted consumed-use** receipt creates +25 propagation, never ordinary reads, vector matches or search hydration. Per-event propagation takes the maximum eligible path, with -5 local and -10 outside the original Community; negative recipient scores do not block positive event traversal. No model or graph edit silently changes historical accepted event identity or restarts admission. Dormant is a relevance state, **not** archive or semantic falsity.

Existing exact owner/policy/receipt/graph proof boundaries remain in place. Keep semantic Memory body and version ownership in `MemoryStore`, accepted-turn authority in Archive, graph/duplicate decisions in Dream/Graph, canonical Freshness scoring and receipt replay in `FreshnessStore`, and minimal orchestration in Cva. Avoid a new general scheduler, shadow Freshness ledger, CLI façade, second activity clock, periodic Perception audit or invented context-consumption observer. Preserve PHY/REL privacy and authorization boundaries.

**Definition of a passed phase:** all stated assertions are exercised by actual collected test output on the final source revision; its report cites exact commands, environment, job/exit status, observed failures/repairs and known limitations. Writing tests, running `cargo check` or passing a different test filter alone does not satisfy an exit gate.

## 2. Execution environment, custody and order

**E0 — freeze a trustworthy working checkpoint.** Inspect Git status/diff, full untracked-file inventory and existing R1–R3 reports. Retain existing source and tests; do not regenerate the subsystem or indiscriminately stage the 76-entry dirty tree. Capture the current diff and source revision in the first R4 report. Treat unrelated worktrees as independent, but avoid simultaneous dependency builds if RAM/process pressure is material.

**E1 — stabilize test artifacts.** The October 3 R3 verifier found that the installed Cargo artifact reclaimer removed `.rlib` files from targets beneath `C:\!bin` during compilation. Use an **isolated verification target outside that root**, e.g. `C:\Users\archa\AppData\Local\Temp\reliquary-freshness-completion`, plus `CARGO_PROFILE_TEST_DEBUG=0` and bounded `-j 2` where needed. Record the actual path, lockfile, Rust version, command, timeout and resource outcome; a dependency compile timeout is **not** a passing or failing test assertion. Run only one mutating edit batch and one Cargo job against the relevant repo/target at a time. Complete the job or cancel it explicitly before another edit/build.

**E2 — execution ownership.** One implementation owner at a time controls `src/freshness_storage.rs`, its streaming module, `cva_freshness.rs`, `cva_reconcile_freshness.rs`, and lifecycle/recovery journal codecs. Delegate only independent oracle tests, benchmark instrumentation or isolated documentation after exact interfaces are frozen. Reinspect actual files before each handoff; never overwrite a concurrently edited shared file. Prefer focused tests after each edit batch, then broader gates at R7.

Execute **R4 → R5 → R6 → R7 → core integration/commit**, followed, if finishing the full accepted roadmap, by **S1 → S2 → S3 → S4 → S5 → S6 → F8 verification/commit**. R5 can have independent fixture authoring while R4 source ownership is active; R7 benchmark preparation may proceed without altering shared core files.

## 3. R4 — canonical event ordering, finite receipts and historical queries

**Source status at inspection:** `src/freshness_storage.rs` already orders events by `(turn, event ID)`, can rebuild canonical projections for out-of-order arrivals, exposes an explicit `FreshnessEventCut`, and holds a 128-entry/256-KiB recent receipt cache backed by `freshness/freshness_storage_stream.rs`. `tests/freshness_owner_contract.rs` and `src/facade/freshness_recovery_tests.rs` already contain same-turn, late-event and cache-turnover cases. This replaces the older handoff's claim that only a rejection watermark and an unbounded lifetime in-memory event map exist. **These mechanisms are present, not fully performance- or failure-verified.**

**R4.1 — freeze total logical order and preappend validation.**
- Specify one stable event ordering: accepted owner REL turn, then stable event ID. At the same turn, validate the documented order of first-publication birth, first admission and eligible score events; never infer semantic order from disk append order or worker finish.
- Audit `prepare_event_postimages_for`, `validate_event_payload`, `prepare_accepted_use_batch`, `ingest_appended` and `Cva::record_accepted_memory_use`. A late event must be accepted at its **original** source cut and recompute later affected postimages; it must not be retimed to the current turn. Both single-Dream-event and multi-source accepted-use paths must fail **before append** for conflicting ID, invalid producer/proof, non-existent birth or impossible source cut. Accepted retry resolves the durable original payload **before graph traversal**.
- Compare forward arrival, reverse arrival, same-turn permuted arrival and mixed Dream/use interleavings against an independent serial logical-history oracle. Assert identical current/historical projections and identical accepted receipt identities after reopen, not identical physical append bytes for different arrival schedules.
- Audit proof selection across graph mutations at the same accepted turn: historical queries must use accepted event cuts and pinned graph proof; no live-graph repropagation on replay.

**R4.2 — bounded receipt residency and exact retry lookup.**
- Retain the existing finite recent cache; prove bounded retained receipt **count and encoded bytes** after tens of thousands of separately accepted uses, before/after reopen. Durable lookup must still find the oldest evicted ID, distinguish exact retry from conflicting reuse, and avoid any extra write or graph traversal on exact retry.
- Inspect `find_use`, `find_event` and `stream::scan_entries` for prefix bounds, corruption behavior, accepted-use embedded events, missing matching receipt and unsupported record versions. Scan only accepted durable history, not trailing partial bytes.
- Birth and first-pass durable state may be proportional to Memories; per-lifetime access-event payloads and identities must not accumulate without bound in RAM.

**R4.3 — replace pathological historical replay without changing results.**
- `canonical_projection` currently re-scans durable event history while selecting the next chronological event. Before optimizing, add a reproducible long-history performance test measuring cold/open, out-of-order append, a historical cut, oldest-receipt retry and memory at several history sizes. Distinguish correctness from benchmark output.
- Implement bounded-memory canonical ordering using a verified design consistent with append-only Container ownership (e.g. bounded sorted runs and merge with a disposable derived spill/index, plus selective checkpointing if measurement justifies it). Do **not** introduce an always-resident full event vector, silently replace immutable effects with new propagation, or make a new authoritative score history. Any derived index must be rebuildable from accepted owner bytes and invalidated by late events/physical transformations.
- Prove the selected algorithm's final projection matches the oracle, including admission between late events, score saturation/remainder, graph proof and duplicate keys. Record measured replay cost and resident memory; there is no invented fixed speedup requirement.

**R4.4 — precise historical read contract.**
- Audit `FreshnessEventCut`, `record_at_cut`, `records_at_cut` and turn-only wrappers. Define and test: no event ID means all eligible events through turn; inclusive ID means events with ID up to it at that turn; pinned graph version excludes later graph proof as documented. Explicitly test multiple same-turn publications/admissions/events and an event arriving late after the historical query was first computed.
- Keep historical reads pure: no owner file mutation, new birth, policy enrollment, side-effecting due notifications or semantic Memory revisions. Reject invalid/unsupported cuts rather than inventing source evidence.

**R4.5 — recovery and acceptance.**
- Add corrupt/partial chunk and injected prefix tests at policy, birth, receipt and event append boundaries; rerun all R2 and R3 focused suites after R4 edits.
- Primary tests: `freshness_storage::tests`, `cva_freshness::r2_tests`, `cva_freshness::recovery_tests`, `tests/freshness_r2_contract.rs`, `tests/freshness_owner_contract.rs`, and a new dedicated long-ledger/order suite if existing tests become unwieldy.
- **Gate:** oracle-equivalent projections for order permutations; late events survive reopen; no partially durable invalid write; fixed recent-cache ceilings and oldest exact retries hold; repeated historical reads are pure; measured long-history algorithm no longer has repeated whole-ledger scans per event. Record results in `docs/freshness-r4-verification-2026-10-03.md`.

## 4. R5 — preservation through reconciliation and physical transforms

**Source status at inspection:** `src/facade/cva_reconcile_freshness.rs` already computes destination activity turns from exact Archive conversation/node identities, deduplicates logical replay entries and replays events using pinned effects. Pending passes/intents currently cause explicit reconciliation refusal. `tests/freshness_reconcile_contract.rs` includes exact birth rebase, disjoint access events and repeated Community-proof reconciliation cases; their existence is not a collected gate.

**R5.1 — reconcile before destination creation.** Inspect `validate_freshness`, `replay_freshness`, `reconcile_diverged` and every existing strict-copy/repack/reclamation caller. Validate all source provenance, policy, conflict, event identity and exact destination activity mapping **before writing the output**, including cross-input common events with differently numbered local source cuts. If the current destination's accepted activity identity index differs from the plan, fail without partial output. Do not deduplicate only by numeric turn.

**R5.2 — canonical destination replay.** Replay first-publication births, original admitted-at-birth flags, later admissions, all accepted Dream/use effect deltas, use receipts and settled Dream IDs in canonical rebased order. Reconstruct score postimages under destination accepted clock **without** rerunning propagation over the merged current graph. Preserve immutable original source provenance separately from destination-local accepted-turn mapping. Verify a second divergence/reconciliation against an already reconciled owner produces neither identity collision nor second credit.

**R5.3 — graph and Community evidence.** Test accepted original event proofs after graph mutation, Community generation changes and subsequent reconciliations. Preserve actual accepted proof/effects as historical authority across the transform; do not claim a current destination Community snapshot proves a source-time event when it does not. If historical source proof cannot be represented or validated safely, explicitly reject the transform **before creating output** rather than weakening proof rules. Cover both available and deliberately absent Community snapshots.

**R5.4 — outstanding work and safety.** Maintain the existing explicit rejection for pending first-cycle Dream passes and publication intents until there is a real durable recovery transfer. Test no partial destination, no discarded admitted anchor and no fabricated +100. If a product requirement demands copying with pending work, implement a separately verified transfer of original begin/graph/accepted-cut/roots/intent receipts; do not silently mark the work settled. A cleanly rejected transform is acceptable under the current documented contract.

**R5.5 — full physical-operation matrix.** For each supported strict copy, repack, reclamation/compaction, legacy or fresh migration, identical-history reconciliation and divergent-history reconciliation: compare original logical Freshness records at current and historical cuts, policy, receipt retry results and first Dream birth/settled identity before and after reopen. Run corrupt and unsupported-provenance negatives. Preserve original source input bytes on failed operations. Use public transform entry points rather than test-only replay helpers.

**R5.6 — acceptance.** Extend `tests/freshness_reconcile_contract.rs`, `src/facade/cva_reconcile_grouped_tests.rs` and transformation/recovery suites; include reordered accepted turns, common + disjoint events, repeated transforms and a second divergent merge. **Gate:** no lost or duplicate effects, exact rebase/proof correctness, read-only historical equivalence, explicit safe refusal for unsupported pending provenance, and no partial output. Record `docs/freshness-r5-verification-2026-10-03.md`.

## 5. R6 — bounded transition delivery and real relevance consumers

**Source status at inspection:** `src/freshness_due.rs` and `src/facade/cva_freshness_due.rs` already contain a sparse rebuildable dispatcher, 64-transition activity drain, coalesced derived notices and reversal support. `src/facade/cva_turn_ingest.rs` calls the accepted-turn adapter. `src/runtime_inference/freshness_access.rs` exposes accepted-use receipts, Ego scores and `select_ego_routine_context`; a universal accepted-context delivery producer and live Ego consumer are **not** demonstrated.

**R6.1 — lifecycle scheduling invariants.** Test 1,000-turn Fresh→Stale and 1,510-turn Stale→Dormant boundaries, including a backlog spanning both boundaries, a second crossing after saturation, and score-reinforcement reversals. Validate custom pinned policy values. Each bounded scheduling step must be proportional to the drain limit, not the whole Web. Live `freshness_record` / lazy score-at-cut must remain correct with arbitrary notification backlog.

**R6.2 — derived-notification contract.** Audit `freshness_refresh_due`, `freshness_on_accepted_activity`, `drain_freshness_notifications`, `FreshnessDueDispatcher::advance/defer`, coalescing and reopen rebuild. Freeze whether delivery is an at-least-once hint or durable consumption log; current code documents **at-least-once derived/coalescing hints**. Do not promise exact-once external notice delivery. Ensure no missed terminal state or fabricated reversal when a long backlog is coalesced. Consumers deduplicate by Memory/boundary/state or another frozen logical notice key.

**R6.3 — consumed-use ownership.** Find the actual host boundary at which a Memory has been included in an **accepted** model context or deliberately opened for use by the user. If an observable acceptance exists, attach one narrow explicit `AcceptedMemoryUseReceipt` with owner, original accepted turn, stable delivery ID and deduplicated Memory IDs **after acceptance**. Test retries, cancellation, preview, reranking, truncated search results, duplicate appearances and no collection from Dream, indexing or passive hydration. If no such host contract exists in this repository, preserve the verified public API and document the integration as **external/unavailable**, not as a fabricated producer or a claim of complete end-to-end use reinforcement. Do not create a new general event bus solely for Freshness.

**R6.4 — Ego selection.** Feed the actual routine selection boundary graded owner-REL Freshness when that consumer exists. Preserve the entire eligible small Web when it fits; respect hard keeps even over budget; use graded preference only in the remaining budget; retain dormant Memories for search, broad audit and protected inclusion. Preserve Perception-disabled operation and do not conflate this with Ego summary creation, PHY summary regeneration or cross-REL summary authorization. If a live Ego summarizer is future-only, provide a passing adapter contract and mark actual app integration explicitly pending instead of inventing a producer.

**R6.5 — worker/lock review and acceptance.** Verify propagation uses frozen graph/Community input with bounded workers; producer checks live version/proof before commit; no full traversal or worker joins monopolize the Container mutation lock. Main tests: `src/freshness_due.rs`, `tests/freshness_owner_contract.rs`, and concrete host/selection integration fixtures where the host boundary exists. **Gate:** bounded backlog, exact boundaries/reversals, reopen, no passive +25, full-small-Web/hard-keep behavior and truthful status of any external consumer. Record `docs/freshness-r6-verification-2026-10-03.md`.

## 6. R7 — full release evidence, measurement, documentation and commit

**R7.1 — integration matrix.** On the final frozen source, collect positive results from all F1 golden/math cases; R2 provenance/atomicity; R3 every deterministic crash prefix; R4 long-ledger/canonical history; R5 copy/repack/reconcile; R6 due and context adapter; independent exhaustive propagation oracle. Run graph cycles, re-entry, excluded fresh cohorts, duplicate chains, dormant intermediates, different worker counts and mixed independent batches. Preserve failures and fixes in the report, not just the final passing test count.

**R7.2 — repository gates.** From an isolated target outside the reclaimer root, run and collect:

```powershell
$env:CARGO_TARGET_DIR = "C:\Users\archa\AppData\Local\Temp\reliquary-freshness-completion"
$env:CARGO_PROFILE_TEST_DEBUG = "0"
cargo fmt --all -- --check
cargo check --locked --tests --examples -j 2
cargo test --locked -j 2
cargo fmt --manifest-path cli/Cargo.toml -- --check
cargo check --manifest-path cli/Cargo.toml --locked -j 2
cargo test --manifest-path cli/Cargo.toml --locked -j 2
python ../engineering-standards/tools/docs_policy/check.py --repo .
python ../engineering-standards/tools/docs_policy/check.py --repo . --changed-from origin/main
python scripts/check_architecture.py --refresh
git diff --check
```

Do not infer root success from CLI or vice versa. The architecture command may invoke Lexicon/Arcana; if the structural index or manifest fails, collect the actual failure and run separately available source-level architecture checks. An incomplete architecture gate remains explicitly unverified, not “green.” Run `archive_roundtrip` graph-corpus smoke if affected.

**R7.3 — real, hash-preserving calibration.** Use `examples/freshness_calibration.rs` in synthetic mode and, when accessible, hash-verified disposable copies of the 14-day, 28-day and Ellis REL fixture files specified in `docs/development.md`. Never alter the authoritative fixture. Capture worker counts, graph size/density, p50/p95/p99 touched Memories, edges examined, maximum frontier width, per-event latency, graph pinning/lock contention and end-to-end Insomnia/Dream overhead under reproducible machine/build conditions. Compare serial vs bounded parallel, including cases where parallelism is slower. Existing real fixtures supply **prospective topology** only; without historical activity/admission/link/use evidence, do not describe these as historical score-trajectory calibration. Report missing fixtures explicitly.

**R7.4 — architecture and documentation reconciliation.** Compare implementation to `architecture.md`, `storage-format.md`, `api.md`, `invariants.md`, `behavioral-contracts.md`, `current-limitations.md`, `maintainer-map.md`, `documentation-coverage.md`, `roadmap.md`, `docs-standard.json` and ADR 0039. Remove stale status claims only for passed gates; retain explicit lack of universal external accepted-delivery/Ego consumer and lack of historical calibration if applicable. Generate `docs/freshness-core-release-verification-2026-10-03.md` with exact source revision, all commands and results, measured figures, blockers, and documentation/architecture impact.

**R7.5 — release custody.** After all applicable gates, stage **only** the Freshness implementation/tests/docs verified as one coherent change. Inspect `git diff --cached` including untracked files explicitly; exclude multiplexing and unrelated dirty modifications. Commit only after source/doc consistency and a completed check suite; push only the intended branch after confirming upstream and avoiding a mistaken merge to main. Do not claim full release if required gates remain unresolved. A separate approved partial checkpoint may be committed with an explicit incomplete status.

**Core release gate:** R4, R5 and R6 recorded accepted exits; comprehensive root + CLI results; architecture and docs status accurately recorded; real-fixture measurements and environmental limits reported; staged change audited. If an explicitly required host-consumption integration is outside the repository, distinguish **library release** from **end-to-end host integration**.

## 7. S1–S6 / F8 — semantic activity relevance extension (separate release)

ADR 0039 accepts the **concept**, not final magnitudes or a completed imported-history policy. Do not treat the example cosine threshold as corpus-validated, or reuse +25/+50 principal by convenience. No semantic event may propagate.

**S1 — finalize a deterministic policy and producer identity.**
- Pin eligible owner/REL, memory lifecycle, admission, authorization and embedding-profile/generation rules. Only **genuinely accepted ingested-turn fragments** are input; generated retrieval/summarization echoes are excluded. Initial qualifying cosine threshold is 0.75, subject to corpus calibration.
- Define a monotone, similarity-weighted **local** bonus with maximum once per Memory per accepted turn across all fragments, upper/combined caps and deterministic tie handling. Freeze constants and policy version only after S2 measurements.
- Bind idempotent identity to accepted owner + exact Archive source conversation/node/turn identity + Memory ID + versioned semantic policy/profile evidence. Source physical local-turn indexes alone are insufficient across reconciliation. Retain the minimum necessary compact identity in existing Freshness durable machinery, not a separate semantic ledger or unbounded in-memory receipt table.

**S2 — offline calibration and independent oracle.** Using permitted frozen corpora or controlled synthetic samples, measure compatible-vector coverage, nearest-neighbor candidate counts, similarity distribution, false positives, throughput, ownership filtering and score trajectories. Use a bounded index/search path, not every fragment × every Memory. Compare several bonus curves/thresholds and publish selected defaults/reasons. Record results in `docs/freshness-semantic-calibration-2026-10-03.md`; never pretend topology-only snapshots prove historical semantic relevance.

**S3 — live ingestion integration and crash safety.** After an accepted owner turn and its fragment evidence exist, perform authorized compatible-vector candidate search, max-per-Memory deduplication, eligibility/self-source filtering and local-only owner score reinforcement at the accepted event cut. Preserve lazy decay remainder, dormant reactivation, saturation, graph independence and all existing delivery/Dream principals. Place durable receipt publication behind the same preappend validation, late-event canonical order, exact retry and R5 replay transforms proved by the core; do not create a new Memory semantic revision. No vector or compatible profile means no event and no fabricated default similarity. One turn with many fragments is still one local semantic credit per Memory.

**S4 — imported-history bootstrap: provenance-anchored approximation, never global subtraction.**
- First import the accepted Archive, generate Memories and complete Dream graph/first-cycle work according to existing birth and admission authority. Only afterward perform the **one-time** semantic bootstrap against the resulting authorized graph/vectors. Distinguish *new import* from regenerate/reopen of an already enrolled owner.
- Retain genuine source turn identity per imported fragment and original birth/admission/Dream evidence per Memory. Match only fragments whose accepted source provenance and authorization can be verified, exclude self and same-import amplification, and deduplicate by source turn + target. Memory **provenance/admission**, not final archive turn count alone, determines when that Memory could receive each retroactive credit. Do **not** subtract `total_imported_turns / 10` from every Memory.
- Freeze and test an import-only **working accumulator** that can represent pre-cap contributions while historical eligible source turns and supported Dream events are projected, then applies appropriate per-Memory post-admission decay and final [-100,+100] clamp. State precisely how chronological event ordering, decay remainder and saturation compare with genuine live execution. Post-hoc graph/vector knowledge and unobserved historical Dream reprocessing mean this is a documented approximation unless the necessary time-stamped records exist. **Before committing an algorithm**, test small constructed histories comparing live chronological replay, replay with cap at each step and proposed uncapped bootstrap; publish explicit divergence bounds/cases rather than claiming 1:1 historical equivalence.
- If exact first Dream settlement cuts, first-cycle roots, compatible source vectors or Memory source/admission provenance are missing, take the documented conservative path: omit unsupported historical credit or enroll an authorized current-turn legacy baseline. Never invent +25 delivered uses, first-cycle events, historical graph state or a retroactive +100 reset. Repeated import with the same stable import identity, Archive regeneration and crash retry must reproduce the same logical state **without another semantic pass**.

**S5 — projection, reconciliation and recovery matrix.** Extend `FreshnessReplayEntry`, existing owner codec/provenance and exact streaming lookup only as needed for versioned local-only semantic effects. Cover new policy alongside old owner histories and explicit migration/rejection. Rebase accepted source cuts through exact Archive conversation/node identities; do not change original provenance when mapping destination turns. Test late semantic events mixed with Dream/use events, reordered fragment arrival, profile mismatch/rotation, repeated/imported duplicates, partial writes and physical transforms. A semantic event's target never triggers a propagating secondary event.

**S6 — deployment and full extension gate.** Focused threshold-neighbor tests (below/at/above), multi-fragment dedupe, score floor/cap, decay remainder, negative/dormant reactivation, source self-exclusion, authorization/profile/generation mismatches, empty vectors, independent legitimate consumed use, retry/second reopen, long-history bounded memory, import-vs-live divergence fixtures, repeated regeneration and twice-reconciled histories. Then rerun **all core R7 gates**, root/CLI checks, documentation/architecture checks and synthetic/real performance measurements. Record `docs/freshness-semantic-release-verification-2026-10-03.md` and update the current API/storage docs **only for implemented behavior**. Stage/commit separately from the verified core release so a semantic-policy/calibration blocker cannot mislabel the core.

## 8. Traceable test-to-phase inventory and execution handoff

| Phase | Existing fixtures/interfaces to preserve and extend | New proof required |
| --- | --- | --- |
| R4 | `freshness_storage::tests`, `freshness_owner_contract`, `freshness_r2_contract`, `freshness_storage_stream` | late mixed-event oracle, preappend crash prefixes, bounded long-ledger memory and replay cost |
| R5 | `freshness_reconcile_contract`, `cva_reconcile_grouped_tests`, `cva_reconcile_freshness` | strict transformation matrix, exact second merge, real source proof validation |
| R6 | `freshness_due` unit tests, `freshness_owner_contract`, `cva_freshness_due`, `freshness_access` | bounded backlog/coalescing + actual available host acceptance boundary |
| R7 | `freshness_propagation_oracle`, `freshness_f7_acceptance`, `freshness_f7_recovery`, `freshness_calibration` | all collected full build/CLI/architecture/documentation/calibration results |
| F8 | ADR 0039 and this S1–S6 contract; no claimed implementation | new independent cosine/locality, delayed ordering, import and regeneration oracle suites |

On a fresh execution session: read this document, R3 verification, the current Git status/diff, and the named source/tests for the **next single subphase**. Do not “redo R2/R3” absent new regression evidence. Implement one subphase, run its focused test(s), collect full output, update its verification record and continue. Stop only at an actual blocker; preserve a precise handoff with the failed command, current dirty paths and the next narrow action. The full project is finished only when the **core milestone** and, if the accepted F8 extension is in the requested scope, the **semantic milestone** have their separate passing release evidence.
## Related docs

- [ADR 0039 — accepted policy and semantic extension](decisions/0039-unified-deterministic-memory-freshness.md)
- [Full F1–F8 implementation contract](generic-staleness-implementation-plan.md)
- [Original remaining-work inventory](freshness-remaining-work-2026-10-03.md)
- [R2 durable-event verification](freshness-r2-verification-2026-10-03.md)
- [R3 Dream/publication recovery verification](freshness-r3-verification-2026-10-03.md)
- [Development, fixture hashes and verification commands](development.md)

## Notes

This is a prospective execution specification. Existing focused test evidence is attributed to its dated reports; no new implementation, full verification or performance result is claimed by writing this plan. When actual implementation diverges from a proposed internal optimization, keep the accepted public contract unchanged and record the final verified choice in the corresponding phase report. The independent multiplexing branch is outside this specification.
