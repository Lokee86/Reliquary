# Dependency Convergence â€” Phase 1 implementation evidence and handoff

Date: 2026-10-22
Status: Phase 1 accepted locally on Windows. All six retained files match the frozen oracle; source, test and documentation gates passed. Unix execution and Windows directory-entry power-loss durability remain platform limits, documented below.

## Purpose

Record where the Phase 1 physical-storage foundation landed, what evidence currently exists, and the final verification provenance and platform limits. The governing requirements remain in the [Phase 1 specification](dependency-convergence-phase1-storage-foundation-spec-2026-10-21.md) and [file lease contract](dependency-convergence-phase1-lock-contract-2026-10-21.md).

## Overview

The implementation introduces a caller-pulled physical frame cursor and separates read-source, writable-session, and explicit repair authority. REL and PHY lifecycle replay loops now consume physical frames from above Container. Ordinary open reports an incomplete physical tail as typed repair-required evidence; tail repair and semantic maintenance have explicit entrypoints. The V2 storage format is retained.

Primary implementation ownership is visible in `src/container/container_scan.rs`, `container_access.rs`, `container_lease.rs`, `container_repair.rs`, `container_version.rs`, and `grouped_clock.rs`; the Container faÃ§ade remains `src/container/container.rs`. REL and PHY replay orchestration remains in `src/facade/cva_lifecycle.rs` and `phylactery_lifecycle.rs`. Explicit semantic maintenance is in `src/facade/semantic_maintenance.rs`, with rewrite/source-publication work in `src/rewrite/file.rs`. Freshness and historical source readers use held-container `PhysicalReadSource` snapshots rather than reopening a mounted path.

## Evidence groups for the 60 specification tasks

The entries below account for every task ID. Grouping is for evidence navigation; the completed gates and platform limits are recorded below.

