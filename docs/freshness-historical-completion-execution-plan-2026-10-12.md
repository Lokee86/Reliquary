# Historical Freshness completion execution plan — 2026-10-12

Parent: [Documentation index](INDEX.md).

## Purpose

Finish the remaining H1–H8 historical Freshness work after the Step 26 audit corrected the earlier completion claims. This plan does **not** redo live F8 or the historical timeline, propagation, Dream, scoring, checkpoint-format, or physical-transform work that already has focused evidence. It closes the missing native orchestration, recovery, importer/embedding integration, and end-to-end acceptance.

## Overview

The remaining work is a native integration/recovery closure, not a redesign of Freshness mathematics. Merge current `main` first, then replace the model-only orchestration with one Cva-owned path through the existing native checkpoint, close byte-level recovery and importer/embedding integration, and rerun end-to-end acceptance on retained evidence.

## Current state

**Live F8 is complete as prospective explicit owner opt-in v1.** Normal live semantic reinforcement remains local and non-propagating.

Historical reconstruction already has:

- frozen H1 manifest/timeline policy;
- canonical timeline/provenance planning;
- historical semantic planning;
- recipient-only import propagation over the completed authorized graph;
- approximate first-cycle Dream event planning;
- sparse chronological scoring with ordinary Freshness saturation/decay;
- native `CVAFRS03` superseding checkpoint acceptance through `Cva::freeze_historical_import` / `FrozenHistoricalImport::prepare` / `Cva::commit_historical_import`;
- copy/repack/reclamation/reconciliation preservation tests; and
- partial model-level orchestration/recovery evidence.

C2–C6 delete the experimental in-memory coordinator and duplicate checkpoint store, derive the canonical manifest and events from authoritative owner state, and integrate native one-shot publication. Native checkpoint construction/publication is private to Cva; the sole caller operation accepts only options. Offline/exclusive reconstruction now ends in FreshnessStore/CVAFRS03. Native checkpoint/receipt byte-prefix recovery, passive due-index rebuild and live-safe retries are verified in C6. C7 freezes the bounded embedding-only contract below; its implementation remains unavailable. Mounted/importer and embedding integration, retained-corpus measurements and release disposition remain C8–C12.

At the time this plan was written, `feat/freshness-semantic-historical-linear` was **66 commits behind `main` and 28 commits ahead**. Current `main` contains later multiplexing/import/runtime ownership work. Convergence is therefore a prerequisite, not cleanup after implementation.

## Release target

Historical reconstruction is complete when a supported **explicit opt-in, REL-local** import path can:

1. finish one complete imported corpus generation;
2. construct and verify the frozen H1 manifest from authoritative owner state;
3. reconstruct semantic and first-cycle Dream historical events without rerunning Insomnia/Dream;
4. propagate/score those events under the accepted historical rules;
5. atomically supersede provisional import Freshness through the existing Cva/Freshness owner;
6. survive crash prefixes, reopen, retry, physical transforms, reconciliation and subsequent legitimate live events;
7. optionally fill missing source embeddings through a bounded explicit embedding-only operation;
8. demonstrate the complete path with integrated fixtures and retained-corpus measurements.

**Default activation is not required for H1–H8 completion.** The first supported release may remain explicit opt-in. PHY Freshness remains out of scope.

## Model assignment

- **Sol** — convergence decisions, ownership/seam changes, durable recovery, importer/provider boundaries and final release review.
- **Luna** — bounded implementation under frozen seams, fixtures, deterministic orchestration and verification.
- If a Luna step encounters a new ownership or durable-format decision, stop that step and return the decision to Sol rather than adding another compatibility layer.

## Hard invariants

1. **One durable owner.** `Cva` / `FreshnessStore` remains the sole accepted score, receipt and import-checkpoint authority. `HistoricalCheckpointStore` must not become a second production ledger.
2. **Use the existing checkpoint.** Prefer the existing `CVAFRS03` native checkpoint. Do not introduce another durable historical event log merely to make orchestration convenient.
3. **No live semantic propagation.** Only historical import semantic events use graph propagation.
4. **No fabricated +25 history.** Historical reconstruction never invents consumed-use events.
5. **Full graph, credit-only time gating.** Authorized future/ineligible Memories may be transit nodes; historical eligibility controls credit, not traversal.
6. **Canonical source-event identity.** Historical event identity is the H1 owner/import/source identity, not a recipient-specific rehash and not a function of worker order or computed effects.
7. **No caller authority substitution.** A production coordinator must derive/verify manifest, graph, Community, vector, provenance, cohort and replacement proof from the frozen Cva owner state. Caller-supplied precomputed plans are not authorization or acceptance evidence.
8. **No mutation during the frozen attempt.** Any owner-input drift between freeze and commit invalidates the attempt and requires refreeze/replanning.
9. **Provider I/O stays outside owner locks.** Optional embedding work uses the existing remote-provider boundary and is complete before the final manifest freeze.
10. **No automatic/default importer activation in this project.** Completion establishes a supported explicit path; default policy can be decided later.

## Remaining sequential work

