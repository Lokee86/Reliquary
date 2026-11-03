# REL/PHY existing-method ownership crosswalk — 2026-11-03

Parent index: [Documentation index](INDEX.md)

**Status:** Design disposition for each tracked Rust inherent method on `Cva` or `Phylactery` at `fea1f099`, not implemented or compiler-qualified. Source scanning uses balanced `impl Type` blocks. Supporting free functions and call sites remain for the migration plan.

## Purpose

Give every tracked inherent method on the existing REL/PHY aggregates an explicit architectural destination so the redesign does not leave a hidden Root responsibility behind.

## Overview

The crosswalk covers 519 unique file-qualified methods in 82 Rust files, classified as owner operations, reads, workflows, semantic services, runtime tasks, maintenance or integration. The target authority is defined in the [Stage 1 storage architecture](rel-phy-storage-architecture-2026-11-03.md).

## Destination legend
- **O**: owner session / typed store command or lifecycle
- **Q**: owner-qualified read or historical proof
- **W**: multi-domain application workflow
- **S**: semantic service policy/computation
- **T**: runtime scheduler/worker
- **M**: exclusive import, history, maintenance or reconciliation
- **I**: external integration

**O/Q** are private typed owner interfaces, not a general public mutable store bag. **W/S/T/M/I** move outside that owner aggregate to their rightful coordinator/domain. A facade may forward an approved product operation without owning state. Code containing `Cva` is renamed to REL domain language at the separately verified hard cut.

## Coverage
- `Cva`: 393 unique file-qualified methods in 68 files.
- `Phylactery`: 126 unique file-qualified methods in 24 files.

## Complete inherent-method mapping

### Cva

**`src/external_source/mod.rs`**
- **I**: `external_index_state`, `external_install`, `external_search`, `external_profile`.

**`src/facade/cva.rs`**
- **O**: `materialize_branch_fragments`, `materialize_path_fragments`, `materialize_branch_episodes`, `materialize_path_episodes`, `append_branch`, `set_rel_metadata`, `set_conversation_title`, `set_conversation_active`, `set_conversation_policy`, `set_memory_temporal_status`, `put_memory_routing_metadata`, `store_file`, `put_conversation_compaction`, `put_interaction_stream`, `put_echo_event`, `correlate_project_revision`, `correlate_project_revision_with_management`, `register_project_file`, `bind_legacy_project_file`, `sync`.
- **Q**: `legacy_scope_kind`, `rel_metadata`, `owner_id`, `owner_uuid`, `latest_global_version`, `transaction_time_ns`, `version_at_or_before`, `archive`, `conversation_policy`, `conversation_metadata`, `historical_conversation_metadata`, `branch_turns`, `historical_branch_turns`, `branch_at`, `fragment_turns`, `fragment_text`, `episodes`, `episodes_for_conversation`, `historical_episodes`, `historical_episodes_for_conversation`, `episode`, `historical_episode`, `episode_turns`, `memory`, `memory_retention_descriptor`, `historical_memory_revision`, `memory_lifecycle_state`, `memory_revision`, `memory_stats`, `memory_ids`, `memory_version`, `memory_body_id`, `memory_routing_metadata`, `stats`, `archive_version`, `record_versions`, `record_version`, `branches`, `historical_branches`, `fragments`, `historical_fragments`, `file`, `files`, `file_bytes`, `conversation_compactions`, `interaction_stream_record`, `interaction_streams_for_session`, `borrowed_interaction_streams_for_session`, `interaction_stream_records`, `echo_events`, `conversation_echo_events`, `echo_records`, `current_rel_semantic_cut`, `project_revision_correlations`, `latest_project_revision_correlation`, `project_file_ref`.
- **W**: `append_node`, `append_node_with_principal`, `publish_memory`, `tombstone_memory`.

**`src/facade/cva_activity.rs`**
- **Q**: `activity_cut_at_archive_version`, `rel_turn_count`, `activity_position_for_turn`.

**`src/facade/cva_archive_vectors.rs`**
- **O**: `put_archive_vectors`.
- **Q**: `archive_vectors`, `archive_vector_infos`, `archive_vector_stats`.

