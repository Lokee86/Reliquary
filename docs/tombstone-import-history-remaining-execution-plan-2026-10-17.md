# Tombstone/import/history/compatibility upgrade — remaining execution plan

Parent: [Tombstoning, import epochs, historical pruning, and compatibility-profile upgrade](tombstone-import-history-compatibility-upgrade-plan-2026-10-17.md).

**Date:** 2026-10-17
**Status:** complete through step 68; canonical source integrated into main, verified fixtures promoted, final gates and recoverable cleanup recorded
**Next step:** none in this execution plan. See [final verification](tombstone-import-history-final-verification-2026-10-18.md) for results and explicit supported boundaries.
**Current integration base:** `main` at `959e98c556603a0578e39362441c2d3f2901187f`. Lifecycle convergence landed in `d6bbc890214c813e3308f65948b1611369e80c0a`; `959e98c` is the follow-up Memory live-mutation retry identity fix.

## Purpose

This continuation fixes the executable order after the architectural decomposition and records verified completion separately from pending work.

## Overview

Execution is complete within the final verification record's supported REL boundaries. The numbered sequence and earlier checkpoints below retain the implementation history; the final acceptance supersedes their historical next-step statements.

## Current state

Completed execution steps:

1. Freeze the integration base.
2. Audit the old `impl/domain-tombstones` worktree and prohibit wholesale merge.
3. Split remaining domain lifecycle work into bounded units.
4. Integrate Memory tombstone semantics.
5. Verify Memory lifecycle/reopen behavior — focused matrix 2/2 passed.
6. Integrate Relationship tombstone semantics.
7. Verify Relationship lifecycle/reopen behavior — focused matrix 2/2 passed.
8. Reconcile Entity merge tombstones with the shared lifecycle contract and add direct Entity tombstones. `cargo check --tests --locked` passed and the existing merge-tombstone reopen regression passed.

9. Reconcile Ego Identity/Anchor terminal lifecycle while preserving persisted formats and retained historical revisions.
10. Verify focused Ego lifecycle behavior: 3/3 passed.
11. Verify combined lifecycle coverage: Memory 3/3, Relationship 2/2, Entity 2/2, Conversation 4/4, CompatibilityProfile 1/1, Ego 3/3 and Community projection regression 1/1 passed. The audit fixed stale Community membership and cross-target Memory mutation retry. The broader library run has nine documented pre-lifecycle import/profile/migration failures after local mock-server rechecks; none is a failed focused lifecycle case.

12. Audit and commit the lifecycle batch on `main`, excluding unrelated Ego-document and benchmark changes. Library and CLI checks pass; the focused lifecycle matrix passes. Broader verification findings remain explicit in the [convergence inventory](domain-tombstone-convergence-split-2026-10-17.md).

Already landed before this convergence run and not remaining work:

- shared `owner_lifecycle` vocabulary;
- Archive `ConversationTombstone`, closure/rebuild/current projection and lifecycle CLI;
- compatibility policy v3 / cosine `0.99`;
- persisted v2 compatibility-profile validation;
- CompatibilityProfile retirement and active-vs-historical profile/generation selection;
- current retrieval exclusion of retired/incomplete profiles;
- `HistoryPruneManifest` foundation.

Authoritative retained fixtures remain untouched.

## 2026-10-17 reassessment after lifecycle convergence

Source/worktree inspection against current `main` changes the interpretation of several remaining steps:

- `impl/import-freshness-settlements` has one unmerged implementation commit, `10a2aaf`. It already removes internal `ImportCheckpoint` ownership, writes ordered `CVAFRS04` settlements, migrates callers to `import_settlements()`, and contains an explicit legacy `CVAFRS03` → one synthetic initial settlement test. Steps 14–17 are therefore **implemented on the branch but not yet integrated/verified on current main**.
- `impl/import-epochs` contains `7706ed0` plus the settlement commit. It already implements durable `ImportStart`/`ImportComplete` storage, deterministic owner+import-key IDs, open-epoch write admission closure, owner-scoped import writes, status/reopen support and native-import start admission. Steps 19–22 are **substantially implemented on the branch but not yet integrated/verified**.
- The same import-epochs worktree has additional dirty WIP in `epoch_scope.rs`, Historical Freshness input/manifest code and import commit code. It derives import source nodes, Import-origin Episodes and a born-Memory cohort from the open epoch. This is **partial step 24/25 work**, not completed architecture: the historical manifest still uses whole-owner Memory IDs for some graph/community/vector proof surfaces, and no completion coordinator owns `append_complete()`.
- `ImportEpochStore::append_complete()` still has no normal caller. Steps 28–31 remain genuinely unimplemented.
- `impl/explicit-history-prune` contains substantial dirty Archive transformed-copy prune/reopen/corruption work, but it predates the converged lifecycle/import architecture. Steps 37–38 are **WIP ready for later merge-forward**, not completed. Memory/Entity/Relationship/vector destructive pruning remains future work.
- `work/vector-prune` has no commits beyond code already present on `main`; there is **no vector/profile destructive-prune implementation yet**.
- Compatibility/profile activation hardening is farther along than the old plan states but **not complete**. Current `main` rejects obvious incomplete Archive row counts, treats unversioned generation payload as inert on reopen, and requires complete current Memory-body coverage before a Memory profile is considered active. However, the current Archive coverage predicate proves row count rather than exact fragment identity; the dedicated incomplete-coverage fixture still exposes this gap. Step 51 remains a real hardening task, narrowed to exact identity/coverage and historical-fill activation boundaries.
- `impl/compatibility-v3-retirement` is patch-equivalent to `main` (`git cherry main ...` reports both commits already applied). It is not a remaining integration lane.

Steps 13–28 are accepted on `integrate/import-freshness-settlements-20261017` in `C:\!bin\workspace\Reliquary-settlement-integration-20261017`. Step 25 is `47dd885`, step 26 is `64126ee` and step 27 is `fdbe3bb`. At the step-28 acceptance checkpoint, the exclusive completion coordinator had eight passing completion cases and real settlement-backed storage fixtures; only empty imports could complete under that checkpoint's implementation. Steps 29–33 now supply verified owner-derived readiness, private completion authority, pending/failed recovery and sequential/native-only coverage. Main promotion remains step 36.

### Known broader failures mapped to remaining work

The step-12 broad library run left nine reproducible non-lifecycle failures. They are now assigned rather than treated as generic red:

- three historical embedding fill/recovery cases plus two semantic-source pending-row fixtures → steps **13–25** and **51–52**; they currently collide with live complete-coverage publication while the import-scoped historical path is still being integrated;
- the legacy `CVAFRS03` characterization fixture with an invented cohort and no accepted Memory/birth evidence → step **15** compatibility verification; the fixture/evidence must satisfy the synthetic-initial-settlement contract rather than bypass it;
- the historical manifest fixture that uses live vector publication and introduces live scoring into provisional import history → steps **24–25**; historical manifest construction must remain isolated from live activation/scoring;
- the incomplete-coverage fixture that can satisfy row count without exact fragment identity coverage → steps **51–52**;
- the legacy typed-REL migration fixture that requests live semantic publication before a REL UUID exists → steps **51–52**; migration must keep that evidence inert until owner identity/activation prerequisites exist.

These are explicit acceptance items for the named steps. They must not be 'fixed' by weakening terminal lifecycle, complete-coverage, import attribution, or historical/live isolation contracts.