| # | Model | Work | Completion gate |
| ---: | :---: | --- | --- |
| **C1** | **Sol** | **Converge current `main` into the feature branch before further implementation.** Merge, rather than silently rebuilding against the pre-multiplexing runtime. Resolve import/runtime/Archive/Freshness conflicts according to current `main` ownership. Preserve all finished F8 semantics and existing H1–H7 primitives. Re-audit exact importer, generation-barrier, vector-worker and owner-mutation seams after the merge. | Feature branch contains current `main`; focused F8, historical primitive, native checkpoint and transformation suites pass; a short post-convergence seam note names the current source owners used by C2–C8. |
| **C2** | **Sol** | **Hard-cut the historical production seam to one native Cva coordinator.** Define one narrow internal/facade operation that accepts only true operation options (for example whether bounded missing-embedding fill is allowed), not caller-authored manifest/plans/authorization sets. It coordinates existing owner modules and returns a structured reconstruction report/result. The experimental in-memory coordinator may remain as a pure test oracle only if it cannot be mistaken for acceptance; otherwise delete it and its duplicate checkpoint model. | There is one documented production call path. No production caller can publish a `HistoricalCheckpointStore` result or bypass `Cva::commit_historical_import`. |
| **C3** | **Sol** | **Build the canonical H1 manifest from frozen authoritative owner state.** Under the current owner mutation boundary, derive corpus/import identity, exact Archive activity order, provenance/availability/cohorts, Memory body/lifecycle bindings, full authorized Memory adjacency and Dream-origin proof, duplicate authority, validated-or-unknown Community proof, vector/profile/source-fragment evidence, provisional replacement proof and complete input token. Reproduce the frozen H1 canonical digest. Reject clock mismatch, unresolved authority, cross-owner evidence, malformed Community, input-policy mismatch and unisolatable provisional scoring. | Golden H1 digest fixtures pass; the coordinator cannot accept an arbitrary manifest digest; every accepted manifest field is traceable to current Cva-owned evidence. |
| **C4** | **Luna** | **Wire canonical historical event production from that manifest.** Generate semantic events using owner/import/exact source key identity and Dream events using owner/import/source Memory identity. Apply exact source/Episode/cohort self-exclusions. Reuse current F8 threshold/bonus and historical recipient-only propagation rules. Preserve one source event across all recipients; do not derive recipient-specific event identities. | Fixtures prove stable IDs across worker counts/retry, correct semantic and Dream exclusions, future-bridge traversal with no future-node credit, one maximum contribution per recipient/source event and no fabricated +25 history. |
| **C5** | **Sol** | **Replace model publication with the native one-shot pipeline.** Orchestrate: generation-complete barrier → optional embedding completion if requested → final manifest freeze → timeline/semantic/Dream planning → historical propagation → sparse chronological scoring → `FrozenHistoricalImport::prepare` → `Cva::commit_historical_import`. The final score events must be covered by the frozen manifest and checkpoint cut. No permanent event queue is retained. | A real REL can execute the whole operation through Cva and reopen with the accepted `CVAFRS03` projection. The model store is not involved in production acceptance. Exact pre-commit drift rejects and leaves the REL unchanged. |
| **C6** | **Sol** | **Close native crash/reopen/idempotence semantics.** Test actual Container bytes, not an in-memory map. Enumerate crash prefixes around the checkpoint append including container framing; reopen each through the normal Cva open path and prove the state is either the old provisional projection or the complete checkpoint, never partial. Prove passive reopen rebuilds the due index, exact import retry remains a no-op after accepted checkpoint, and a retry after legitimate post-checkpoint live growth does not erase that growth. Include a real multi-recipient accepted-use receipt after the checkpoint. | Byte-prefix/reopen/retry suite passes against native files; no half checkpoint, duplicate award, global event-ID recipient suppression, or lost post-checkpoint live event is possible. |
| **C7** | **Sol** | **Freeze the optional embedding-only operation contract.** It is explicit opt-in and runs before the final manifest freeze. Pin owner/profile/source generation, maximum sources/fragments/work, concurrency, cancellation semantics and coverage reporting. Reuse existing embedding/provider execution outside owner locks. Do not run Insomnia or Dream, and do not synthesize text. Failed/cancelled fill leaves no historical checkpoint and can be retried. | The operation has a small bounded interface, explicit resource limits and deterministic retained evidence suitable for inclusion in the H1 vector proof. |
| **C8** | **Luna** | **Implement the bounded embedding-only operation and integrate the current importer/runtime entry point.** Fill only missing genuine accepted source-fragment vectors, retain them through the existing vector authority, then restart/finalize manifest freeze. Add an explicit historical-reconstruction option to the current native import workflow or its post-generation management surface; do not enable it by default. Preserve already-enrolled-owner protection and reject noncanonical original Archive acceptance order. | One real import fixture reaches generation completion, optionally fills missing vectors, reconstructs through the native Cva path, commits, reopens and can then enable prospective live F8 after the historical cut. |
| **C9** | **Luna** | **Run the integrated correctness matrix.** Cover local-live-vs-propagating-historical semantic divergence, future transit bridge, stale/malformed Community behavior, duplicate representative authority, actual first-cycle proof/no-link/fallback Dream precedence, missing/unsupported provenance, Archive clock mismatch, graph/vector/body drift, unauthorized/cross-owner evidence, repeated import, regenerated already-enrolled owner, crash/reopen, post-checkpoint +25 use, copy/repack/reclamation and divergent reconciliation. | The matrix passes through the production coordinator and native checkpoint, not through supplied-plan/model shortcuts. Unsupported cases reject explicitly without changing accepted Freshness. |
| **C10** | **Luna** | **Run retained-corpus acceptance and measurements.** Use already retained hash-verified corpora when they contain the required source/provenance/vector/Dream evidence; otherwise create the retained fixture by running the supported importer/generation path from a retained source export. Measure CPU, peak temporary memory/spill, embedding fill cost when used, semantic candidate/vector coverage, match distribution, graph/edge traversal, checkpoint size/commit/reopen cost and final Fresh/Stale/Dormant distribution. Do not substitute empty-event model timing or topology-only snapshots. | At least one authoritative retained corpus completes the exact production path with reproducible hashes and reported coverage/performance. Any missing evidence that prevents this keeps the release blocked rather than being guessed. |
| **C11** | **Luna** | **Run the full release gate.** Execute focused C2–C10 suites, all existing F8/Historical/Core Freshness regressions, full root tests, CLI tests, formatting, documentation policy, architecture checks and patch hygiene on the converged branch. Update API/storage/architecture/invariants/current-limitations only to describe actually supported behavior. | All required gates pass on the final source; a new dated historical release verification record contains exact commands/results and supersedes the partial Step 22/24/25 records without deleting their historical evidence. |
| **C12** | **Sol** | **Final production disposition.** Audit the final implementation against ADR 0039 and H1. If C1–C11 are green, mark H1–H8 complete as a supported explicit opt-in REL-local feature and merge-ready. Default activation may remain off. Keep PHY explicitly deferred. If retained-corpus semantic quality argues against default enablement, record that as a product policy decision rather than mislabeling the implementation incomplete. | One truthful final disposition exists; roadmap/index/status docs distinguish “implemented explicit opt-in” from “enabled by default.” |

## C1 post-convergence seams — 2026-10-12

