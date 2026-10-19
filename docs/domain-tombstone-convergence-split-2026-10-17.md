# Domain tombstone convergence split — 2026-10-17

Parent: [Tombstone/import upgrade execution contract](tombstone-import-history-execution-contract.md).

## Purpose

Freeze the integration base for the remaining domain-tombstone work, audit the dirty `impl/domain-tombstones` worktree against that base, and split the work into bounded Memory, Entity, Relationship, and Ego integration units.

This document is an integration inventory, not a new architecture specification.

## Frozen integration base

Canonical integration base:

```text
main
2fb0501418abccbb6605790d7a3fa77b82c7e9e8
Add conversation tombstone CLI and lifecycle inspection
```

All remaining worktrees must converge onto this revision or a descendant of it. They must not redefine shared lifecycle, conversation-tombstone, compatibility-profile, or prune-manifest contracts independently.

The root working tree already contains unrelated dirty Ego documentation and retrieval benchmark artifacts. Those are not part of this convergence work and must be preserved.

## Source worktree

```text
C:\!bin\workspace\Reliquary-domain-tombstones
branch: impl/domain-tombstones
HEAD: 127119239601718ebb0dc95d38075d30fbf07804
```

The worktree contains 42 tracked modified files plus three untracked files.

A raw comparison against current `main` reports 109 changed files because this worktree is based before the lifecycle/conversation/profile work that has since landed. **Do not merge or cherry-pick the worktree wholesale.** Only the dirty domain-lifecycle additions listed below are candidates for transplantation.

## Already superseded by main

These worktree artifacts must not be integrated:

- `src/owner_lifecycle.rs` — untracked duplicate. The canonical shared lifecycle vocabulary is already on `main`.
- `docs/tombstone-import-history-compatibility-upgrade-plan-2026-10-17.md` — untracked duplicate. The canonical plan is already on `main`.
- Any apparent removal/reversion of Archive `ConversationTombstone`, lifecycle CLI, compatibility-profile retirement, policy v3/0.99, active-profile filtering, or `HistoryPruneManifest` in a `git diff main` comparison. Those are main-forward changes absent only because this worktree has an older base.
- Any runtime-vector/profile behavior that would replace `get_active(...)`, `active_memory_compatibility_profiles()`, current-generation completeness checks, or retired-profile exclusion with the older pre-retirement behavior.

The existing `main` Entity guard against resurrection through the legacy retired-Entity path is canonical behavior. The Entity unit below extends that behavior into the shared lifecycle API; it must not remove the terminal guarantee during convergence.

## Split 1 — Memory

### Core owner files

- `src/memory/memory_codec.rs`
- `src/memory/memory_error.rs`
- `src/memory/memory_model.rs`
- `src/memory/memory_rebuild.rs`
- `src/memory/memory_store.rs`
- `src/memory/processing_epoch_store.rs`

### Consumer/current-projection files

- `src/retrieval/lexical_index/memory.rs`
- `src/runtime_inference/dream_pair_history.rs`
- `src/runtime_inference/insomnia/store.rs`

### Shared-file hunks owned by the Memory unit

- `src/facade/cva.rs`
  - `tombstone_memory`
  - Memory lifecycle inspection
  - Memory retention descriptor
  - historical Memory revision access
  - graph current-projection exclusion after Memory tombstone
- `src/facade/phylactery.rs`
  - same Memory lifecycle/read API surface
- `src/facade/cva_global_validation.rs`
  - Memory tombstone global-version accounting only
- `src/facade/phylactery_lifecycle.rs`
  - Memory tombstone global-version accounting and reopen validation only
- `src/lib.rs`
  - Memory tombstone/retention exports and test wiring only

### Shared graph seam required by Memory

Memory deletion must remove a Memory node/relations from the **current** semantic projection while retaining the historical graph record. The corresponding Memory portions of:

- `src/semantic_graph/graph_store.rs`
- `src/semantic_graph/graph_store_read.rs`
- `src/semantic_graph/graph_store_validation.rs`
- `src/semantic_graph/community_store.rs`

belong to the Memory unit, but these files must be integrated by hunk because Entity uses the same current-vs-historical exclusion mechanism.

### Acceptance carried forward

- terminal Memory tombstone;
- exact retry allocates no new global/domain version;
- current Memory lookup/lexical/graph projection excludes the tombstoned Memory;
- historical Memory revision remains readable;
- Dream/Insomnia/processing-epoch historical references remain valid;
- REL Memory tombstone does not mutate Archive;
- no provenance cascade.

## Split 2 — Entity

### Core owner files