**`src/facade/cva_communities.rs`**
- **O**: `set_community_name`, `publish_community_semantic_name`.
- **Q**: `community_snapshot`, `community_stats`, `community_semantic_name`, `community_semantic_names`, `community_lineage`.
- **S**: `refresh_communities_leiden`.

**`src/facade/cva_compatibility_profiles.rs`**
- **O**: `establish_compatibility_profile`, `retire_compatibility_profile`.
- **Q**: `compatibility_profile`, `compatibility_profiles`, `compatibility_profile_is_retired`, `active_memory_compatibility_profile`, `active_memory_compatibility_profiles`, `active_archive_compatibility_profiles`, `compatibility_profile_stats`.
- **S**: `verify_compatibility_endpoint`.

**`src/facade/cva_conversation.rs`**
- **Q**: `conversation_summaries`, `conversation_turns`, `historical_conversation_turns`, `conversation_turn_page`, `conversation_branch_start_node_ids`.

**`src/facade/cva_conversation_tombstone.rs`**
- **Q**: `tombstone_conversation`, `conversation_tombstone`, `conversation_lifecycle_state`.

**`src/facade/cva_description_prune.rs`**
- **M**: `description_retention`, `entity_description_structure`, `relationship_description_structure`, `validate_pruned_descriptions`.

**`src/facade/cva_dream_begin_transport.rs`**
- **Q**: `materialized_dream_begin`, `current_dream_begin_payload`.

**`src/facade/cva_dream_population.rs`**
- **M**: `validate_external_birth_cohort`, `external_birth_cohort_at_memory_prefix`, `validate_insomnia_birth_cohort`, `insomnia_birth_cohort_at_memory_prefix`, `direct_birth_ids_at_memory_prefix`, `dream_population_at_prefix`.

**`src/facade/cva_ego.rs`**
- **O**: `create_ego_anchor`, `update_ego_anchor`, `delete_ego_anchor`, `put_ego_web_synthesis`.
- **Q**: `ego_version`, `ego_anchors`, `ego_anchor`, `ego_web_synthesis`, `ego_anchor_lifecycle`, `historical_ego_anchor`.

**`src/facade/cva_episode_policy.rs`**
- **W**: `finalize_inactive_path_episodes`, `finalize_explicit_path`, `finalize_create_memory_path`.

**`src/facade/cva_file_memory.rs`**
- **O**: `link_file_to_memory`, `file_memory_links`, `files_for_memory`.

**`src/facade/cva_freshness.rs`**
- **O**: `initial_dream_pass_settled`, `freshness_event_proof_is_available`, `set_freshness_policy_before_enrollment`, `enroll_legacy_freshness_baseline`, `freshness_initialize`, `freshness_admit`, `validate_initial_dream_event_identity`, `freshness_publication_cohort`, `freshness_same_publication_cohort`, `initial_dream_graph_snapshot`, `freshness_replay_initialize`, `freshness_replay_admit`, `freshness_replay_event`, `freshness_replay_accepted_use_receipt`, `freshness_publication_birth_ids`, `freshness_replay_publication_birth`, `freshness_replay_settled_dream_pass`, `require_freshness_memory`.
- **Q**: `freshness_policy`, `freshness_has_pending_publication`, `freshness_is_publication_birth`, `freshness_has_pending_dream_passes`, `freshness_record`, `freshness_record_at_cut`, `freshness_record_at`, `freshness_records_at`, `pending_initial_dream_pass`, `freshness_replay_entries`, `initial_dream_committed_roots`, `initial_dream_committed_evidence`, `freshness_settled_dream_pass_ids`.
- **W**: `freshness_prepare_memory_publication`, `persist_accepted_freshness_birth`, `consume_publication_intent`, `freshness_commit_event`, `begin_initial_dream_pass`, `accept_initial_dream_settlement`, `record_initial_dream_roots`, `settle_initial_dream_pass`, `recover_committed_initial_dream_event`.

**`src/facade/cva_freshness_due.rs`**
- **T**: `freshness_refresh_due`, `drain_freshness_notifications`, `freshness_on_accepted_activity`, `process_freshness_due`, `buffer_freshness_crossing`.