Current main cut: `ca2076cf657925cabdb6e48e39ce6e8d4619ff5c`. The merge preserves current Gateway/Instance ownership and removes the retired mixed-owner host/vector modules. The F8 runtime fixture now uses a mounted REL through Gateway; exact-owner background status includes pending semantic turns.

| Seam for C2–C8 | Current source owner |
| --- | --- |
| Native offline/exclusive import and retry | `src/imports/commit.rs`: `Cva::import_native`; pure normalization/preparation in `src/imports/` |
| Import receipts and per-Node Episode coverage | Archive `archive_import.rs` / `episode_import.rs`; `CVAIMPT1` remains Archive authority |
| Generation completion | Insomnia queue/completion plus settled first-cycle Dream proof; current `freeze_historical_import` rejects unfinished work and pending passes |
| Mounted owner lifecycle and quiescence | Gateway `runtime_host_gateway.rs`, exact admission tokens, REL execution lifecycle and existing owner mutation sequencer |
| Genuine source vectors and live F8 | `runtime_host_rel_vectors.rs`; prepare/commit through Cva semantic adapters, provider embedding outside owner locks, commit under revalidated lease and `RelShared::mutate` |
| Exact-owner work inventory | `runtime_host_gateway_background.rs`: `background_status_for`; ambient selection is not operation authority |
| Native historical publication/reopen | `cva_historical_import.rs`, `historical_import_input.rs`, `historical_import_transform.rs`, Cva lifecycle validation and FreshnessStore `CVAFRS03` |

C2–C8 must compose these owners. Native import receipts acknowledge source/queue publication, not completed inference. The current freeze guard is not yet a mounted generation/quiescence operation. Caller-provided manifest hashes remain a C3 defect; the in-memory checkpoint composition remains experimental. Mounted REL Dream integration is still limited as recorded in [Current limitations](current-limitations.md); convergence does not claim to close that gate.

### C1 disposition and verification

**C1 complete.** Main `ca2076c` is merged; finished F8 and historical primitives are preserved. C2 is next. Historical production remains blocked by C2–C12.

- `cargo check --locked -j 2`: passed.
- Focused library filters: semantic 72 passed (1 measurement ignored), historical 61, import_checkpoint_storage 2, background 10.
- Full library: 1,208 passed, 3 ignored. All integration targets: 138 passed, 0 failed. Doctests passed (0 tests).
- CLI format/check/tests passed: 19 tests. Root format, both documentation-policy modes and patch hygiene passed.
- `python scripts/check_architecture.py --refresh` passed: 75 P4 fixtures, 161 classified APIs, 11 external ownership fixtures and 23 Pitlord rules. The fresh snapshot has 66,519 source nodes and **0 relationships**; this is not edge-based architecture proof.

Windows verification used process-local TMP/TEMP in the short worktree directory `.t`. Longer `target/test-tmp` and `target/t` paths exceeded or reached the native Windows rename bound for external plans. An intermediate retry exhausted disk space; only generated branch-local PDB files were removed, recovering about 17 GB. Failed attempts are not passing evidence.

The initial plain full `cargo test --locked -j 2` stopped at LNK1318 PDB LIMIT for the `live_runtime_adapter` test and `dream_write_validate` example. The complete runtime matrix passed through `cargo test --lib` and each integration target. The affected targets were built with `cargo rustc --locked -j 2 --test live_runtime_adapter -- -C debuginfo=0` and the equivalent `--example dream_write_validate` command; the resulting live-runtime executable passed. Build profiles are unchanged. The original plain full command is not claimed green.

Documentation impact:
- Inspected: AGENTS, canonical owner docs, roadmap and this plan.
- Updated: merged docs, canonical REL worker wording and C1 seam/evidence.
- Not affected: numerical policy, H1 policy and release scope.
- Compliance check: both required documentation-policy modes passed.
- Known documentation gaps: C2–C12 remain pending.

Architecture impact:
- Standards added or changed: none.
- Ownership or boundary impact: main Gateway/Instance and native import owners preserved; retired host modules stay deleted.
- Pitlord enforcement impact: main policy retained; refreshed check passed with zero-relationship limitation above.
- Other verification impact: authenticated mounted F8 fixture and exact-owner pending status migrated; runtime gates passed with recorded Windows adjustments.
- Known architectural gaps: native coordinator, manifest authority, generation/embedding and recovery remain later steps.

## C2 disposition and verification

**C2 complete: one owner-only operation boundary.** C3 is next. The declared integration window continues through C3–C5; this is not a released reconstruction pipeline.

- Deleted the experimental model executor, duplicate checkpoint store and their five model-only tests. No aliases, test-oracle store or second ledger remain.
- Cva::reconstruct_historical_freshness takes only HistoricalReconstructionOptions and returns a structured report/error. The embedding-fill request is false by default and explicitly unavailable until C7–C8.
- FrozenHistoricalImport, PreparedHistoricalImport and native freeze/prepare/commit are private to the Cva owner module. Existing native checkpoint and H7 fixtures moved beneath that module; shared Insomnia setup remains test-only.
- Owner readiness uses the existing durable REL identity, pinned policy, Insomnia inventory and Dream state. A valid imported owner receives PipelineUnavailable until C3–C5 integrate verified manifest/events. Rejected options and unfinished generation leave bytes, versions and accepted scores unchanged.
- The eventual accepted path is coordinator -> owner-derived manifest/events -> private native freeze/prepare/commit -> FreshnessStore / CVAFRS03. Publication cannot use caller-authored proof, graph, cohort, scoring or authorization arrays.

Verification:
- cargo check --locked -j 2 and cargo check --locked --tests -j 2 passed.
- Historical filter: 58 passed; native import_checkpoint_storage: 2 passed; semantic: 72 passed, 1 measurement ignored.
- Full library: 1,205 passed, 3 ignored. CLI format/check/tests passed, 19 tests.
- Source audit searched all 806 source files: no model-store/executor references remain; native freeze calls occur only in owner-module tests.
- Architecture refresh and Pitlord passed: 23 rules, 68,247 source nodes, 2,367 relationships; snapshot sha256:57bf46c455a377e1d9fe3828b382b2992d24f56bc92557415db166d839eb1179. P4 and external source guards passed.
- Both documentation-policy modes, root formatting and patch hygiene passed. Native tests used short process-local TMP/TEMP under .t; generated fixtures were removed.

