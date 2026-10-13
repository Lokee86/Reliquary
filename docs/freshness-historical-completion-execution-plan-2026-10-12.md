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

C2–C6 delete the experimental in-memory coordinator and duplicate checkpoint store, derive the canonical manifest and events from authoritative owner state, and integrate native one-shot publication. Native checkpoint construction/publication is private to Cva; the sole caller operation accepts only options. Offline/exclusive reconstruction now ends in FreshnessStore/CVAFRS03. Native checkpoint/receipt byte-prefix recovery, passive due-index rebuild and live-safe retries are verified in C6. C7 freezes the bounded embedding-only contract below, and C8 implements it through the explicit configured offline post-generation surface. Mounted historical management remains unavailable. C9 integrated native correctness is verified below; retained-corpus measurements and release disposition remain C10–C12.

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
| **C10** | **Luna** | **Normalize retained pre-release fixtures, then run retained-corpus acceptance and measurements.** Execute the [N1–N10 retained fixture normalization plan](freshness-retained-fixture-normalization-plan-2026-10-12.md): correct mixed-fragment semantic evidence and accepted-Archive clock authority, advance the 14-day/28-day/Ellis authoritative fixtures to current standard state with disposable fixture-only tooling, delete that tooling after verification, then measure the exact production reconstruction path. | At least one authoritative normalized retained corpus completes the exact production path with reproducible before/after hashes and reported coverage/performance; no general unreleased-format migration framework remains in the product tree. |
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

**C7 complete: implementation contract frozen; C8 implementation is recorded below.** This section is the canonical C8 handoff. The existing public `fill_missing_embeddings=false` default is unchanged; a new endpoint-free request returns EmbeddingFillUnavailable; C8's explicit bounded endpoint/configured-runtime operation supports requested fill.

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


## C8 disposition and verification

**C8 complete: bounded embedding-only operation and explicit configured offline post-generation entry.** Final library/CLI/documentation/architecture gates are recorded below. C9 integrated correctness is next; historical production release remains gated through C12.

- ConfiguredRuntime::reconstruct_historical_freshness accepts only true reconstruction options, lowerable C7 limits and cooperative cancellation on an exclusively held offline Cva. It selects the existing configured embedding route only for a new requested fill. Cva's explicit endpoint operation uses the same private owner path. The endpoint-free operation still rejects new fill requests; exact accepted retries resolve before endpoint/route/limits/cancellation checks.
- The owner derives exact accepted source rows in canonical activity order, pins profile/latest retained generation/full prefix and verifies native readiness/clock/provenance/replacement gates before inference. Genuine exact-turn Fragment bindings use Archive's existing identity and rendering. No accepted Node, activity tick, text synthesis, target-Memory embedding, Insomnia or Dream rerun is introduced.
- Existing generation rows remain byte-identical, including mixed/invalid evidence. Missing mixed rows are not filled and remain explicit unavailable coverage. Every successful source batch publishes and syncs an immutable FragmentId-ordered union through existing PackedVectors/ArchiveVectors/VectorGeneration. Partial progress reopens and retry requests only remaining missing rows.
- All C7 ceilings, checked arithmetic, hydration guards, cumulative full-matrix copies/inspection work and conservative temporary/proof reserves are preflighted. Combined budgets can reject populations below individual ceilings; no silently truncated prefix is reconstructed.
- EmbeddingEndpoint's bounded capability defaults to unsupported. The simulator and OpenAI-ready implementation support it. Configured remote calls bypass ordinary batch fan-out, issue one request, refuse redirects, have no implicit retry, bound response hydration and retain the existing finite timeout. Offline execution acquires no runtime mutex. Mounted Gateway historical management remains unavailable; no unsafe mounted fallback is provided.
- Cancellation is observed before/after provider work and before writes/freeze/acceptance. Failure returns structured coverage/cost/physical-write and generation-sync progress with no fill checkpoint. Exact final owner drift rejects. Once existing native checkpoint acceptance starts, it completes append/sync rather than reporting cancellation after acceptance.
- H1 requested-complete embedding_pass commits the final retained profile/generation/cut and typed node/position/mode/evidence coverage digest. Transient attempt/call/batch costs stay outside the manifest. The final manifest refreezes after successful fill, then ordinary native preparation/commit ends in FreshnessStore/CVAFRS03. No durable embedding ledger or alternate coordinator is added.
- A real schema-1 import fixture invokes import_native, actual Insomnia grouped publication and first-cycle Dream settlement, then configured local HTTP embedding and native reconstruction. It reopens/retries without provider work and enables prospective F8 only after acceptance. This is synthetic integration evidence; retained-corpus acceptance remains C10.