| Task IDs | Concrete evidence / source ownership | Current disposition |
| --- | --- | --- |
| P1.0.01â€“P1.0.07 | Retained Phase 0 capture and reviewed ledger: [Phase 0](dependency-convergence-phase0-2026-10-21.md), [target DAG](dependency-convergence-target-dag-2026-10-21.md), [lease contract](dependency-convergence-phase1-lock-contract-2026-10-21.md), and `.verification-tmp/phase1-architecture.log`. Original before-image/capture is retained; it was not reset to clear findings. | Inventory and before-image evidence retained; no claim that every per-task baseline oracle has been re-run here. |
| P1.1.01â€“P1.1.08 | Specification sections 3â€“4; concrete OS identity, lock, alias, source snapshot and publication contract in the lease contract. Physical frame contract resides in `container_scan.rs`; errors in `container_error.rs`. | Contract and implementation exist; lease, alias, process, mount and physical-boundary regression groups passed. |
| P1.2.01â€“P1.2.09 | `container_scan.rs` owns shared framing and pull cursor, bounded reads and local scan clock state. `container_tests.rs`, `publication_envelope_clock_tests.rs`, `transaction_time_tests.rs`, and `.verification-tmp/phase1-focused-complete.log` contain focused evidence. | Final check, full build `--no-run`, focused regression run and comprehensive library run passed; all six retained-file oracle comparisons passed. |
| P1.3.01â€“P1.3.09 | `container_lease.rs`, `container_access.rs`, `container.rs`, `file_lease_process_tests.rs`; read-source identity and publication rules in the lease contract. `.verification-tmp/phase1-focused-complete.log`, `phase1-physical-guard.log`, and `phase1-integration-acceptance.log` are run records. | Lease, mount, alias and semantic-maintenance focused groups passed; physical-boundary guard and public integration suite passed. The comprehensive current library test run also completed successfully. |
| P1.3.10â€“P1.3.15 | `semantic_maintenance.rs` and its tests provide explicit compaction reclamation and retired-Entity repair; `container_repair.rs` holds explicit tail repair; `cva_lifecycle.rs` and `phylactery_lifecycle.rs` do caller-driven replay. | Ordinary open no longer owns these correction actions. Focused semantic-maintenance tests passed, including rejection of complete owner-invalid envelopes with and without a torn tail. |
| P1.3.16â€“P1.3.18 | `rewrite/file.rs`, `rewrite/plan.rs`, `rewrite/model.rs` and `tests/history_rewrite_contract.rs`; lease contract describes source pinning and guarded destination publication. Historical import/reconciliation adaptations are in `facade/cva_historical_import.rs` and related facade/import modules. | Rewrite, publication, import, reconciliation and mounted-runtime focused and public integration groups passed. All six retained-file oracle comparisons passed. |
| P1.4.01â€“P1.4.04 | REL/PHY lifecycle ingest order remains facade-owned. Existing caller loops route frames through owner ingest/finish logic. Historical, Freshness, Dream, Insomnia, import and store callers were migrated to caller-owned cursor loops; `entity_resolution_store.rs` is included. | Structural migration, focused replay checks and comprehensive library tests passed; all six retained-file oracle comparisons passed. |
| P1.4.05â€“P1.4.07 | Physical version and envelope responsibilities remain in `container_version.rs` / `publication_envelope.rs`; Container has no facade/runtime replay callback. See final source graph evidence in the verification ledger below. | Focused invalid-evidence and replay checks and the comprehensive library run passed. |
| P1.5.01â€“P1.5.05 | Legacy `open_scanned` and visitor callsites were removed/migrated; cursor entrypoints are in `container_scan.rs`. Enforcement lives in `scripts/check_physical_boundary.py`, `scripts/check_dependency_convergence.py`, `scripts/check_component_dag.py`, and `tools/pitlord/`. | Physical boundary guard passed with 13 fixtures; logical DAG passed with 34 components and 163 allowlisted edges; final source graph passed with 99,800 nodes, 9,233 relationships and 25 rules. |
| P1.6.01â€“P1.6.02 | Updated `architecture.md`, `invariants.md`, `storage-format.md`, `behavioral-contracts.md`, roadmap/maintainer docs, parent convergence spec and `docs/INDEX.md`; Phase 2/3 handoff below. | Documentation checks and final Pitlord docs acceptance passed; the latter mapped 1,227/1,227 documentation items and reported 428 code mappings and 22 mapped documentation changes. |
| P1.6.03â€“P1.6.06 | Verification ledgers in `.verification-tmp/phase1-*.log`; check, no-run build, focused regressions, CLI tests, formatting, physical guard, DAG, public integrations, comprehensive library tests, source graph and DDocs checks have completed. | Local acceptance gates passed; final source hashes, test provenance and platform limits are recorded below. |

## Replay order and clock parity

The physical cursor yields one complete physical frame at a time and applies physical version, envelope, reservation, and transaction-time observations in scan order. Owner lifecycle remains the consumer: it routes each frame to the same owner ingest paths and performs owner finishing after the physical stream is exhausted. This preserves the former replay sequencing without a Container-invoked semantic callback.

Cursor clock/frontier state is local to each scan. Historical and bounded scans therefore do not mutate the live Container clock state or poison a later replay. Full-open lifecycle code explicitly installs the completed scan clock state after successful traversal. `container_version.rs`, `grouped_clock.rs`, and `transaction_time_tests.rs` are the implementation/evidence locations. Focused and comprehensive tests passed; all six retained files match the frozen accepted-state, history and clock oracle.

### Source-reviewed lifecycle replay oracle

I compared current `src/facade/cva_lifecycle.rs` and `phylactery_lifecycle.rs` with their retained `git show HEAD:<path>` versions. The old versions put the same frame dispatch inside `Container::open_scanned` callbacks; the new versions put it in the owner lifecycle's `while cursor.next_frame()` loops. For each complete frame, the physical observer advances reservations/envelope clocks first, then yields `latest_global = next_version - 1`; this is the same pre-callback ordering as the old `scan_payloads` observer. `ObjectRef`, payload, clock frontier and owner-specific route ordering are preserved. On complete valid input, full-open clock state is applied after the loop. The intentional behavior differences are that errors no longer trigger a physical tail truncate and the removed compaction/Entity corrections below no longer run during ordinary open.