- `src/perception/entity_codec.rs`
- `src/perception/entity_error.rs`
- `src/perception/entity_model.rs`
- `src/perception/entity_owner.rs`
- `src/perception/entity_rebuild.rs`
- `src/perception/entity_store.rs`
- `src/perception/entity_store_read.rs`

### Historical-reference consumers

- `src/perception/entity_resolution_store.rs`
- `src/perception/entity_resolution_store_write.rs`

### Shared-file hunks owned by the Entity unit

- `src/facade/cva_global_validation.rs`
  - Entity tombstone global-version accounting only
- `src/facade/phylactery_lifecycle.rs`
  - Entity tombstone global-version/reopen accounting only
- `src/lib.rs`
  - Entity lifecycle/retention exports only

### Shared graph seam required by Entity

Entity deletion uses the same non-destructive current-projection exclusion mechanism as Memory. Entity-specific hunks in:

- `src/semantic_graph/graph_store.rs`
- `src/semantic_graph/graph_store_read.rs`
- `src/semantic_graph/graph_store_validation.rs`

belong to this unit.

Historical graph validation must accept a tombstoned Entity identity when validating persisted history, while current graph APIs must exclude it.

### Existing-main rule

Current `main` already rejects resurrection of legacy retired/tombstoned Entity IDs. Preserve that rule while adding:

- explicit direct Entity tombstone;
- shared `LifecycleState` exposure;
- historical Entity revision access;
- retention descriptors;
- exact retry semantics.

Do not regress the existing replacement/retired-Entity behavior.

## Split 3 — Relationship

### Core owner files

- `src/perception/relationship_codec.rs`
- `src/perception/relationship_error.rs`
- `src/perception/relationship_model.rs`
- `src/perception/relationship_owner.rs`
- `src/perception/relationship_rebuild.rs`
- `src/perception/relationship_store.rs`
- `src/perception/relationship_store_read.rs`

### Shared-file hunks owned by the Relationship unit

- `src/facade/cva_global_validation.rs`
  - Relationship tombstone global-version accounting only
- `src/facade/phylactery_lifecycle.rs`
  - Relationship tombstone global-version/reopen accounting only
- `src/lib.rs`
  - Relationship lifecycle/retention exports only

### Dependency rule

Relationship historical validation may refer to tombstoned Memories and Entities. Therefore this unit depends on the Memory and Entity historical-presence APIs, but it must not own or duplicate them.

Relationship tombstoning itself does not require graph mutation or provenance cascade; the independently identified Relationship simply disappears from current Relationship projection while its historical participants/evidence remain reconstructable.

## Split 4 — Ego

### Core owner files

- `src/ego/ego_codec.rs`
- `src/ego/ego_error.rs`
- `src/ego/ego_store.rs`
- `src/ego/ego_store_anchors.rs`
- `src/ego/ego_store_identities.rs`
- `src/ego/ego_store_read.rs`

### Facade files

- `src/facade/cva_ego.rs`
- `src/facade/phylactery_ego.rs`

### Shared-file hunk

- `src/lib.rs`
  - Ego lifecycle/historical-read exports only

### Scope of this unit

This is primarily normalization of existing Ego delete/tombstone behavior:

- make Identity/Anchor tombstones terminal;
- make repeat delete clock-neutral;
- expose shared lifecycle inspection;
- expose historical tombstoned payload;
- preserve legacy tombstone payload reconstruction.

It does not introduce a new Ego persistence owner or universal lifecycle ledger.

## Shared convergence files

The following files contain cross-domain hunks and must never be copied wholesale from the old worktree:

- `src/facade/cva.rs`
- `src/facade/phylactery.rs`
- `src/facade/cva_global_validation.rs`
- `src/facade/phylactery_lifecycle.rs`
- `src/lib.rs`
- `src/semantic_graph/community_store.rs`
- `src/semantic_graph/graph_store.rs`
- `src/semantic_graph/graph_store_read.rs`
- `src/semantic_graph/graph_store_validation.rs`

Integrate these against current `main` by semantic hunk after the corresponding owner unit is ready.

## Test split

The untracked `src/memory/domain_lifecycle_tests.rs` currently combines all domains. Do not import it as one monolithic test module.

Split its evidence into:

1. Memory lifecycle tests
   - terminal tombstone;
   - idempotency;
   - historical payload;
   - lexical exclusion;
   - graph current-projection exclusion;
   - REL Archive passivity.
2. Entity lifecycle tests
   - terminal tombstone;
   - idempotency;
   - surface-index exclusion;
   - graph current-projection exclusion;
   - historical retention.