Documentation impact:
- Inspected: canonical owner/API/storage/invariants/limits/maintainer docs and this plan.
- Updated: C2 API, ownership, limitations, fixture routing and verification.
- Not affected: numerical policy, H1 digest policy and durable formats.
- Compliance check: both required policy modes passed.
- Known documentation gaps: operational reconstruction remains C3–C12 work.

Architecture impact:
- Standards added or changed: none.
- Ownership or boundary impact: one Cva coordinator; native publication private; duplicate owner deleted.
- Pitlord enforcement impact: policy unchanged; refreshed graph/check passed.
- Other verification impact: byte-preserving rejection plus native checkpoint/transformation regressions and full library/CLI gates.
- Known architectural gaps: manifest, events, native orchestration/recovery, bounded embedding and importer integration remain later steps.

## Native coordinator shape

The preferred architecture is a deep facade-owned coordinator rather than a new subsystem:

```text
current import/generation owner
        |
        v
Cva historical reconstruction coordinator
        |
        +-- verify generation barrier
        +-- optional bounded embedding-only fill
        +-- build/verify H1 manifest from owner state
        +-- historical timeline/provenance planner
        +-- semantic + Dream event producers
        +-- import-only propagation
        +-- sparse chronological scorer
        +-- FrozenHistoricalImport::prepare
        +-- Cva::commit_historical_import
        |
        v
existing FreshnessStore / CVAFRS03 authority
```

The interface should expose operation options and a result/report, not every internal plan. Timeline, propagation, semantic, Dream and scoring modules remain implementation details behind the coordinator.

## Recovery oracle details

C6 must exercise native storage rather than only `ImportCheckpoint::decode`:

1. Create a real imported REL with provisional birth/Dream scoring and freeze a valid historical attempt.
2. Produce a valid prepared checkpoint and capture the exact append bytes including outer Container framing.
3. For every meaningful prefix boundary, copy the pre-commit REL, append/truncate to that prefix and reopen using the normal Cva recovery path.
4. Assert either:
   - no accepted historical checkpoint and the exact pre-commit Freshness projection; or
   - one complete accepted checkpoint and the exact reconstructed projection.
5. Never allow a partially decoded checkpoint, partially replaced score map, missing receipt identity, or a due index inconsistent with the accepted projection.
6. After a complete checkpoint, append one real accepted-use event with multiple recipients, reopen, retry the historical operation and prove the accepted-use postimage is preserved exactly.

If the underlying Container guarantees atomic record visibility at a lower layer, the tests should prove that guarantee at the Cva boundary rather than duplicating a second transaction protocol.

## Integrated import oracle

C8/C9 need at least one fixture that begins before reconstruction rather than injecting historical plans:

```text
source export
  -> current importer
  -> canonical Archive accepted order
  -> Insomnia completes
  -> Dream completes
  -> source vectors complete or bounded fill
  -> native historical reconstruction
  -> CVAFRS03 accepted
  -> reopen
  -> optional prospective live F8 activation
  -> new accepted turn / legitimate live reinforcement
```

The fixture must prove that historical reconstruction supersedes only isolated provisional import scoring and that later live activity is ordinary current-time Freshness, not part of the historical replay.

## Explicit non-goals

- PHY Freshness or a shared REL/PHY clock.
- Cross-owner historical propagation.
- Automatic reconstruction for every existing REL.
- Retrofitting already-enrolled owners whose provisional scoring cannot be isolated.
- Reconstructing historical consumed-use events.
- Reconstructing historical graph versions or edge timestamps.
- Making live semantic reinforcement propagate.
- A second durable historical event ledger.
- Default semantic/historical activation policy.
- New Insomnia or Dream inference solely for reconstruction.

## Completion accounting

The old linear plan remains the design/execution history. For release accounting after this plan:

- Steps 1–21 and 23 remain accepted for their scoped evidence after C1 convergence regression.
- **Step 22 closes through C6.**
- **Step 24 closes through C2–C8.**
- **Step 25 closes through C9–C11.**
- Step 26's 2026-10-12 blocked audit remains historical evidence; **C12** is the final post-fix disposition.

Do not mark H1–H8 complete until C1–C12 are satisfied on the converged branch.

## Related docs

- [Combined Freshness production review — 2026-10-12](freshness-combined-production-review-2026-10-12.md)
- [Original F8 + H1–H8 linear implementation plan](freshness-semantic-historical-linear-implementation-plan-2026-10-11.md)
- [Historical Freshness manifest and timeline policy](freshness-historical-manifest-policy.md)
- [Historical import reconstruction specification](freshness-historical-import-reconstruction-spec-2026-10-03.md)
- [Historical checkpoint recovery model evidence](freshness-historical-checkpoint-recovery-verification-2026-10-12.md)
- [Historical import model composition evidence](freshness-historical-import-coordinator-verification-2026-10-12.md)
- [Historical reconstruction partial verification](freshness-historical-reconstruction-verification-2026-10-12.md)
- [ADR 0039 — unified deterministic Memory Freshness](decisions/0039-unified-deterministic-memory-freshness.md)

## Notes

This plan is intentionally narrower than the original 26-step sequence. It preserves accepted primitive behavior and treats current `main` as the source of truth for runtime/import ownership after C1 convergence. If convergence changes an internal file/module name without changing the accepted contract, update the implementation routing rather than reopening the policy.


## C3 disposition and verification

**C3 complete: canonical H1 manifest from exclusively borrowed authoritative Cva state.** C4 event production is next; the declared integration window continues through C5.

- Private freeze accepts no supplied digest. The simplified planner manifest hash is deleted. Checked typed canonical encoding reproduces H1 policy digest 0cad2e7fd763de37753a54c28df1c7b6303c6c818145492ca4a709023c420908 and commits all fifteen required top-level fields.
- Required retained Node content/ancestry, exact Archive prefix/order, complete native grouped publications and proven singleton births, local provenance, cohort maximum availability and body/revision/lifecycle/scoring bindings are verified. Missing undeclared singleton provenance is explicit unsupported/untouched; missing declared evidence rejects.
- Full typed graph and authorized Memory adjacency, existing duplicate authority, actual retained Dream proof, validated-or-unknown Community, profile/generation/source/target vector commitments, disjoint provisional replacement identities and accepted payload-family prefixes are frozen. No second durable manifest store is introduced.
- The previously transcribed planner/checkpoint H1 policy constant was wrong. It now matches the descriptor golden digest; unsupported wrong-digest unreleased checkpoints reject without a compatibility path.
- Native byte-preserving/reopen/retry/H7, complete-field digest sensitivity, singleton/unsupported, malformed Community, legitimate-use isolation and non-activity cycle regressions pass. The coordinator verifies this proof then reports PipelineUnavailable until C4–C5 complete.