| Owner | Per-frame dispatch order | Post-stream finishing and validation order |
| --- | --- | --- |
| REL / Cva | 1. Feed versioned owner payloads to rewrite-lineage validation; reject a rewrite-genesis record without versioned identity. 2. `classify_open_record`: Archive, Memory and Graph routes ingest their respective open state with object/frontier; ContainerVersion is physically observed and has no owner ingest; Publication decodes one envelope and tries Memory â†’ Entity â†’ Relationship â†’ Archive â†’ Graph in that order, rejecting unsupported bodies; Fallback routes through owner-maintenance/import/external-index, all legacy-capable states, then interaction-stream/compaction/Echo/Ego, entity-resolution/history/files/metadata, Freshness policy discovery and record ingest, deferred Dream-begin validation, processing epochs and Dream pairs. | Install full physical clock state; close write admission if an import epoch is open; bind owner and validate rewrite baselines/Dream owner; finish Archive and validate file references/prune markers/history; finish Memory and validate prune history; finish Entity and Relationship and local references; validate Ego, Memory provenance, processing epochs, Dream pairs and file-to-Memory targets; finish Graph and validate source history/rewrite baseline; finish Communities and Insomnia, then validate/rebuild Insomnia schedule; replay deferred Dream population if indicated; rebuild Freshness from a second caller-owned physical cursor, validate activation and rebuild due dispatch; finish packed/profile/vector/generation stores and validate semantic global versions; finish compaction evidence without reclaiming it; validate import owner/receipts; construct Cva and run prune, settlement/completion, and entity-resolution validation. |
| PHY / Phylactery | 1. Feed versioned payloads to rewrite-lineage validation; reject rewrite genesis without owner identity. 2. Reject REL-only records. 3. Ingest owner-maintenance evidence. 4. Decode publication envelopes and try Memory â†’ Entity â†’ Relationship â†’ Graph, rejecting unsupported bodies; otherwise feed legacy-capable Memory/Entity/Graph/Relationship states. 5. Ingest Communities, packed vectors, Memory vectors, compatibility profiles, PHY profile, processing epochs, Dream pairs, Ego, and entity resolutions in that order. | Install full physical clock state; revalidate PHY identity; finish Memory, Entity and Relationship; validate local references and reject principal Entity records; validate Ego/Memory version, provenance, processing epochs and Dream pairs; finish Graph and validate rewrite baseline; finish Communities and semantic global versions; finish packed/profile/vector stores; construct Phylactery and validate prune evidence and entity-resolution consistency. |

REL's Fallback dispatch expands, in source order, to: owner maintenance; import epochs; external index; Archive, Memory, Graph, Entity and Relationship open states; Communities; Insomnia; packed vectors; Memory vectors; Archive vectors; compatibility profiles; vector generations; interaction streams; compactions; Echo; Ego; entity resolutions; project history; project files; REL metadata; Freshness derived-birth policy detection and `CVAFRP01` ingest; Dream-begin validation/defer and (unless deferred) Dream Freshness ingest; processing epochs; Dream pairs. This is the same ordering present in the prior callback body. The separately matched Archive/Memory/Graph/ContainerVersion/Publication route behavior is listed in the table and also remains in source order.

The independent second REL Freshness/Dream replay cursors are also caller-owned, use the same framing/version parser, and keep their own clock state; they do not republish or mutate the Container's live clock. Successful open retains owner finishing and validation. Current lifecycle code no longer calls ordinary-open compaction recovery or retired-Entity reference repair; it still validates the replayed entity-resolution store. Current `HEAD` callbacks did invoke those repairs after constructing each facade, so that specific post-replay mutation is intentionally replaced by explicit maintenance, rather than silently omitted from owner validation.

### Physical reader call-site classification