**`src/facade/cva_graph.rs`**
- **O**: `set_memory_relation`, `set_memory_relation_with_origin`, `set_memory_relations`, `set_memory_relations_with_origin`, `set_entity_association`, `set_principal_association`, `set_semantic_relations_with_origin`.
- **Q**: `graph_version`, `memory_graph_version`, `graph_stats`, `graph_relations`, `semantic_graph_relations`, `entity_associations_for_memory`, `memories_for_entity`, `principal_associations_for_memory`, `memories_for_principal`, `principal_entity`, `memories_for_phy_principal`, `semantic_graph_neighbors`, `graph_neighbors`, `shortest_memory_path`.

**`src/facade/cva_historical_dream.rs`**
- **Q**: `historical_completed_initial_dream_passes`, `historical_dream_representatives`.

**`src/facade/cva_historical_freshness.rs`**
- **Q**: `historical_freshness_timeline`.

**`src/facade/cva_historical_import.rs`**
- **M**: `freeze_historical_import`, `freeze_historical_import_with_fill`, `commit_historical_import`, `semantic_scope_for_import_settlement`, `validate_historical_import_settlements`, `historical_reconstruction_owner`.

**`src/facade/cva_history_prune.rs`**
- **M**: `prune_tombstoned_history`.

**`src/facade/cva_lifecycle.rs`**
- **O**: `create`, `create_project`, `create_organization`, `create_connection`, `open_project`, `open_organization`, `open_connection`, `create_rel`, `create_rel_with_uuid`, `create_legacy_scope_with_uuid`, `initialize`, `open`, `repair_retired_entity_references`.

**`src/facade/cva_memory_retrieval.rs`**
- **S**: `build_memory_retrieval_index`, `retrieve_memories_with_index`, `retrieve_memories`.

**`src/facade/cva_memory_vectors.rs`**
- **O**: `put_memory_vectors`.
- **Q**: `memory_vectors`, `memory_vector_location`, `memory_vector_infos`, `memory_vector_stats`.
- **S**: `build_missing_memory_vectors`.

**`src/facade/cva_packed_vectors.rs`**
- **O**: `put_packed_vectors`.
- **Q**: `packed_vectors`, `packed_vector_infos`, `packed_vector_stats`, `prunable_packed_vector_profiles`, `packed_vector_prune_marker`, `packed_vector_retention_descriptor`, `validate_pruned_packed_vectors`.

**`src/facade/cva_principal_repack.rs`**
- **M**: `repack_missing_user_principal`.

**`src/facade/cva_reconcile.rs`**
- **M**: `compare`, `reconcile`.

**`src/facade/cva_reconcile_promote.rs`**
- **M**: `reconcile_and_promote`.

**`src/facade/cva_repack.rs`**
- **M**: `repack_project_backed_attachments`, `removable_project_content_ids`.

**`src/facade/cva_semantic_fragment.rs`**
- **S**: `semantic_fragment_sources`, `semantic_fragment_position`, `semantic_evidence_position`, `semantic_source_excludes_memory`, `prepare_semantic_fragment`.

**`src/facade/cva_semantic_live.rs`**
- **W**: `semantic_freshness_floor`, `semantic_freshness_pending_fragments`, `pending_semantic_fragments`, `process_semantic_freshness`, `validate_semantic_activation`, `prepare_semantic_archive_batch`, `commit_semantic_archive_batch`.

**`src/facade/cva_semantic_source.rs`**
- **W**: `freshness_replay_semantic_event`, `apply_accepted_semantic_reinforcement`, `apply_prepared_semantic_reinforcement`, `search_eligible_semantic_targets`.

**`src/facade/cva_turn_ingest.rs`**
- **W**: `ingest_turn`, `ingest_turn_with_receipt`, `files_for_source`, `historical_files_for_source`.

**`src/facade/cva_vector_generations.rs`**
- **O**: `publish_vector_generation`, `publish_vector_generation_without_live_semantic`, `publish_vector_generation_inner`.
- **Q**: `vector_generation`, `current_vector_generation`, `vector_generation_at`, `historical_vector_generation`, `vector_version`, `vector_generation_stats`.
- **S**: `build_archive_vector_generation`, `build_archive_vector_generation_verified`.

