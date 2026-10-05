# Memory Freshness remaining-work specification — 2026-10-03

## Purpose

Replace the execution handoff for the unfinished F2–F7 implementation with a frozen, source-reviewed work list. This document supplements the accepted policy, ADR 0039, and the main implementation plan; it does not change their scoring or ownership decisions.

Execution resumed on 2026-10-03 at the user's request, using Luna for bounded delegated work. R1, R2 and R3's **focused** gates have passing test evidence. R2's six focused runtime tests passed after a narrow compact-postimage repair, and its combined Cargo check passed. R3's direct and grouped publication, partial Dream graph, admission-before-event, missing settled marker and canonical duplicate recovery tests also pass. The Cargo reclaimer interfered with builds targeting the repository; the R3 verifier used an isolated temporary target outside its configured root. R4–R7 remain independently open, and the complete Freshness regression/production gate remains unverified. See the [dated R2 report](freshness-r2-verification-2026-10-03.md) and [dated R3 report](freshness-r3-verification-2026-10-03.md) for exact evidence.

**Current execution specification:** [Freshness completion execution plan](freshness-completion-execution-plan-2026-10-03.md) supersedes the stale checkpoint-level R4–R7 implementation details below and adds detailed F8 semantic/import milestones. This document remains the dated historical inventory and R1–R3 verification handoff.

## Overview

Repository: `C:\!bin\workspace\Reliquary`.
Branch: `feat/deterministic-memory-freshness`.
Checkpoint: uncommitted working-tree changes, including new files. No implementation commit, push, or PR has been made in this execution.
The resumed R1 checkpoint passes `cargo check --locked --tests --examples`, 20 focused library tests, and all 11 tests in the five Freshness integration suites. This establishes R1 buildability; the independently outstanding R3–R7 gates still prevent a production-readiness claim.

### Resumed R1 evidence — 2026-10-03

- `cargo check --locked --tests --examples`: passed (job `job_dEDyx_IEyjxuAcGpTSX5Uc2S`).
- `cargo test --locked --lib freshness`: 20 passed (job `job_IjevxT7IMcKXW2AtcuSzfSh4`).
- Five Freshness integration suites: 11 passed (job `job_cxCqUQyFQPa08reDKt-4Z7GR`).
- Corrected type/borrow mismatches, invalid authority/body fixtures, decay-boundary arithmetic, and divergent-turn arithmetic. The simulated duplicate test now includes its deterministic lexical candidate lane.
- `archive_open_profile` no longer declares an allocator that conflicts with Lore's global allocator. Open timings remain available; retained/peak allocator bytes are explicitly unavailable.
- Earlier failures were collected: two library fixture expectations and the duplicate test's empty candidate set. Their corrected focused runs passed. Root/CLI full tests, architecture enforcement, and measurements remain R7 work.

### Frozen verification evidence

| Check | Result |
| --- | --- |
| F1 standalone reference suite | 17 tests passed |
| Baseline library `cargo check --locked` | Passed before the combined implementation |
| First combined check | Failed on missing Dream module registration; registration subsequently added, unverified |
| Subsequent combined checks | Dependency-build timeout, then cancelled at user's request; no passing result |
| Full library tests | Not completed |
| CLI checks/tests | Not run |
| Documentation structure and change-impact checks | Passed, including the new checkpoint document |
| Architecture refresh/enforcement | Timed out during Lexicon indexing; not passed |
| Synthetic/real-fixture calibration | Harness written; not run |

### Code already present, without integrated acceptance