Verification:
- Full library: 1,213 passed, 3 ignored. Focused historical and canonical-owner regressions passed.
- Root all-integration test compilation, root/CLI formatting, CLI tests (19), both documentation-policy modes and patch hygiene passed.
- Architecture refresh passed: 23 rules, 68,772 source nodes, 2,367 relationships; snapshot sha256:989a3363ab788647fdf62688b30afc1b13e058347284f9de5a86a789986c41b0. P4/external ownership guards passed.
- Short process-local TMP/TEMP fixtures were removed. An initial supplied-hash conflict fixture failed because identical native input now derives identical manifests; changing real retained owner input corrected the fixture. That failed attempt is not passing evidence.

Documentation impact:
- Inspected: canonical owner/API/storage/invariants/limits/maintainer docs, H1 policy and this plan.
- Updated: H1 native routing, policy correction, source ownership and C3 evidence.
- Not affected: shipped numerical policy, durable framing and opt-in release scope.
- Compliance check: both required documentation-policy modes passed.
- Known documentation gaps: C4–C12 remain pending.

Architecture impact:
- Standards added or changed: none.
- Ownership or boundary impact: only Cva derives accepted manifest proof; graph/Dream representative authority and existing FreshnessStore remain owners.
- Pitlord enforcement impact: policy unchanged; refreshed graph/check passed.
- Other verification impact: native complete proof, unsupported preservation, input/isolation rejection and full library/CLI regression coverage.
- Known architectural gaps: event integration, one-shot publication/recovery, bounded embedding and importer/release acceptance remain C4–C12.


## C4 disposition and verification

**C4 complete: canonical historical semantic/Dream identities and exclusions through the native owner path.** C5 native one-shot publication is next.

- Private freeze derives and retains a canonical batch from the verified H1 projection; operation inputs contain no supplied roots, exclusions or event identities.
- Semantic identity uses the frozen typed owner UUID/import/exact source map and raw 32 bytes. An independent golden verifies encoding; changed evidence/amounts keep the same identity. Strongest-fragment reduction and provenance/cohort/availability/lifecycle/profile exclusions precede top-64 selection.
- Propagation inherits frozen recipient exclusions while retaining bridges. Mixed events sort by (position, raw event ID), without kind priority or recipient rehashing.
- Actual Dream roots/duplicate origin are verified against accepted first-cycle graph transactions. Existing authority resolves representatives; roots must be strictly older. Verified no-link precedes later adjacency, including archived sources. Unusable markers are unsupported; the pure verified direct-link approximation contract remains.
- Credit-excluded records are unadmitted; unsupported provenance and disjoint provisional history stay untouched. Checkpoint input digest/extent come from the manifest frozen prefix, with drift rejection.
- Native Import/Insomnia/Dream/vector fixtures verify complete grouped self-exclusion, multi-root source identity, archived duplicate origin, no-link precedence, mixed ordering, unchanged bytes and reopen equality. Publication remains PipelineUnavailable until C5.

Verification:
- Historical: 71 passed. Full library: 1,218 passed, 3 ignored. CLI: 19 passed.
- Final frozen-input correction passed the historical matrix again, root integration-target compilation, CLI check/format, both documentation-policy modes and patch hygiene.
- Architecture refresh/Pitlord and P4/external ownership guards passed.
- Temporary fixtures were removed. Missing-fragment, stale exclusion expectation and knowledge-versus-extracted duplicate fixture failures were corrected; failed attempts are not passing evidence.

Documentation impact:
- Inspected: canonical owner/API/storage/invariants/limits/maintainer docs, H1 policy and this plan.
- Updated: C4 routing, identity/proof/exclusion rules, scope and verification.
- Not affected: live F8 policy/identity, durable framing and opt-in REL-local release scope.
- Compliance check: both required documentation-policy modes passed.
- Known documentation gaps: C5–C12 remain pending.

Architecture impact:
- Standards added or changed: none.
- Ownership or boundary impact: producers stay below private Cva freeze; Dream scan shares its existing owner; FreshnessStore/CVAFRS03 remain sole publication authority.
- Pitlord enforcement impact: policy unchanged; refreshed graph/check passed.
- Other verification impact: golden identity, native generation/vector/reopen/exclusion and library/CLI regression evidence.
- Known architectural gaps: publication/recovery, bounded embedding, importer and retained-corpus/release closure remain C5–C12.

Final architecture evidence: sha256:c9651678ffe85a0b23824b4244ba844f01536dbb584607f093d95cee8b6dcf63; 23 rules, 68906 source nodes, 2369 relationships.

## C5 disposition and verification

**C5 complete: native one-shot reconstruction through Cva → FreshnessStore → CVAFRS03.** Final documentation/architecture checks are recorded below. C6 native crash-prefix recovery is next; this is not H1–H8 release completion.

- The exclusive Cva operation verifies settled generation, derives the final manifest/timeline and canonical semantic/Dream batch, propagates using the graph/Community/availability retained by that manifest, scores chronological sparse recipient contributions, prepares the native checkpoint and commits through the existing owner.
- Production preparation accepts no supplied score plan. Raw source identity and position remain shared across recipients; contributions must target admitted reconstructed records and stay within the frozen cut. One maximum contribution per recipient/source is emitted. No permanent event queue or alternate ledger is retained.
- Exact accepted retries return the stored checkpoint before new owner-input/generation checks, with committed=false and no append/rescoring. Legitimate subsequent activity is preserved. Optional embedding fill still explicitly rejects until C7–C8.
- Native fixtures execute the public operation with genuine Import/Insomnia/Dream and retained vector evidence, prove multi-recipient semantic scoring, Dream scoring, CVAFRS03 bytes and normal reopen, accepted retry/no writes and retry after live growth. A clock-neutral retained-file change invalidates the frozen input before projection replacement, preserving bytes and accepted scores.
- Mounted callers still require existing Gateway quiescence/admission. Exhaustive crash framing/reopen/due-index/multi-recipient-use recovery remains C6; importer/embedding integration and release acceptance remain C7–C12.