This inventory classifies the main production reader families by consumer and access authority; test-only physical cursors are omitted here and remain covered by the task/test tables.

| Reader class | Concrete production paths | Access and purpose |
| --- | --- | --- |
| Header/full physical inspection | `src/container/container_scan.rs`, `container_access.rs`; `Container::open`/`open_read_only` and owner `open_from_container` in both lifecycle files | Full bounded-memory framing and physical clock scan. Writable open holds writer authority; inspection/open-read-only holds read authority. Incomplete physical tail is evidence, not an implicit write. |
| Historical and prefix views | `facade/historical_epoch_input.rs`, `historical_import_base.rs`, `historical_import_input.rs`, `historical_import_transform.rs`, `historical_global_frontier.rs`, `historical_ego_fingerprint.rs`, `historical_graph_fingerprint.rs`, `historical_retained_diagnostics.rs`, `imports/epoch_scope.rs`, `imports/completion.rs`, `perception/entity_resolution_store.rs` | Bounded cursor or cursor over a `PhysicalReadSource` borrowed from an already held owner. Prefix boundaries preserve accepted-prefix semantics; repeated scans have independent clock state. |
| Freshness | `facade/cva_freshness.rs`, `freshness_storage.rs`, `freshness/freshness_storage_stream.rs` | Historical record/range replay from a pinned source; owner-mounted reads derive a source from the held Container, avoiding a conflicting second path lease. |
| Dream replay/population | `facade/cva_dream_journal_replay.rs`, `cva_dream_population.rs`, `runtime_inference/dream_pair_history_tests.rs` (test caller), `runtime_inference` Dream history paths | Physical frames/prefixes are pulled by the replay/population caller; semantic Dream validation remains above Container. |
| Rewrite, reconciliation, prune, repack and reclamation | `rewrite/file.rs`; `facade/cva_reconcile.rs`, `cva_reconcile_repack.rs`, `cva_reconcile_promote.rs`; `cva_history_prune.rs`, `phylactery_history_prune.rs`, `cva_repack.rs`, `cva_principal_repack.rs`, `storage_reclamation.rs` | Source snapshots and output verification use pinned read evidence; destination writers remain distinct and are retained through verification/publication. In-place tail repair uses the separately authorized repair path. |
| Import and epoch replay | `facade/cva_historical_import.rs`, `imports/epoch_scope.rs`, `imports/completion.rs`, `imports/reconciliation.rs` and `reconciliation_replay.rs` | Caller-owned epoch/prefix replay under semantic admission rules, with the physical lease still enforced independently. |
| Runtime mounts | `runtime_inference/runtime_host_gateway_mount.rs`, `project_environment/lifecycle.rs` and facade open routes | A mounted writable owner retains its writer lease; a second independent writable open conflicts. Mounted history reads use the owner-held source evidence. |

### Raw physical mutation authority

| Physical operation | Central implementation | Current semantic/maintenance caller families |
| --- | --- | --- |
| Create/header, append framed payload, read and sync | `src/container/container.rs` | Owner stores and facade writers (Archive/Memory/Entity/Relationship/Graph, Freshness, import, reconciliation and rewrite destination writers); writes pass Container's writer-lease/admission checks, independently of semantic import admission. |
| Version allocation and publication-envelope append | `src/container/container_version.rs`, using Container append authority | Owner publication paths and explicit global reservation/allocation flows; physical clock validation remains centralized. |
| Chunk prefix rewrite, split, adjacent merge, tail-chunk truncation | `src/container/container.rs` | Archive conversation-compaction allocator/store (`archive/conversation_compaction_allocator.rs`, `conversation_compaction_store.rs`); only explicit authorized compaction maintenance should drive reclamation. |
| Incomplete physical-tail repair and accepted-prefix copy | `src/container/container_repair.rs` plus owner validation in `facade/semantic_maintenance.rs` | Explicit REL/PHY repair workflow; it inspects under writer lease, stages/replays the accepted prefix, truncates/syncs only after validation, then verifies repaired replay. Ordinary open is not an authority source. |
| New transformed-copy output and final publication | `src/rewrite/file.rs` and the facade rewrite/reconciliation coordinators | Distinct destination creation, verification, and guarded publication; source access remains pinned read evidence. |