| Area | Existing implementation to retain and review |
| --- | --- |
| F2 score engine | `src/freshness/score.rs`: signed scores, admission anchors, lazy decay, remainder conservation, thresholds and sparse boundary index |
| Versioned policy | `src/freshness/policy.rs`, `CVAFRP01`: numerical validation and owner policy pinning |
| F4 propagation | `src/freshness/propagation.rs`: origin-specific search, per-event physical maximum, descending-strength frontier barriers, bounded workers, independent-event batch API and statistics |
| Durable projection | `src/freshness_storage.rs`, `src/facade/cva_freshness.rs`: owner binding, initialization/admission/event chunks, effect/postimage validation, history queries and replay seams |
| First Dream recovery | `src/freshness_dream_journal.rs`: begun pass, roots, accepted settlement cut, settled marker and publication-intent/birth receipts |
| Producers | `src/runtime_inference/dream_freshness.rs`, `freshness_access.rs`, direct/frontier/host Dream paths and grouped Insomnia commit plumbing |
| F5 reconciliation | `src/facade/cva_reconcile_freshness.rs` and repack wiring: preflight, Archive identity rebasing, pinned effect replay, birth/admission and settled-pass preservation |
| F6 selection seam | Policy-aware Ego grades and focused budget/hard-keep selector; no complete live context consumer |
| Due scheduling | `src/freshness_due.rs`: bounded dispatcher and transition/reversal types; Cva/runtime integration not present at checkpoint |
| Streaming storage seam | `Container::visit_payloads` plus read-only scan support: one payload at a time; not yet used to bound Freshness receipt/history memory |
| F7 evidence | Five new Freshness integration test files and `examples/freshness_calibration.rs`; tests and measurements remain unverified |

## Remaining implementation sequence

Complete one gate at a time. Begin from this working tree; do not regenerate the subsystem or restart F1. One owner should control shared persistence/facade edits. Parallel work may cover independent test or documentation files only after interfaces are frozen. Builds and workspace mutations share a resource lock: finish the edit batch before starting Cargo, and collect its result before resuming edits.

### R1 — Establish a buildable checkpoint

**Latest direct verification (2026-10-03 14:59 UTC):** R1's buildability/failure-inventory gate is satisfied. Combined compile and 23 focused library tests pass; the current five integration suites have 13 passes and one publication-intent failure. See the [dated R1 verification record](freshness-r1-verification-2026-10-03.md) for the exact failure and source-reviewed cause. Earlier all-passing evidence above describes an older working tree. Known authority/body/arithmetic fixtures are now corrected; the remaining failure belongs to R2/R3.

**Scope:** signatures, module registration, compile diagnostics and invalid fixtures only.

1. Run `cargo check --locked --tests --examples` with sufficient dependency-build time. Collect the complete result.
2. Correct all API/type/borrow mismatches, then run focused Freshness tests.
3. Known fixture corrections still outstanding in `tests/freshness_recovery_contract.rs`: use an allowed authority kind (`direct`, not `inferred`); preserve the original title and content during metadata enrichment. Titles are part of immutable Memory bodies.
4. Recheck reconciliation test arithmetic: 999 common turns + 9 left turns + 1 right turn equals 1009. A right event expected at destination 1010 requires another accepted turn. The acceptance agent was asked to correct this; verify actual source.
5. Remove impossible/dead checks such as `turn > u64::MAX`.

**Exit gate:** library, tests and examples compile; focused test failures are recorded individually. A buildable checkpoint is not an F2–F7 completion claim.

### R2 — Freeze and validate the durable event contract

**Complete — focused exit gate verified (2026-10-03).** Source validation, event provenance, canonical effects/postimage pre-append checks, distinct explicit legacy enrollment and R2 regression tests are implemented. Post-repair results: `freshness_storage::tests` **4/4**, `cva_freshness::r2_tests` **1/1**, `freshness_r2_contract` **1/1**, plus successful combined `cargo check --locked --tests --examples -j 2`. These tests verify invalid-batch atomicity (durable bytes and projections), no-op exact retries after graph changes/reopen, explicit conflicting-identity rejection and first-publication/legacy separation. Earlier external Lore compile timeouts were not assertion failures. Consult the [R2 verification record](freshness-r2-verification-2026-10-03.md) for job IDs, details and remaining broader gates.