3. Relationship lifecycle tests
   - terminal tombstone;
   - idempotency;
   - historical participants/evidence;
   - surviving Memory/Entity references.
4. Ego lifecycle tests
   - Identity and Anchor terminal deletion;
   - clock-neutral retry;
   - historical payload after reopen.

A later combined REL/PHY lifecycle matrix may exercise the four completed units together, but it is not the implementation vehicle.

## Integration order

Use this order:

```text
Memory
  -> Entity
  -> shared Memory/Entity graph projection seam
  -> Relationship
  -> shared global-version/reopen validation
  -> Ego
  -> combined lifecycle matrix
```

This order minimizes temporary dependency shims:

- Relationship can consume final Memory/Entity historical-presence APIs.
- Graph projection is implemented once for Memory/Entity and not reworked later.
- Ego remains independent and can be integrated last.

## Step disposition

The requested convergence steps are now complete:

1. **Integration base frozen** at `2fb0501418abccbb6605790d7a3fa77b82c7e9e8`.
2. **Domain-tombstone worktree reviewed** against current `main`; raw merge is prohibited and superseded lifecycle/conversation/profile/prune changes are identified above.
3. **Dirty domain work split** into Memory, Entity, Relationship, and Ego units with explicit shared-hunk ownership and integration order.

No implementation from the dirty domain worktree had been merged at the steps 1–3 checkpoint.

## Step 4 checkpoint — Memory tombstone semantics

Memory tombstone semantics are now integrated into the current `main` working tree, without committing the convergence batch yet.

Implemented:

- durable `CVAMEMT1` Memory tombstone records paired with the existing Memory/global version publication path;
- terminal re-admission/update rejection for tombstoned Memory IDs;
- idempotent repeated tombstones returning the original publication coordinates without allocating another version;
- current Memory projection removal while preserving historical Memory revisions and body access;
- current routing metadata, entity-mention and lexical exclusion for tombstoned Memories;
- historical Dream-pair, Insomnia-attempt and processing-epoch validation against historical Memory identity;
- historical Entity-resolution, Relationship-evidence, and Archive file→Memory validation against tombstoned Memory identity while their write paths still require current/live Memories;
- derived semantic-graph visibility exclusion without mutating graph history or graph version;
- reopen reconstruction of Memory tombstones and current graph exclusion;
- Memory retention descriptors and tombstone/global-version accounting for later explicit prune work;
- REL and PHY facade lifecycle/historical-read APIs.

Verification at this checkpoint:

- `git diff --check` passed;
- `cargo check --lib --locked --offline` passed on the integrated tree;
- `cargo check --lib --locked` passed after the historical-reference consumer adjustments;
- `cargo check --tests --locked` passed;
- an immediately preceding check failed only because dependency `.rmeta` artifacts were missing from the shared target cache; rerunning rebuilt that cache and reached a successful Reliquary compile;
- dedicated Memory lifecycle behavior tests remain step 5 and are not claimed complete here.

No Entity, Relationship, or Ego tombstone implementation was imported as part of step 4.

## Step 5 checkpoint — focused Memory lifecycle verification

Added `src/memory/memory_lifecycle_tests.rs` and wired it as a Memory-only lib test module.

The focused matrix covers:

- terminal Memory tombstone publication on the existing Memory/global clocks;
- clock-neutral exact delete retry and revision-conflict rejection;
- terminal re-admission/update rejection;
- current Memory-ID exclusion;
- lexical exclusion;
- semantic-graph current-projection exclusion without graph-version mutation;
- historical payload, lifecycle coordinates and retention descriptor access;
- PHY reopen preservation;
- REL Archive-version/activity passivity;
- surviving Relationship evidence that references a tombstoned Memory;
- surviving Archive file→Memory links;
- enforcement that **new** Relationship writes still require live Memory evidence.

The first isolated run passed the PHY case and failed the REL reopen case with `Relationships(MissingLocalMemory(...))`. The defect was that Relationship publication and historical reopen shared the same live-reference validator. The fix separates live local-reference validation from historical local-reference validation: new writes use `contains_memory`, while reopen validation uses `contains_historical_memory`. The same Memory current-vs-historical distinction was applied to Entity-resolution reopen/repair validation and Archive file→Memory reopen validation.

Verification:

- initial shared-target run was invalidated by a target-cache linker failure (`LNK1181`, missing `libhashbrown-...rlib`), not a test failure;
- isolated target run before the fix: 1 passed, 1 failed, exposing the Relationship reopen defect;
- isolated target rerun after the fix: **2 passed, 0 failed**;
- test command: `cargo test --lib memory_lifecycle_tests --locked` with an isolated `CARGO_TARGET_DIR`.