**`src/facade/cva_vector_recovery.rs`**
- **M**: `rebuild_derived_vectors`.

**`src/facade/historical_import_base.rs`**
- **M**: `historical_import_base`.

**`src/facade/historical_import_transform.rs`**
- **M**: `validate_historical_import_copy`, `validate_historical_principal_repack`.

**`src/facade/historical_reconstruction.rs`**
- **M**: `reconstruct_historical_freshness`, `reconstruct_historical_freshness_with_endpoint`.

**`src/facade/review_clocks.rs`**
- **Q**: `review_clock`.

**`src/facade/storage_reclamation.rs`**
- **M**: `reclaim_storage`.

**`src/imports/commit.rs`**
- **M**: `import_native`, `commit_prepared_import`, `with_import_write_scope`, `refresh_import_write_admission`, `native_import_receipts`, `open_import_epoch`, `import_epoch_status`, `import_epoch_statuses`, `import_watermarks`.

**`src/imports/completion.rs`**
- **M**: `complete_native_import`, `completion_epochs_from_bytes`, `validate_import_completion_settlements`.

**`src/imports/epoch_scope.rs`**
- **M**: `validate_semantic_import_scope`, `validate_import_settlement_scope`, `import_epoch_scope`, `import_epoch_scope_for`, `import_epoch_scope_through`.

**`src/imports/readiness.rs`**
- **M**: `native_import_readiness`, `derive_import_readiness`.

**`src/perception/entity_principal.rs`**
- **W**: `resolve_principal_entity_mention`, `sync_principal_profile`, `principal_mention_matches`.

**`src/retrieval/archive_search.rs`**
- **S**: `search_archive`.

**`src/retrieval/conversation_search.rs`**
- **S**: `search_conversation_branch`, `search_conversation_branch_with_vector`, `search_conversation_branch_inner`.

**`src/retrieval/lexical_search.rs`**
- **S**: `complete_memory_lexical_matches`, `memory_lexical_candidates`, `lexical_candidates`, `raw_archive_candidates`, `lexical_candidates_from_fragments`, `search_files`.

**`src/retrieval/memory_search.rs`**
- **S**: `search_memories`.

**`src/retrieval/search.rs`**
- **S**: `search`, `search_with_result_limit`, `search_with_config`, `search_fragments_with_vector`.

**`src/retrieval/search_rank.rs`**
- **S**: `rank_search_candidates`, `diversify_candidates`, `fragment_overlap`.

**`src/retrieval/semantic_search.rs`**
- **S**: `semantic_search`, `semantic_search_vector`, `search_current_generation`.

**`src/rewrite/lineage.rs`**
- **Q**: `history_generation`, `rewrite_genesis`.

**`src/rewrite/plan.rs`**
- **M**: `plan_rewrite`.

**`src/runtime_inference/dream_candidates.rs`**
- **S**: `dream_memory_context`, `dream_candidates`.

**`src/runtime_inference/dream_cooldown.rs`**
- **O**: `mark_dream_processed`.
- **Q**: `dream_eligible_epoch`, `dream_was_processed`.

**`src/runtime_inference/dream_freshness.rs`**
- **T**: `freshness_trace_run_initial_dream_fixture`, `freshness_trace_flush`.

**`src/runtime_inference/dream_lifecycle.rs`**
- **W**: `reconcile_dream_lifecycle`, `revise_lifecycle`.

**`src/runtime_inference/dream_pair_history.rs`**
- **O**: `mark_dream_pair_evaluated`.
- **Q**: `dream_pair_records`.

**`src/runtime_inference/dream_publisher.rs`**
- **W**: `publish_dream_pair`.

**`src/runtime_inference/dream_temporal.rs`**
- **S**: `dream_temporal_analysis`.

**`src/runtime_inference/freshness_access.rs`**
- **W**: `record_accepted_memory_use`, `ego_memory_freshness`, `select_ego_routine_context`.