**Scope:** Freshness owner, codecs and facade publication paths.

- Persist immutable event identity, producer kind/source identity, accepted REL cut, policy version, graph/community proof and complete per-Memory candidate effects. Define which fields establish receipt equivalence.
- A retry resolves the accepted durable event before propagating again. Later graph/community changes must not change its results or cause a genuine retry to collide with its original receipt.
- Validate the whole proposed operation before any append: owner, enrollment, valid anchors, positive principal bounds, event/source eligibility, exclusions, proof and every computed postimage. Invalid input must not append a chunk that makes future open fail.
- Check both numerical and semantic proof validity. Memory graph versions must not be confused with overall graph versions; stale Community snapshots use unknown/outside costs. A generation number alone is not proof that membership was appropriate for the event.
- Reject reused IDs with incompatible accepted provenance. Do not permit arbitrary public pass IDs or arbitrary effect maps to manufacture a first-cycle event.
- Separate first-publication proof from explicit legacy enrollment. Old explicitly enrolled Memories must not be treated as newly created.
- Keep score writes independent of Memory revisions and semantic global-version allocation.
- Validate malformed/truncated record handling without making Container own Freshness semantics.

**Exit gate:** invalid batches leave durable bytes/projections unchanged; true retries remain no-ops after reopen and graph changes; conflicting receipt identities fail explicitly.

### R3 — Complete publication and Dream crash recovery

**Focused verification (2026-10-03): passed.** The acceptance cases below have directly collected focused runtime results; see [R3 implementation and verification](freshness-r3-verification-2026-10-03.md). The bullet list preserves the frozen original work specification, so its checkpoint descriptions are historical, not assertions about the current code. Full combined/CLI validation remains R7 work.

**Scope:** lifecycle open/replay, direct/grouped publication, Dream adapters.

- Direct publication intents must bind to the first accepted Memory revision and mutation identity, not the current metadata revision. The checkpoint facade still checks `current_mutation_id` in initialization retry paths.
- Recover the crash gap between an accepted direct Memory publication and its Freshness initialization from durable birth intent plus accepted Memory proof.
- Recover the grouped gap between durable InsomniaCompletion and Freshness enrollment from that completion's records and original accepted cut. Retrying an already completed episode may stage zero new records.
- Ordinary open may reconstruct the derived projection from accepted durable proof; it must not append enrollment, reset anchors, or invent +100 for legacy records.
- Retain the original first successful settlement cut across retries. The pass begin cut is not its admission/event cut.
- Recover genuine first-cycle links from committed attempt history, including missing per-pair receipts and links subsequently rewired/inactivated. The checkpoint adapter scans current active graph only.
- Use existing Dream duplicate representative authority after lifecycle settlement. Representatives outside the top-K candidate set remain eligible when pre-existing provenance proves them.
- Exclude the new source and freshly initialized same-batch peers from first-cycle credit. Define cohort membership from exact grouped completion provenance; episode equality alone must not suppress a later independently created Memory.
- Prevalidate settlement effects before source admission. Multiple journal chunks are acceptable if every durable crash prefix has an idempotent recovery path; one event's propagated effects remain an indivisible batch.

**Exit gate:** deterministic crash-prefix tests cover birth, grouped completion, pair graph write, accepted settlement, admission, score event and missing settled marker. Exactly one admission and one logical linkage event survive all retries.

### R4 — Canonical ordering and bounded receipt storage

**Initial focused baseline (2026-10-03; R4 remains open):** `cargo test --locked --test freshness_owner_contract -j 2` passed **14/14** with the isolated temporary Cargo target (job `job_eHLZfcUBFHi1IDl08mCUBIiZ`). This includes forward/reversed accepted-use arrival, same-turn stable event/graph cuts, old-use deduplication after recent-cache turnover and owner scheduling contracts. The mixed-producer case of a delayed accepted Dream event arriving after a later accepted-use event, plus broader streaming/long-history assurance, still warrants explicit R4 acceptance. No R4-specific implementation edits were made in this verification step.

