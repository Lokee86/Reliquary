# Tombstone/import/history/compatibility upgrade — remaining execution plan

Parent: [Tombstoning, import epochs, historical pruning, and compatibility-profile upgrade](tombstone-import-history-compatibility-upgrade-plan-2026-10-17.md).

**Date:** 2026-10-17
**Status:** authoritative continuation; lifecycle implementation and focused matrix complete through step 12
**Next step:** 13
**Integration base:** `main` at `2fb0501418abccbb6605790d7a3fa77b82c7e9e8`, plus the verified lifecycle convergence batch from steps 4–12.

## Purpose

This continuation fixes the executable order after the architectural decomposition and records verified completion separately from pending work.

## Overview

Complete the lifecycle batch before integrating settlements, epochs, pruning and retained-fixture repair. The numbered sequence below remains authoritative.

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

## Execution sequence

### Lifecycle convergence

**9 — Sol — Ego lifecycle reconciliation (complete).** Normalize existing Ego Identity/Anchor deletion into the shared terminal lifecycle contract without creating a new lifecycle owner. Done when deletion is terminal, exact retry is clock-neutral, historical payload remains readable, and existing persisted Ego tombstone behavior is preserved.

**10 — Luna — Focused Ego lifecycle tests (complete).** Verify Identity and Anchor deletion, retry, historical reads, reopen and current-projection exclusion.

**11 — Luna — Combined REL/PHY lifecycle matrix (complete).** Exercise Conversation, Memory, Entity, Relationship, CompatibilityProfile and Ego together. Include the new direct-Entity runtime/reopen cases that compiled in step 8 but were not fully executed. Prove direct-only deletion, terminal semantics, historical reads and clock-neutral retry across owners.

**12 — Sol — Converge and commit lifecycle work (complete).** Audit the accumulated diff, separate unrelated pre-existing benchmark/Ego-document edits, and commit the lifecycle batch onto current `main`. The old dirty domain-tombstone worktree stops being authoritative.

### Historical Freshness settlements

**13 — Sol — Merge-forward `impl/import-freshness-settlements`.** Reconcile the settlement branch with current lifecycle code; do not reintroduce stale owner/lifecycle behavior.

**14 — Luna — Verify ordered `CVAFRS04` settlements.** Multiple import settlements must encode, rebuild and reopen in deterministic order.

**15 — Luna — Verify legacy `CVAFRS03` compatibility.** Decode one legacy singleton checkpoint as exactly one synthetic completed initial-import settlement.

**16 — Sol — Remove singleton checkpoint ownership.** Eliminate `FreshnessStore.import_checkpoint: Option<ImportCheckpoint>` as canonical internal state. Legacy handling remains only in codec/migration.

**17 — Luna — Migrate remaining Freshness callers/tests.** No runtime/internal caller may depend on the singleton checkpoint path.

### Import epochs

**18 — Sol — Merge-forward `impl/import-epochs`.** Bring durable import epoch work onto the settlement-enabled lifecycle base.

**19 — Luna — Verify `ImportStart`.** Codec/store/rebuild/idempotency must round-trip; a native-only REL must require no marker.

**20 — Luna — Verify `ImportComplete` storage semantics.** Completion records must round-trip, dedupe exact retry and reject conflicting evidence independently of coordinator wiring.

**21 — Sol — Bind deterministic `import_id` to native import identity.** Same owner + canonical import input yields the same ID; conflicting input cannot reuse it.

**22 — Sol — Finish open-epoch mutation fencing.** Unrelated live writes cannot enter an open import epoch, including after reopen. Existing safe reads remain available.

**23 — Luna — Import crash/retry matrix.** Cover cuts before start, after start, after Archive ingestion, after Episode creation, after queue admission and after receipt publication. Matching retry resumes one epoch without duplicate receipt/Episode/markers.

### Repeated-import Historical Freshness

**24 — Sol — Finish epoch-scoped cohort/evidence derivation.** Remove the whole-owner `self.memory_ids()` assumption. Inputs are attributable to one import and may include only actually affected pre-existing records.

**25 — Sol — Finish incremental reconstruction from `ImportStart`.** Canonical Freshness at the start marker is the base; prior accepted-use/live history remains authoritative; unaffected records are not rewritten.

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

**37 — Sol — Merge-forward `impl/explicit-history-prune`.** Reconcile Archive pruning with final tombstones/import epochs without overwriting newer lifecycle code.

**38 — Luna — Re-run Archive conversation prune tests.** Conversation payload becomes unavailable only after explicit prune while Archive/global versions, activity, topology and tombstone history remain exact.

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

**51 — Sol — Audit and harden profile/vector activation.** A crash or failed partial build cannot make an incomplete profile operationally active.

**52 — Luna — Partial-publication crash tests.** Cover profile created before vectors, partial Memory vectors and partial Archive generation. Incomplete evidence may remain inert, never active.

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
Ego lifecycle
-> combined lifecycle verification + lifecycle commit
-> Freshness settlements
-> import epochs
-> import completion coordinator
-> repeated-import Historical Freshness
-> explicit history prune
-> vector/profile prune
-> profile activation hardening
-> retained REL repair
-> benchmark rerun
-> fixture promotion
-> final docs/audit/tests/cleanup
```

The next executable action is **step 13**: merge-forward the ordered Historical Freshness settlement commit onto the converged lifecycle base.

## Related docs

- [Architectural decomposition](tombstone-import-history-compatibility-upgrade-plan-2026-10-17.md)
- [Execution contract](tombstone-import-history-execution-contract.md)
- [Lifecycle convergence inventory](domain-tombstone-convergence-split-2026-10-17.md)

## Notes

Do not advance a verification step on compilation alone. Preserve unrelated dirty work and authoritative fixtures throughout convergence.