## Execution sequence

### Lifecycle convergence

**9 — Sol — Ego lifecycle reconciliation (complete).** Normalize existing Ego Identity/Anchor deletion into the shared terminal lifecycle contract without creating a new lifecycle owner. Done when deletion is terminal, exact retry is clock-neutral, historical payload remains readable, and existing persisted Ego tombstone behavior is preserved.

**10 — Luna — Focused Ego lifecycle tests (complete).** Verify Identity and Anchor deletion, retry, historical reads, reopen and current-projection exclusion.

**11 — Luna — Combined REL/PHY lifecycle matrix (complete).** Exercise Conversation, Memory, Entity, Relationship, CompatibilityProfile and Ego together. Include the new direct-Entity runtime/reopen cases that compiled in step 8 but were not fully executed. Prove direct-only deletion, terminal semantics, historical reads and clock-neutral retry across owners.

**12 — Sol — Converge and commit lifecycle work (complete).** Audit the accumulated diff, separate unrelated pre-existing benchmark/Ego-document edits, and commit the lifecycle batch onto current `main`. The old dirty domain-tombstone worktree stops being authoritative.

### Historical Freshness settlements

**13 — Sol — Integrate existing settlement commit `10a2aaf`.** Merge-forward/cherry-pick semantically onto current `959e98c`, resolving against the converged lifecycle code. Do not reimplement the settlement model and do not reintroduce stale owner/lifecycle behavior.

**14 — Luna — Verify branch-ready ordered `CVAFRS04` settlements on current main.** The implementation and unit coverage already exist on `10a2aaf`; rerun them after integration and prove multiple settlements encode, rebuild and reopen deterministically.

**15 — Luna — Verify branch-ready legacy `CVAFRS03` compatibility on current main.** The branch already has an explicit one-synthetic-initial-settlement test; rerun it after integration.

**16 — Sol — Confirm singleton checkpoint ownership is gone after integration.** `10a2aaf` already removes `ImportCheckpoint` references internally. Verify current main contains no canonical `FreshnessStore.import_checkpoint` path; legacy handling remains codec/migration-only.

**17 — Luna — Verify the branch-ready Freshness caller migration.** `10a2aaf` already moves callers/tests to ordered settlements. Search and targeted tests must prove no runtime/internal caller depends on the singleton checkpoint path.

### Import epochs

**18 — Sol — Integrate durable epoch admission commit `7706ed0` onto the settlement-enabled base (complete).** Preserve current lifecycle code and keep the later dirty epoch-scope WIP separate until the committed admission layer is green.

**19 — Luna — Verify branch-ready `ImportStart` (complete).** Codec/store/rebuild/idempotency already exist on the epoch branch; after integration prove round-trip, reopen and zero-marker native-only REL behavior.

**20 — Luna — Verify branch-ready `ImportComplete` storage semantics (complete).** Store/codec logic already exists, but no normal caller completes epochs yet. Prove round-trip, exact-retry dedupe and conflicting-evidence rejection independently of coordinator wiring.

**21 — Sol — Verify the existing deterministic `import_id` binding (complete).** The epoch branch derives the ID from owner UUID + canonical import key. Confirm that contract survives integration and conflicting input cannot reuse an open epoch.

**22 — Sol — Verify and harden the branch-ready open-epoch mutation fence (complete).** `ImportEpochStore::append_start()` already closes owner write admission and native import uses an owner write scope. Prove unrelated live writes remain fenced after reopen and only explicitly scoped import writes bypass the fence.

**23 — Luna — Import crash/retry matrix (complete).** Cover cuts before start, after start, after Archive ingestion, after Episode creation, after queue admission and after receipt publication. Matching retry resumes one epoch without duplicate receipt/Episode/markers.

### Repeated-import Historical Freshness

**24 — Sol — Finish the partial epoch-scoped cohort/evidence WIP (complete on the isolated integration branch).** Source Nodes, complete Import Episodes and first-publication births are bound to the current persisted ImportStart. Proof targets derive from canonical pre-start Freshness plus epoch births; result IDs contain only births and import-attributed provisional/planned effects. Terminal history remains exclusion proof through Memory retention descriptors. Event selection uses finite physical boundaries and candidate identities rather than whole-prefix identity sets. The final 20-test focused attribution/base/legacy/accepted-use matrix passes; new-epoch publication remains closed.

**25 — Sol — Finish incremental reconstruction from `ImportStart` on top of the partial WIP (complete on the isolated integration branch).** The accepted implementation replaces import-attributable provisional scoring with an authenticated sparse before→after settlement while preserving canonical pre-start and legitimate interval history. Canonical Freshness at the start marker is the base; prior accepted-use/live history remains authoritative; unaffected records are not rewritten.

Add a versioned settlement/replay representation authenticating physical start/base and exact source/cohort evidence while preserving CVAFRS03/CVAFRS04 compatibility. Persist affected sparse replacements and authenticated zero-result/zero-activity no-ops. Distinguish same-cut effects by accepted identity/order, preserve generating-Episode/publication-cohort exclusions for existing recipients and reused/grounding sources, and keep conversion/reconciliation closed until ordered settlement boundaries are retained.

**26 — Luna — Later-import accepted-use fixture (complete on the isolated integration branch).** Prove pre-import accepted-use/live Freshness events survive later import settlement unchanged.

**27 — Luna — Existing-Memory affected fixture (complete on the isolated integration branch).** Prove imported historical semantic/Dream evidence can legitimately update an eligible pre-existing Memory while unrelated Memories remain untouched.

### Import completion

**28 — Sol — Exclusive import completion coordinator (complete on the isolated integration branch).** `Cva::complete_native_import(import_id, now_ns)` derives exact start, receipt and accepted CVAFRS05 settlement evidence internally. Acceptance covers synchronized empty-import completion, append-free exact retry/reopen, recovery after uncertain append, physical settlement-before-completion binding, pending evidence and preservation of a newer epoch's fence. Nonempty receipts remain Pending until step 29; caller-supplied proofs cannot authorize completion. Low-level visibility restriction remains step 30.

**29 — Sol — Import-attributed readiness (complete on the isolated integration branch).** Expose read-only `Cva::native_import_readiness(import_id)` for the current open epoch, returning `{ import_id, blocker: Option<ImportReadinessBlocker>, vectors: ImportVectorReadiness }`. Reject unknown, completed and noncurrent IDs. Derive blockers from authoritative owner stores for only this import's exact imported EpisodeIds, local output IDs, each backed by its exact retained InsomniaCompletion record, a revision-1 same-Episode Memory record and matching retained first mutation; IDs without that evidence are Memory/ConflictingEvidence, authenticated `CVAFRS05` settlement, import-born initial Dream pass, and mention keys on each active, non-superseded Dream-processed output Memory that must terminate in Perception as Resolved or Rejected; Dormant/unresolved keys remain blockers, while only inactive, archived or superseded Memories have no mention obligation. Routed external PHY references return typed `Memory/Unsupported` because the coordinator cannot prove PHY publication/ownership. Vectors are `NotRequired` only when there are no source Nodes and no Memory outputs; otherwise report `SupportedUnavailable`, which does not certify complete vector coverage. Do not wait for semantic work outside authenticated settlement, automatic Ego work (there is no import scheduling hook), or unrelated owner work. Completed outcome/marker layout is unchanged. The focused acceptance evidence is recorded below. Existing pending Dream passes remain blockers after archival/terminal deletion; new-pass eligibility follows the Dream owner, including nonarchived superseded births.