## Negative evidence for corruption and open purity

These source-reviewed tests directly exercise refusal/non-mutation boundaries; listing them is not a statement that the currently running full library suite has completed.

| Contract | Test/source evidence | Assertion exercised |
| --- | --- | --- |
| Incomplete tail is explicit | `container/container_tests.rs::incomplete_tail_requires_explicit_repair_plan`; `facade/semantic_maintenance_tests.rs::rel_tail_repair_requires_explicit_semantic_validation_and_keeps_one_source_lease` and PHY counterpart | Ordinary open returns `RepairRequired { valid_end }`; explicit repair validates accepted-prefix replay and checks final source length. |
| Complete malformed clock/envelope is not truncatable tail | `container/publication_envelope_clock_tests.rs::v2_complete_invalid_envelopes_are_rejected_without_tail_repair`, `v1_rejects_complete_malformed_publication_prefix`, plus `facade/historical_recovery_tests.rs::historical_native_complete_corrupt_checkpoint_rejects_without_tail_repair` | Complete invalid records fail as corruption/version errors and source bytes remain identical. |
| Complete owner-invalid envelope is never repaired, with or without an incomplete tail | `facade/semantic_maintenance_tests.rs::rel_repair_refuses_complete_unaccepted_owner_frame_with_or_without_torn_tail` and `phy_repair_refuses_complete_unaccepted_owner_frame_with_or_without_torn_tail` | Both REL and PHY reject the complete unknown-owner envelope whether it stands alone or precedes a torn tail; ordinary open and explicit repair fail, and source bytes remain identical. Included in the seven-test semantic-maintenance group. |
| Cursor bounded cut and clock purity | `container/container_tests.rs::bounded_physical_cursor_preserves_live_clock_state`; `publication_envelope_clock_tests.rs::bounded_versioned_visitor_rejects_midframe_malformed_clock_and_v1_envelopes_read_only` (test name retained, body now uses cursor) | Historical/bounded traversal does not publish its local clock state; a cut inside a frame and malformed clock are rejected without source mutation. |
| Incomplete reservation/direct-allocation clock state | `publication_envelope_clock_tests.rs::every_incomplete_reserved_binding_tail_preserves_clock_and_reservation` and `every_incomplete_direct_allocation_tail_recovers_without_advancing_clock` | Every incomplete physical prefix refuses ordinary open, repairs explicitly, preserves accepted reservation/time state and then permits the expected next append. |
| Semantic open has no implicit correction | `facade/semantic_maintenance_tests.rs::rel_open_preserves_retired_resolution_evidence_until_explicit_repair` and PHY counterpart | Stale retired-reference evidence survives open unchanged; explicit repair is separately exercised. |
| Tail and multi-recipient settlement recovery | `facade/historical_recovery_tests.rs::historical_native_checkpoint_every_container_byte_prefix_reopens_and_retries` and `historical_native_due_reopen_and_every_live_use_prefix_preserve_all_recipients` | Byte-prefix state is either accepted or explicitly repaired; recipient projection, retry/idempotence and byte identity assertions are retained. |

## Before-image retention and caller classification

The machine-readable [source and retained-fixture hash record](../benchmarks/dependency-convergence-phase1-verification-2026-10-22.json)
pins the authored Rust manifest, every changed Rust source, the original Phase 0
capture, the frozen oracle and all six retained fixture hashes/sizes/timestamps.
Git blob and working-file capture hashes are recorded separately because the
Windows checkout uses different line endings; their normalized contents match.