Verification:
- cargo test --locked --lib historical -j 2: 75 passed.
- cargo test --locked --lib -j 2: 1,223 passed, 3 ignored. Native import-checkpoint filter: 4 passed.
- cargo check --locked --tests -j 2: passed, including compilation of all integration-test targets.
- CLI locked check and tests: passed, 19 tests. Root and CLI formatting checks passed.
- Full root integration execution remains part of C11; the converged Windows all-target link/PDB limitation is recorded in C1. No full-release claim is made here.
- Final review closed a native checkpoint validation gap: final credit-excluded replacements now accept only canonical unadmitted creation records. Native archived-duplicate publication/reopen/retry and codec corruption/roundtrip fixtures pass. The first duplicate fixture incorrectly expected prior admission; the actual native Dream alias is already unadmitted, and the expectation was corrected. Initial store_file signature compilation was also corrected. Failed attempts are not passing evidence. Temporary fixtures are removed before architecture verification.

Documentation impact:
- Inspected: architecture, invariants, storage-format, maintainer-map, current-limitations, roadmap, API, H1 policy and active completion plan; shared engineering rules.
- Updated: supported C5 operation, frozen propagation inputs, native publication/retry/drift behavior and remaining gates.
- Not affected: live F8 numerical policy, durable checkpoint framing, PHY scope and default activation.
- Compliance check: both structural and changed-from-origin/main documentation-policy modes passed.
- Known documentation gaps: C6–C12 verification/disposition remains pending.

Architecture impact:
- Standards added or changed: none.
- Ownership or boundary impact: Cva derives all production plans and retains verified propagation inputs; existing FreshnessStore/CVAFRS03 remains sole acceptance authority. No generalized store or queue is introduced.
- Pitlord enforcement impact: policy unchanged; refresh passed all 23 rules without violations.
- Other verification impact: native public operation, multi-recipient source identity, semantic/Dream scoring, file drift and reopen/retry regressions.
- Known architectural gaps: exhaustive native recovery, mounted/importer/embedding boundaries, retained-corpus measurement and release gates remain C6–C12.

Final C5 gates:
- Both documentation-policy modes and git diff --check passed on the final implementation; required ownership guards passed (75 P4 fixtures, 161 classified public APIs, 11 external ownership fixtures).
- Architecture refresh passed: sha256:6e9f1332083655835663b7480401c2262d17516e34869f660a194c3783bd5d66; 69,021 source nodes, 2,369 relationships, 23 rules, no violations.


## C7 frozen embedding-only contract

**C7 complete: implementation contract frozen; C8 implementation remains unavailable.** This section is the canonical C8 handoff. The existing public `fill_missing_embeddings=false` default is unchanged; requesting true still returns EmbeddingFillUnavailable until C8 passes its gate.

### Entry, ownership and pinned input

The post-generation import/management operation accepts reconstruction options, an existing configured EmbeddingEndpoint and a cooperative cancellation flag. Endpoint selection is capability configuration, never caller-supplied vector/profile/manifest authority. One optional bounded-fill settings object may lower the limits below; omitted settings use these v1 ceilings. Reject zero, overflow or values above a ceiling before provider I/O or writes. No source IDs, score plans, authorization sets or arbitrary generation IDs are accepted.

Resolve an exact accepted historical retry first: return the stored report without embedding, probing, changing activation or rescoring, even when fill is requested. Otherwise require the same complete single-owner import/readiness and canonical Archive-clock gates as native reconstruction, before any provider work. REL only; live F8 remains disabled. Already-enrolled or regenerated owners reject through existing owner proof. No Insomnia, Dream, Community rebuild, target-Memory embedding or synthesized summaries are scheduled.

Cva privately derives a fill plan under exclusive owner access. Pin UUID/import/corpus identity, canonical source order/activity cut, Archive version, complete owner input token/prefix, all relevant immutable source/content/fragment bindings and the selected profile/generation. Select the latest retained generation by vector_version, exactly as H1 currently does. Verify its retained profile against the configured endpoint using the existing compatibility probe. If no generation exists, require exactly one retained profile; zero or ambiguous profiles reject (profile creation is a separate operation). Never silently switch vector spaces. Pin endpoint/route identity for the operation; replacement invalidates the plan.

Mounted integration uses the existing Gateway owner lifecycle/admission fence, closes new mutating admission and quiesces all owner workers/operations for this explicit operation. Do not merely pause vectors or keep an owner mutex across inference. Offline integration retains exclusive ownership. Provider probing and embedding run outside owner/runtime locks; reenter a short, revalidated owner mutation boundary for each durable publication. Owned snapshots passed to the provider carry no mutable Cva references. Release the fence on every exit. C8 must expose a concrete lifecycle path or reject mounted execution; it cannot approximate quiescence with a second coordinator.

### Population and explicit resource ceilings

All byte limits mean bytes, not tokens. Bounds include retained-row copying and validation, not only new provider rows. Check arithmetic before allocating; preflight itself consumes bounded inspection/allocation counters and rejects the whole attempt if population or work cannot fit. Validate declared content/matrix lengths before hydration; do not allocate an oversized input merely to discover its limit. Never reconstruct a silently truncated fill prefix.

| Resource per invocation | Default and hard ceiling |
| --- | --- |
| Accepted sources inspected | 65,536 |
| Retained fragments inspected / resulting generation rows | 131,072 each |
| Sources requiring missing genuine rows / new rows | 4,096 each |
| New exact-turn fragment records | 4,096 |
| One source text / total source text sent | 256 KiB / 16 MiB |
| Profile dimensions | 4,096 |
| Retained plus new matrix bytes materialized for publication | 256 MiB |
| Row-component validation/copy work | 67,108,864 f32 components |
| Source/fragment binding inspections, including repeated visits | 67,108,864 |
| Peak operation-owned temporary buffers, including plan/text/output/matrix | 512 MiB |
| Inputs per source embedding call | 16 |
| Concurrent endpoint calls | 1 |
| Source embedding calls | 256 |
| Compatibility probes | 2 calls, 4 fixed inputs total |
| Total endpoint calls / inputs including probes | 258 / 4,100 |