**Scope:** event ledger, projection replay and streaming queries.

- Canonical score effects are ordered by accepted `(REL turn, event ID)`, independent of worker completion/receipt arrival.
- The checkpoint has a nondecreasing event-turn watermark that rejects older accepted events. This is insufficient for a delayed Dream recovery or accepted-use receipt after a later event.
- Resolve delayed accepted events through deterministic replay or an explicitly specified recovery barrier. Never move their accepted clock cut forward to avoid a regression.
- Validate ordering before durable writes. Currently a global watermark rejection can occur during ingest after a facade append.
- Replace the unbounded in-memory event map and full `history: Vec<JournalEntry>`. Retain bounded recent receipts with exact durable fallback; eviction must never permit duplicate reinforcement.
- Use the streaming Container visitor for receipt lookup and historical projection. One-per-Memory birth/pass state is acceptable; one retained ID/payload per lifetime access event is not.
- Define precise historical queries with both activity and event/graph cuts. The checkpoint's turn-only APIs cannot distinguish publications and events within the same accepted turn.

**Exit gate:** forward and reversed arrival schedules produce identical canonical projections; delayed accepted events survive reopen; long-history tests demonstrate bounded receipt memory and indefinite deduplication.

### R5 — Close transformation and proof preservation

**Scope:** reconcile, copy, repack, reclamation and migration.

- Preserve policy, birth/admission anchors, accepted effects, receipts and pending recovery provenance in strict copies/repack/reclamation.
- Rebase source cuts by exact accepted Archive `(conversation ID, node ID)`. Verify mappings against the actual destination activity index after replay, not only a predicted merged order.
- Replay authoritative accepted deltas in canonical destination order; do not copy source postimages or rerun propagation over the merged graph.
- Deduplicate common events by logical accepted provenance and pinned effects. Local numerical source positions may differ after a prior reconciliation.
- Preserve Community/graph proof evidence across reconciliation. The checkpoint output does not replay Community snapshots; merely retaining source generation/version numbers can make the next reconciliation reject its own previously accepted events.
- Preserve birth-admitted initialization, publication-birth receipts and settled Dream IDs.
- Pending pass/intent handling must preserve recoverability or fail explicitly before creating the destination. Missing essential provenance is an explicit error, never a score reset.

**Exit gate:** reconcile with reordered turns, common events, disjoint events and a second reconciliation preserves canonical scores and receipts; reopen agrees with the in-process projection; unsupported provenance leaves inputs unchanged and no partial output.

### R6 — Connect bounded due scheduling and retain F6 scope

**Scope:** Freshness dispatcher and narrow REL/Ego adapters.

- Wire dispatcher construction/rebuild into Cva, and refresh affected entries after enrollment, admission and event effects.
- Drain a fixed bounded number of transitions on an accepted REL activity scheduling seam. No per-turn scan of the whole Web.
- Process backlogged transitions at their exact boundary so Fresh→Stale and Stale→Dormant are both observable; add reverse crossing notices after reinforcement.
- Keep current reads correct through pure lazy evaluation even while notifications lag.
- Keep dormant Memories searchable and preserve hard-keeps/broad audit. If the complete small Web fits, retain it.
- The focused Ego selector and accepted-use API are integration seams. Do not fabricate consumption events from searches, hydration or a nonexistent live context consumer.
- Check pool/lock ownership: workers use pinned read-only snapshots, total workers stay bounded, and graph/community commit proof is checked without holding Container locks through unnecessary traversal.

**Exit gate:** bounded backlog, exact boundaries, reopen rebuild, reversals, full-small-Web and hard-keep contracts pass. Remaining application context integration is described accurately.

### R7 — Verification, measurements and documentation closeout