**30 — Luna — Restrict `append_complete()` to the coordinator (complete on the isolated integration branch).** No normal direct caller may complete an epoch prematurely.

**31 — Luna — Pending/failed completion tests (complete on the isolated integration branch).** Import-owned pending or terminal-failed work must leave the epoch open with inspectable status.

**32 — Luna — Sequential-import test (complete on the isolated integration branch).** Import A → normal activity → import B → normal activity. B starts from A's completed canonical state and creates a distinct ordered settlement.

**33 — Luna — Native-only REL test (complete on the isolated integration branch).** A REL with no imports has zero import markers and otherwise behaves normally.

**34 — Sol — Import-epoch reconciliation (complete; explicit unsupported owner boundaries retained).** Same ID + exact evidence dedupes; conflicting same ID rejects; one-sided complete imports replay as one complete epoch with destination watermarks.

**35 — Luna — Transform preservation tests.** Reopen, migration/repack and ordinary reclamation preserve completed import epochs and Freshness settlements exactly.

**36 — Sol — Integrate import epoch + settlement architecture into `main`.** `main` has one canonical import/history model and no active singleton-checkpoint architecture.

### Explicit destructive history pruning

**37 — Sol — Merge-forward the existing dirty Archive-prune WIP only after import architecture stabilizes.** The worktree already contains transformed-copy Archive prune/reopen/corruption machinery, but it predates lifecycle convergence and import epochs. Reconcile it semantically rather than copying files wholesale.

**38 — Luna — Re-run and repair the existing Archive prune tests on the integrated architecture.** The worktree already contains `history_prune_tests`; conversation payload becomes unavailable only after explicit prune while Archive/global versions, activity, topology, tombstone and import history remain exact.

**39 — Sol — Wire canonical domain retention descriptors into prune planning.** Use the Memory/Entity/Relationship descriptors already implemented in steps 4, 6 and 8; do not create duplicate descriptor types.

**40 — Luna — Memory body prune tests.** Tombstoned Memory body bytes may be removed while structural history survives; unauthorized missing payload remains corruption.

**41 — Sol — Entity/Relationship payload pruning.** Preserve Entity replacement history and Relationship participant/evidence/version topology while allowing tombstoned descriptive payload removal.

**42 — Luna — Entity/Relationship prune/reopen tests.** Surviving references and structural history remain valid after prune.

### Vector/profile destructive pruning

**43 — Sol — CompatibilityProfile/vector retention and eligibility.** Preserve profile IDs, generation IDs, binding/row identity and source/version topology. Numeric payload is eligible only for retired, unneeded generations.

**44 — Luna — Packed-vector pruning.** Authorized packed numeric payload returns explicit `PayloadUnavailable::Pruned`; row/binding identity survives.

**45 — Luna — Archive-vector pruning.** Retired/pruned Archive generations lose only authorized numeric payload.

**46 — Luna — Memory-vector pruning.** Retired/pruned Memory vector payload follows the same retention contract.

**47 — Sol — Active/shared-reference safety gates.** Any active/current or shared payload reference blocks destructive removal.

**48 — Luna — Retired-but-unpruned history test.** Retirement alone never destroys historical numeric vectors.

**49 — Luna — Retired+pruned history test.** Identity/topology remains readable while numeric access returns the explicit pruned result.

**50 — Sol — Integrate generalized history pruning.** Merge the completed prune architecture into `main`; keep ordinary `reclaim_storage()` conservative.

### Profile/vector activation hardening

**51 — Sol — Finish profile/vector activation hardening.** Landed code already rejects obvious incomplete Archive row counts, treats unversioned generation payload as inert on reopen, excludes retired profiles, and only exposes Memory profiles with complete current-body coverage. Close the remaining exact-fragment-identity coverage hole and ensure historical/offline fill cannot accidentally publish a live-active generation or live scoring before the owner/import boundary permits it.

**52 — Luna — Close activation regression coverage.** Add an equal-row-count/wrong-fragment-identity Archive fixture, explicit partial-Memory-vector/crash coverage, profile-before-vectors/retired-profile checks, and a legacy/pre-UUID migration case that must not request live semantic activation prematurely. Incomplete or historical-fill evidence may remain inert, never active.

### Retained REL repair

**53 — Sol — Build one retained-REL repair command.** Operate on disposable copies only; support legacy import framing migration, profile retirement, optional prune/reclaim and verification.

**54 — Luna — Repair first14d scratch copy.** Keep `38b2c3d1ded65fdbc2a76086f1d2875c03eb4470afa63de8bbd0689cbf51c266`; retire redundant `69aa8cff...`; no re-embedding.

**55 — Luna — Repair first28d scratch copy.** Keep `e012fa5173c94058e83749208bd3250628a4d533d01d96fe711c86a813c14258`; retire `69aa8cff...`, `82418298...`, and `f8a5ba9f...`; no re-embedding.

**56 — Luna — Repair Ellis scratch copy.** Keep `3aad70f4c0083a74be98cc41b51515559c7ed26669597641e562299424ddcd82`; retire `2f404c70...`; no re-embedding.

### Benchmark closure

**57 — Luna — Re-run the frozen Ellis retrieval baseline/comparison.** Run against one unambiguous canonical active embedding profile; this is the benchmark path that originally exposed the lifecycle problem.

**58 — Sol — Compare repaired retrieval results.** Check against the frozen gold/baseline and compatibility calibration; profile cleanup must not introduce an unacceptable retrieval regression.

### Optional physical cleanup and fixture promotion

**59 — Sol — Optional destructive profile/vector prune + ordinary reclaim on scratch copies.** Authoritative fixtures remain untouched.

**60 — Luna — Record size/invariant evidence.** Measure recovered bytes and compare semantic/version/history state exactly.

**61 — Sol — Choose promoted fixture form.** Decide retired-only vs retired+pruned based on evidence.

**62 — Sol — Promote verified fixtures.** Replace authoritative artifacts only after verification and update hashes/provenance.

### Final documentation, audit and cleanup

**63 — Luna — Update final documentation.** Architecture, storage format, invariants, native import, Freshness, migration/reconciliation, limitations and ADR/index docs describe only the canonical architecture.

**64 — Sol — Remove obsolete architecture.** Delete singleton-checkpoint code, duplicate lifecycle helpers, abandoned branch adapters and migration-only scaffolding after consumers move.

**65 — Luna — Run targeted final gates.** Formatting, `cargo check --tests --locked`, focused lifecycle/import/prune/vector tests, CLI tests and docs policy.

**66 — Luna — Run full final suite.** Library/integration/CLI/retained-fixture tests are green, or pre-existing unrelated failures are explicitly separated.

**67 — Sol — Refresh structural evidence.** Use Lexicon/Arcana plus direct source verification to prove no old checkpoint owner path, duplicate lifecycle owner or unexpected reverse dependency remains.

**68 — Sol — Final verification and cleanup.** Write the verification record, commit remaining work and remove obsolete implementation worktrees/branches after all evidence is integrated.

## Critical path

```text
integrate existing Freshness settlements
-> integrate existing import epoch admission
-> finish epoch-scoped attribution/repeated-import Freshness
-> import completion coordinator
-> import reconciliation/transform preservation
-> integrate Archive prune WIP
-> Memory/Entity/Relationship prune
-> vector/profile prune
-> retained REL repair
-> benchmark rerun
-> fixture promotion
-> final docs/audit/tests/cleanup
```