The Phase 0 source capture and reviewed graph evidence are retained as the historical before-image; the current implementation refresh does not overwrite that source capture. Physical scans are classified as live owner replay, bounded/historical read, stable rewrite source, or explicit repair. Writable owner opens take writer authority before physical validation. Mounted-owner history reads borrow a pinned read source from the held Container instead of attempting a second path-based lease. Rewrites retain source evidence while constructing and verifying a distinct destination, then perform guarded publication.

## Explicit side effects and access paths

Ordinary open/replay no longer performs implicit tail truncation, stale conversation-compaction reclamation, or retired-Entity reference canonicalization. The respective actions are explicit physical tail repair (`container_repair.rs`) and semantic maintenance methods in `facade/semantic_maintenance.rs` (`maintain_conversation_compactions` and retired-Entity reference repair). These methods retain owner validation and writer authority; opening or holding a writer lease alone does not authorize cleanup.

Writer exclusion and coherent read evidence are OS-handle based as detailed in the lease contract. New destination creation uses exclusive creation and a destination lease; transformed-copy publication retains verification and destination authority through publication. The contract documents Windows and Unix primitives, file identity, alias/replacement refusal, crash release, unsupported locking refusal, and known platform durability limits. Final lock/publication gates are still subject to root verification.

## Obsolete paths removed

The semantic visitor-driven `Container::open_scanned` route and its callback-oriented historical visitor APIs have been removed from production callers. Callers now own their `while cursor.next_frame()` loops; Container exposes physical frame/clock evidence and does not execute owner ingest code. Redundant frame scans are converged on the shared cursor. Remaining compatibility or duplicate-scanner findings, if any, must be surfaced by the final source-ratchet run rather than inferred away.

## Phase 2/3 handoff

Phase 2 should move semantic state transitions and replay responsibilities into owner-local boundaries while preserving the current facade as composition until each owner has a tested seam. It should use the 34-component target DAG and the retained Phase 0 source capture rather than treating current imports as intended ownership. Phase 3 should own cross-owner repair/reconciliation policy, including when stale semantic evidence requires refusal, repair, or retained diagnostics. Neither phase should return semantic callbacks to Container or reintroduce implicit open-time mutation. The current lease and `PhysicalReadSource` contracts are lower-boundary inputs, not substitutes for those semantic decisions.

## Final verification ledger

Completed evidence:

- `.verification-tmp/phase1-check-complete.log`: final Rust check passed.
- `.verification-tmp/phase1-cleanup-check.log` and `phase1-cleanup-cli-check.log`: root and CLI checks passed after the final staging-cleanup edit.
- `.verification-tmp/phase1-build-complete.log`: full build with `--no-run` completed successfully.
- `.verification-tmp/phase1-focused-complete.log`: focused lease/process, mount, alias, semantic-maintenance, reconciliation, history and lifecycle groups passed; `Failed groups: []`.
- `.verification-tmp/phase1-cli-tests-acceptance.log`: CLI suite reports **32 passed, 0 failed, 0 ignored**. Final root and CLI formatting checks also passed (`phase1-fmt-final.log`, `phase1-cli-fmt-final.log`).
- `.verification-tmp/phase1-physical-guard.log`: physical allowlist, callback, handle-escape and open-purity guards passed with **13 fixtures**.
- `.verification-tmp/phase1-dag.log`: logical DAG passed with **34 components**, **163 allowlisted edges**, no cycles/upward dependencies, and all listed negative fixtures passing.
- `.verification-tmp/phase1-docs-full.log` and `phase1-docs-changed.log`: both documentation checks passed.
- Final acceptance edits also passed both policy checks (`phase1-docs-full-final.log`, `phase1-docs-changed-final.log`), indexes/links (`phase1-ddocs-final.log`), documentation ownership (`phase1-pitlord-docs-final.log`), root/CLI formatting and `git diff --check`. `phase1-final-source-manifest.log` confirms all **1,109** indexed Rust sources match the final authored source, with **zero mismatches**.
- `.verification-tmp/phase1-ddocs-acceptance.log`: documentation indexes and links passed after refreshing the generated index section.
- `.verification-tmp/phase1-tests-complete.log`: the comprehensive library run completed with **1,956 passed, 0 failed, 5 ignored, 1 filtered**. The filtered exhaustive corpus test had passed separately in its prior dedicated run.
- `.verification-tmp/phase1-cleanup-repair-final.log` and `phase1-cleanup-container-final.log`: after the final cleanup edit, **7 maintenance tests** and **47 container/recovery tests** passed, including exhaustive physical byte-prefix recovery.
- `.verification-tmp/phase1-integration-acceptance.log`: all **25 public integration targets** completed with **163 passed, 0 failed, 1 ignored**, plus **0 doctests**. Together with the library run, the pipeline reports **2,119 passed, 0 failed, 6 ignored across 27 groups**.
- `.verification-tmp/phase1-pitlord-docs-acceptance.log`: exit 0; **1,227/1,227** documentation items mapped, with **428 code mappings** and **22 mapped documentation changes**.
- `.verification-tmp/phase1-source-cleanup-acceptance.log`: final source-graph acceptance passed with snapshot SHA-256 `01a79a32693b56f712748f2a62c973ebf5a4bc9e8d34f98ba1b5a1c64ec7c69b`, **99,800 nodes**, **9,233 relationships**, and **25 rules**. The debt ledger contains **4,625** rows with no new or stale debt; **9 stale rows** were removed, **9 equivalent rows** were rebound (7 test-only and 2 existing Phase 5 runtime open-read-only rows), **5 test rows** were added, and all **92 Phase 1 rows** are test-only.

