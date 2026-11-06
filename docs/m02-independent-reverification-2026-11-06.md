# M02 independent implementation reverification — 2026-11-06

**Verdict: NOT ACCEPTED.** The independently reported unactivated Archive-vector mapping defect is repaired in the current, uncommitted working tree. A second supported native 2.1 reclamation case fails. This report assesses behavior, not the earlier M02 acceptance label.

Comparison: immutable `Reliquary-reference` pin `f62b1fb4`; rebuild committed HEAD `2592b7c7` plus uncommitted M02 changes. Contract: `Reliquary-crate-first/docs/architecture/maintenance-history-parity-phase-2026-11-05.md`, M02 §4.

## Contract axis — FAIL

1. **Fixed specific defect (verified):** `gm_audit_reclamation_removes_unactivated_archive_vector_mapping_and_backing` now passes: valid REL with an unactivated Archive-vector mapping and exclusive packed backing reclaims two frames. `storage_reclamation` test filter passes 6/6, including an activated mapping and a second unactivated mapping sharing the packed matrix; generation IDs and rows survive independent reopen. M02 source diff correctly restricts retained Archive-vector mappings to IDs referred to by accepted generation records before marking packed backing reachable.
2. **New independently reproduced P1 gap:** Native 2.1 external derived-index records use `RELXIDX2` (`crates/reliquary-owner-session/src/records/external_index/compact_index.rs`). `storage_reclamation/scan.rs::removable_backing` has no support for this family; it errors even for a well-formed, successfully reopened owner containing a durable derived-index discard record. New test `gm_audit_reclamation_handles_native_external_index_discard` **fails** with `unsupported reclamation frame family RELXIDX2`. Source remains protected (fail-closed), but supported reclamation cannot run on such RELs. In reference `src/facade/storage_reclamation_scan.rs` the superseded external-index snapshot families have explicit reachability/discard processing.
3. **Packed matrix safety gap in a prospective index fix:** `reclaim_storage_copy_inner` currently builds `referenced_packed` from Memory-vector and activated Archive-vector mappings only. It does **not** collect packed-matrix IDs from current `RELXIDX2` snapshot chunk references (`IndexedChunk.matrix`). A naive change to merely allow the index frame without reachability analysis would risk discarding indexed vector backing. This is a source-confirmed missing required design path, not a demonstrated deletion, since the current scanner refuses the frame.
4. **Contract decision still needed:** `reject_unactivated_generations` deliberately refuses an unreceipted `RELVGEN1` physical generation. Reference reclamation discards truly unpublished generation backing; M02 contract lists unactivated generations among reclaimable categories. Either establish native 2.1 safety evidence for reclamation or record an explicitly approved scoped limitation. Do not confuse unactivated mappings (case 1) with unactivated generation frames.

## Architecture and safety axis — PARTIAL PASS

The changed M02 scan uses domain decoders, retains all accepted generation mappings (including retired profile history), and includes negative shared-backing/fault-injection tests. Unknown frame families fail closed, which prevents silent data loss. However, an owner-aware external index reachability/retention boundary is still absent and full frame-family support is not proven. No new architectural dependency is implied by a proper fix.

## Executed evidence

- `cargo test -p reliquary-owner-session gm_audit_reclamation_removes_unactivated_archive_vector_mapping_and_backing --locked --offline -j 1 --target-dir target-codex-check -- --nocapture`: **1 passed**.
- `cargo test -p reliquary-owner-session storage_reclamation --locked --offline -j 1 --target-dir target-codex-check -- --test-threads=1`: **6 passed**.
- `cargo test -p reliquary-owner-session gm_audit_reclamation_handles_native_external_index_discard --locked --offline -j 1 --target-dir target-codex-check -- --nocapture`: **1 failed**, `unsupported reclamation frame family RELXIDX2`.
- A broader owner-session suite was attempted but could not compile during concurrent M03 edits: `semantic_reconciliation/replay.rs` declared a missing `imports` module (E0583). That is **not** evidence of an M02 defect or an M02 pass.
- New independent regression is in `crates/reliquary-owner-session/src/session/gm_parity_regressions.rs`; left red intentionally as an acceptance ratchet.

## Required M02 remediation

1. Parse native `RELXIDX2` snapshots/discards, retain the current valid namespace heads and last relevant discard barrier, and reclaim superseded derived snapshots when admissible. Invalid/damaged records must follow the owner's existing fail-closed/damaged-index contract.
2. Include packed backing referenced by every retained indexed chunk in the physical reachability set; do not retain orphaned matrices only referenced by discarded snapshots. Handle sharing with Memory and Archive vector owners.
3. Add independent REL fixtures with multiple namespaces and index generations, discard barriers, current and superseded snapshots, shared packed matrices, invalid frames, no-op reclamation, correct query/read behavior after reopen, source immutability, and injected failure/collision cases.
4. Resolve the unactivated-generation compatibility/safety decision explicitly, and rerun full M02/owner/workspace tests and the GM cross-domain acceptance scenarios when the concurrent M03 compile work is stable.

**No M02 acceptance or GM acceptance is justified by these checks.** Current M02 fix is uncommitted, and the independent external-index regression remains failing.