Steps 29–33 are accepted for the bounded REL-only readiness/completion contract below. Main promotion remains step 36; destination-aware reconciliation and transform acceptance remain steps 34–35.

### Steps 29–33 acceptance (2026-10-18)

`Cva::native_import_readiness(import_id)` derives exact owner evidence without writing bytes, advancing clocks or opening the epoch fence. Local output IDs require their retained same-epoch InsomniaCompletion, revision-1 same-Episode Memory and first-mutation chain. An authenticated scoped settlement covers semantic/Freshness work and preserves the distinction between born Memories and affected existing recipients. Required initial Dream work and active nonsuperseded Dream-processed mention keys must finish; Perception Resolved/Rejected are terminal, while unresolved/Dormant keys block. A pass already pending before archival or terminal deletion still blocks; new passes obey Dream's eligibility rules. Nonarchived superseded births still require their pass. Unrelated pending work and unscheduled Ego work do not block.

PHY references remain `Memory/Unsupported`. Vector status is `NotRequired` only with no source Nodes and no Memory outputs; otherwise `SupportedUnavailable` reports unavailable certification, not vector completeness. Empty imports still require an authenticated true no-op settlement. No persisted format changes were introduced.

Production completion append requires an opaque coordinator authority whose constructor is private to `completion.rs`; the direct append seam exists only under `cfg(test)` for storage fixtures. Pending and terminal-failed imported work retains an inspectable blocker, unchanged bytes/clocks and the closed write fence across reopen. Real sequential nonempty imports A and B, separated by live activity, create distinct ordered settlements and survive reopen/exact completion retry. Native-only REL rebuild retains zero epoch markers.

Verification:
- Final import module run: **48 passed, 2 failed** (50 cases). All readiness, completion, recovery, sequencing and native-only acceptance cases pass. The two failures are the retained divergent-reconciliation fixtures `divergent_source_revision_conflicts_preserve_both_inputs` and `import_receipt_survives_reclaim_and_divergent_reconciliation`: both currently attempt another import with an open epoch. They remain step-34 work; refusal guards are preserved.
- Repeated native-import characterization after real processing/settlement/completion: **1 passed**. Four ordinary repeated-import fixtures now use that same real lifecycle rather than raw completion markers.
- Earlier broad run: **1,418 library passed, 27 failed, 5 ignored; all 141 integration tests passed**. It preceded the four fixture repairs, characterization repair and final archived-pending regression. Nine local mock-server/filesystem failures passed on recheck with normal OS access. Three external-window failures persist outside the sandbox; baseline comparison could not execute because dependency artifacts were missing. They remain unresolved, without a pre-existing-failure claim. Remaining historical embedding/source/migration/vector-coverage failures stay mapped to later work; this is not a green full-suite claim.
- Library `cargo check --tests --locked` and CLI check passed before the final small fixture/regression edits. Final library test source compiles in the focused run; the final CLI run also compiles the production library and passes **22/22 CLI tests**. The step-28 external-workflow comparison failed during dependency compilation (`E0463` missing crates), before test execution; cache-only fingerprint repair does not establish a baseline test result.
- Library/CLI formatting and whitespace checks pass. Both final documentation-policy modes report the same five existing Ego findings. Architecture evidence is recorded below.

Documentation impact:
- Inspected: repository/shared guidance, architecture, invariants, storage format, maintainer map, limitations, roadmap, version-history plan/ADR 0002, native-import/API/behavioral references and execution plans.
- Updated: API/readiness/completion contract, ownership, recovery, storage-format commentary, coverage routing and active execution pointers.
- Not affected: persisted marker/receipt formats, CLI syntax, normalization schemas, authoritative retained fixtures and prior acceptance records.
- Compliance check: both policy modes retain the five existing Ego findings (one broken index link and four missing decision-inventory sections); final rerun is recorded below.
- Known documentation gaps: those existing Ego findings remain outside this batch.

Architecture impact:
- Standards added or changed: none.
- Ownership or boundary impact: Import/CVA derives readiness from exact Archive/Insomnia/Memory/Dream/Perception/Freshness authority. Container retains generic mechanics; no new owner, proof flag or Ego scheduling hook is introduced. Production append authority is confined to the coordinator.
- Pitlord enforcement impact: source guards pass 92 multiplexing fixtures/166 classified APIs and 11 external-ownership fixtures plus production boundaries. Refresh reached Lexicon Rust analysis and was stopped after stalling; no Arcana/Pitlord structural pass is claimed.
- Other verification impact: exact local output provenance, affected existing recipients, unrelated work, archived/superseded Dream rules, terminal Perception, unsupported PHY, pending/failed reopen, sequential imports and native-only REL are covered.
- Known architectural gaps: destination-aware reconciliation (34), transform acceptance (35), scoped vector certification and mounted PHY publication remain open. No large-owner performance claim is added.


## Related docs

- [Architectural decomposition](tombstone-import-history-compatibility-upgrade-plan-2026-10-17.md)
- [Execution contract](tombstone-import-history-execution-contract.md)
- [Lifecycle convergence inventory](domain-tombstone-convergence-split-2026-10-17.md)

## Notes

Do not advance a verification step on compilation alone. Preserve unrelated dirty work and authoritative fixtures throughout convergence.

### Historical step 24 attribution groundwork checkpoint

This records the earlier partial checkpoint at `fb0da91`; the acceptance below supersedes its attribution-specific pending items. Publication/replay and completion items remain open under step 25 onward.

Work continues on `integrate/import-freshness-settlements-20261017`, following the accepted epoch admission head `d259433`. Main and authoritative retained fixtures remain unchanged. Three Luna lanes implemented and reviewed epoch scope derivation, physical-prefix provisional-event selection and pre-start Freshness base replay.

The private scope binds the exact persisted ImportStart payload/offset, base Archive activity cut, receipt source Nodes, complete Import Episode paths and first-publication born-Memory evidence. Existing Memories remain a separate recipient/proof population. Provisional-event selection uses finite physical start/freeze prefixes and exact born-source or semantic-source identity, including same-cut receipts; mixed import/external fragment evidence rejects. The base replay reads only frames physically before ImportStart, including accepted birth evidence and prior settlements. Legacy manifests retain their original map shape and import identity when no epoch exists.

This is **partial step 24 and a step 25 prerequisite**, not completed repeated-import reconstruction. The production new-epoch path explicitly refuses publication rather than returning an earlier import's settlement or feeding owner-wide replacements into the existing settlement codec. CVAFRS03/CVAFRS04 persisted bytes are unchanged. No normal completion caller or worker-admission bypass was added.

The broader run additionally exposed missing legacy accepted-use refusal in initial manifest construction. The source now refuses that unsupported path before publication, retaining the existing accepted-use protection fixtures rather than relaxing their contract. Accepted settlement retries still resolve before this construction guard and preserve subsequent live use.

Remaining acceptance work before advancing:

- Introduce a versioned settlement/replay representation that authenticates the physical start/base and import source/cohort evidence while retaining existing codec compatibility.
- Preserve pre-start accepted effects already included in a replacement base without replaying same-cut effects twice; retain post-settlement same-cut live effects by accepted identity/order.
- Hash the full existing recipient proof population, then persist only import-born/affected sparse replacements; remove whole-owner result attribution.
- Permit authenticated no-op settlements for no births, no eligible recipients and zero activity without inventing admission anchors.
- Wire the pre-start base into native event planning and prove the later-import accepted-use and existing-Memory affected fixtures (steps 26–27).
- Replace full-prefix event-ID materialization with bounded physical-interval selection before enabling the new epoch path on large retained histories; the current helper is preparatory and no bounded-residency acceptance is claimed for it.
- Keep conversion/reconciliation refusal until ordered settlement boundaries are preserved by their replay representation.

Checkpoint verification:

- All **nine new attribution/base/identity regressions** pass on the final library test binary, including prior settlement frames after their frozen input prefix. Both existing accepted-use protection fixtures now pass after the legacy construction guard was restored.
- Library/test compilation, library formatting, CLI formatting, CLI check and all **22 CLI tests** passed. The normal build output remained locked, so checks used repository-local `target-step24`; scratch output remained repository-local.
- A pre-final-guard broad library run completed with **1,386 passed, 31 failed and 5 ignored**. Subsequent outside-sandbox reruns resolved eleven environment-only failures (mock HTTP, hard links and Lore project operations). The two accepted-use failures were repaired and their focused tests passed. The remaining observed failures are seven historical-fill/semantic/profile/migration cases (steps 51–52), eight incomplete import completion/continuation/reconciliation cases (steps 28–35), and three external-source `Storage` cases that still reproduce outside the sandbox. No final full-suite acceptance is claimed.
- Both documentation-policy checks report the same five pre-existing Ego findings: one broken index link and four missing inventory sections; no new changed-document findings were reported.
- Architecture refresh passed the 92-fixture/166-API multiplexing guard and eleven-fixture external ownership guard, then stalled in Lexicon Rust analysis and was stopped after approximately ten minutes. Arcana/Pitlord refresh acceptance remains open.

## Step 24 attribution acceptance (2026-10-17)

The final 20-test focused matrix passes, including the three new native attribution/empty-import/tombstone fixtures, finite same-cut event selection, exact retry/reopen scope identity, physical pre-start/prior-settlement replay, legacy compatibility and accepted-use protections. Library/test compilation, both formatting checks and all 22 CLI tests pass. New-epoch publication and provider work remain closed before legacy preparation; no persisted codec or completion authority changed.

The broad library run passed 1,404 tests with 17 failures and 5 ignored before two step 24 fixture corrections. Both corrected fixtures pass in the final focused matrix. The other 15 failures map to the existing seven activation/historical-fill/migration and eight completion/continuation/reconciliation cases. No final full-suite acceptance is claimed. Documentation policy retains five existing Ego findings; architecture source guards pass, while refreshed Lexicon analysis stalled and was stopped after about ten minutes. Full details and CLI verification status belong to the [acceptance record](import-step24-attribution-acceptance-2026-10-17.md).

## Settlement integration checkpoint (2026-10-17)

Settlement commit `10a2aaf` was cherry-picked onto `959e98c` as `44f52b0` in `integrate/import-freshness-settlements-20261017`. Main remains at `959e98c`; promotion waits for the completed import architecture at step 36.

The integration audit repaired same-activity-cut live accepted-use replay, replaced the invented-cohort legacy fixture with real accepted Memory/birth evidence and passive Cva reopen coverage, corrected the CVAFRS04 policy-corruption offset, and added persisted multi-settlement replay coverage. The 35-test focused settlement/legacy/live-use/transform/lifecycle matrix passes; formatting and `cargo check --tests --locked` pass. CLI formatting/checks and all 22 CLI tests passed. The architecture refresh passed both source guards (92 multiplexing fixtures, 166 classified APIs and 11 external-ownership fixtures), then timed out after ten minutes in Lexicon Rust analysis; Arcana/Pitlord did not run. The scoped settlement repairs and eight current-behavior docs are committed as `957bf45` after `44f52b0`. Steps 13–17 have passed their focused acceptance on the lifecycle-converged isolated branch; final structural acceptance remains open. Both documentation-policy checks report five existing findings outside the changed canonical docs (one broken Ego-plan link and four missing sections in the Ego decision inventory).

Steps 24–25 must explicitly handle zero-result and zero-activity imports, preserve the canonical Freshness base at ImportStart, and distinguish same-cut pre-start evidence from later live receipts by accepted identity/order rather than activity cut alone. The current codec/store requires nonempty replacements/cohort and an advancing cut; no completed repeated-import claim is made.

Luna prepared isolated epoch codec/storage/reopen tests in `7e2f337` on `7706ed0`; they are not integrated or Cargo-verified yet. The next integration batch remains step 18 after settlement verification and commit.

## Epoch admission integration checkpoint (2026-10-17)

Durable epoch admission was integrated on top of the accepted settlement branch as `ef86f04`, with marker/storage coverage in `9480f5f`, write-admission hardening in `5e97169`, and phase-cut crash/retry coverage in `9973ff7`.

Final hardening moved immutable source-revision conflict validation ahead of `ImportStart`, added the missing `WriteAdmissionClosed` display path, and added a regression proving invalid immutable source revisions do not open or poison an epoch and do not change bytes, clocks or epoch status. This preserves the rule that rejected admission is physically and semantically inert.

Final acceptance passed:

- multiplexing coverage guard: 92 fixtures and 166 classified public APIs;
- external-source ownership guard: 11 fixtures plus production-boundary check;
- `cargo fmt --check` and CLI formatting;
- `cargo check --tests --locked`;
- 5 epoch codec/store/identity tests;
- native-only no-marker reopen;
- exact retry with no marker/semantic-clock change;
- completed-epoch storage/rebuild/idempotent retry;
- durable start + unrelated-write fencing;
- open-epoch conflict/policy rejection;
- invalid source revision admission neutrality;
- phase-cut interrupted-import recovery without duplicate work;
- 7 import settlement storage/legacy/replay tests;
- 18 lifecycle regression tests;
- CLI check plus 22/22 CLI tests.

Steps 18–23 are accepted on the isolated integration branch. Production completion remains intentionally unavailable: no normal caller owns `append_complete()` and no derived-work/Freshness readiness coordinator exists yet. That is later step 28–31 work, not part of this checkpoint.

### Steps 24–25 merge-forward audit

Luna's read-only audit of the original dirty epoch WIP found these acceptance blockers; preserve the WIP as reference rather than copying it wholesale:

- Select retries by the current ImportStart identity, not the latest owner settlement. Bind actual start digest, base cut/prefix, cohort and exact source nodes through construction and validation.
- Rebuild the canonical Freshness base from frames physically present before ImportStart. A settlement's frozen input byte length is not its accepted physical frame position.
- Hash proof for the entire pre-existing recipient set (body/revision/lifecycle/provenance/base state), then persist only the affected sparse replacements. Restrict import-produced Memory writes to the exact epoch Episode set.
- Bound event attribution by the frozen physical interval and exact source identity, including same-activity-cut receipts; never substitute an unbounded end offset.
- Retain conversion/replay refusal until the replay representation can preserve settlement boundaries. Removing the guard alone silently drops authority.
- Define authenticated no-op settlements for no births/no eligible recipients/no activity without inventing admitted anchors. No-op and existing-Memory effects require distinct fixtures.


## Step 25 incremental settlement acceptance