Probe texts are the existing fixed compatibility suite, not historical source text. Probe bytes count against the total text budget. The endpoint transport for this operation must have at most one in-flight request and no implicit retry, hedging or fan-out beyond these budgets. The current remote defaults (16 concurrent requests) are not authorization for this lane. C8 must configure/verify the existing provider boundary accordingly; if it cannot enforce that boundary, reject before starting. No automatic backoff or retry occurs inside the operation. Cancellation cannot interrupt an opaque synchronous endpoint call; transport retains its existing finite request timeout. No wall-time or monetary ceiling is claimed.

Order sources by canonical activity position, then exact UTF-8 source key; order fragment rows by raw FragmentId bytes. Deduplicate source/fragment identities before budgeting. Batch deterministic contiguous missing rows, respecting both input-count and byte limits. Bounds apply even for a no-provider-work invocation; an accepted checkpoint retry bypasses planning altogether.

### Genuine missing-row selection and native vector publication

A source row is eligible only for an actually accepted user/assistant Node within the pinned cut and an exact single-turn fragment (start=end=that Node, exact conversation, one turn). Read the immutable retained content through Archive's existing fragment_text path, including its existing rendering; do not invoke a model to rewrite, summarize or expand it. If that exact fragment does not exist, Cva may ask Archive to materialize its existing deterministic exact-turn Fragment identity after full preflight. This appends a binding to genuine content, not a new accepted Node or activity tick. Include those records in mutation and resource accounting.

Reuse all existing rows of the selected retained generation byte-for-byte, including mixed/unusable rows, and retain their immutable bindings. Request only absent eligible exact-turn rows. Existing invalid, zero, incompatible or mixed evidence is reported/excluded under H1; it is not missing and is never re-embedded by this operation. Do not fill missing mixed-fragment rows, unrelated fragments or target Memory bodies to satisfy a completeness heuristic. If the existing source-readiness check still reports a source unavailable (for example an unrepresented mixed fragment), report that coverage; never fabricate completed-empty evidence. Opt-in completion means bounded work completed, not universal semantic coverage.

The existing live SemanticArchiveBatch and build_archive_vector_generation_verified paths are unsuitable directly: they embed the whole population, and the live path materializes only post-activation fragments and may accept live receipts. C8 adds narrow private historical prepare/commit adapters under Cva, reusing compatibility validation, Archive fragments, PackedVectors, ArchiveVectors and VectorGeneration publication. No new durable queue, receipt type, store or manifest ledger is introduced.

Retain each successful batch through existing immutable packed matrix/binding/generation records. Publish the union of pinned retained rows plus validated new rows in canonical FragmentId order; map each copied row by identity, not its former index. Check vector count, dimensions, normalization, finite/nonzero values and checked byte width using existing validators. Revalidate all pinned non-fill inputs and the expected prior generation/prefix before appending. Any unrelated mutation, profile/route change or competing generation rejects InputDrift; never silently drop the prepared batch. Advance the private expected token only for this operation's own verified fragment/vector writes.

Generation source_archive_version covers the pinned source cut and any materialized exact fragments. Generation publication validates references through existing vector authority; keep F8 disabled so its ordinary prospective callback accepts no live events. Synchronize published generations before reporting a batch durable. Copying a matrix for each batch is permitted only within cumulative component-work and per-publication memory ceilings; preflight includes that amplification. C8 may publish one final union instead if it keeps all temporary work bounded. Do not rewrite existing vectors or introduce a segmented format merely to satisfy this contract.

### Cancellation, failure, recovery and retry

Check cancellation before planning/probes, before each provider call, after each return, before fragment/vector writes and before final freeze/checkpoint commit. Once observed, dispatch no more requests and discard uncommitted output. Cancellation during an uninterruptible request waits for return/timeout; no checkpoint is accepted afterwards. A cancellation racing the existing atomic checkpoint append cannot undo it: if acceptance has begun, complete append/sync and return the accepted outcome. Do not report cancelled after a committed checkpoint.

Invalid provider output, provider error, budget exhaustion, drift or cancellation stops the invocation without a historical checkpoint. Durable exact fragments and complete vector generations from earlier successful work may remain; incomplete physical appends use existing Container recovery. Reopen and retry recompute missing rows from retained authority and reuse completed compatible evidence. No cleanup deletes genuine accepted content or committed vectors. Rows without a published generation are not readiness evidence; C8 may reuse them only after proving their immutable source/profile binding, otherwise retry them within the new invocation's budget. No exactly-once provider-call claim is made across a crash.

Before final manifest freeze, reread all authoritative inputs after fill and rerun native readiness/canonical-clock gates. Never commit the pre-fill manifest/token. Reconstruction continues only after successful fill disposition; failure never falls back silently to no-fill reconstruction. The no-fill operation remains independently available and records unavailable semantic sources as today.

### Coverage report and H1 retained evidence

Return a structured fill report on success and on stopped attempts: disposition (complete, cancelled, provider-error, invalid-output, budget-exceeded, input-drift or unsupported), owner/import/profile, initial and final generation IDs/cuts, source/fragment totals, missing/reused/new rows, unavailable sources and explicit exclusion reasons, attempted/completed calls/inputs/bytes and durable progress. Sort identity/reason entries canonically. Stop reports identify whether any fragment/vector writes survived; no stop report is acceptance evidence.

