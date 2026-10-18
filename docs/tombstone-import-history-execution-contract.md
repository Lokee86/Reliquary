# Tombstone/import upgrade execution contract

Parent: [Documentation index](INDEX.md).

Current integration base: `main` at `959e98c556603a0578e39362441c2d3f2901187f`. Lifecycle convergence landed in `d6bbc890214c813e3308f65948b1611369e80c0a`; `959e98c` preserves live Memory mutation retry identity. Existing uncommitted benchmark and Ego documentation work must be preserved. Canonical plan: [implementation plan](tombstone-import-history-compatibility-upgrade-plan-2026-10-17.md). Domain convergence split: [2026-10-17 inventory](domain-tombstone-convergence-split-2026-10-17.md).

## Purpose

Record the frozen implementation seams and distinguish verified progress from accepted architectural direction.

## Overview

The original 50-step decomposition remains the architectural breakdown. Steps 1–12 are committed on `main`; steps 13–28 are accepted on the isolated integration branch, with step 28 bounded to exclusive empty-import completion. Step 29 must derive nonempty import readiness from authoritative owners. The [remaining execution plan](tombstone-import-history-remaining-execution-plan-2026-10-17.md) owns execution order and verification limitations. Main promotion remains step 36.

## Frozen lifecycle contract

Terminal tombstones belong to the existing domain owner and advance only its existing clock plus the existing global publication clock. The shared lifecycle contract contains typed identities, state, idempotency and payload availability only. It owns no persistence or version allocation.

Direct closure: Archive conversations own their nodes, branch and metadata revisions and attachment exposure; Memory owns its revisions/body exposure; Entity and Relationship own their revisions, with replacement/endpoints retained structurally; CompatibilityProfile owns activation and generation/vector exposure; Ego owns its existing document/anchor lifecycle. Independently identified derived objects survive source deletion. Explicit cascade lists exact selected targets and never traverses provenance implicitly.

## Frozen import contract

Import metadata belongs to import/CVA ownership. Every actual import opens one durable deterministic ImportStart before its first mutation and closes with ImportComplete only after import-attributed work and sparse Freshness settlement are durable. No-import owners have no markers. Open imports fence unrelated writes, including after restart; matching input resumes and conflicting input fails. Markers capture exact existing clocks without allocating semantic tickets.

## Frozen prune contract

Tombstones preserve historical payload. Only an explicit transformed-copy prune may remove selected tombstoned payload after each domain proves its structural retention and surviving-reference obligations. Reopen verification compares clocks, transaction times, identities, topology, activity, import and Freshness history. Source files remain untouched. Ordinary reclaim stays conservative.

## Compatibility contract

Current endpoint verification uses policy 3 and cosine 0.99. Persisted policy-2 artifacts retain their exact identity and bytes. New artifacts use policy 3. Stored artifact validation and current endpoint comparison are separate responsibilities.

## Progress

Implementation in progress. No step is marked complete until repository evidence and its acceptance checks pass.

### Completed execution through step 12

- **1–3:** froze the `2fb0501` integration base, audited the obsolete domain-tombstone worktree, and split remaining domain changes into bounded units.
- **4–5:** Memory terminal tombstones and focused reopen/current-projection tests completed; focused matrix passed 2/2.
- **6–7:** Relationship terminal tombstones and focused reopen/current-projection tests completed; focused matrix passed 2/2.
- **8:** Entity merge tombstones were reconciled with the shared lifecycle contract and direct Entity tombstones added. `cargo check --tests --locked` passed and the existing merge-tombstone reopen regression passed. The direct-Entity runtime cases were subsequently executed and passed in the step-11 combined lifecycle matrix.

- **9–11:** Ego lifecycle reconciliation and the combined focused matrix passed: Memory 3, Relationship 2, Entity 2, Conversation 4, CompatibilityProfile 1, Ego 3 and Community 1 tests. Community membership validity and Memory mutation-target retry guards were corrected during audit. **12:** the audited lifecycle batch is committed onto `main`, preserving unrelated Ego-document and benchmark work. The broader library failures are documented separately; this is not a claim that final gates 65–68 have passed.

Already landed before convergence: Archive conversation tombstones, shared lifecycle vocabulary, compatibility policy v3/0.99, persisted-v2 profile validation, CompatibilityProfile retirement/active filtering, and the `HistoryPruneManifest` foundation.

### Post-lifecycle branch reassessment