**`src/runtime_inference/insomnia/cva.rs`**
- **O**: `claim_insomnia_episode`, `renew_insomnia_lease`, `complete_insomnia_episode`, `fail_insomnia_episode`, `terminal_insomnia_episode`, `retry_terminal_insomnia_episode`, `insomnia_work`, `insomnia_attempts`, `insomnia_stats`.
- **W**: `queue_insomnia_episode`, `materialize_live_path_and_queue`, `finalize_inactive_path_and_queue`, `finalize_explicit_path_and_queue`, `request_create_memory`, `finalize_import_path_and_queue`, `finalize_canonical_imports_and_queue`, `queue_episode_result`.

**`src/runtime_inference/insomnia/processor.rs`**
- **W**: `process_claimed_insomnia_episode`, `process_claimed_insomnia_episode_routed`, `claimed_episode_input`.

**`src/runtime_inference/insomnia/processor/application.rs`**
- **Q**: `hydrate_grouped_import_completion_bodies`, `replay_grouped_import_completion`.

**`src/runtime_inference/insomnia/runtime_step.rs`**
- **W**: `claim_runtime_insomnia`, `hydrate_runtime_insomnia_evidence`, `prepare_runtime_insomnia_application`, `commit_runtime_insomnia_prepared`, `commit_runtime_insomnia_verified`, `fail_runtime_insomnia`.

**`src/runtime_inference/insomnia/worker.rs`**
- **T**: `drain_insomnia_backlog`, `drain_insomnia_backlog_with_progress`, `drain_insomnia_backlog_routed`, `drain_insomnia_backlog_routed_with_progress`.

**`src/runtime_inference/runtime_vector_step.rs`**
- **W**: `accept_runtime_compatibility_profile`, `prepare_runtime_memory_vector_batch`, `commit_runtime_memory_vector_batch`.

### Phylactery

**`src/facade/phylactery.rs`**
- **O**: `publish_memory`, `set_profile`, `set_memory_temporal_status`, `put_memory_routing_metadata`, `sync`.
- **Q**: `owner_id`, `owner_uuid`, `profile`, `latest_global_version`, `transaction_time_ns`, `version_at_or_before`, `memory`, `memory_retention_descriptor`, `historical_memory_revision`, `memory_lifecycle_state`, `memory_revision`, `memory_stats`, `memory_ids`, `memory_version`, `memory_body_id`, `memory_routing_metadata`.
- **W**: `tombstone_memory`.

**`src/facade/phylactery_communities.rs`**
- **O**: `set_community_name`, `publish_community_semantic_name`.
- **Q**: `community_snapshot`, `community_stats`, `community_semantic_name`, `community_semantic_names`, `community_lineage`.
- **S**: `refresh_communities_leiden`.

**`src/facade/phylactery_compatibility_profiles.rs`**
- **O**: `establish_compatibility_profile`, `retire_compatibility_profile`.
- **Q**: `compatibility_profile`, `compatibility_profiles`, `compatibility_profile_is_retired`, `active_memory_compatibility_profile`, `active_memory_compatibility_profiles`, `compatibility_profile_stats`.
- **S**: `verify_compatibility_endpoint`.

**`src/facade/phylactery_dream_candidates.rs`**
- **S**: `dream_memory_context`, `dream_candidates`, `dream_temporal_analysis`.

**`src/facade/phylactery_dream_lifecycle.rs`**
- **W**: `reconcile_dream_lifecycle`, `revise_dream_lifecycle`.

**`src/facade/phylactery_dream_publisher.rs`**
- **W**: `publish_dream_pair`.

**`src/facade/phylactery_ego.rs`**
- **O**: `put_ego_identity`, `create_ego_identity`, `update_ego_identity`, `activate_ego_identity`, `delete_ego_identity`, `put_ego_personality`, `create_ego_anchor`, `update_ego_anchor`, `delete_ego_anchor`, `put_ego_web_synthesis`.
- **Q**: `ego_version`, `ego_identity`, `active_ego_identity_id`, `ego_identities`, `ego_identity_by_id`, `ego_personality`, `ego_anchors`, `ego_anchor`, `ego_web_synthesis`, `ego_anchor_lifecycle`, `historical_ego_anchor`, `ego_identity_lifecycle`, `historical_ego_identity`.

