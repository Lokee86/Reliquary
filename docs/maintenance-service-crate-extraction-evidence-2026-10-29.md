# Maintenance service crate extraction — M0 evidence

**Status:** M0 inventory captured; implementation in progress; tests and acceptance pending (2026-10-29).

This is the function-to-owner crosswalk required by the [maintenance service extraction specification](maintenance-service-crate-extraction-spec-2026-10-29.md). It records the initial source baseline and subsequent in-progress extraction state. Tests and acceptance have not been completed.

## Baseline and worktree state

- Repository: `Reliquary-crate-first`.
- Observed HEAD: `20e8955291eb1d048d9883d5b025a23eed1419bf`.
- The specification names `b0bca3cd` as its reference checkpoint. The observed HEAD does not match that abbreviated reference; the initial inventory baseline was HEAD `20e8955291eb1d048d9883d5b025a23eed1419bf`.
- At the initial baseline, `git status --short` showed only the unrelated Graph extraction evidence modification and `.temp/` untracked:

  ```text
   M docs/graph-crate-extraction-evidence-2026-10-26.md
  ?? .temp/
  ```

- Those two paths were outside the maintenance extraction and remain unrelated.
- At the initial baseline, `crates/reliquary-maintenance/Cargo.toml` had no dependencies and `crates/reliquary-maintenance/src/lib.rs` was the placeholder boundary. Thus the initial `cargo tree -p reliquary-maintenance --depth 1 --offline` was dependency-free. The current in-progress extraction now depends on `reliquary-memory-core` for the existing `MemoryId` type:

  ```text
  reliquary-maintenance v0.1.0
  └── reliquary-memory-core v0.1.0
  ```

  `reliquary-maintenance` now owns the existing explicit-selection and semantic-closure implementations, and both REL and PHY rewrite planners call those service functions. This is in-progress production wiring; it does not establish completed tests or acceptance.

## Function-to-owner crosswalk

