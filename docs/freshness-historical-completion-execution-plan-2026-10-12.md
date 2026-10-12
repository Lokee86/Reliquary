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

The remaining defect is architectural, not mathematical: `freshness::historical_import::execute_historical_import` and `HistoricalCheckpointStore` are an experimental in-memory composition path, while the real Cva/Freshness checkpoint is a separate native path. A released implementation must have **one native owner path** from verified imported owner state to the existing durable checkpoint.

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