Verification:
- Final focused historical_embedding filter: 14 passed, including native importer/configured HTTP, immutable-row/mixed/invalid coverage, actual partial-vector reopen/retry, cancellation, shape/value errors, drift, all lowerable resource ceilings and physical-write-free preflight rejection.
- Actual OpenAI-ready transport fixtures prove redirect refusal, bounded response and one request even with ordinary batch_size=1/concurrency=16.
- Initial adapter compilation errors and a test attempting to rebuild a manifest after acceptance were corrected; a reused test endpoint's cumulative call assertion was also corrected. Failed attempts are not passing evidence.
- Full library: 1,238 passed, 3 ignored. CLI: 19 passed. Root/CLI formatting and locked checks passed; cargo check --locked --tests compiles all integration targets. Full root integration execution remains C11.
- Final review explicitly disables reqwest transport retries and rejects new zero-norm rows even for VectorNormalization::None; existing retained invalid rows stay excluded/unmodified. The final 14-test focused suite, integration-target compilation, root formatting and CLI compilation were rerun after those localized corrections. The full library/CLI execution above preceded these final corrections; no full-release claim is made.
- Both documentation-policy modes and git diff --check passed. Temporary fixtures were removed. Required ownership guards passed (75 fixtures, 161 public APIs, 11 external ownership fixtures).
- Architecture refresh/Pitlord passed: sha256:c8dc6f0493d830bfcde9e05c9b4e4a7f807fca4ee4e27870759a02217c318576; 70,035 source nodes, 2,379 relationships, 23 rules, no violations.

Documentation impact:
- Inspected: canonical API/architecture/invariants/storage/limits/roadmap/maintainer docs, native import, H1/C7 contracts, provider compatibility, vector publication and current runtime boundaries.
- Updated: supported exclusive/configured entry, endpoint capability/budgets, immutable retention/coverage/retry, remaining C9–C12 gates, behavioral/coverage routing and this disposition.
- Not affected: persistent record formats/versions, Freshness numerical policy, live F8 local/prospective semantics, default activation and PHY scope.
- Compliance check: both structural and changed-from-origin/main documentation-policy modes and git diff --check passed; final confirmation after this evidence update is recorded in the commit.
- Known documentation gaps: C9 correctness, C10 retained-corpus measurement and C11–C12 release evidence remain pending.

Architecture impact:
- Standards added or changed: none.
- Ownership or boundary impact: private Cva adapters reuse Archive/vector/provider owners; Container gains only an internal physical allocation guard. ConfiguredRuntime composes the exclusive owner, with no mounted Gateway escape.
- Pitlord enforcement impact: static policy unchanged; refreshed evidence/enforcement passed all 23 rules.
- Other verification impact: actual importer/HTTP/native checkpoint, bounded transport, partial-vector reopen/retry, cancellation and retained evidence.
- Known architectural gaps: mounted historical management is unavailable; no release/default activation claim before C9–C12.


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

## C9 integrated native correctness matrix

C9 verifies the public offline Cva reconstruction operation through FreshnessStore and the native CVAFRS03 checkpoint. The fixtures start from real Archive/Import Episodes, actual omission/extraction Insomnia completions, native Memory publications and first-cycle Dream settlement. Retained vectors and graph/Community evidence are generated through their existing owners. No supplied historical score plan or model checkpoint participates in acceptance. Private owner-derived freeze/prepare reads are assertions and stale-token rejection probes only.