Step 5 is complete.

## Step 6 checkpoint — Relationship tombstone semantics

Relationship tombstone semantics are now integrated into the current `main` working tree.

Implemented:

- durable `CVARELT1` Relationship tombstone records published through the existing Relationship/global version path;
- terminal Relationship IDs: later publication/replay of the same ID is rejected with `TerminalTombstone`;
- exact repeated tombstone returns the original tombstone coordinates without allocating another Relationship/global version;
- tombstoning removes the Relationship from the current Relationship projection and participant indexes without touching its participant Entities or evidence Memories;
- historical Relationship revisions remain readable after deletion;
- lifecycle inspection returns the terminal tombstone coordinates;
- retention descriptors preserve revision/version/kind/participant/evidence/mutation/timestamp structure for later history pruning;
- reopen rebuild recognizes tombstone records and reconstructs the terminal lifecycle state;
- Relationship tombstone global versions participate in REL and PHY global-version conflict validation;
- the live-vs-historical Memory evidence split discovered in step 5 remains intact: new Relationship writes require current/live Memory evidence, while persisted surviving relationships may reference tombstoned historical Memories;
- no provenance cascade is performed.

Integration note: compilation exposed that an earlier capped file read/write had accidentally truncated `src/lib.rs`. The file was restored from the frozen `HEAD` base and only the intended uncommitted Memory export, Relationship export, and focused Memory test-module changes were reapplied. A source-tree search confirms no `[TRUNCATED ...]` artifact remains.

Verification:

- `cargo check --lib --locked` passed;
- `cargo check --tests --locked` passed;
- `git diff --check` passed;
- focused Relationship runtime/reopen behavior tests remain step 7 and are not claimed complete here.

Step 6 is complete.

## Step 7 checkpoint — focused Relationship lifecycle verification

Added `src/perception/relationship_lifecycle_tests.rs` and wired it as a Relationship-only lib test module.

The focused matrix covers:

- terminal Relationship tombstone publication on the existing Relationship/global clocks;
- exact retry returning the original tombstone coordinates without advancing either clock;
- revision-conflict rejection for invalid retries;
- terminal re-publication/re-admission rejection;
- current `relationship()`, `relationships()`, and participant-index exclusion;
- direct-only deletion: participant Entity and evidence Memory remain live and unchanged;
- historical Relationship revision access;
- tombstoned historical payload access through the common lifecycle API;
- retention descriptor preservation of revision/global/Relationship versions, participants and evidence;
- PHY reopen reconstruction of the tombstone and current-index exclusion;
- REL Archive-version/activity passivity;
- REL Memory and Entity version passivity.

The first compile reached only two test-harness borrow errors because reopened owner bindings were not declared mutable for `memory()` access. No lifecycle assertion ran or failed in that attempt. After correcting those two bindings, the focused command `cargo test --lib relationship_lifecycle_tests --locked` completed with **2 passed, 0 failed**.

Step 7 is complete.

## Step 8 checkpoint — Entity lifecycle reconciliation

Entity tombstoning is now reconciled with the shared owner-lifecycle contract without replacing the existing merge/retirement machinery.

The two persisted Entity tombstone forms are intentionally distinct but share one terminal lifecycle state:

- existing `CVAENTT1` — merge/retirement tombstone carrying `replacement_id`;
- new `CVAENTT2` — direct Entity tombstone with no replacement.

Implemented:

- existing merge-retired Entity IDs now populate the shared terminal tombstone coordinate map;
- direct `tombstone_entity(id, expected_revision)` support for REL and PHY owners;
- exact direct-delete retry is clock-neutral and returns the original tombstone coordinates;
- direct and merge tombstones both prevent Entity re-publication/replay under the same ID;
- current Entity lookup and all surface/normalized/alias indexes exclude terminal IDs;
- current semantic-graph projection excludes directly tombstoned Entities without mutating historical graph relations or graph version;
- historical Entity revisions remain readable;
- shared lifecycle inspection and historical payload APIs expose tombstoned state;
- retention descriptors preserve Entity revision/global/Entity versions, global ID, kind, mutation/timestamps, and optional merge replacement;
- reopen recognizes both `CVAENTT1` and `CVAENTT2`;
- Entity tombstone global versions participate in REL and PHY global-version conflict validation;
- persisted graph relations, Relationships, and Entity-resolution state may retain references to terminal historical Entity IDs during reopen;
- new Relationship and Entity-resolution writes still require current/live Entities;
- legacy merge behavior remains intact: `retired_entity_replacement()` and canonical active retargeting still use the replacement chain, while direct tombstones have no replacement and are not auto-retargeted;
- no provenance cascade is introduced.