**`src/facade/phylactery_graph.rs`**
- **O**: `set_memory_relation`, `set_memory_relation_with_origin`, `set_memory_relations`, `set_memory_relations_with_origin`, `set_entity_association`, `set_semantic_relations_with_origin`.
- **Q**: `graph_version`, `memory_graph_version`, `graph_stats`, `graph_relations`, `semantic_graph_relations`, `entity_associations_for_memory`, `memories_for_entity`, `semantic_graph_neighbors`, `graph_neighbors`, `shortest_memory_path`.

**`src/facade/phylactery_history_prune.rs`**
- **M**: `prune_tombstoned_history`, `validate_prune_evidence`, `entity_retention`, `relationship_retention`, `profile_retention`.

**`src/facade/phylactery_lifecycle.rs`**
- **O**: `create`, `create_with_uuid`, `initialize`, `open`, `repair_retired_entity_references`.

**`src/facade/phylactery_memory_retrieval.rs`**
- **S**: `build_memory_retrieval_index`, `retrieve_memories_with_index`, `retrieve_memories`.

**`src/facade/phylactery_memory_source.rs`**
- **O**: `publish_memory_with_source_ref`, `publish_memory_with_source_ref_and_temporal_inference`, `backfill_memory_source_ref`.

**`src/facade/phylactery_memory_vectors.rs`**
- **O**: `put_memory_vectors`.
- **Q**: `memory_vectors`, `memory_vector_location`, `memory_vector_infos`, `memory_vector_stats`.
- **S**: `build_missing_memory_vectors`.

**`src/facade/phylactery_packed_vectors.rs`**
- **O**: `put_packed_vectors`.
- **Q**: `packed_vectors`, `packed_vector_infos`, `packed_vector_stats`.

**`src/facade/review_clocks.rs`**
- **Q**: `review_clock`.

**`src/facade/storage_reclamation.rs`**
- **M**: `reclaim_storage`.

**`src/perception/entity_principal.rs`**
- **W**: `resolve_principal_entity_mention`.

**`src/retrieval/lexical_search.rs`**
- **S**: `memory_lexical_candidates`.

**`src/retrieval/memory_search.rs`**
- **S**: `search_memories`.

**`src/rewrite/lineage.rs`**
- **Q**: `history_generation`, `rewrite_genesis`.

**`src/rewrite/plan.rs`**
- **M**: `plan_rewrite`.

**`src/runtime_inference/dream_cooldown.rs`**
- **O**: `mark_dream_processed`.
- **Q**: `dream_eligible_epoch`, `dream_was_processed`.

**`src/runtime_inference/dream_pair_history.rs`**
- **O**: `mark_dream_pair_evaluated`.
- **Q**: `dream_pair_records`.

**`src/runtime_inference/runtime_vector_step.rs`**
- **W**: `accept_runtime_compatibility_profile`, `prepare_runtime_memory_vector_batch`, `commit_runtime_memory_vector_batch`.

## Critical interpretation

- Store reads with a `&mut self` API (e.g. `memory`) can still be **Q** when mutability is needed for lazy payload access; do not infer semantic mutation from Rust mutability.
- REL turn/Memory birth, graph tombstone exclusion and Insomnia grouped completion are **W** because they cross distinct owners; lower stores retain their own codecs, proofs and acceptance.
- Freshness due notification processing is **T**, derived/rebuilt from durable owner state, not a persisted state owner.
- Maintenance is **M** and must operate through explicit exclusive typed owner APIs, never unrestricted mutable-store fields.
- Final code relocation must also cover non-inherent helper functions and validate each surprising classification against its method body. This inventory's method coverage is lexical, not compiler resolution or a migration success claim.

## Related docs

- [Final Stage 1 storage architecture](rel-phy-storage-architecture-2026-11-03.md)
- [Current implementation inventory](current-system-inventory-2026-11-03.md)
- [Storage ownership decision](decisions/0044-canonical-rel-phy-owner-session-architecture.md)

## Notes

The exact method-to-crate and free-function migration destinations are still part of the later physical dependency architecture and migration plan. Every inherent method is accounted for here, but the map is not a claim of new code compilation.