| Required case | Native evidence and expected behavior |
| --- | --- |
| Local live versus propagating historical semantic events | A genuine prospective live run credits one matching root locally; its sink stays at lazy score 70. Historical reconstruction credits the sink to 72 with current Community evidence, or 67 with a stale snapshot under unknown-outside-v1. The 300-turn omission tail avoids saturation hiding the difference. |
| Future and unsupported transit bridges | An actual first-cycle Dream event at position 32 traverses the completed authorized graph through a bridge available at 331 or lacking provenance. The older sink receives 30 credit and ends at 70; the bridge receives no historical credit. Unsupported Freshness retains its original projection. |
| Current/stale/malformed Community | Current membership gives the local hop cost; stale membership uses unknown-outside-v1. Duplicate/foreign retained membership rejects native reopen without rewriting either malformed evidence or the canonical accepted owner. |
| Duplicate representative authority | Actual duplicate settlement creates one alias event through the existing canonical representative. The alias reopens unadmitted. A component with no active representative rejects without accepting a checkpoint. |
| Actual first-cycle/no-link/fallback precedence | Retained actual first-cycle evidence supplies the event. Verified no-link remains no-link despite later final-graph Dream edges. A legacy settled marker without complete retained proof remains Unsupported and does not invent final-graph fallback. Native fallback evidence is unavailable, rather than promoted from unverified final edges. |
| Missing/unsupported/foreign provenance | Isolated supported singleton admission reconstructs at its real source position; missing provenance retains its original projection. Actual retained Memory revisions with foreign structured authority or unresolved declared activity reject. Ordinary invalid provenance/foreign graph endpoints/body rewrites reject before writes. |
| Archive clock mismatch and genuine use | Canonical order conflicting with accepted Archive ordinals rejects. A real consumed-use receipt is not superseded by reconstruction. |
| Graph/vector/body/revision drift | Native prepared tokens reject graph/vector/Memory revision changes without changing the accepted projection; the public operation refreezes owner evidence. Semantic body replacement is forbidden by immutable Memory publication. |
| Unsupported replacement isolation | A provisional live Dream event crossing supported replacement and untouched unsupported Memories rejects without file/projection change. |
| Source cohorts and event identity | Existing semantic/Dream identity, complete grouped source exclusions, same-position event ordering and duplicate fixtures now also accept through the public operation and verify exact native projection after reopen. |
| Repeated import/regenerated enrollment | Public native retry is a byte-stable no-op. Exact source replay, settled Dream retry and regenerated queues preserve accepted anchors and records. |
| Crash/reopen/retry/due/post-checkpoint use | C6 actual Container byte-prefix tests rerun: 485/668 checkpoint-frame cuts plus 1,072 live-use frame cuts, complete corrupt-frame rejection, unsaturated post-checkpoint +25 and passive due-index recovery. |
| Copy/repack/reclamation | Public checkpoint acceptance now backs physical-transform fixtures. Exact copy, prefix reconciliation/promotion, orphan reclamation and project-attachment repack run with optional embedding fill both false and true, preserving score/receipt/checkpoint authority and retained vector proof. |
| Divergent reconciliation | Native checkpointed branches reject unsupported source rebasing, manifest/policy conflicts and owner/corpus transplants before output or promotion, preserving input bytes. |
| Current importer/runtime and embedding bounds | C8 native import plus configured HTTP embedding fixture reruns, including explicit post-generation reconstruction, native reopen, subsequent prospective activation, partial-fill retry/cancellation and transport budgets. |

Validation results are recorded below. C10 retained-corpus acceptance/measurements, C11 full release gate and C12 final production review remain open. C9 does not activate mounted management or default historical reconstruction.