| Existing function / value | Current location and role | M0 disposition |
| --- | --- | --- |
| `Cva::prune_tombstoned_history` | `src/facade/cva_history_prune.rs`; source admission, import/settlement gates, physical-prefix check, target validation, owner evidence, content selection, copy, manifest and reopen verification | **Stay Root** for source authority, physical I/O, output and commit. Inspect the already-modified worktree file against baseline before any migration. |
| `HistoryPruneRequest`, `HistoryPruneReport` | `src/facade/cva_history_prune.rs`; public API values | **Stay Root/public API** unless a compiled lower consumer proves relocation necessary; preserve existing public paths. |
| `prune_clocks`, `prune_error` | `src/facade/cva_history_prune.rs`; clock projection and Root error construction | **Stay Root**; small adapters, not a meaningful service extraction. |
| `Phylactery::prune_tombstoned_history`, `validate_prune_evidence` | `src/facade/phylactery_history_prune.rs`; PHY owner admission, proof and mutation orchestration | **Stay Root** for owner authority and mutation. |
| `entity_retention`, `relationship_retention`, `profile_retention` | `src/facade/phylactery_history_prune.rs`; PHY retention checks using PHY-specific facts | **Stay owner/Root**; do not generalize or duplicate owner contracts in Maintenance. |
| `fixed_identity`, `append_part`, `hash_part`, `phy_clocks` | `src/facade/phylactery_history_prune.rs`; PHY identity/digest/clock support | **Stay owner/Root** unless a concrete lower shared policy consumer is found. |
| `archive_retention_descriptor`, `validate_archive_prune_history`, `validate_archive_markers`, `hash_bytes` | `src/facade/cva_archive_history_prune.rs`; Archive records, retention descriptors and proof validation | **Stay Archive/Root**; owner codec and evidence are authoritative below Maintenance. |
| `Cva::description_retention`, entity/relationship description structure helpers, `validate_pruned_descriptions` | `src/facade/cva_description_prune.rs`; description-specific retention and validation | **Stay owner/Root**; requires owner facts and validation authority. |
| `validate_prune_history`, `descriptor` | `src/facade/memory_history_prune.rs`; Memory-specific prune evidence and descriptors | **Stay Memory owner/Root**. |
| `validate_source_history` | `src/facade/owner_maintenance.rs`; source history proof/fingerprint | **Stay Root/History boundary**; do not move cross-owner source admission into Maintenance. |
| `owner_history_digest`, `digest_bytes` | `src/facade/history_prune_manifest.rs`; cross-owner history digest support | **Stay Root** for source fingerprints. The canonical `CVAHPR01` encoding, parsing, projection and replay remain in `reliquary-maintenance-store`; do not duplicate that implementation. |
| `Cva::repack_project_backed_attachments`, `removable_project_content_ids` | `src/facade/cva_repack.rs`; source inspection, relocation and publication | **Stay Root/Container** for I/O and publication. Assess only a narrowly pure decision helper if real callers support it. |
| `Cva::repack_missing_user_principal`, `rewrite_missing_user_principal` | `src/facade/cva_principal_repack.rs`; owner read, rewrite and atomic publication | **Stay Root/owner**; mutation and publication are not service policy. |
| `IdentityPolicy`, `MemoryBase`, `RevisionPolicy`, `MemorySelection`, `FreshnessRewriteMode`, `ArchiveSelection`, `ProvenancePolicy`, `RewriteRequest` | `src/rewrite/model.rs`; existing rewrite request contract and serialized policy | **Keep public model in Root initially**; move only values proven necessary for real lower call sites and re-export the same type. |
| `MemoryRewriteDecision`, `MemoryRevisionSelection`, `RewritePlan`, `RewriteReport` | `src/rewrite/model.rs`; existing plan/report contract | **Keep public types in Root initially**; avoid parallel plan models and preserve API paths. |
| `apply_explicit_selection`, `apply_semantic_closure` | `crates/reliquary-maintenance/src/lib.rs`; deterministic candidate decision policy | **Moved and wired**: both actual REL and PHY `plan_rewrite` paths call these service functions. Root retains source reads and closure discovery, while the service applies the existing candidate decisions. |
| `revision_numbers` | `src/rewrite/plan.rs`; deterministic mapping from requested revision policy and supplied version lookup to selected revisions | **Possible adjacent candidate** if it can consume supplied immutable version facts without moving owner reads. |
| `memory_link_closure` | `src/rewrite/plan.rs`; closure traversal currently fetches `crate::Memory` through a callback | **Stay Root for reads**. A lower policy could process supplied parent/supersession links, but do not move the owner-fetching callback or introduce an owner projection without proving the need. |
| `finish_plan` | `src/rewrite/plan.rs`; final plan assembly | **Stay Root** for plan construction. Its duplicate include/exclude pass was removed; explicit choices are applied by Maintenance before closure expansion, preserving exclusion precedence and closure rejection behavior. |
| `parse_id`, `id_hex`, `uuid_hex`, `source_head_digest`, `memory_has_provenance`, `validate_common`, `complete_plan_report`, `finalize_plan_digest` | `src/rewrite/plan.rs`; request parsing, filesystem source fingerprint, source-derived facts, plan formatting/digest | **Mostly stay Root**; specifically `source_head_digest` is filesystem authority, not Maintenance policy. Reassess individual pure helpers only if required by the selected seam. |
| `latest_rel_memory`, `latest_phy_memory`, `selected_revisions_rel`, `selected_revisions_phy`, `Cva::plan_rewrite`, `Phylactery::plan_rewrite` | `src/rewrite/plan.rs`; REL/PHY source access and planners | **Planner entry points and owner reads stay Root**. Rewire both planners to the extracted policy if M1 selects it. |
| `semantic_closure_rel`, `semantic_closure_phy`, common `semantic_closure`, relation retention helpers, `copy_semantic_state_rel/phy` | `src/rewrite/semantic_state.rs`; owner-specific semantic reads and output replay | **Stay Root/owner** unless a separate exact pure policy slice is evidenced; do not move replay writers. |
| `plan_rewrite_file`, `rewrite_file`, `rewrite_from_plan`, `validate_emitted`, emit/copy helpers | `src/rewrite/file.rs`; path-based input, physical emission and output verification | **Stay Root/Container**; no file or writer authority in Maintenance. |

The broader specification-listed `cva_reconcile*` family, storage-reclamation helpers and rewrite copy/emission functions also contain Root admission, owner-state conflict decisions, physical relocation, or publication. No crosswalk evidence found in this M0 pass supports moving their orchestration wholesale. A narrow pure existing decision can be reconsidered only with exact production callers and source-level proof.

## Candidate seam and boundaries

The extraction is the existing rewrite selection policy shared by both `Cva::plan_rewrite` and `Phylactery::plan_rewrite`: `apply_explicit_selection` and `apply_semantic_closure`. It uses `reliquary-memory-core::MemoryId` via a direct `reliquary-memory-core` dependency. Both production planners call the service while source reads, source fingerprints, filesystem access, closure discovery, and plan orchestration remain in Root. `revision_numbers` was not selected.