Run focused suites first, then required repository checks after source stabilizes:

```text
cargo fmt --check
cargo check --locked
cargo test --locked
cargo fmt --manifest-path cli/Cargo.toml -- --check
cargo check --manifest-path cli/Cargo.toml --locked
cargo test --manifest-path cli/Cargo.toml --locked
python ../engineering-standards/tools/docs_policy/check.py --repo .
python ../engineering-standards/tools/docs_policy/check.py --repo . --changed-from origin/main
python scripts/check_architecture.py --refresh
```

Run the graph-corpus `archive_roundtrip` smoke path if affected.

- Differential propagation: independent exhaustive oracle, distinct origins, cycles, re-entry, excluded bridges, dormant intermediates, multiple worker counts and independent-event batches.
- Owner contracts: policy pinning, no semantic revisions, passive reads, true-use dedupe, late receipts and exact historical cuts.
- Recovery/transform contracts: all R2–R5 failure/retry cases, strict copy/repack/reclamation/migration and divergent replay.
- Measure p50/p95/p99 touched Memories, examined edges, frontier width and latency; compare bounded worker counts against serial. Report slowdowns as measured, not presumed speedups.
- Profile real fixtures on disposable copies only, verifying SHA-256 before/after. Commands and authoritative hashes are in `docs/development.md`.
- Existing fixtures support prospective topology profiling. They do not supply a historical accepted-use/first-pass Freshness ledger; do not label prospective results historical score calibration.
- Reconcile architecture/API/storage/invariants/behavioral contracts/limitations/maintainer map/coverage/roadmap with verified behavior. Their current edits include status statements that can lag unfinished code.
- Update F2–F7 status only when each corresponding gate passes. Provide required documentation and architecture impact reports with actual commands/results and material gaps.

**Exit gate:** all required checks collected; benchmark evidence recorded reproducibly; implementation/docs agree; only then commit the completed implementation.

## Related docs

- [Main F1–F7 implementation specification](generic-staleness-implementation-plan.md)
- [Memory Freshness policy](memory-staleness-plan.md)
- [ADR 0039](decisions/0039-unified-deterministic-memory-freshness.md)
- [Development and calibration commands](development.md)
- [R2 event-contract implementation and verification record](freshness-r2-verification-2026-10-03.md)
- [R3 publication/Dream crash-recovery verification record](freshness-r3-verification-2026-10-03.md)
- [Behavioral contracts](behavioral-contracts.md)
- [Current limitations](current-limitations.md)

## Checkpoint documentation and architecture impact

Documentation impact:
- Inspected: accepted F1–F7/policy/ADR documents, repository guidance, current producer/storage/reconciliation/lifecycle source and new contract tests.
- Updated for this pause: this specification, documentation index and main implementation-plan handoff link.
- Not affected by respec: Rust implementation and the accepted numerical policy.
- Compliance check: structural and change-impact documentation checks passed.
- Known documentation gaps: earlier implementation-document edits still need reconciliation with verified final behavior under R7.

Architecture impact:
- Standards added or changed: none.
- Ownership or boundary impact: no new implementation boundary in this pause; remaining work retains Freshness semantics outside Container.
- Pitlord enforcement impact: existing working-tree ownership additions remain unverified.
- Other verification impact: stopped the combined build; architecture refresh previously timed out.
- Known architectural gaps: R2–R6 enumerate outstanding durability, ordering, memory-bounds, proof, scheduling and consumer integration work.

## Notes

The accepted defaults remain score −100..100, creation +100, access +25, first-cycle linkage +50, decay one point per ten admitted REL turns, destination hop costs five/ten, and Fresh/Stale/Dormant thresholds. Saturation preserves the configured decay remainder, including at −100.

No broad Perception audit, replacement review scheduler, complete Ego summary pipeline, or PHY freshness clock is added by this handoff. Existing source is a partial implementation to finish, not evidence that its acceptance gates passed.