Verification on 2026-10-12:
- `cargo test --locked --lib -j 2`: 1,251 passed, 3 ignored, no failures, including the integrated native matrix and all 2,225 native crash-prefix cases.
- `cargo test --locked --lib historical_ -j 2`: initial unrestricted run had 102 passes and one local HTTP fixture read timeout under concurrent verification load. The full library run passed that fixture. Its test-only read deadline was increased from 5 to 30 seconds, with a separate 60-second accept deadline per request; the exact fixture rerun passed after this correction. Production transport limits are unchanged. Restricted-access failures are not acceptance evidence.
- `cargo fmt --check` and `cargo check --locked --tests -j 2`: passed after the fixture correction; all integration targets compile. Full root integration execution remains C11.
- CLI formatting, locked check and locked tests: passed; 19 tests passed.
- Architecture refresh/enforcement passed: snapshot `sha256:9dc3510761a3596e675babdab5b75d451941cc72555dd841075f87754c0a5d2e`; 70,262 source nodes, 2,379 relationships, 23 rules, no violations. Ownership guards passed: 75 coverage fixtures, 161 public APIs and 11 external ownership fixtures.
- Both documentation-policy modes and `git diff --check` passed. C9 is complete; C10–C12 remain open.

Documentation impact:
- Inspected: repository/shared standards, architecture, invariants, storage-format, maintainer-map, current-limitations, roadmap, version-history plan and branching decision, API, behavioral-contracts and documentation-coverage.
- Updated: this execution ledger, behavioral-contracts, maintainer-map, current-limitations, roadmap and documentation-coverage for the integrated native verification scope.
- Not affected: public API, storage format, scoring policy, production ownership and provider contract.
- Compliance check: final structural/change-impact policy and diff checks recorded below.
- Known documentation gaps: retained-corpus measurements and final release disposition remain C10–C12.

Architecture impact:
- Standards added or changed: none.
- Ownership or boundary impact: test-only native owner composition; Cva/FreshnessStore remains sole acceptance authority.
- Pitlord enforcement impact: fresh source graph/enforcement recorded below; no rules weakened.
- Other verification impact: full library execution plus integration-target compilation; existing Windows all-target execution limitations remain explicit.
- Known architectural gaps: mounted historical operation remains unavailable; C12 production review remains open.

## C10 recovery decision — 2026-10-12

The retained-corpus diagnosis exposed two product-level design errors plus pre-release fixture age, not a requirement for a maintained migration subsystem. The recovery sequence is now the [Retained Freshness fixture normalization plan](freshness-retained-fixture-normalization-plan-2026-10-12.md): use existing mixed-turn Archive fragment vectors rather than exact-turn-only embeddings; treat accepted Archive activity positions as authoritative for existing REL chronology; normalize the three valuable pre-release fixtures once with disposable exact-hash tooling; rerun C10; then delete that conversion implementation. Production `migrate_file` remains unchanged.

The diagnosis below remains historical evidence for why that correction was necessary. Its exact-turn coverage conclusions describe the implementation at the time of diagnosis and are superseded as a design requirement by N1–N4.

## C10 retained-corpus failure diagnosis — 2026-10-12

**C10 is not complete.** The requested local diagnosis reproduces rejection on verified writable copies of all three frozen authoritative RELs. The originals retain their pinned hashes. No model, embedding, Insomnia or Dream inference ran; no production gate, policy or persistent format changed. Regenerating a different corpus would not resolve these retained-REL compatibility failures and is not the requested disposition.

Machine-readable evidence: [retained-corpus diagnosis](../benchmarks/freshness-historical-corpus-diagnosis-2026-10-12.json). It retains source and binary/harness hashes, exact failing source identities/timestamps, all eleven timestamp conflicts, queue/branch coverage, legacy publication counts and native rejection preservation assertions. The native operation alone attempts acceptance; independent read-only prerequisite probes expose failures hidden behind its first error.

### First rejection and independent failures

