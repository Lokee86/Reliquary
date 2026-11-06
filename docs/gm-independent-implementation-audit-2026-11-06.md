# GM independent implementation and reference-parity audit — 2026-11-06

**Verdict: NOT ACCEPTED.** Reopens M01, M02 and M04 on independently reproduced defects, and M03 on a missing supported reference contract. The previous `ee22c6e5` GM acceptance record is historical evidence of a decision, **not** valid completion authority. P01/GM-dependent acceptance is blocked.

Reference baseline: `Reliquary-reference` HEAD `f62b1fb4`, compared with `Reliquary-rebuild` HEAD `ee22c6e5` (original clean branch before audit). Canonical contract: `Reliquary-crate-first/docs/architecture/maintenance-history-parity-phase-2026-11-05.md`; canonical tracker: `docs/architecture/implementation-rebuild-capabilities-2026-11-03.json`. This is an independent two-axis implementation review, **not** a duplicate of the accepted GM matrix.

## Behavior / contract axis — failed

### M01 — P0: valid fresh-history rewrite succeeds while dropping Memory-linked Relationship and Entity closure

- Contract: M01 retains selected Memory's supported semantic dependencies and associated Entity/Relationship state without inventing or dropping accepted evidence. Reference `src/rewrite/semantic_state.rs`, `semantic_closure` lines approximately 121–159, selects a Relationship if its local Memory evidence intersects retained Memories; also retains its local Entity participants and copies Relationship revisions.
- Rebuild: `crates/reliquary-owner-session/src/session/fresh_history_rewrite.rs`, `copy_derived_state` around lines 286–425, selects Entities only from Memory mentions or active Graph edges, and copies no Relationship records. Neither the Application planner nor executor expands Relationship-evidence closure.
- Executed repro: `session/gm_parity_regressions.rs::gm_audit_rewrite_must_retain_memory_linked_relationship_and_entity`. Valid PHY containing an accepted Memory, an Entity and a Relationship referencing that Memory was rewritten with Memory retained, derived state enabled and a new owner. Rewrite successfully published; resulting Perception owner lacked the supported Relationship (and its linked Entity closure). Test **FAILED** with `M01 silently dropped a Relationship supported by a retained Memory`.
- Important distinction: an initial standalone-only Entity test was discarded from the evidence because the reference intentionally excludes unsupported standalone semantic state outside the selected closure. Only the Memory-linked test is relied on here.
- Required fix: compute complete owner-qualified Relationship/Entity/Memory/Graph closure, retain all selected accepted revisions and tombstones, replay through native owners, verify opened output against explicit selected semantic IDs/history. Add REL and PHY tests, including cross-owner references, missing evidence, prune/tombstone, and selective filters.

### M04 — P0: historical whole-owner digest conflates two different accepted histories

- Contract: M04 proves retained revisions at an explicit historical cut. Reference `src/facade/historical_entity_fingerprint.rs` and `historical_relationship_fingerprint.rs` enumerate every retained revision up to the cut, not only the final current projection.
- Rebuild: `crates/reliquary-perception-store/src/history.rs::history_at` reduces Entity/Relationship revisions into one entry per ID; `crates/reliquary-owner-session/src/session/historical_proof/fingerprint.rs` hashes this current-only projection.
- Executed repro: `session/gm_parity_regressions.rs::gm_audit_historical_proof_distinguishes_retained_intermediate_entity_revisions`. Two valid PHYs with the **same** owner, cut, initial/final Entity revisions and mutation identities but **different intermediate revisions** yield identical `HistoricalOwnerProof.digest` values. Test **FAILED** with `M04 proof ignores retained intermediate Entity revision history`.
- Required fix: add ordered accepted revision history, tombstone and prune evidence into historical projection/fingerprint for Entity and Relationship; verify Memory/Graph/Ego and cross-domain cut semantics similarly. A final-state-only digest cannot be used as a whole-history proof.

### M03 — P1: divergent completed native-import epoch reconciliation is absent

- Contract/reference: `src/facade/cva_reconcile.rs` dispatches divergent import cases to `src/imports/reconciliation.rs::reconcile_import_epochs`. Explicit reference acceptance tests include `src/imports/reconcile_tests.rs::divergent_completed_zero_output_epochs_replay_with_destination_watermarks`, `divergent_completed_grouped_memory_epoch_replays_through_destination_owners`, and `multiple_right_only_epochs_replay_across_live_activity_and_empty_import`. The M03 spec requires import settlement replay where evidence is supported.
- Rebuild: `crates/reliquary-owner-session/src/session/semantic_reconciliation/replay.rs::require_compatible_fixed_state` rejects all unequal import statuses as `UnsupportedState("divergent native import settlement authority")`; `semantic_reconciliation.rs::require_supported_tail` lacks the import-epoch settlement replay contract. The M03/GM acceptance records explicitly treat this supported reference behavior as an exclusion.
- Status: **source-confirmed missing capability; independent 2.1 behavioral fixture still required.** Distinguish unsafe divergent *continuation of the same open epoch* (may be a legitimate typed conflict) from compatible independent *completed* epochs. Implement supported replay, or obtain an explicit approved scope change; don't silently call it full parity.