- Settlement implementation commit `10a2aaf` is not on `main`, but already contains the ordered `CVAFRS04` model, legacy `CVAFRS03` synthetic-initial migration, caller migration to `import_settlements()`, and removal of internal `ImportCheckpoint` references. Steps 13–17 are therefore integration/verification work, not new design.
- Durable epoch admission commit `7706ed0` is not on `main`, but already contains `ImportStart`/`ImportComplete` codecs/store state, deterministic owner+import-key IDs, open-epoch write admission closure, owner-scoped import writes, reopen/status support and native-import start admission. Steps 18–22 are integration/verification/hardening work.
- The old import-epochs worktree contains reference-only partial step 24/25 work and must not be copied wholesale. The isolated integration branch now has accepted step 24 source/birth/recipient attribution and accepted step 25 versioned incremental settlement publication/replay.
- The exclusive `Cva::complete_native_import` coordinator owns normal completion publication. Step 28 accepts only empty imports with real scoped settlements; nonempty attributed readiness remains step 29 and low-level caller restriction remains step 30.
- Archive transformed-copy prune/reopen/corruption machinery exists as dirty WIP in `impl/explicit-history-prune`; Memory/Entity/Relationship/vector destructive pruning does not.
- `work/vector-prune` contains no additional implementation beyond code already on `main`.
- Compatibility/profile activation hardening is partially landed: obvious incomplete Archive row counts cannot publish/activate, unversioned generation payload is inert on reopen, retired profiles are excluded, and Memory profiles are active only with complete current-body coverage. The current Archive predicate still proves row count rather than exact fragment identity, so exact-coverage and historical-fill activation boundaries remain open.

### Remaining execution contract

The authoritative remaining sequence is [steps 13–68](tombstone-import-history-remaining-execution-plan-2026-10-17.md). The order is intentionally convergence-first:

1. lifecycle convergence and its focused matrix are complete;
2. the audited domain-lifecycle batch is committed;
3. integrate ordered Historical Freshness settlements and remove singleton checkpoint ownership;
4. integrate owner-level import epochs, write fencing, crash recovery and deterministic import identity;
5. finish incremental repeated-import Historical Freshness and the import completion coordinator;
6. integrate completed epochs into reconciliation/repack/reclamation;
7. integrate explicit transformed-copy history pruning using the already-canonical domain retention descriptors;
8. add vector/profile destructive pruning and active/shared-reference safety;
9. retain the already-landed profile/vector activation hardening and close only the missing partial-Memory-vector regression coverage;
10. repair retained RELs without re-embedding, rerun the Ellis benchmark that exposed the multi-profile ambiguity, and only then promote fixtures;
11. remove obsolete architecture, refresh structural evidence, run final gates and clean implementation worktrees.

The immediate next executable action is **step 29: import-attributed readiness**. Step 28's exclusive coordinator is accepted for empty imports; it derives receipt and settlement evidence, synchronizes before reopening writes and preserves exact completion retries. Nonempty imports remain Pending. Steps 13–28 are accepted on the isolated integration branch; main promotion remains step 36.

### Known non-lifecycle red mapped forward

The lifecycle commit does not claim the broader library is green. Nine reproducible failures remain and are assigned to the remaining plan:

- five historical embedding/pending-row failures → settlement/import integration plus activation-boundary hardening;
- one legacy `CVAFRS03` invented-cohort fixture → legacy synthetic-initial settlement verification;
- one historical manifest/live-vector fixture → repeated-import historical/live isolation;
- one equal-count-but-not-exact Archive coverage failure → exact fragment identity activation hardening;
- one pre-UUID typed-REL migration/live-publication failure → activation/migration ordering.

These failures are acceptance evidence for later steps, not justification to weaken the frozen lifecycle/import/prune contracts.

### Historical 2026-10-17 convergence checkpoint

At that checkpoint the integration base was frozen at `2fb0501418abccbb6605790d7a3fa77b82c7e9e8`. The dirty `impl/domain-tombstones` worktree was reviewed against that historical base and must not be merged wholesale because it predates landed conversation-tombstone, compatibility-profile, lifecycle and prune-manifest work. Its remaining domain changes are split into Memory, Entity, Relationship and Ego units with shared-hunk ownership recorded in the [domain convergence inventory](domain-tombstone-convergence-split-2026-10-17.md).

## Related docs

- [Implementation plan](tombstone-import-history-compatibility-upgrade-plan-2026-10-17.md)
- [ADR 0040](decisions/0040-terminal-tombstones-and-import-epochs.md)
- [Maintainer map](maintainer-map.md)

## Notes

Original implementation-start revision: `127119239601718ebb0dc95d38075d30fbf07804`. Historical lifecycle convergence base: `2fb0501418abccbb6605790d7a3fa77b82c7e9e8`. Current lifecycle-converged head: `959e98c556603a0578e39362441c2d3f2901187f`. Direct hashing previously confirmed all three authoritative REL fixture hashes match recorded acceptance inputs. Existing benchmark/Ego work remains preserved.

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