The isolated integration branch implements CVAFRS05 scoped settlements. The canonical pre-start Freshness projection supplies existing records; epoch births and recipients affected by provisional or planned scoring define the sparse replacement set. Legitimate receipts accepted during the frozen interval are folded into affected replacements, while exact pre-start identities are not counted again. Replay distinguishes receipts included in a replacement from later receipts at the same activity cut by a finite physical-prefix identity check.

The new version binds physical start and freeze coordinates, the exact start digest, receipt source Nodes, born cohort, eligible recipient proof population, sparse result records and superseded import event identities. CVAFRS03 decoding and CVAFRS04 read/write behavior remain supported. Empty and zero-activity epochs produce authenticated empty settlements without synthetic admissions or activity. A narrow exclusive Cva append scope admits only verified settlement/policy publication; no completion marker or background-worker authority is added.

Per-record base proof identities remain separate from eligibility policy. Generating Episode paths and canonical publication cohorts determine semantic and Dream exclusions, including cross-member grounding/content evidence. Commit-time input drift still checks the raw frozen bytes. Reopen instead verifies a coordinate-bound immutable-prefix digest that excludes mutable Archive compaction allocations; replacing an old conversation summary must not invalidate accepted scoring. Conversion/reconciliation refusal remains in place until ordered physical boundaries can be preserved.

Step 25 is accepted on the isolated integration branch. Current-source focused verification passes 17/17 settlement/epoch/replay cases plus 3/3 cohort/exclusion regressions. The full current-source library run completed with 1,405 passed, 16 failed and 5 ignored; none of the 16 failures is a step-25 focused regression. They remain mapped to scoped embedding-fill/profile activation, import completion/continuation, legacy migration, exact Archive coverage, and one Windows Lore file-lock case. `cargo fmt --all -- --check` passes. The CLI source/public surface is unchanged from the accepted step-24 baseline (22/22); a fresh CLI rebuild was attempted, but rustc 1.97.1 ICE'd while compiling Lore's multi-crate-type library; this is recorded as a toolchain/dependency-build failure rather than a Reliquary test failure, and no fresh CLI pass is claimed.

Documentation impact:
- Inspected: architecture, invariants, storage format, maintainer map, current limitations, roadmap, API, version-history plan, ADR 0002, active execution contract and parent plan, plus shared documentation/architecture standards.
- Updated: current architecture/invariants/format/maintainer/API/limitations and this execution record; final continuation pointers follow acceptance.
- Not affected: CLI syntax, graph corpus import representation, pruning/retained fixtures and unrelated Ego documentation.
- Compliance check: both documentation-policy modes retain the same five existing Ego-document findings; the final step-25 documentation/source checks are recorded with acceptance below.
- Known documentation gaps: the existing broken Ego-plan index link and four missing Ego decision-inventory sections remain outside this change.

Architecture impact:
- Standards added or changed: none.
- Ownership or boundary impact: Cva authenticates owner/epoch composition and exclusively publishes; FreshnessStore retains settlement encoding and receipt replay. Container retains generic physical mechanics only.
- Pitlord enforcement impact: final source guards pass (92 multiplexing fixtures, 166 classified APIs, 11 external-owner fixtures and production boundaries). The refresh was stopped during stalled Lexicon Rust analysis; Arcana/Pitlord structural acceptance remains open.
- Other verification impact: regression coverage includes authenticated no-op/truncation/corruption, pre-start and interval accepted use, later equal-cut use, sparse unaffected records, compaction mutation, terminal-source scope authentication, exact retry and reopen. The focused current-source matrix passes.
- Known architectural gaps: completion/readiness, scoped worker admission, scoped embedding fill, sequential completed-epoch acceptance and physical-boundary conversion remain later work. Equal-cut identity lookup is bounded-memory prefix scanning, without a new production performance claim. An expanded source-terminal fixture exposed an existing Insomnia reopen dependency on current-only Episodes (`Insomnia(MissingEpisode)`); scope authentication now uses retained paths, while end-to-end completed-work/source-tombstone lifecycle acceptance remains open before promotion.

Final step-25 repository gates: `cargo fmt --all -- --check` and `git diff --check` pass; the multiplexing coverage guard passes 92 fixtures and classifies 166 public APIs; the external-owner guard passes 11 fixtures plus production boundaries. Both documentation-policy modes report the same five pre-existing Ego-document findings (one broken index link and four missing inventory sections) and no step-25-specific finding. A fresh CLI rebuild was attempted but failed in rustc 1.97.1 with an internal compiler error while compiling Lore's multi-crate-type library; no Reliquary CLI test executed, and the unchanged accepted step-24 CLI baseline remains 22/22 without being misreported as a fresh step-25 pass.


## Step 26 later-import accepted-use fixture (2026-10-18)

`src/facade/historical_import_accepted_use_tests.rs` isolates a live Memory from a later nonempty native import. Three hundred accepted live turns lower its score before a real accepted-use receipt, keeping the award below saturation. The fixture reopens before import, checks the canonical ImportStart base, finishes import generation through existing test-only scopes, and publishes through the native freeze/prepare/commit path. The imported birth is present, while the original Memory is absent from the sparse replacement map.

Assertions compare the full pre-import and settlement-cut records against an independently decayed expected record, and against a no-use counterfactual that must differ. The original live byte prefix remains intact. Reopen, an accepted reconstruction retry (`committed: false`) and an exact accepted-use retry preserve the same score/anchors; the final byte comparison proves these retries append no bytes. Normal open-epoch admission remains fenced; this fixture does not claim completed-import sequencing (step 32) or affected-existing-Memory scoring (step 27).

Luna independently reviewed fixture assertions and continuation-document ownership. The fresh repository-local build passed all 7 selected tests: the dedicated accepted-use fixture, three adjacent attribution cases and three epoch-scope cases. The first attempt failed in the old cache with a missing `prost` dependency artifact; a clean target completed successfully. `cargo fmt --check` and `git diff --check` pass. Both documentation-policy modes retain only the five existing Ego-document findings. No fresh full-suite or CLI pass is claimed for this test-only step; their prior limitations remain recorded under step 25. Step 26 is accepted; step 27 is next.

Documentation impact:
- Inspected: the active execution plan, execution contract, parent architectural plan, repository guidance and existing step-25 acceptance.
- Updated: this acceptance record and current continuation pointers after verification.
- Not affected: public API/CLI, storage format, production ownership, retained fixtures and prior acceptance records.
- Compliance check: formatting/whitespace pass; both documentation-policy modes report the same five existing Ego findings.
- Known documentation gaps: the existing Ego index link and four missing inventory sections remain outside this change.

Architecture impact: no production implementation, ownership, format, standard or Pitlord policy changes; this step adds regression evidence only. Completion/readiness, sequential completed imports, scoped worker admission and the existing structural/CLI build gaps remain under their existing acceptance records.

## Step 27 existing-Memory affected fixture (2026-10-18)

`src/facade/historical_import_existing_memory_tests.rs` creates two admitted, eligible pre-start Memories and ages both with 600 live activity turns. A later native import publishes one Memory through the actual Import Episode/Insomnia path. Its completed initial Dream pass records a direct Dream link to only the first pre-start Memory. The frozen native planner must derive exactly one ActualFirstCycle Dream event with the expected owner/import/source identity, retained proof digest, strictly later source position and 50-point root award.

