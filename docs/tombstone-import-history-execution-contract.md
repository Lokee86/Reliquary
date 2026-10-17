# Tombstone/import upgrade execution contract

Parent: [Documentation index](INDEX.md).

Integration base: main at `2fb0501418abccbb6605790d7a3fa77b82c7e9e8`; existing uncommitted benchmark and Ego documentation work must be preserved. Canonical plan: [implementation plan](tombstone-import-history-compatibility-upgrade-plan-2026-10-17.md). Domain convergence split: [2026-10-17 inventory](domain-tombstone-convergence-split-2026-10-17.md).

## Purpose

Record the frozen implementation seams and distinguish verified progress from accepted architectural direction.

## Overview

The original 50-step decomposition remains the architectural breakdown, but execution has been rebaselined around convergence. Steps 1–8 are complete in the working tree; authoritative continuation begins at step 9 in the [remaining execution plan](tombstone-import-history-remaining-execution-plan-2026-10-17.md). Integration and retained-fixture promotion still require direct verification.

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
- **8:** Entity merge tombstones were reconciled with the shared lifecycle contract and direct Entity tombstones added. `cargo check --tests --locked` passed and the existing merge-tombstone reopen regression passed. New direct-Entity runtime cases are compiled and remain part of the combined lifecycle matrix in step 11.

- **9–11:** Ego lifecycle reconciliation and the combined focused matrix passed: Memory 3, Relationship 2, Entity 2, Conversation 4, CompatibilityProfile 1, Ego 3 and Community 1 tests. Community membership validity and Memory mutation-target retry guards were corrected during audit. **12:** the audited lifecycle batch is committed onto `main`, preserving unrelated Ego-document and benchmark work. The broader library failures are documented separately; this is not a claim that final gates 65–68 have passed.

Already landed before convergence: Archive conversation tombstones, shared lifecycle vocabulary, compatibility policy v3/0.99, persisted-v2 profile validation, CompatibilityProfile retirement/active filtering, and the `HistoryPruneManifest` foundation.

### Remaining execution contract

The authoritative remaining sequence is [steps 9–68](tombstone-import-history-remaining-execution-plan-2026-10-17.md). The order is intentionally convergence-first:

1. lifecycle convergence and its focused matrix are complete;
2. the audited domain-lifecycle batch is committed;
3. integrate ordered Historical Freshness settlements and remove singleton checkpoint ownership;
4. integrate owner-level import epochs, write fencing, crash recovery and deterministic import identity;
5. finish incremental repeated-import Historical Freshness and the import completion coordinator;
6. integrate completed epochs into reconciliation/repack/reclamation;
7. integrate explicit transformed-copy history pruning using the already-canonical domain retention descriptors;
8. add vector/profile destructive pruning and active/shared-reference safety;
9. harden profile/vector activation against partial publication;
10. repair retained RELs without re-embedding, rerun the Ellis benchmark that exposed the multi-profile ambiguity, and only then promote fixtures;
11. remove obsolete architecture, refresh structural evidence, run final gates and clean implementation worktrees.

The immediate next executable action is **step 13: Historical Freshness settlement integration**.

### 2026-10-17 convergence checkpoint

The integration base is frozen at `2fb0501418abccbb6605790d7a3fa77b82c7e9e8`. The dirty `impl/domain-tombstones` worktree was reviewed against that base and must not be merged wholesale because it predates landed conversation-tombstone, compatibility-profile, lifecycle and prune-manifest work. Its remaining domain changes are split into Memory, Entity, Relationship and Ego units with shared-hunk ownership recorded in the [domain convergence inventory](domain-tombstone-convergence-split-2026-10-17.md).

## Related docs

- [Implementation plan](tombstone-import-history-compatibility-upgrade-plan-2026-10-17.md)
- [ADR 0040](decisions/0040-terminal-tombstones-and-import-epochs.md)
- [Maintainer map](maintainer-map.md)

## Notes

Original implementation-start revision: `127119239601718ebb0dc95d38075d30fbf07804`. Current frozen convergence base: `2fb0501418abccbb6605790d7a3fa77b82c7e9e8`. A later `cargo check --tests --locked --offline` on this convergence base succeeded. Direct hashing previously confirmed all three authoritative REL fixture hashes match recorded acceptance inputs. Existing benchmark/Ego work remains preserved.
