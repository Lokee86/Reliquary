# Tombstone/import/history/compatibility upgrade — remaining execution plan

Parent: [Tombstoning, import epochs, historical pruning, and compatibility-profile upgrade](tombstone-import-history-compatibility-upgrade-plan-2026-10-17.md).

**Date:** 2026-10-17
**Status:** authoritative continuation; isolated integration accepted through step 23; repeated-import attribution is next
**Next step:** 24
**Current integration base:** `main` at `959e98c556603a0578e39362441c2d3f2901187f`. Lifecycle convergence landed in `d6bbc890214c813e3308f65948b1611369e80c0a`; `959e98c` is the follow-up Memory live-mutation retry identity fix.

## Purpose

This continuation fixes the executable order after the architectural decomposition and records verified completion separately from pending work.

## Overview

Lifecycle convergence is complete. Integrate the already-implemented settlement and epoch foundations before finishing repeated-import attribution, completion coordination, pruning and retained-fixture repair. The numbered sequence below remains authoritative.

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

Steps 13–23 are now integrated and verified on the isolated lifecycle-converged branch. Settlement acceptance is recorded below, and durable epoch admission/storage/fencing/crash recovery passed its final acceptance matrix. The next executable action is step 24; main promotion remains step 36.

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

**24 — Sol — Finish the partial epoch-scoped cohort/evidence WIP.** The dirty epoch worktree already derives source nodes, Import Episodes and born-Memory cohort from the open epoch. Remove the remaining whole-owner `self.memory_ids()` use from import-result proof/reconstruction surfaces and make all superseded/affected evidence import-attributable.

**25 — Sol — Finish incremental reconstruction from `ImportStart` on top of the partial WIP.** The current WIP isolates import-attributable provisional events but is not yet a verified complete before→after reconstruction. Canonical Freshness at the start marker is the base; prior accepted-use/live history remains authoritative; unaffected records are not rewritten.

**26 — Luna — Later-import accepted-use fixture.** Prove pre-import accepted-use/live Freshness events survive later import settlement unchanged.

**27 — Luna — Existing-Memory affected fixture.** Prove imported historical semantic/Dream evidence can legitimately update an eligible pre-existing Memory while unrelated Memories remain untouched.

### Import completion

**28 — Sol — Implement the import completion coordinator.** One coordinator owns the transition from open epoch to settled epoch and is the only normal path allowed to publish `ImportComplete`.

**29 — Sol — Implement import-attributed readiness.** Readiness covers import-caused Insomnia, Memory publication, vectors, semantic work, Dream, Perception, Ego and Historical Freshness; unrelated owner background work must not block completion.

**30 — Luna — Restrict `append_complete()` to the coordinator.** No normal direct caller may complete an epoch prematurely.

**31 — Luna — Pending/failed completion tests.** Import-owned pending or terminal-failed work must leave the epoch open with inspectable status.

**32 — Luna — Sequential-import test.** Import A → normal activity → import B → normal activity. B starts from A's completed canonical state and creates a distinct ordered settlement.

**33 — Luna — Native-only REL test.** A REL with no imports has zero import markers and otherwise behaves normally.

**34 — Sol — Import-epoch reconciliation.** Same ID + exact evidence dedupes; conflicting same ID rejects; one-sided complete imports replay as one complete epoch with destination watermarks.

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

The next executable action is **step 24**: finish epoch-scoped cohort/evidence attribution and repeated-import Historical Freshness on the accepted epoch-admission base. Do not copy the old dirty epoch-scope WIP wholesale; merge only the audited attributable pieces.

## Related docs

- [Architectural decomposition](tombstone-import-history-compatibility-upgrade-plan-2026-10-17.md)
- [Execution contract](tombstone-import-history-execution-contract.md)
- [Lifecycle convergence inventory](domain-tombstone-convergence-split-2026-10-17.md)

## Notes

Do not advance a verification step on compilation alone. Preserve unrelated dirty work and authoritative fixtures throughout convergence.

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