Verification:

- `cargo check --tests --locked` passed with the Entity lifecycle test module registered;
- existing runtime regression `entity_tests::retired_entity_cannot_be_republished_before_or_after_reopen` passed (1/1) against the integrated code;
- an earlier focused Entity runtime command compiled successfully but ran zero tests because the new module had not yet been registered; that empty run is not counted as verification;
- a subsequent fully linked focused run was cancelled externally during the long lib-test build, so no runtime result is claimed for the new direct-delete tests here;
- `git diff --check` passed.

Step 8 is complete.

The authoritative continuation is now the [remaining execution plan](tombstone-import-history-remaining-execution-plan-2026-10-17.md). The original next action at this checkpoint was step 9; the verified continuation is recorded below.

## Step 9 checkpoint — Ego lifecycle reconciliation

EgoStore retains ownership and the existing Identity/Anchor deleted-record formats. Shared lifecycle inspection exposes terminal state on the existing Ego version without inventing global semantic tickets. Exact deletion retry with the original pre-delete expected revision returns `false`; conflicting revision retries reject. Updates, activation and replay cannot revive a terminal ID. New explicit historical getters retain every nondeleted payload revision across reopen; deletion revisions have no document payload.

Focused Ego fixtures cover Identity, PHY/REL Anchors, revisions, current exclusion, retries, reopen and malformed resurrection records. Combined audit also found that a Memory tombstone could leave a same-graph-version Community snapshot falsely current. Community validity now checks actual live membership, preserving historical snapshots while suppressing deleted members and forcing a new derived generation on refresh.

The audit also tightened Memory mutation retry identity: a mutation ID cannot return a tombstoned historical payload when retried against a different target ID. A focused before/after-reopen regression covers the rejection and clock neutrality.

Steps 9–11 are complete. Focused runtime results: Memory 3/3, Relationship 2/2, Entity 2/2, Conversation 4/4, CompatibilityProfile 1/1, Ego 3/3 and Community projection 1/1 passed. Conversation fixtures were corrected to keep optional Episode/source-Node fields paired while retaining their content-source provenance. The audited lifecycle batch is committed in step 12. Library and CLI checks pass; broader library failures and remaining gate results are recorded below.

## Overview

The bounded Memory, Entity, Relationship and Ego units converge onto the current lifecycle foundation. Verified checkpoints above distinguish integrated work from remaining acceptance.

## Related docs

- [Remaining execution plan](tombstone-import-history-remaining-execution-plan-2026-10-17.md)
- [Execution contract](tombstone-import-history-execution-contract.md)

## Notes

The old domain-tombstone worktree is reference material only and must not be merged wholesale.

## Broader verification findings during step 12

The complete library executable finished with 1,374 passed, 19 failed and 5 ignored in 416.58 seconds. All 19 failures were rerun individually outside the sandbox: ten initially failing cases pass when rerun with local mock-server access outside the execution sandbox. Nine non-lifecycle failures remain reproducible in unchanged import/profile/migration paths:

- Three historical embedding fill/recovery cases and two semantic-source pending-row fixtures conflict with the pre-existing rule that every published generation must already have complete coverage.
- The dedicated incomplete-coverage fixture exposes a row-count check that does not prove exact fragment identity coverage.
- The legacy `CVAFRS03` characterization fixture supplies an invented cohort with no accepted Memory/birth evidence.
- A historical manifest fixture uses live vector publication and therefore introduces live scoring into an isolated provisional import history.
- A legacy typed-REL migration fixture requests live semantic publication without a REL UUID before migration.

These failures are recorded for settlement integration and profile activation hardening rather than changing those future contracts during the lifecycle commit. This is source/history-based attribution, not a claim that an untouched-HEAD full baseline was rebuilt. `cargo check --tests --locked --offline --target-dir target/lifecycle-verification` passes. Integration/CLI results and the final refreshed structural gate are recorded when available. Authoritative retained fixtures remain untouched.

The first structural refresh found the existing root characterization test file missing from ownership policy. It is now assigned to the existing facade area; the policy check passes on the generated snapshot. The final refresh after the source freeze passes all 23 rules with 75,501 source nodes and 2,389 relationships (snapshot `8dc23c5cddc11225aefb635ccbf28fc3c2a41815fca06a7a1c7dd8b56cf54d8f`). CLI tests pass 22/22 outside the sandbox with local mock-server access. Integration and graph-smoke verification remain in progress.