H1 retains its existing top-level schema and frozen numerical policy digest. For no-fill keep vector_proof.embedding_pass="not-requested". For a successfully requested fill use a typed map with exactly contract_version (u64 1), disposition ("complete"), profile_id (raw32), selected_generation_id (raw32 or null), source_archive_version (u64 or null), and coverage_digest (raw32). The digest is H("reliquary:historical-embedding-coverage:v1", the final ordered source_records' node/position/mode/evidence objects), using H1 typed canonical encoding and the same retained evidence that supplies those records. An existing generation with zero missing rows still yields this requested-complete disposition.

Final H1 proof derives selected generation, immutable binding/row hashes and coverage anew from retained owner state, not a caller report. Do not hash attempt number, provider completion order, initial generation, cost, batch/concurrency settings or reused-versus-new counts: interrupted retries reaching identical retained evidence must not acquire a different manifest because of transient progress. Those facts belong only in the operation report. No separate durable embedding-pass log is needed; accepted CVAFRS03 already retains the resulting manifest identity. Exact accepted retries return it before fresh planning.

### C8 verification gate

C8 must exercise the real post-generation importer/management entry and native checkpoint: default disabled/no provider calls; opt-in missing exact rows; zero-work reuse; profile ambiguity/incompatibility; mixed/invalid/source-unavailable distinctions; all resource ceilings and checked overflow; malformed provider output; cancellation at each boundary; owner/route/generation drift during provider work; partial durable fill then reopen/retry; no Insomnia/Dream/F8 receipt effects; provider calls outside locks; native manifest coverage digest; final refreeze; accepted retry with fill requested; prospective F8 activation only after acceptance. Include a test endpoint that detects lock-held callbacks and counts inputs/calls, plus real immutable row-byte comparisons and reopen assertions. Failure cases assert no CVAFRS03 and unchanged accepted Freshness, while reporting allowed vector progress.

Documentation impact:
- Inspected: canonical ownership/storage/limits/roadmap, H1 policy, native reconstruction/source/vector adapters, provider compatibility probes and current runtime prepare/embed/commit boundary.
- Updated: this C7 contract, H1 handoff, index/maintainer routing and remaining-work status.
- Not affected: Rust behavior/public API, existing persistent formats, Freshness numerical policy, default activation and PHY scope.
- Compliance check: both documentation-policy modes and git diff --check passed; final confirmation is recorded below.
- Known documentation gaps: C8 implementation and C9–C12 integrated/release evidence remain pending.

Architecture impact:
- Standards added or changed: none; frozen future operation limits and lifecycle contract.
- Ownership or boundary impact: planned Cva historical adapters reuse existing Archive/vector/provider owners; sole checkpoint authority remains FreshnessStore/CVAFRS03.
- Pitlord enforcement impact: no source or static policy changes; C6 refreshed enforcement remains the baseline.
- Other verification impact: C8 executable gates specified; C7 is documentation-only and does not claim runtime coverage.
- Known architectural gaps: actual mounted fence/provider budget enforcement and importer implementation remain C8; release acceptance remains C9–C12.

Final C7 gates: documentation-only; both documentation-policy modes and git diff --check passed. No Rust/API/storage/static-policy change, so runtime tests and architecture refresh were not rerun. C6 is the last executed runtime/architecture baseline. C8 must implement and verify this contract before fill becomes available.

## C6 disposition and verification

**C6 complete: native checkpoint/live-use crash-prefix, reopen, due-index and retry closure.** Final documentation/architecture results are recorded below. C7 bounded embedding-only contract is next; H1–H8 release completion remains C12.

- Native public reconstruction produces one CVAFRS03 frame. Every byte prefix, including zero bytes, all partial u64 frame headers and payload/checksum prefixes, reopens through normal Cva as exactly the old provisional or complete accepted projection. Clocks and complete file bytes are verified; retry and second reopen are byte-identical.
- Checkpoint matrices: 485 prefixes without vectors; 668 with retained genuine source vectors, shared semantic recipients and an archived alias. A fully framed corrupt checkpoint checksum rejects without repairing or rewriting the file.
- Native public accepted-use produces one CVAFRS02 multi-recipient frame after 998 real accepted Archive turns following the checkpoint. All 1,072 frame prefixes recover wholly absent or wholly accepted. Historical retry preserves live growth; receipt retry awards all three unique Memories exactly once across worker counts and reopen. Duplicated/reordered input IDs do not duplicate credit.
- Each recipient visibly moves from -1 to +24 through the actual +25 receipt, avoiding score saturation as a false oracle. Stored checkpoint identity remains unchanged. Accepted retries perform no append or rescore.
- Passive reopen reconstructs the due dispatcher and exact Fresh-to-Stale boundary at turn 1,016. Bounded drains recover all three notices without score/file mutation; notifications remain derived and at-least-once.
- Total: 2,225 actual native frame-prefix cases. Existing production recovery passed without changes; only owner-local test fixtures and documentation change. No alternate ledger, codec or caller-authored score plan is introduced.

Verification:
- cargo test --locked --lib historical_native_ -j 2 -- --nocapture: 9 passed, including all new native recovery cases.
- cargo test --locked --lib -j 2: 1,226 passed, 3 ignored.
- cargo check --locked --tests -j 2: passed, including all integration-target compilation.
- Root/CLI formatting, CLI locked check and tests passed (19 tests).
- Full root integration execution remains C11 (C1 records the Windows link/PDB limit).
- Corrected initial non-Debug unwrap_err test compilation and reran. Failed compilation is not passing evidence. Temporary fixtures were removed.

Documentation impact:
- Inspected: canonical owner/API/storage/limits/roadmap docs, active plan, Container/due code and shared engineering rules.
- Updated: C6 native recovery evidence, actual file/receipt atomicity, live-safe retry and remaining C7–C12 work; removed completed recovery work from the future roadmap.
- Not affected: H1 encoding, Freshness mathematics, checkpoint framing/version, live F8 activation, PHY scope and importer/provider boundaries.
- Compliance check: both structural and changed-from-origin/main documentation-policy modes passed.
- Known documentation gaps: C7–C12 acceptance/disposition remains pending.

Architecture impact:
- Standards added or changed: none.
- Ownership or boundary impact: unchanged; tests remain under the private native Cva owner module and invoke public operations against actual files. Container/Freshness remain authorities; due state stays derived.
- Pitlord enforcement impact: policy unchanged; refreshed enforcement passed all 23 rules without violations.
- Other verification impact: exhaustive framing prefixes, complete corruption rejection, shared recipients/alias, real multi-recipient use and passive due recovery.
- Known architectural gaps: bounded embedding, mounted/importer integration, retained-corpus measurement and release gates remain C7–C12.

Final C6 gates:
- Both documentation-policy modes and git diff --check passed; ownership guards passed (75 P4 fixtures, 161 classified public APIs, 11 external ownership fixtures).
- Architecture refresh passed: sha256:343c7147f978746fb7cabb4d8a837532a1d25c572c2f3a8e9331dca8849aeb4b; 69,110 source nodes, 2,369 relationships, 23 rules, no violations.