| Frozen REL | First production rejection | Independent diagnosis |
| --- | --- | --- |
| ChatGPT first 14 days | `ArchiveClockMismatch`: source `ad466a08-b249-43c4-8595-a7ac10bc6d51` is canonical position 116 but accepted Archive position 254. | 5,711 of 5,857 positions differ between timestamp-priority canonical order and existing accepted order. No timestamp regression or accepted-clock ancestry violation exists. All 374 Import Episodes are complete. Their receipts cover 5,578 positions; all 279 uncovered positions belong only to noncanonical branches. `admission_proof` independently rejects `import has unprocessed Archive tail/branch`. |
| ChatGPT first 28 days | `TimestampRegression`: ancestor `b90e35b9-2276-4fcb-b3a0-f1877fbc1a39` is timestamped 1780639210724739000 ns; descendant `4c46af2d-a013-4628-afef-fceeea94c52f` is 1780639210281125000 ns, 443,614,000 ns earlier. | Eleven nearest-activity-ancestor regressions: one 0.443614 s, one 0.862295 s and nine 62.507896 s. Accepted Archive order respects every activity ancestry edge. All 1,211 Import Episodes are complete; their receipts cover 16,027 of 16,465 positions. All 438 uncovered positions belong only to noncanonical branches. `admission_proof` independently rejects `import has unprocessed Archive tail/branch`. Canonical clock comparison is not reached because timestamp validation fails. |
| Ellis pre-relationship | `import Insomnia generation is not complete`. | 128 complete and 15 pending Import Episodes; no Processing, Failed or Terminal work. Existing chronology exactly matches all 2,972 accepted positions with no timestamp or ancestry-order conflict. Completion receipts cover 2,712 positions. The 260 uncovered positions are on canonical branches. None of the pending Episodes has current Memory outputs. `admission_proof` independently rejects `import corpus has incomplete or non-import Episodes`. |

The chronology failures come from `HistoricalTimeline::plan` / `validate_archive_clock`, composed by `Cva::historical_freshness_timeline`. H1 v1 requires timestamp-priority topological order to equal original accepted order and rejects any ancestor timestamp regression. The files can have valid accepted ancestry while failing those stronger requirements. The whole-generation check is in `historical_reconstruction_owner`; the complete-all-Archive-positions requirement is in `input::admission_proof`. Completing every queued canonical Episode does not satisfy that latter check when unselected branches remain in Archive.

### Legacy publication compatibility is an additional blocker

| Evidence | 14-day | 28-day | Ellis |
| --- | ---: | ---: | ---: |
| Current Memories | 1,087 | 2,816 | 678 |
| Decoded native completion payloads | 374 `CVAINSC3` | 1,211 `CVAINSC3` | 128 `CVAINSC5` |
| Current Memories with retained first-revision record matching first mutation and source Episode | 1,087 | 2,816 | 678 |
| Current Memories with source Episode binding | 1,087 | 2,816 | 678 |
| Completion `freshness_birth_memory_ids` | 0 | 0 | 0 |
| Current native Freshness publication-birth proofs | 0 | 0 | 0 |
| Current Freshness records | 0 | 0 | 0 |
| Settled native first-cycle markers / retained completed pass records | 0 / 0 | 0 / 0 | 0 / 0 |

These files retain publication evidence; it is incorrect to describe them as lacking all provenance or all generation receipts. The decoder reads their legacy completion records successfully. However, `admission_proof` constructs grouped births only from `freshness_birth_memory_ids`, which these pre-v6 completions do not carry. With no grouped assignment, its direct-singleton path requires a newer native Freshness birth proof and would reject `unverified direct singleton publication`. The subsequent manifest gate also demands a settled native first-cycle marker for every Memory, and the provisional-record proof requires a Freshness record. All three requirements fail for these pre-Freshness files. Removing only the first chronology or queue rejection would therefore not make them reconstructible.

### Dream and semantic coverage gaps

The pure historical Dream planner supports `CompletedGraphApproximation`. The native adapter in `historical_events.rs` marks every Memory without a retained completed pass as `Unsupported` and supplies an empty link list to that planner. Its completed-direct-Dream fallback is therefore not wired for these retained files. Current decoded Memory relations are Dream-origin labeled (11,240 / 34,152 / 8,419), but those labels alone are not proof of original direct first-cycle publication: the legacy graph decoder can default origin to Dream, and duplicate rewrites must remain excluded. A provenance reader/accepted approximation policy is required; treating all final edges as first-cycle evidence would not be a valid fix.