The implementation resolves the M1 source details as follows:

1. The service is the sole include/exclude and closure decision implementation; the repeated selection pass was removed from `finish_plan`.
2. The candidate tuple uses the existing `MemoryId` from `reliquary-memory-core`; no parallel ID, request, plan, or report model was introduced.

The spec-listed prune path contains executable policy, but its central procedure is interleaved with owner admission, payload reads and output work. PHY-specific retention functions depend on PHY-owned facts. Neither is as clean a shared first extraction as the dual REL/PHY rewrite selection policy.

## Existing focused test inventory

The following named owner-level tests are available for production parity evidence:

- `src/rewrite/semantic_state_tests.rs`: `relationship_evidence_and_current_graph_references_expand_retained_memories`, `rel_copy_remaps_local_evidence_and_writes_only_a_current_graph_baseline`, `phy_copy_preserves_authoritative_ego_and_omits_synthesis_cache`.
- `src/rewrite/model.rs`: `request_round_trips_as_a_policy_file_with_defaults`, `policy_file_rejects_unknown_fields` (request compatibility only; not sufficient policy parity by themselves).
- `src/facade/history_prune_tests.rs`: `explicit_prune_removes_only_tombstoned_body_and_preserves_structure`, `shared_live_body_and_nonterminal_targets_are_rejected_before_output`, `retired_profile_prune_keeps_vector_topology_and_marks_matrix_unavailable`, `memory_body_prune_retains_revision_topology_and_rejects_unbound_marker`, `entity_relationship_descriptions_prune_with_topology_and_references_intact`.
- `src/facade/cva_principal_repack.rs`: `native_turn_backfill_preserves_publication_clock_and_atomic_bundle`, `repack_backfills_only_user_turns_and_preserves_entity_state`, `repack_rejects_conflicting_existing_user_principal` (only relevant if repack is selected).
- `src/facade/cva_reconcile_tests.rs`: `compare_identical_copies`, `compare_detects_one_side_ahead`, `compare_detects_divergent_tails`, and selected divergent-reconcile cases (only relevant if reconciliation is selected).

The service crate contains two focused unit tests for explicit-selection error ordering/exclusion precedence and semantic-closure missing/excluded/retained cases; these passed (2/2). The external consumer `history_rewrite_contract` passed all 16 tests, exercising the real rewrite API. Prune, repack, and reconciliation tests above remain inventory only because those paths were not moved.

## M0 outcome

M0 selected the shared deterministic rewrite-selection policy. The initial baseline is recorded above. Extraction code and REL/PHY call-site wiring are implemented. The specification reference checkpoint differs from the observed full HEAD and should be reconciled in the final evidence record.

## Verification evidence

- Baseline `cargo check -p reliquary-maintenance --locked --offline -j 1` passed before edits.
- `cargo test -p reliquary-maintenance --locked --offline -j 1`: passed, 2 tests.
- `cargo test --test history_rewrite_contract --locked --offline -j 1`: passed, 16 tests.
- `cargo check --workspace --all-targets --locked --offline -j 1`: passed.
- `cargo test --manifest-path cli/Cargo.toml --locked --offline -j 1`: passed, 32 tests with repository-local TEMP/TMP. The CLI lockfile was refreshed offline before the locked run.
- `cargo fmt --all -- --check`, CLI formatting, and `git diff --check`: passed.
- `ddocs check --indexes --links`: passed after reconciling `docs/INDEX.md`.
- `cargo tree -p reliquary-maintenance --depth 1 --offline`: direct dependency is only `reliquary-memory-core`.
- Architecture/ownership guard failed on six existing `src/context_engine` files reported as lacking an owner. This remains an unresolved repository architecture finding; no architecture policy was relaxed.
- Fresh Lexicon/Arcana/Pitlord graph acceptance was not run; no fresh graph result is claimed.
- `cargo test --lib --locked --offline -j 1 -- --test-threads=8`: passed, 1,457 passed and 5 ignored, with loopback access and short repository-local `.t` TEMP/TMP/TMPDIR. An earlier run with the longer `.maintenance-test-temp` path produced three external gateway failures; the isolated case passed with the shorter path, and the full short-path rerun passed. Restricted loopback also caused configured-runtime test failures in an earlier run.