The affected Memory enters the sparse settlement without becoming an import birth. The unrelated pre-start Memory remains authenticated in the base but absent from sparse initial/replacement records. Full-record expectations apply one reinforcement and ordinary decay independently of the native historical scorer; a no-Dream counterfactual must differ and the awarded score remains below saturation. The fixture verifies provisional pass supersession, original byte-prefix preservation, append-free commit retry, both Memories' settlement-cut projections and reopened reconstruction retry.

Luna implemented the fixture and independently reviewed its eligibility, evidence and counterfactual assertions, acceptance wording and current continuation pointers. All 8 focused attribution/epoch tests pass, including the new Dream fixture and step 26 accepted-use fixture. The library test target compiled successfully. The first `cargo check --locked --jobs 1` failed before reaching Reliquary with missing encryption dependency artifacts (`E0463` in `aes-gcm`); a targeted repository-local encryption-cache rebuild repaired it, and the repeated check passed. Step 27 is accepted; step 28 is next. No fresh full-suite or CLI pass is claimed for this test-only step; their prior limitations remain under step 25.

Documentation impact:
- Inspected: repository guidance, shared documentation standards, current architecture/invariants/storage/maintainer/limitations/roadmap, behavioral contracts and the active execution plans.
- Updated: behavioral contract and verification routing, this acceptance record and continuation pointers after verification.
- Not affected: production ownership, public API/CLI, persisted formats, retained fixtures and earlier acceptance records; this change adds a focused regression fixture.
- Compliance check: `cargo fmt --check` and `git diff --check` pass; both documentation-policy modes report the same five existing Ego findings. All 8 selected tests and the repository check pass after the cache repair.
- Known documentation gaps: the existing broken Ego index link and four missing Ego decision-inventory sections remain outside this change.

Architecture impact: no production implementation, ownership, format, standard or Pitlord policy changes. Normal attributed worker admission and import completion remain pending under steps 28 onward; this fixture uses the existing test-only owner scope.

## Step 28 exclusive completion coordinator (2026-10-18)

The implementation in `src/imports/completion.rs` exposes the offline/exclusive `Cva::complete_native_import(import_id, now_ns)` operation. It derives the start, canonical Archive receipt and accepted scoped CVAFRS05 settlement rather than accepting caller-supplied proof. The existing ImportComplete layout binds canonical start/receipt digests, the exact settlement payload digest and owner post cuts. No semantic clock or persisted format is added.

At the step-28 acceptance checkpoint, only a receipt with zero conversations and an authenticated empty-effect, non-advancing settlement proved readiness. Missing receipt/settlement evidence returned typed Pending; every nonempty receipt returned `AttributedReadinessUnavailable` and stayed fenced pending step 29. Completion did not run reconstruction or bypass workers. This historical checkpoint is not the current implementation contract: steps 29–33 now provide verified owner-attributed readiness and bounded completion coverage.

Physical scans authenticate settlement-before-completion order and retain only small per-import completion evidence. Reopen validates this binding after existing historical settlement validation. Publication uses a staged epoch store, synchronizes before acknowledgement and releases ordinary admission only afterward. A retry rescans physical markers so a frame accepted before an uncertain append/sync outcome is not appended again. Completed retries return the original marker, preserving newer activity and any newer open epoch's fence.

Existing storage fixtures now derive real zero-output Insomnia completion and accepted scoped settlement evidence rather than inventing settlement digests.

Step 28 is accepted for the bounded exclusive empty-import coordinator. Fresh verification: all eight completion cases pass; the full import selection is 29 passed and six failed, with those six blocked by the existing nonempty completion/sequencing/reconciliation gaps. The full Cargo-built library test executable completes with 1,416 passed, 15 failed and 5 ignored, including both exhaustive historical byte-prefix recovery cases passing. Its 15 failures remain in historical embedding/profile coverage, nonempty import continuation/sequencing and pre-UUID migration; none is a completion-coordinator regression. The library and CLI compile successfully under `cargo test --manifest-path cli/Cargo.toml --locked --jobs 1`, and all 22 CLI tests pass. The standalone `cargo check --locked --jobs 1` fails before Reliquary because tempfile cannot find freshly built windows-sys/once_cell metadata; this cache failure is not reported as a successful check. The first import test build hit schannel E0463; a targeted dependency-cache cleanup and serial rebuild produced the verified executable. The only subsequent test-source cleanup removes an unused glob import.

Formatting and whitespace checks pass. Both documentation-policy modes report the same five pre-existing Ego findings. Architecture source guards pass 92 multiplexing fixtures, 166 classified APIs and 11 external-owner fixtures plus production-boundary checks. The refresh attempt is stopped after reaching Lexicon Rust analysis; no Arcana/Pitlord structural pass is claimed. These broader limitations remain visible and do not authorize nonempty completion. Steps 29–33 are now accepted as recorded above; main promotion remains step 36.

Documentation impact:
- Inspected: repository/shared guidance, architecture, invariants, storage format, maintainer map, limitations, roadmap, version-history plan/ADR 0002, native-import/API/behavioral references and active execution plans.
- Updated: canonical completion ownership, API/recovery/format/limits/behavioral references, existing Pitlord ownership description and this execution record; continuation pointers follow acceptance.
- Not affected: provider normalization schemas, CLI syntax, semantic clock allocation, CVAFRS03/CVAFRS04 compatibility and authoritative retained fixtures.
- Compliance check: formatting/whitespace pass; both documentation-policy modes retain five pre-existing Ego findings. Focused completion 8/8 and CLI 22/22 pass; full library 1,416/15/5. Standalone check cache failure and structural refresh limits are recorded above.
- Known documentation gaps: the existing broken Ego index link and four missing Ego decision-inventory sections remain outside this change.

Architecture impact:
- Standards added or changed: none.
- Ownership or boundary impact: Import/CVA alone coordinates completion; Archive retains receipt authority, Freshness retains accepted settlement/replay authority and Container retains generic mechanics. There is no caller-supplied readiness capability.
- Pitlord enforcement impact: existing native-import ownership area covers the coordinator; its description is updated. Visibility restriction of low-level append_complete remains step 30; storage-only tests still exercise that seam directly. Source guards pass; Arcana/Pitlord structural refresh remains unverified.
- Other verification impact: focused cases cover empty completion, exact retry/reopen, pending evidence, invalid identity/time, uncertain append, mismatched digest, physical ordering and preservation of a newer open fence.
- Known architectural gaps: nonempty attributed readiness (step 29), scoped workers, full pending/terminal-failure owner matrix (step 31), completed nonempty sequencing and transform/reconciliation acceptance remain later work. Completion attempts scan physical owner history; no new large-owner performance acceptance is claimed.


## Step 34 replay investigation (2026-10-18)

Read-only investigation confirms that divergent reconciliation must preserve epoch ordering before its current guards can be removed. `cva_reconcile_freshness::validate_freshness` and `FreshnessStore::replay_entries` refuse accepted settlements; divergent repack currently replays semantic owner tails without import markers. The physical-copy validation in `historical_import_transform.rs` covers unchanged coordinates, not destination rebasing.

A destination replay must classify exact duplicates, one-sided completed epochs and conflicting same-ID evidence before output creation. A one-sided epoch requires destination pre/post watermarks, a coherent import-owned replay interval, destination start/freeze prefixes and prefix digest, and a settlement before its completion marker. Reusing source offsets or simply removing refusal guards would lose authority. Cross-branch Freshness effects and subsequent epochs must retain destination event order and canonical base state. This investigation does not accept step 34 or authorize main promotion.