### M02 — P1: unreachable Archive-vector mapping and packed backing are not reclaimed

- Reference: `src/facade/storage_reclamation_scan.rs` lines approximately 283–322 deletes **unactivated Archive-vector mapping chunks** and their unreferenced packed matrices, as well as superseded external snapshot indexes, when ownership/reachability is proven.
- Rebuild: `crates/reliquary-owner-session/src/session/storage_reclamation/scan.rs::removable_backing` retains all `RELAVEC1` Archive-vector mapping frames, and `storage_reclamation.rs` treats all ArchiveVector mapping backing as reachable, regardless of activation; `reject_unactivated_generations` rejects orphan `RELVGEN1` generation records. Unactivated generation records and unactivated Archive **mapping** records are different concepts.
- Executed native 2.1 repro: `session/gm_parity_regressions.rs::gm_audit_reclamation_removes_unactivated_archive_vector_mapping_and_backing`. A valid REL with a never-activated Archive-vector mapping and its exclusively owned packed backing was independently reopened, then passed to `reclaim_storage_copy`. The operation succeeded but returned **0 removed frames**, rather than reclaiming the two unreachable payloads. The test **FAILED**, `left: 0, right: 2`. The source remained unchanged. Unlike an orphan `RELVGEN1` generation record, this mapping has no active generation receipt, so the generation error is not equivalent to reclaiming unactivated mappings.
- Still open: determine which superseded external-index families are applicable to native 2.1, test retained vectors and other owners, and fail closed for unknown pointer carriers.

## Standards / architecture axis — requires correction

- The approved storage/service/application dependency guard was previously green and no new production owner dependency is required to fix the above. This axis does not establish semantic parity.
- **P1 publication race:** M01 `fresh_history_rewrite.rs::rewrite_memory_history_copy` and M03 `semantic_reconciliation.rs::reconcile_divergent_rel_copy` use `fs::rename(staged,destination)` after an early `destination.exists()`. This is not a documented atomic **no-replace** publication guarantee if another writer creates destination between check and rename. Verify platform behavior, then use verified same-volume no-replace publication with lease and injected race/collision proof; do not silently overwrite a third-party owner.
- Historical proof fingerprints currently use `Debug` formatted Rust structs instead of an explicit versioned canonical semantic encoding. Fixing the missing revision fields should include a stable schema and deterministic comparison fixtures, not only adjusting a debug-string snapshot.
- Three red regressions are placed in `crates/reliquary-owner-session/src/session/gm_parity_regressions.rs`; these intentionally make the owner-session suite fail until the defects are corrected. This is a gate ratchet, not product code.

## Verification evidence and scope

- The original clean `ee22c6e5` HEAD passed `cargo test --workspace --locked --offline -j 1 --target-dir target-codex-check -- --test-threads=1` on this host (all crate, integration and doc tests). **This does not prove the untested contracts.**
- Two regressions independently failed, then both **failed again together** after relocation into `session/gm_parity_regressions.rs`:
  `cargo test -p reliquary-owner-session gm_audit_ --locked --offline -j 1 --target-dir target-codex-check -- --nocapture` — **0 passed, 2 failed**.
- A third native 2.1 reclamation regression was then added and **FAILED** independently:
  `cargo test -p reliquary-owner-session gm_audit_reclamation_removes_unactivated_archive_vector_mapping_and_backing --locked --offline -j 1 --target-dir target-codex-check -- --nocapture` — **0 passed, 1 failed**.
- Three defects have direct, executable reproductions. M03 is a source-confirmed reference-parity gap not yet independently reproduced in native 2.1. No converted 2.0 corpus was tested here.
- Historical acceptance records are not deleted, but their completion claims are superseded by this audit. Mark M01–M04 as reopened and GM as blocked; P01 should not be accepted while GM is blocked.

## Required next steps (priority)

1. Repair M01 Relationship-aware selected closure and independently verify all retained revisions; make its red regression green.
2. Repair M04 complete revision-history projection and canonical proof digest; make its red regression green and add more converging-trajectory fixtures.
3. Validate and implement M03 completed import-epoch reconciliation parity against frozen reference acceptance scenarios; keep genuine conflicts typed.
4. Correct M02 reachability reclamation for unactivated Archive mappings and exclusive packed backing, then test applicable superseded indexes, preserving active backing and receipts.
5. Close M01/M03 no-replace destination-race proof and rerun full architecture, formatting, locked/offline workspace checks/tests, REL+PHY end-to-end transformations, and independent GM cross-domain matrix.

**GM decision: rejected on this audit.** Re-accept only after the red cases pass and the outstanding reference contracts are individually proven or explicitly rescoped.