The 14-day and 28-day files have no Archive vector generation. Ellis has 498 retained Archive rows and 678 current target bindings in its latest generation, but zero usable exact-turn source evidence; its source vectors are mixed-turn evidence. All three therefore have zero usable exact-turn historical semantic source coverage. This is reported coverage, not their initial hard rejection. Existing Memory vectors cannot be substituted for genuine source vectors. Rejection timings and whole-audit process CPU/peak working set are recorded only as diagnostic costs, not accepted reconstruction performance; checkpoint/commit/reopen performance, reconstruction distributions, temporary allocation/spill peaks and semantic match quality remain unmeasured.

### Diagnosis and required decisions

The tested native implementation is restricted to a newly generated, complete cohort carrying recent Freshness publication/settlement evidence. It does not currently reconstruct these retained pre-Freshness RELs. This is a production scope/compatibility gap exposed by C10, not a successful acceptance result and not evidence that the frozen files are generally corrupt.

Before implementation changes, decide how the native owner should support retained accepted clocks versus timestamp-priority chronology, timestamp/ancestry disagreement, canonical-complete snapshots with unprocessed alternate branches, and partial canonical generation such as Ellis. Add an owner-verified adapter for retained legacy first-publication/cohort evidence and initial Freshness enrollment, and connect only a justified Dream approximation path. Preserve explicit unavailable semantic coverage where source vectors are absent. Do not erase pending work, rewrite Archive clocks, fabricate new-format receipts or replace the Cva/FreshnessStore/CVAFRS03 authority merely to pass this fixture gate. Native Graph/provenance/replacement checks beyond the demonstrated barriers are not claimed green.

Reproduction:

```text
cargo test --locked --lib historical_retained_corpus_native_acceptance --no-run -j 2
python scripts/freshness_historical_corpus.py --binary <library-test-executable> --output target/historical-diagnosis/report.json
```

The runner invokes the explicitly ignored, environment-bound fixture once per frozen REL. Each diagnostic fixture passed its source/copy hash and rejected-operation byte/projection preservation assertions; the runner's native acceptance gate returned nonzero because all inputs rejected. End-of-run hashes of all three writable copies also equal their source hashes. The fixture's passing audit assertions must not be counted as C10 acceptance. Full release execution remains C11.

Final diagnostic verification: `cargo fmt --check`, `cargo check --locked --tests -j 2`, Python compilation, both documentation-policy modes and `git diff --check` passed. Ownership guards passed: 75 fixtures, 161 classified public APIs and 11 external ownership fixtures. No production implementation or source-owner boundary changed; C11 retains full root/CLI execution and architecture refresh. Initial diagnostic-only compilation assumptions about ID ordering and legacy missing Freshness records were corrected before the final recorded run; failed attempts are not acceptance evidence.

Documentation impact:
- Inspected: H1 policy, active C1–C12 plan, architecture/invariants, storage/completion compatibility, native manifest/event/clock owners, retained-corpus calibration evidence and current limitation/verification routing.
- Updated: this diagnosis, machine-readable evidence, limitations and diagnostic development/coverage/maintainer/behavioral routing.
- Not affected: production APIs, scoring/activation policy, persistent formats, originals and existing native acceptance rules.
- Compliance check: final structural/change-impact documentation, formatting/compilation, ownership guards and patch checks recorded before commit.
- Known documentation gaps: retained-REL remediation/acceptance, complete C10 measurements and C11–C12 disposition remain open.

Architecture impact:
- Standards added or changed: none.
- Ownership or boundary impact: diagnostic-only owner-local fixtures and runner; production owners unchanged.
- Pitlord enforcement impact: ownership guards retained; no policy rule changes. Full refresh remains the C11 gate.
- Other verification impact: actual frozen copies, independent read-only gates and rejection preservation.
- Known architectural gaps: retained-REL clock/cohort/Dream compatibility requires explicit owner decisions; mounted management remains unavailable.