## Step 34 bounded source-only replay acceptance (2026-10-18)

The first reconciliation slice preserves the left CVA as an unchanged physical prefix and replays complete source-only zero-output epochs from finite right-side intervals. It inventories physical starts/completions and scoped settlements before creating output, rejects open epochs and source revision conflicts, deduplicates same-ID evidence only when the exact Archive receipt, accepted Archive interval records, ordered normalized Insomnia completions and original start/completion timestamps agree, and reports `CvaReconcileConflict::ImportEpoch` for conflicting same-ID evidence. Source Cva opens independently validate start/scope, immutable prefixes and settlement-before-completion evidence.

Destination `ImportStart.pre` comes from the actual left prefix plus already replayed right activity. Native Historical Freshness reconstruction freezes each destination interval and derives its canonical base, source/cohort proof, start/freeze prefixes and immutable-prefix digest. The exclusive completion coordinator then binds and syncs the destination settlement before opening writes. No source physical coordinates or settlement replacement bytes are copied into a rebased epoch. Left epoch evidence and existing Insomnia work/attempts remain unchanged. Queue admission is limited to newly replayed Episode IDs. Repeated physical marker occurrences refuse replay rather than creating overlapping intervals.

This slice refuses right-tail Memory publications, accepted Freshness effects, file publication, other unhandled owner payloads and retained `CVAINSW1`/`CVAINSA1` journals. Outstanding right-source Insomnia work is also refused. The generic Freshness refusal guards remain for unsupported derived-owner histories. Eligible-recipient proof populations and physical/manifest digests are destination evidence, not source-only duplicate identity; both source settlements are validated before normalization. Insomnia completion ordering and accepted Archive record order are retained in duplicate comparison.

Focused verification: **56/56 import-module tests pass**, including both formerly failing divergent fixtures forwarded through real processing/settlement/completion. Six new cases cover rebased watermarks/scope validation and unchanged left work/attempts; same-ID dedupe plus live Archive tails; same-ID conflict before output; open-epoch refusal; multiple right epochs separated by live activity including an empty import; and refusal of a real terminal/retry/complete journal without mutating either input. All **141 integration tests** and **22 CLI tests** pass. Library `cargo check --tests --locked` and CLI check pass. The first broad Cargo run could not launch the missing library test executable; its integration targets did execute. The library-only retry completed with **1,441 passed, 11 failed, 5 ignored**; both exhaustive byte-prefix recovery cases and the repeated post-rebase reconciliation assertion pass. All 11 failure names were present in the preceding broad run: four historical embedding cases, two semantic source cases, pre-UUID migration, incomplete Archive vector identity coverage and three external-window workflows. No green full-library claim is made. The final payload guard also refuses body/routing-only completions. After that conservative restriction, the focused rerun passes **56/56**, the CLI passes **22/22**, and library/CLI compile checks pass. The broad library run preceded only that final restriction; its six reconciliation cases include the byte-identical post-rebase repeat merge.

Documentation impact:
- Inspected: repository/shared guidance; architecture, invariants, storage, ownership, limitations, roadmap, version-history/ADR 0002, native import/API and active execution plans.
- Updated: canonical source-only reconciliation behavior, refusal boundaries, implementation/test routing and this record.
- Not affected: persisted formats, ordinary non-import reconciliation, CLI syntax, authoritative retained fixtures and unrelated main-checkout work.
- Compliance check: both documentation-policy modes retain five existing Ego findings. Library/CLI formatting and whitespace pass; both final documentation-policy modes report the same five existing findings.
- Known documentation gaps: existing Ego index link and four missing decision-inventory sections remain outside this batch.

Architecture impact:
- Standards added or changed: none.
- Ownership or boundary impact: Import/CVA inventories epochs and coordinates finite replay; Archive/Insomnia retain source/completion authority; native Freshness reconstruction and the exclusive coordinator own destination settlement/completion. Container mechanics/formats are unchanged. No generic replay framework or proof bypass was introduced.
- Pitlord enforcement impact: source guards pass 92 fixtures/166 classified APIs and 11 external-ownership fixtures plus production boundaries. A bounded refresh reached Lexicon Rust analysis and timed out after 75 seconds; no Arcana/Pitlord structural pass is claimed.
- Other verification impact: 56 import tests, 141 integration tests, 22 CLI tests and both compile checks pass; library-only retry is 1,441 passed/11 failed/5 ignored with no new failure names against the preceding broad run.
- Known architectural gaps: step 34 is **not complete**. Grouped nonzero Memory publication, initial Dream/Perception and accepted Freshness events still need interval-aware owner adapters preserving pinned accepted effects against destination canonical state. File/other-owner replay and retained queue journals remain unsupported. Step 35 transform acceptance and step 36 main promotion remain open; no performance claim is added.


### Step 34 next implementation seam

The remaining nonzero publication adapter belongs beside native Insomnia `processor/application.rs::commit_application`, not in Container or in caller-supplied readiness flags. Retain the entire accepted source `InsomniaCompletion` at its physical interval position. Validate its IDs, exact first-publication provenance and destination mutation conflicts; preserve Memory semantic fields, mutation IDs, source/grounding/temporal evidence, body bytes and routing metadata. Rebase only global tickets and contiguous Memory-local version coordinates, then append the full grouped completion and advance the embedded global range with `commit_embedded_version_range_at`. Apply the grouped Memory bodies/records/routing state and Insomnia completion through their owners, then derive proven birth evidence at the destination Archive cut.

The generic `cva_reconcile_memory` helper currently decomposes completion records into separate ordinary Memory writes and emits a completion with empty records/bodies/routing metadata. That remains insufficient for native import readiness. `stage_grouped_insomnia` is a conflict-check reference, not an exact replay substitute: it regenerates publication fields, including clearing `source_ref`. Memory-local ticket allocation should remain inside MemoryStore or use checked arithmetic from its current watermark. Subsequent Dream/Perception and accepted Freshness adapters must preserve pinned accepted effects in finite destination intervals before removing the derived-owner refusal guards.


Local verification encountered a cache-maintenance race: the background cleanup log confirms deletion of this worktree's verification files during the run, causing missing executable, library and metadata errors before test execution. Cache-only fingerprint repair was limited to missing artifacts. For the final 56-case rerun, only the verified cache maintenance service was temporarily stopped and automatically restarted in a `finally` block with its original configuration. No service configuration, source fixture or main code was changed by that repair.


## Final execution acceptance (2026-10-18)

Steps 29–35 and 37–66 now have implementation and verification evidence in the [final verification record](tombstone-import-history-final-verification-2026-10-18.md). This supersedes the earlier bounded source-only checkpoint and its next implementation seam. Ordered concrete-owner replay includes nonzero grouped Memory publication, initial Dream/Perception evidence, accepted effects, attachments and retry journals. Exact transform authority is retained. REL pruning, exact activation coverage and explicit legacy framing are implemented. All three retained RELs are repaired and promoted in retired-only form with original bytes intact; the 27-case Ellis comparison is identical before/after repair. Optional destructive cleanup was declined to preserve authenticated original prefixes. Full library acceptance separates three existing external-host failures; integration and CLI suites pass. Final structural acceptance passes all 23 Pitlord rules with refreshed Lexicon/Arcana evidence. Source main integration (steps 36/50), fixture promotion and recoverable cleanup (67–68) are complete and recorded in the linked record.