The comprehensive suite and long corpus executable were built before the final
staging-cleanup edit. That edit changes only failure cleanup in
`copy_accepted_prefix`: lease-acquisition and late-validation errors remove the
owned stage only after matching canonical identities, and cleanup retains the
lease while removing the file. Ordinary replay and accepted-frame processing
are unchanged. The corpus comparison never invokes explicit tail repair.
The post-edit checks and 54 focused tests above validate the changed repair and
physical-storage surface; the final graph and source hash record include it.

The named opt-in corpus test completed successfully: **6/6 files verified**, zero
semantic mismatches, and identical source bytes and modification times. Its log
is `.verification-tmp/phase1-corpus-acceptance.log` (6,372.74 seconds). It ran with
assertions enabled using:

```text
cargo test --locked --config 'profile.test.package.reliquary-memory.opt-level=1' --config 'profile.test.package.sha2.opt-level=3' --config 'profile.test.package.serde_json.opt-level=3' --features history-oracle-fixtures --lib canonical_current_corpus_matches_frozen_history_oracle -- --ignored --nocapture
```

The machine-readable evidence records the final 1,109-file Rust manifest,
101 changed source hashes, enforcement hashes, final architecture snapshot,
unchanged original capture and all six fixture hashes/sizes/timestamps.

### Platform limits and handoff

Acceptance was executed on Windows. Unix lock/identity/publication paths were
source-reviewed but not executed on this host. Windows directory-entry
durability across sudden power loss is not claimed; the tested contract covers
process exclusion and atomic publication. Unsupported locking or hard-link
publication refuses rather than silently weakening that contract.

Later semantic ownership work remains in Phases 2–7. In particular, the retained
Freshness-to-Archive payload interpretation is Phase 2 debt, and runtime/facade
composition remains Phase 5 debt. Neither is a physical Container inversion.

## Related docs

- [Phase 1 implementation specification](dependency-convergence-phase1-storage-foundation-spec-2026-10-21.md)
- [Phase 1 file lease contract](dependency-convergence-phase1-lock-contract-2026-10-21.md)
- [Phase 0 source graph and reviewed ledger](dependency-convergence-phase0-2026-10-21.md)
- [Dependency convergence target DAG](dependency-convergence-target-dag-2026-10-21.md)
- [Storage format](storage-format.md)

## Notes

This is the local implementation acceptance and handoff record; the specification remains the governing requirement. Log files are retained under `.verification-tmp/`.