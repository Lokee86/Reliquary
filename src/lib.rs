#[path = "archive/archive.rs"]
pub mod archive;
#[path = "freshness/mod.rs"]
pub mod freshness;
pub use freshness::{
    CommunityMembership, FreshnessBoundaryIndex, FreshnessError, FreshnessRecord, FreshnessState,
    MemoryAdjacency, PropagationError, PropagationEvent, propagate,
};
#[path = "freshness_due.rs"]
mod freshness_due;
pub use freshness_due::{
    FreshnessDue, FreshnessDueDispatcher, FreshnessRefresh, FreshnessReversal,
};
#[path = "freshness_dream_journal.rs"]
mod freshness_dream_journal;
#[path = "freshness_storage.rs"]
mod freshness_storage;
pub use freshness_dream_journal::InitialDreamPass;
#[path = "archive/archive_activity.rs"]
mod archive_activity;
#[path = "archive/archive_codec.rs"]
mod archive_codec;
#[path = "archive/archive_conversation.rs"]
mod archive_conversation;
#[path = "archive/archive_conversation_metadata.rs"]
mod archive_conversation_metadata;
#[path = "archive/archive_error.rs"]
mod archive_error;
#[path = "archive/archive_history.rs"]
mod archive_history;
#[path = "archive/archive_history_codec.rs"]
mod archive_history_codec;
#[path = "archive/archive_history_model.rs"]
mod archive_history_model;
#[path = "archive/archive_lookup.rs"]
mod archive_lookup;
#[path = "archive/archive_model.rs"]
mod archive_model;
#[path = "archive/archive_object_index.rs"]
mod archive_object_index;
#[path = "archive/archive_rebuild.rs"]
mod archive_rebuild;
#[path = "archive/archive_record_index.rs"]
mod archive_record_index;
#[path = "archive/archive_search.rs"]
mod archive_search;
#[path = "archive/archive_store.rs"]
mod archive_store;
#[path = "retrieval/archive_vector_codec.rs"]
mod archive_vector_codec;
#[path = "retrieval/archive_vector_error.rs"]
mod archive_vector_error;
#[path = "retrieval/archive_vector_model.rs"]
mod archive_vector_model;
#[path = "retrieval/archive_vector_rebuild.rs"]
mod archive_vector_rebuild;
#[path = "retrieval/archive_vector_store.rs"]
mod archive_vector_store;
#[path = "chronos/chronos.rs"]
pub mod chronos;
#[path = "chronos/chronos_absolute.rs"]
mod chronos_absolute;
#[path = "chronos/chronos_absolute_calendar.rs"]
mod chronos_absolute_calendar;
#[path = "chronos/chronos_boundary.rs"]
mod chronos_boundary;
#[path = "chronos/chronos_boundary_endpoint.rs"]
mod chronos_boundary_endpoint;
#[path = "chronos/chronos_calendar.rs"]
mod chronos_calendar;
#[path = "chronos/chronos_detection.rs"]
mod chronos_detection;
#[path = "chronos/chronos_detection_context.rs"]
mod chronos_detection_context;
#[path = "chronos/chronos_detection_explicit.rs"]
mod chronos_detection_explicit;
#[path = "chronos/chronos_detection_model.rs"]
mod chronos_detection_model;
#[path = "chronos/chronos_duration.rs"]
mod chronos_duration;
#[path = "chronos/chronos_edit_distance.rs"]
mod chronos_edit_distance;
#[path = "chronos/chronos_event_relative.rs"]
mod chronos_event_relative;
#[path = "chronos/chronos_fuzzy.rs"]
mod chronos_fuzzy;
#[path = "chronos/chronos_inference.rs"]
mod chronos_inference;
#[path = "chronos/chronos_inference_apply.rs"]
mod chronos_inference_apply;
#[path = "chronos/chronos_inference_error.rs"]
mod chronos_inference_error;
#[path = "chronos/chronos_inference_model.rs"]
mod chronos_inference_model;
#[path = "chronos/chronos_inference_verify.rs"]
mod chronos_inference_verify;
#[path = "chronos/chronos_loose_calendar.rs"]
mod chronos_loose_calendar;
#[path = "chronos/chronos_loose_explicit.rs"]
mod chronos_loose_explicit;
#[path = "chronos/chronos_loose_relative.rs"]
mod chronos_loose_relative;
#[path = "chronos/chronos_match.rs"]
mod chronos_match;
#[path = "chronos/chronos_model.rs"]
mod chronos_model;
#[path = "chronos/chronos_normalize.rs"]
mod chronos_normalize;
#[path = "chronos/chronos_number.rs"]
mod chronos_number;
#[path = "chronos/chronos_parser.rs"]
mod chronos_parser;
#[path = "chronos/chronos_processing_epoch.rs"]
mod chronos_processing_epoch;
#[path = "chronos/chronos_recurrence.rs"]
mod chronos_recurrence;
#[path = "chronos/chronos_recurrence_interval.rs"]
mod chronos_recurrence_interval;
#[path = "chronos/chronos_relative.rs"]
mod chronos_relative;
#[path = "chronos/chronos_relative_offset.rs"]
mod chronos_relative_offset;
#[path = "chronos/chronos_resolution.rs"]
mod chronos_resolution;
#[path = "chronos/chronos_resolution_model.rs"]
mod chronos_resolution_model;
#[path = "chronos/chronos_season.rs"]
mod chronos_season;
#[path = "chronos/chronos_time_of_day.rs"]
mod chronos_time_of_day;
#[path = "chronos/chronos_vocabulary.rs"]
mod chronos_vocabulary;
#[path = "semantic_graph/community_codec.rs"]
mod community_codec;
#[path = "semantic_graph/community_error.rs"]
mod community_error;
#[path = "semantic_graph/community_leiden.rs"]
mod community_leiden;
#[path = "semantic_graph/community_lineage.rs"]
mod community_lineage;
#[path = "semantic_graph/community_model.rs"]
mod community_model;
#[cfg(test)]
#[path = "semantic_graph/community_routing.rs"]
mod community_routing;
#[path = "semantic_graph/community_scan_merge.rs"]
mod community_scan_merge;
#[path = "semantic_graph/community_scan_merge_plan.rs"]
mod community_scan_merge_plan;
#[path = "semantic_graph/community_scan_merge_reduce.rs"]
mod community_scan_merge_reduce;
#[path = "semantic_graph/community_scan_merge_workers.rs"]
mod community_scan_merge_workers;
#[path = "semantic_graph/community_store.rs"]
mod community_store;
#[path = "retrieval/compatibility_profile_codec.rs"]
mod compatibility_profile_codec;
#[path = "retrieval/compatibility_profile_error.rs"]
mod compatibility_profile_error;
#[path = "retrieval/compatibility_profile_model.rs"]
mod compatibility_profile_model;
#[path = "retrieval/compatibility_profile_probe.rs"]
mod compatibility_profile_probe;
#[path = "retrieval/compatibility_profile_rebuild.rs"]
mod compatibility_profile_rebuild;
#[path = "retrieval/compatibility_profile_store.rs"]
mod compatibility_profile_store;
#[path = "retrieval/compatibility_vector.rs"]
mod compatibility_vector;
#[path = "config_security/config.rs"]
pub mod config;
#[path = "config_security/config_codec.rs"]
mod config_codec;
#[path = "config_security/config_credentials.rs"]
mod config_credentials;
#[path = "config_security/config_error.rs"]
mod config_error;
#[path = "config_security/config_io.rs"]
mod config_io;
#[path = "config_security/config_object.rs"]
mod config_object;
#[path = "runtime_inference/configured_decision.rs"]
mod configured_decision;
#[path = "runtime_inference/configured_general.rs"]
mod configured_general;
#[path = "runtime_inference/configured_runtime.rs"]
mod configured_runtime;
#[path = "container/container.rs"]
pub mod container;
#[path = "container/container_error.rs"]
mod container_error;
#[path = "container/container_version.rs"]
mod container_version;
#[path = "context_engine.rs"]
mod context_engine;
#[path = "archive/conversation_compaction_allocator.rs"]
mod conversation_compaction_allocator;
#[path = "archive/conversation_compaction_codec.rs"]
mod conversation_compaction_codec;
#[path = "archive/conversation_compaction_model.rs"]
mod conversation_compaction_model;
#[path = "archive/conversation_compaction_store.rs"]
mod conversation_compaction_store;
#[path = "archive/conversation_metadata_codec.rs"]
mod conversation_metadata_codec;
#[path = "archive/conversation_metadata_index.rs"]
mod conversation_metadata_index;
#[path = "archive/conversation_metadata_model.rs"]
mod conversation_metadata_model;
#[path = "retrieval/conversation_search.rs"]
mod conversation_search;
#[path = "retrieval/conversation_search_model.rs"]
mod conversation_search_model;
#[path = "config_security/credential.rs"]
mod credential;
#[path = "config_security/credential_codec.rs"]
mod credential_codec;
#[path = "config_security/credential_crypto.rs"]
mod credential_crypto;
#[path = "facade/cva.rs"]
pub mod cva;
#[path = "facade/cva_activity.rs"]
mod cva_activity;
#[path = "facade/cva_archive_vectors.rs"]
mod cva_archive_vectors;
#[path = "facade/cva_communities.rs"]
mod cva_communities;
#[path = "facade/cva_compatibility_profiles.rs"]
mod cva_compatibility_profiles;
#[path = "facade/cva_conversation.rs"]
mod cva_conversation;
#[path = "facade/cva_ego.rs"]
mod cva_ego;
#[path = "facade/cva_error.rs"]
mod cva_error;
#[path = "facade/cva_file_memory.rs"]
mod cva_file_memory;
#[path = "facade/cva_freshness.rs"]
mod cva_freshness;
#[path = "facade/cva_global_validation.rs"]
mod cva_global_validation;
#[path = "facade/cva_graph.rs"]
mod cva_graph;
#[path = "facade/cva_lifecycle.rs"]
mod cva_lifecycle;
pub use cva_freshness::{FreshnessEventCut, FreshnessEventProof};
#[path = "facade/cva_freshness_due.rs"]
mod cva_freshness_due;
pub use cva_freshness_due::FreshnessTransitionNotice;
#[path = "facade/cva_memory_publish.rs"]
mod cva_memory_publish;
#[path = "facade/cva_memory_retrieval.rs"]
mod cva_memory_retrieval;
#[path = "facade/cva_memory_vectors.rs"]
mod cva_memory_vectors;
#[path = "facade/cva_open_route.rs"]
mod cva_open_route;
#[path = "facade/cva_packed_vectors.rs"]
mod cva_packed_vectors;
#[path = "facade/cva_principal_repack.rs"]
mod cva_principal_repack;
#[path = "facade/cva_reconcile.rs"]
mod cva_reconcile;
#[path = "facade/cva_reconcile_archive.rs"]
mod cva_reconcile_archive;
#[path = "facade/cva_reconcile_conflict.rs"]
mod cva_reconcile_conflict;
#[path = "facade/cva_reconcile_conflict_map.rs"]
mod cva_reconcile_conflict_map;
#[path = "facade/cva_reconcile_entity.rs"]
mod cva_reconcile_entity;
#[path = "facade/cva_reconcile_error.rs"]
mod cva_reconcile_error;
#[path = "facade/cva_reconcile_freshness.rs"]
mod cva_reconcile_freshness;
#[path = "facade/cva_reconcile_graph.rs"]
mod cva_reconcile_graph;
#[path = "facade/cva_reconcile_interaction.rs"]
mod cva_reconcile_interaction;
#[path = "facade/cva_reconcile_memory.rs"]
mod cva_reconcile_memory;
#[path = "facade/cva_reconcile_promote.rs"]
mod cva_reconcile_promote;
#[path = "facade/cva_reconcile_relationship.rs"]
mod cva_reconcile_relationship;
#[path = "facade/cva_reconcile_repack.rs"]
mod cva_reconcile_repack;
#[path = "facade/cva_repack.rs"]
mod cva_repack;
#[path = "facade/cva_turn_ingest.rs"]
mod cva_turn_ingest;
#[path = "facade/cva_vector_generations.rs"]
mod cva_vector_generations;
#[path = "facade/cva_vector_recovery.rs"]
mod cva_vector_recovery;
#[path = "runtime_inference/decision_endpoint.rs"]
mod decision_endpoint;
#[path = "runtime_inference/dream_candidate_error.rs"]
mod dream_candidate_error;
#[path = "runtime_inference/dream_candidate_model.rs"]
mod dream_candidate_model;
#[path = "runtime_inference/dream_candidate_ranking.rs"]
mod dream_candidate_ranking;
#[path = "runtime_inference/dream_candidates.rs"]
mod dream_candidates;
#[path = "runtime_inference/dream_canonical.rs"]
mod dream_canonical;
#[path = "runtime_inference/dream_classifier.rs"]
mod dream_classifier;
#[path = "runtime_inference/dream_classifier_error.rs"]
mod dream_classifier_error;
#[path = "runtime_inference/dream_classifier_model.rs"]
mod dream_classifier_model;
#[path = "runtime_inference/dream_classifier_schema.rs"]
mod dream_classifier_schema;
#[path = "runtime_inference/dream_community_naming.rs"]
mod dream_community_naming;
#[path = "runtime_inference/dream_community_naming_context.rs"]
mod dream_community_naming_context;
#[path = "runtime_inference/dream_community_naming_error.rs"]
mod dream_community_naming_error;
#[path = "runtime_inference/dream_community_naming_schema.rs"]
mod dream_community_naming_schema;
#[path = "runtime_inference/dream_cooldown.rs"]
mod dream_cooldown;
#[path = "runtime_inference/dream_duplicate_index.rs"]
mod dream_duplicate_index;
#[path = "runtime_inference/dream_freshness.rs"]
mod dream_freshness;
#[path = "runtime_inference/dream_lifecycle.rs"]
mod dream_lifecycle;
#[path = "runtime_inference/dream_lifecycle_error.rs"]
mod dream_lifecycle_error;
#[path = "runtime_inference/dream_lifecycle_model.rs"]
mod dream_lifecycle_model;
#[path = "runtime_inference/dream_owner_candidates.rs"]
mod dream_owner_candidates;
#[path = "runtime_inference/dream_owner_duplicate.rs"]
mod dream_owner_duplicate;
#[path = "runtime_inference/dream_owner_publisher.rs"]
mod dream_owner_publisher;
#[path = "runtime_inference/dream_owner_vectors.rs"]
mod dream_owner_vectors;
#[path = "runtime_inference/dream_pair_context.rs"]
mod dream_pair_context;
#[path = "runtime_inference/dream_pair_history.rs"]
mod dream_pair_history;
#[path = "runtime_inference/freshness_access.rs"]
mod freshness_access;
pub use freshness_access::{AcceptedMemoryUseReceipt, EgoFreshnessCandidate, EgoMemoryFreshness};
#[path = "runtime_inference/dream_processor.rs"]
mod dream_processor;
#[path = "runtime_inference/dream_processor_error.rs"]
mod dream_processor_error;
#[path = "runtime_inference/dream_processor_frontier.rs"]
mod dream_processor_frontier;
#[path = "runtime_inference/dream_processor_model.rs"]
mod dream_processor_model;
#[path = "runtime_inference/dream_publisher.rs"]
mod dream_publisher;
#[path = "runtime_inference/dream_publisher_error.rs"]
mod dream_publisher_error;
#[path = "runtime_inference/dream_publisher_model.rs"]
mod dream_publisher_model;
#[path = "runtime_inference/dream_temporal.rs"]
mod dream_temporal;
#[path = "runtime_inference/dream_verifier.rs"]
mod dream_verifier;
#[path = "runtime_inference/dream_verifier_error.rs"]
mod dream_verifier_error;
#[path = "runtime_inference/dream_verifier_model.rs"]
mod dream_verifier_model;
#[path = "runtime_inference/dream_verifier_schema.rs"]
mod dream_verifier_schema;
#[path = "archive/echo_codec.rs"]
mod echo_codec;
#[path = "archive/echo_model.rs"]
mod echo_model;
#[path = "archive/echo_store.rs"]
mod echo_store;
#[path = "ego/ego_codec.rs"]
mod ego_codec;
#[path = "ego/ego_error.rs"]
mod ego_error;
#[path = "ego/ego_model.rs"]
mod ego_model;
#[path = "ego/ego_store.rs"]
mod ego_store;
#[path = "runtime_inference/embedding_endpoint.rs"]
mod embedding_endpoint;
#[path = "perception/entity_admission.rs"]
mod entity_admission;
#[path = "perception/entity_audit.rs"]
mod entity_audit;
#[path = "perception/entity_audit_collisions.rs"]
mod entity_audit_collisions;
#[path = "perception/entity_audit_model.rs"]
mod entity_audit_model;
#[cfg(test)]
#[path = "perception/entity_audit_tests.rs"]
mod entity_audit_tests;
#[path = "perception/entity_candidate_model.rs"]
mod entity_candidate_model;
#[path = "perception/entity_candidate_rank.rs"]
mod entity_candidate_rank;
#[path = "perception/entity_candidate_routing.rs"]
mod entity_candidate_routing;
#[cfg(test)]
#[path = "perception/entity_candidate_test_support.rs"]
mod entity_candidate_test_support;
#[cfg(test)]
#[path = "perception/entity_candidate_tests.rs"]
mod entity_candidate_tests;
#[path = "perception/entity_candidates.rs"]
mod entity_candidates;
#[path = "perception/entity_codec.rs"]
mod entity_codec;
#[cfg(test)]
#[path = "perception/entity_creation_guard_tests.rs"]
mod entity_creation_guard_tests;
#[path = "perception/entity_error.rs"]
mod entity_error;
#[path = "perception/entity_identity_guard.rs"]
mod entity_identity_guard;
#[path = "perception/entity_merge.rs"]
mod entity_merge;
#[path = "perception/entity_model.rs"]
mod entity_model;
#[path = "perception/entity_owner.rs"]
mod entity_owner;
#[path = "perception/entity_principal.rs"]
mod entity_principal;
#[cfg(test)]
#[path = "perception/entity_principal_tests.rs"]
mod entity_principal_tests;
#[path = "perception/entity_rebuild.rs"]
mod entity_rebuild;
#[path = "perception/entity_reconciliation.rs"]
mod entity_reconciliation;
#[cfg(test)]
#[path = "perception/entity_reconciliation_candidate_tests.rs"]
mod entity_reconciliation_candidate_tests;
#[path = "perception/entity_reconciliation_candidates.rs"]
mod entity_reconciliation_candidates;
#[path = "perception/entity_reconciliation_commit.rs"]
mod entity_reconciliation_commit;
#[path = "perception/entity_reconciliation_error.rs"]
mod entity_reconciliation_error;
#[path = "perception/entity_reconciliation_inference.rs"]
mod entity_reconciliation_inference;
#[path = "perception/entity_reconciliation_model.rs"]
mod entity_reconciliation_model;
#[path = "perception/entity_reconciliation_owner.rs"]
mod entity_reconciliation_owner;
#[path = "perception/entity_reconciliation_prepare.rs"]
mod entity_reconciliation_prepare;
#[path = "perception/entity_reconciliation_schema.rs"]
mod entity_reconciliation_schema;
#[path = "perception/entity_reconciliation_staged.rs"]
mod entity_reconciliation_staged;
#[path = "perception/entity_reconciliation_support.rs"]
mod entity_reconciliation_support;
#[cfg(test)]
#[path = "perception/entity_reconciliation_tests.rs"]
mod entity_reconciliation_tests;
#[path = "perception/entity_resolution_batch.rs"]
mod entity_resolution_batch;
#[cfg(test)]
#[path = "perception/entity_resolution_batch_equivalence_tests.rs"]
mod entity_resolution_batch_equivalence_tests;
#[cfg(test)]
#[path = "perception/entity_resolution_batch_failure_tests.rs"]
mod entity_resolution_batch_failure_tests;
#[cfg(test)]
#[path = "perception/entity_resolution_batch_retry_tests.rs"]
mod entity_resolution_batch_retry_tests;
#[cfg(test)]
#[path = "perception/entity_resolution_batch_tests.rs"]
mod entity_resolution_batch_tests;
#[path = "perception/entity_resolution_codec.rs"]
mod entity_resolution_codec;
#[path = "perception/entity_resolution_commit.rs"]
mod entity_resolution_commit;
#[path = "perception/entity_resolution_decision.rs"]
mod entity_resolution_decision;
#[path = "perception/entity_resolution_engine.rs"]
mod entity_resolution_engine;
#[path = "perception/entity_resolution_evaluate.rs"]
mod entity_resolution_evaluate;
#[path = "perception/entity_resolution_evidence.rs"]
mod entity_resolution_evidence;
#[cfg(test)]
#[path = "perception/entity_resolution_lifecycle_tests.rs"]
mod entity_resolution_lifecycle_tests;
#[cfg(test)]
#[path = "perception/entity_resolution_migration_tests.rs"]
mod entity_resolution_migration_tests;
#[path = "perception/entity_resolution_model.rs"]
mod entity_resolution_model;
#[path = "perception/entity_resolution_owner.rs"]
mod entity_resolution_owner;
#[path = "perception/entity_resolution_prepare.rs"]
mod entity_resolution_prepare;
#[path = "perception/entity_resolution_prepared.rs"]
mod entity_resolution_prepared;
#[path = "perception/entity_resolution_processor.rs"]
mod entity_resolution_processor;
#[path = "perception/entity_resolution_processor_support.rs"]
mod entity_resolution_processor_support;
#[cfg(test)]
#[path = "perception/entity_resolution_processor_test_support.rs"]
mod entity_resolution_processor_test_support;
#[cfg(test)]
#[path = "perception/entity_resolution_processor_tests.rs"]
mod entity_resolution_processor_tests;
#[cfg(test)]
#[path = "perception/entity_resolution_reconcile_tests.rs"]
mod entity_resolution_reconcile_tests;
#[path = "perception/entity_resolution_store.rs"]
mod entity_resolution_store;
#[cfg(test)]
#[path = "perception/entity_resolution_test_support.rs"]
mod entity_resolution_test_support;
#[cfg(test)]
#[path = "perception/entity_resolution_tests.rs"]
mod entity_resolution_tests;
#[path = "perception/entity_resolver.rs"]
mod entity_resolver;
#[path = "perception/entity_resolver_error.rs"]
mod entity_resolver_error;
#[path = "perception/entity_resolver_model.rs"]
mod entity_resolver_model;
#[cfg(test)]
#[path = "perception/entity_resolver_reason_tests.rs"]
mod entity_resolver_reason_tests;
#[path = "perception/entity_resolver_schema.rs"]
mod entity_resolver_schema;
#[path = "perception/entity_store.rs"]
mod entity_store;
#[cfg(all(test, feature = "entity-calibration"))]
#[path = "perception/entity_zero_bootstrap_calibration_support.rs"]
mod entity_zero_bootstrap_calibration_support;
#[cfg(all(test, feature = "entity-calibration"))]
#[path = "perception/entity_zero_bootstrap_calibration_tests.rs"]
mod entity_zero_bootstrap_calibration_tests;
#[path = "archive/episode_builder.rs"]
mod episode_builder;
#[path = "archive/episode_codec.rs"]
mod episode_codec;
#[path = "archive/episode_index.rs"]
mod episode_index;
#[path = "archive/episode_model.rs"]
mod episode_model;
#[path = "archive/episode_policy.rs"]
mod episode_policy;
#[path = "archive/episode_store.rs"]
mod episode_store;
#[path = "archive/file_index.rs"]
mod file_index;
#[path = "archive/file_memory_link_codec.rs"]
mod file_memory_link_codec;
#[path = "archive/file_memory_link_index.rs"]
mod file_memory_link_index;
#[path = "archive/file_memory_link_model.rs"]
mod file_memory_link_model;
#[path = "archive/file_memory_link_store.rs"]
mod file_memory_link_store;
#[path = "archive/file_model.rs"]
mod file_model;
#[path = "archive/file_store.rs"]
mod file_store;
#[path = "archive/fragment_model.rs"]
mod fragment_model;
#[path = "archive/fragment_store.rs"]
mod fragment_store;
#[path = "archive/fragmenter.rs"]
mod fragmenter;
#[path = "runtime_inference/general_endpoint.rs"]
mod general_endpoint;
#[path = "semantic_graph/graph_codec.rs"]
mod graph_codec;
#[path = "semantic_graph/graph_error.rs"]
mod graph_error;
#[path = "semantic_graph/graph_model.rs"]
mod graph_model;
#[path = "semantic_graph/graph_query.rs"]
mod graph_query;
#[path = "semantic_graph/graph_rebuild.rs"]
mod graph_rebuild;
#[path = "semantic_graph/graph_store.rs"]
mod graph_store;
#[path = "runtime_inference/insomnia.rs"]
mod insomnia;
#[path = "runtime_inference/interaction_background.rs"]
mod interaction_background;
#[path = "runtime_inference/interaction_error.rs"]
mod interaction_error;
#[path = "runtime_inference/interaction_model.rs"]
mod interaction_model;
#[path = "runtime_inference/interaction_runtime.rs"]
mod interaction_runtime;
#[path = "runtime_inference/interaction_session.rs"]
mod interaction_session;
#[path = "archive/interaction_stream.rs"]
mod interaction_stream;
#[path = "archive/interaction_stream_codec.rs"]
mod interaction_stream_codec;
#[path = "archive/interaction_stream_persistence.rs"]
mod interaction_stream_persistence;
#[path = "archive/interaction_stream_record.rs"]
mod interaction_stream_record;
#[path = "archive/interaction_stream_store.rs"]
mod interaction_stream_store;
#[path = "retrieval/lexical_index.rs"]
mod lexical_index;
#[path = "retrieval/lexical_search.rs"]
mod lexical_search;
#[path = "config_security/master_key.rs"]
mod master_key;
#[path = "config_security/master_key_entropy.rs"]
mod master_key_entropy;
#[path = "memory/memory_codec.rs"]
mod memory_codec;
#[path = "memory/memory_codec_scalar.rs"]
mod memory_codec_scalar;
#[path = "memory/memory_error.rs"]
mod memory_error;
#[path = "memory/memory_model.rs"]
mod memory_model;
#[path = "memory/memory_provenance_model.rs"]
mod memory_provenance_model;
#[path = "memory/memory_rebuild.rs"]
mod memory_rebuild;
#[path = "retrieval/memory_retrieval.rs"]
mod memory_retrieval;
#[path = "retrieval/memory_retrieval_build.rs"]
mod memory_retrieval_build;
#[path = "retrieval/memory_retrieval_error.rs"]
mod memory_retrieval_error;
#[path = "retrieval/memory_retrieval_index.rs"]
mod memory_retrieval_index;
#[path = "retrieval/memory_retrieval_model.rs"]
mod memory_retrieval_model;
#[path = "retrieval/memory_retrieval_traversal.rs"]
mod memory_retrieval_traversal;
#[path = "retrieval/memory_retrieval_vectors.rs"]
mod memory_retrieval_vectors;
#[path = "memory/memory_routing_codec.rs"]
mod memory_routing_codec;
#[path = "memory/memory_routing_model.rs"]
mod memory_routing_model;
#[path = "retrieval/memory_search.rs"]
mod memory_search;
#[path = "memory/memory_source_time.rs"]
mod memory_source_time;
#[path = "memory/memory_store.rs"]
mod memory_store;
#[path = "memory/memory_temporal_codec.rs"]
mod memory_temporal_codec;
#[path = "memory/memory_temporal_codec_kind.rs"]
mod memory_temporal_codec_kind;
#[path = "retrieval/memory_vector_codec.rs"]
mod memory_vector_codec;
#[path = "retrieval/memory_vector_error.rs"]
mod memory_vector_error;
#[path = "retrieval/memory_vector_model.rs"]
mod memory_vector_model;
#[path = "retrieval/memory_vector_rebuild.rs"]
mod memory_vector_rebuild;
#[path = "retrieval/memory_vector_store.rs"]
mod memory_vector_store;
#[path = "facade/migration.rs"]
pub mod migration;
#[path = "runtime_inference/model_auth.rs"]
mod model_auth;
#[path = "runtime_inference/model_switchboard.rs"]
mod model_switchboard;
#[path = "runtime_inference/model_switchboard_codec.rs"]
mod model_switchboard_codec;
#[path = "runtime_inference/openai_codex_device_auth.rs"]
mod openai_codex_device_auth;
#[path = "runtime_inference/openai_codex_general.rs"]
mod openai_codex_general;
#[path = "runtime_inference/openai_ready_embedding.rs"]
mod openai_ready_embedding;
#[path = "runtime_inference/openai_ready_embedding_response.rs"]
mod openai_ready_embedding_response;
#[path = "runtime_inference/openai_ready_general.rs"]
mod openai_ready_general;
#[path = "retrieval/packed_vector_codec.rs"]
mod packed_vector_codec;
#[path = "retrieval/packed_vector_error.rs"]
mod packed_vector_error;
#[path = "retrieval/packed_vector_model.rs"]
mod packed_vector_model;
#[path = "retrieval/packed_vector_rebuild.rs"]
mod packed_vector_rebuild;
#[path = "retrieval/packed_vector_store.rs"]
mod packed_vector_store;
#[path = "facade/phylactery.rs"]
pub mod phylactery;
#[path = "facade/phylactery_communities.rs"]
mod phylactery_communities;
#[path = "facade/phylactery_compatibility_profiles.rs"]
mod phylactery_compatibility_profiles;
#[path = "facade/phylactery_dream_candidates.rs"]
mod phylactery_dream_candidates;
#[path = "facade/phylactery_dream_canonical.rs"]
mod phylactery_dream_canonical;
#[path = "facade/phylactery_dream_lifecycle.rs"]
mod phylactery_dream_lifecycle;
#[path = "facade/phylactery_dream_publisher.rs"]
mod phylactery_dream_publisher;
#[path = "facade/phylactery_ego.rs"]
mod phylactery_ego;
#[path = "facade/phylactery_error.rs"]
mod phylactery_error;
#[path = "facade/phylactery_graph.rs"]
mod phylactery_graph;
#[path = "facade/phylactery_lifecycle.rs"]
mod phylactery_lifecycle;
#[path = "facade/phylactery_memory_retrieval.rs"]
mod phylactery_memory_retrieval;
#[path = "facade/phylactery_memory_source.rs"]
mod phylactery_memory_source;
#[path = "facade/phylactery_memory_vectors.rs"]
mod phylactery_memory_vectors;
#[path = "facade/phylactery_packed_vectors.rs"]
mod phylactery_packed_vectors;
#[path = "facade/phylactery_profile_codec.rs"]
mod phylactery_profile_codec;
#[path = "facade/phylactery_profile_model.rs"]
mod phylactery_profile_model;
#[path = "facade/phylactery_profile_store.rs"]
mod phylactery_profile_store;
#[path = "memory/processing_epoch_model.rs"]
mod processing_epoch_model;
#[path = "memory/processing_epoch_store.rs"]
mod processing_epoch_store;
#[path = "project_environment.rs"]
mod project_environment;
#[cfg(test)]
#[path = "project_environment_git_tests.rs"]
mod project_environment_git_tests;
#[cfg(test)]
#[path = "project_environment_lore_tests.rs"]
mod project_environment_lore_tests;
#[cfg(test)]
#[path = "project_environment_tests.rs"]
mod project_environment_tests;
#[path = "archive/project_file_binding_codec.rs"]
mod project_file_binding_codec;
#[path = "archive/project_file_binding_store.rs"]
mod project_file_binding_store;
#[path = "archive/project_history_codec.rs"]
mod project_history_codec;
#[path = "archive/project_history_model.rs"]
mod project_history_model;
#[path = "archive/project_history_store.rs"]
mod project_history_store;
#[path = "facade/rel_metadata_codec.rs"]
mod rel_metadata_codec;
#[path = "facade/rel_metadata_model.rs"]
mod rel_metadata_model;
#[path = "facade/rel_metadata_store.rs"]
mod rel_metadata_store;
#[path = "perception/relationship_codec.rs"]
mod relationship_codec;
#[path = "perception/relationship_error.rs"]
mod relationship_error;
#[path = "perception/relationship_model.rs"]
mod relationship_model;
#[path = "perception/relationship_owner.rs"]
mod relationship_owner;
#[path = "perception/relationship_rebuild.rs"]
mod relationship_rebuild;
#[path = "perception/relationship_store.rs"]
mod relationship_store;
#[path = "facade/review_clocks.rs"]
mod review_clocks;
#[path = "config_security/runtime_config.rs"]
mod runtime_config;
#[path = "runtime_inference/runtime_host.rs"]
mod runtime_host;
#[path = "runtime_inference/runtime_vector_step.rs"]
mod runtime_vector_step;
#[path = "retrieval/search.rs"]
mod search;
#[path = "retrieval/search_error.rs"]
mod search_error;
#[path = "retrieval/search_model.rs"]
mod search_model;
#[path = "retrieval/search_rank.rs"]
mod search_rank;
#[path = "semantic_graph/semantic_node_model.rs"]
mod semantic_node_model;
#[path = "retrieval/semantic_search.rs"]
mod semantic_search;
#[path = "retrieval/semantic_search_error.rs"]
mod semantic_search_error;
#[path = "retrieval/semantic_search_model.rs"]
mod semantic_search_model;
#[path = "archive/source_attachment_index.rs"]
mod source_attachment_index;
pub mod staleness;
#[path = "facade/storage_reclamation.rs"]
mod storage_reclamation;
#[path = "facade/storage_reclamation_scan.rs"]
mod storage_reclamation_scan;
#[cfg(test)]
#[path = "facade/storage_reclamation_tests.rs"]
mod storage_reclamation_tests;
#[path = "archive/turn_ingest_codec.rs"]
mod turn_ingest_codec;
#[path = "archive/turn_ingest_model.rs"]
mod turn_ingest_model;
#[path = "archive/turn_ingest_store.rs"]
mod turn_ingest_store;
#[path = "retrieval/vector_generation_codec.rs"]
mod vector_generation_codec;
#[path = "retrieval/vector_generation_error.rs"]
mod vector_generation_error;
#[path = "retrieval/vector_generation_model.rs"]
mod vector_generation_model;
#[path = "retrieval/vector_generation_rebuild.rs"]
mod vector_generation_rebuild;
#[path = "retrieval/vector_generation_store.rs"]
mod vector_generation_store;
#[path = "retrieval/vector_generation_validation.rs"]
mod vector_generation_validation;

pub use archive::Archive;
pub use archive_error::ArchiveError;
pub use archive_history_model::ArchiveRecordVersion;
pub use archive_model::{ArchiveStats, Branch, ContentId, ConversationSummary, Node, ResolvedTurn};
pub use archive_search::{
    ArchiveSearchHit, MAX_ARCHIVE_SEARCH_QUERY_BYTES, MAX_ARCHIVE_SEARCH_RESULTS,
};
pub use archive_vector_error::ArchiveVectorError;
pub use archive_vector_model::{
    ArchiveVectorId, ArchiveVectorInfo, ArchiveVectorSet, ArchiveVectorStats,
};
pub use community_error::CommunityError;
pub use community_model::{
    COMMUNITY_ALGORITHM_VERSION, COMMUNITY_LEIDEN_RESOLUTION, COMMUNITY_LEIDEN_SEED,
    COMMUNITY_NAMING_CONTRACT_VERSION, Community, CommunityId, CommunityLineageLink,
    CommunityLineageTransition, CommunitySemanticName, CommunitySemanticNameSource,
    CommunitySnapshot, CommunityStats, DEFAULT_COMMUNITY_NAMING_REPRESENTATIVES,
    DREAM_COMMUNITY_NAME_RETAIN_JACCARD_PERMILLE, MAX_COMMUNITY_SEMANTIC_NAME_BYTES,
};
pub use compatibility_profile_error::CompatibilityProfileError;
pub use compatibility_profile_model::{
    COMPATIBILITY_MIN_COSINE, COMPATIBILITY_POLICY_VERSION, COMPATIBILITY_PROBE_SUITE_VERSION,
    CompatibilityProbeReference, CompatibilityProfile, CompatibilityProfileId,
    CompatibilityProfileStats, CompatibilityReport,
};
pub use config::ReliquaryConfig;
pub use config_error::ConfigError;
pub use configured_decision::ConfiguredDecisionEndpoint;
pub use configured_general::ConfiguredGeneralEndpoint;
pub use configured_runtime::{
    ArchiveVectorBuildReport, ConfiguredInsomniaOptions, ConfiguredInsomniaReport,
    ConfiguredRuntime, ConfiguredRuntimeError, EmbeddingProbeReport, InsomniaTerminalFailure,
};
pub use container::{
    Container, ContainerError, ContainerIdentity, FileKind, FormatVersion, ObjectRef,
    ReliquaryScopeKind,
};
pub use conversation_compaction_model::{
    ConversationCompaction, ConversationCompactionError, MAX_CONVERSATION_COMPACTION_SUMMARY_BYTES,
};
pub use conversation_metadata_model::ConversationMetadata;
pub use conversation_search_model::ConversationSearchHit;
pub use credential::{Credential, CredentialError, CredentialId, CredentialsConfig, SecretString};
pub use cva::Cva;
pub use phylactery_profile_model::{MAX_PHYLACTERY_PROFILE_NAME_BYTES, PhylacteryProfile};
pub use runtime_config::RuntimeConfig;
pub use storage_reclamation::StorageReclamationReport;
pub type Reliquary = Cva;
pub use chronos_detection_model::{TemporalDetection, TemporalIndication, TemporalIndicationKind};
pub use chronos_inference::{
    CHRONOS_INFERENCE_CONTRACT_VERSION, CHRONOS_INFERENCE_SYSTEM_PROMPT, TemporalInferencer,
};
pub use chronos_inference_error::TemporalInferenceError;
pub use chronos_inference_model::{TemporalInference, TemporalInferenceResolution};
pub use chronos_model::{
    TemporalAnalysis, TemporalAnchor, TemporalApproximateDuration, TemporalClockPrecision,
    TemporalDuration, TemporalDurationApproximation, TemporalDurationRange, TemporalDurationUnit,
    TemporalEventDirection, TemporalEventRelation, TemporalFrequency, TemporalGranularity,
    TemporalInterval, TemporalMatch, TemporalMatchKind, TemporalOrigin, TemporalPattern,
    TemporalTimeOfDay, TemporalWeekday,
};
pub use chronos_model::{
    TemporalAnalysis as DreamTemporalAnalysis, TemporalAnchor as DreamTemporalAnchor,
    TemporalFrequency as DreamTemporalFrequency, TemporalGranularity as DreamTemporalGranularity,
    TemporalMatch as DreamTemporalMatch, TemporalMatchKind as DreamTemporalMatchKind,
    TemporalOrigin as DreamTemporalOrigin, TemporalPattern as DreamTemporalPattern,
    TemporalWeekday as DreamTemporalWeekday,
};
pub use chronos_resolution_model::{
    TemporalAssessment, TemporalResolution, TemporalResolutionStatus,
};
pub use context_engine::{
    COMPACTED_CONTEXT_INSTRUCTIONS, COMPACTION_INPUT_PERCENT, COMPACTION_TARGET_OF_TRIGGER_PERCENT,
    CompactionBudget, CompactionCheckpoint, CompactionError, CompactionPlan, ContextEvidence,
    ContextMessage, ContextRequest, ContextRole, ContextTokenCounter, ContextTurn, ContextView,
    DEFAULT_COMPACTION_TRIGGER_PERCENT, EvidenceKind, MAX_SUMMARY_TOKENS, MIN_RAW_TAIL_TURNS,
    RAW_TAIL_OF_TARGET_PERCENT, SUMMARY_OF_TARGET_PERCENT, budget_satisfied, checkpoint_applies,
    compaction_budget, compaction_needed, normalize_summary, plan_compaction,
    plan_session_compaction, render_assistant_content, render_assistant_content_from_echo,
    request_context, request_messages,
};
pub use cva_error::CvaError;
pub use cva_principal_repack::PrincipalBackfillRepackResult;
pub use cva_reconcile::{CvaComparison, CvaReconcileResult, CvaRelation};
pub use cva_reconcile_conflict::CvaReconcileConflict;
pub use cva_reconcile_error::CvaReconcileError;
pub use cva_repack::ProjectAttachmentRepackResult;
pub use cva_vector_recovery::DerivedVectorRecovery;
pub use decision_endpoint::{DecisionEndpoint, DecisionEndpointError, SimulatedDecisionEndpoint};
pub use dream_candidate_error::DreamCandidateError;
pub use dream_candidate_model::{
    DEFAULT_DREAM_CANDIDATE_LIMIT, DEFAULT_DREAM_LEXICAL_LIMIT, DEFAULT_DREAM_PRIOR_SEMANTIC_QUOTA,
    DEFAULT_DREAM_SEMANTIC_LIMIT, DEFAULT_DREAM_TEMPORAL_LIMIT, DreamCandidate,
    DreamCandidateConfig, DreamCandidateSet, DreamMemoryContext, MAX_DREAM_CANDIDATE_LIMIT,
};
pub use dream_classifier::DreamClassifier;
pub use dream_classifier_error::DreamClassificationError;
pub use dream_classifier_model::{
    DREAM_CLASSIFIER_CONTRACT_VERSION, DreamEvidenceSide, DreamPairClassification,
    DreamPairEvidence, DreamRelationDirection, DreamRelationKind,
};
pub use dream_classifier_schema::{DREAM_CLASSIFIER_SYSTEM_PROMPT, dream_classifier_schema};
pub use dream_community_naming::{DreamCommunityNamer, DreamCommunityNamingResult};
pub use dream_community_naming_error::DreamCommunityNamingError;
pub use dream_community_naming_schema::{
    DREAM_COMMUNITY_NAMING_SYSTEM_PROMPT, dream_community_naming_schema,
};
pub use dream_cooldown::DEFAULT_DREAM_REPROCESS_COOLDOWN_NS;
pub use dream_lifecycle_error::DreamLifecycleError;
pub use dream_lifecycle_model::DreamLifecycleResult;
pub use dream_processor::DreamProcessor;
pub use dream_processor_error::DreamProcessError;
pub use dream_processor_frontier::{
    DEFAULT_DREAM_FRONTIER_SIZE, DEFAULT_DREAM_INFERENCE_CONCURRENCY, DreamMemoryProcessOutcome,
};
pub use dream_processor_model::{DreamProcessResult, DreamProcessedPair};
pub use dream_publisher_error::DreamPublicationError;
pub use dream_publisher_model::DreamPublicationOutcome;
pub use dream_verifier::DreamVerifier;
pub use dream_verifier_error::DreamVerificationError;
pub use dream_verifier_model::{
    DREAM_VERIFIER_CONTRACT_VERSION, DreamPairVerification, DreamVerificationPolicy,
    DreamVerificationSignal, DreamVerificationVerdict,
};
pub use dream_verifier_schema::{DREAM_VERIFIER_SYSTEM_PROMPT, dream_verifier_schema};
pub use echo_model::{EchoError, EchoEvent, EchoEventKind};
pub use ego_error::EgoError;
pub use ego_model::{
    EgoAnchor, EgoAnchorId, EgoAnchorPriority, EgoIdentity, EgoIdentityId, EgoPersonality,
    EgoWebSynthesis, MAX_EGO_ANCHOR_CHARS,
};
pub use embedding_endpoint::{
    EmbeddingEndpoint, EmbeddingEndpointError, EmbeddingMode, SimulatedEmbeddingEndpoint,
    VectorNormalization,
};
pub use entity_admission::MAX_ENTITY_ADMISSION_CONTEXT_MEMORIES;
pub use entity_audit_model::{
    EntityAuditCollision, EntityAuditEntity, EntityAuditReport, EntityAuditResolutionViolation,
};
pub use entity_candidate_model::{
    DEFAULT_ENTITY_CANDIDATE_GRAPH_NEIGHBORS, DEFAULT_ENTITY_CANDIDATE_LEXICAL_MEMORIES,
    DEFAULT_ENTITY_CANDIDATE_SUPPORT_MEMORIES, EntityCandidate, EntityCandidateConfig,
    EntityCandidateError, EntityCandidateSet, MAX_ENTITY_ADMISSION_SURFACE_MEMORIES,
    MAX_ENTITY_CANDIDATE_CONTEXT_TERMS, MAX_ENTITY_CANDIDATE_GRAPH_NEIGHBORS,
    MAX_ENTITY_CANDIDATE_LEXICAL_MEMORIES, MAX_ENTITY_CANDIDATE_SUPPORT_MEMORIES,
    MAX_ENTITY_CANDIDATE_SURFACE_MATCHES,
};
pub use entity_error::EntityError;
pub use entity_model::{
    Entity, EntityDraft, EntityGlobalId, EntityId, EntityMergeOutcome, EntityRef, EntityStats,
    MAX_ENTITY_ALIASES, MAX_ENTITY_KIND_BYTES, MAX_ENTITY_NAME_BYTES, MAX_ENTITY_SUMMARY_BYTES,
};
pub use entity_reconciliation_error::EntityReconciliationError;
pub use entity_reconciliation_model::{
    DEFAULT_ENTITY_RECONCILIATION_PAIR_LIMIT, DEFAULT_ENTITY_RECONCILIATION_ROUNDS,
    EntityReconciliationCandidate, EntityReconciliationRelation, EntityReconciliationReport,
};
pub use entity_resolution_batch::EntityResolutionBatchOutcome;
pub use entity_resolution_engine::EntityResolutionEngine;
pub use entity_resolution_model::{
    DEFAULT_ENTITY_RESOLUTION_DORMANT_TTL_NS, DEFAULT_ENTITY_RESOLUTION_PENDING_TTL_NS,
    EntityResolutionCompaction, EntityResolutionDormant, EntityResolutionPending,
    EntityResolutionReason, MAX_ENTITY_RESOLUTION_CANDIDATES, MemoryEntityMentionKey,
    MemoryEntityResolution, MemoryEntityResolutionStatus,
};
pub(crate) use entity_resolution_prepared::{
    EntityResolutionEvaluation, EntityResolutionPreparation, EntityResolutionPrepared,
};
pub use entity_resolver::{
    EntityResolver, MAX_ENTITY_RESOLVER_EVIDENCE_MEMORIES, MAX_ENTITY_RESOLVER_EVIDENCE_TEXT_BYTES,
};
pub use entity_resolver_error::EntityResolverError;
pub use entity_resolver_model::{
    EntityAdmissionDecision, EntityAdmissionOutput, EntityMaterialization,
    EntityResolutionDecision, EntityResolutionOutcome, EntityResolverOutput,
};
pub use entity_resolver_schema::{
    ENTITY_ADMISSION_CONTRACT_VERSION, ENTITY_ADMISSION_SYSTEM_PROMPT,
    ENTITY_MATERIALIZATION_CONTRACT_VERSION, ENTITY_MATERIALIZATION_SYSTEM_PROMPT,
    ENTITY_RESOLVER_CONTRACT_VERSION, ENTITY_RESOLVER_SYSTEM_PROMPT, entity_admission_schema,
    entity_materialization_schema, entity_resolver_schema,
};
pub use episode_model::{
    DEFAULT_EPISODE_MAX_INPUT_BYTES, Episode, EpisodeBoundary, EpisodeBuildResult, EpisodeConfig,
    EpisodeId, EpisodeOrigin,
};
pub use episode_policy::{DEFAULT_EPISODE_INACTIVITY_NS, EpisodePolicy};
pub use file_memory_link_model::FileMemoryLink;
pub use file_model::{FileId, FileSearchHit, StoredFile};
pub use fragment_model::{
    DEFAULT_FRAGMENT_OVERLAP, DEFAULT_FRAGMENT_TURNS, Fragment, FragmentConfig, FragmentId,
};
pub use general_endpoint::{GeneralEndpoint, GeneralEndpointError, SimulatedGeneralEndpoint};
pub use graph_error::GraphError;
pub(crate) use graph_model::GraphNodeRecord;
pub use graph_model::{
    GraphDirection, GraphNeighbor, GraphRelation, GraphRelationChange, GraphRelationKind,
    GraphRelationOrigin, GraphStats, MemoryGraphPath, SemanticGraphNeighbor, SemanticGraphRelation,
    SemanticGraphRelationChange, SemanticGraphRelationKind,
};
#[cfg(feature = "entity-calibration")]
pub use insomnia::enrich_entity_calibration;
pub use insomnia::{
    DEFAULT_INSOMNIA_BACKPRESSURE_DELAY_NS, DEFAULT_INSOMNIA_LEASE_NS,
    DEFAULT_INSOMNIA_MAX_ATTEMPTS, DEFAULT_INSOMNIA_POLL_NS,
    DEFAULT_INSOMNIA_PROGRESS_INTERVAL_SECS, DEFAULT_INSOMNIA_RETRY_DELAY_NS,
    DEFAULT_INSOMNIA_SCOPE, DEFAULT_INSOMNIA_WORKERS, EpisodeSchedulingResult,
    INSOMNIA_EXTRACTOR_CONTRACT_VERSION, INSOMNIA_OWNERSHIP_SYSTEM_PROMPT, INSOMNIA_SYSTEM_PROMPT,
    InsomniaAttempt, InsomniaCandidate, InsomniaDrainResult, InsomniaError, InsomniaEvidenceResult,
    InsomniaEvidenceTurn, InsomniaExtraction, InsomniaExtractionError, InsomniaExtractor,
    InsomniaLeaseToken, InsomniaOwnership, InsomniaPriority, InsomniaProcessError,
    InsomniaProcessResult, InsomniaProgressEvent, InsomniaProgressReporter, InsomniaRejection,
    InsomniaRoutingMetadata, InsomniaSemanticStage, InsomniaStats, InsomniaWork, InsomniaWorkState,
    InsomniaWorkerConfig, InsomniaWorkerError, MAX_INSOMNIA_CANDIDATES,
    MAX_INSOMNIA_EVIDENCE_BYTES, MAX_INSOMNIA_EVIDENCE_REQUESTS, MAX_INSOMNIA_EVIDENCE_TURNS,
    MAX_INSOMNIA_WORKERS, insomnia_schema,
};
pub use interaction_background::{
    DEFAULT_RUNTIME_DREAM_BATCH, RuntimeBackgroundConfig, RuntimeBackgroundError,
    RuntimeBackgroundResult, RuntimeDreamCompletion, RuntimeDreamFailure,
};
pub use interaction_error::InteractionError;
pub use interaction_model::{
    InteractionAttachment, InteractionRole, InteractionSession, InteractionTurn,
};
pub use interaction_runtime::{InteractionCompletion, InteractionReceipt, InteractionRuntime};
pub use interaction_stream_record::{
    InteractionStreamRecord, InteractionStreamStatus, InteractionTurnStatus,
    ResolvedInteractionTurn,
};
pub use lodestone_packed::{PackedVectors, ScalarType, VectorSchema};
pub use master_key::{
    JsonMasterKeyStore, MASTER_KEY_BYTES, MasterKey, MasterKeyError, MasterKeyStore,
};
pub use memory_error::MemoryError;
pub use memory_model::{
    Memory, MemoryBodyId, MemoryDraft, MemoryId, MemoryRef, MemoryRevisionId, MemorySourceRef,
    MemoryStats, MemoryTemporalInference,
};
pub use memory_provenance_model::MemoryProvenance;
pub use memory_retrieval_error::MemoryRetrievalError;
pub use memory_retrieval_model::{
    DEFAULT_MEMORY_RETRIEVAL_BUDGET, DEFAULT_MEMORY_RETRIEVAL_COMMUNITIES,
    DEFAULT_MEMORY_RETRIEVAL_MAX_DEPTH, DEFAULT_MEMORY_RETRIEVAL_SEEDS,
    DEFAULT_MEMORY_RETRIEVAL_SUBCENTROIDS, MemoryRetrievalConfig, MemoryRetrievalHit,
    MemoryRetrievalIndex, MemoryRetrievalMode, MemoryRetrievalResult,
};
pub use memory_routing_model::{
    MAX_MEMORY_ENTITY_MENTIONS, MAX_MEMORY_ROUTING_TEXT_BYTES, MemoryEntityMention,
    MemoryRoutingMetadata, MemoryTextField,
};
pub use memory_search::{
    MAX_MEMORY_LEXICAL_SEARCH_QUERY_BYTES, MAX_MEMORY_LEXICAL_SEARCH_RESULTS,
    MemoryLexicalSearchHit,
};
pub use memory_vector_error::MemoryVectorError;
pub use memory_vector_model::{
    MemoryVectorBuildResult, MemoryVectorId, MemoryVectorInfo, MemoryVectorLocation,
    MemoryVectorSet, MemoryVectorStats,
};
pub use migration::{MigrationError, MigrationResult, migrate_file};
pub use model_auth::ModelRequestAuth;
pub use model_switchboard::{
    DecisionModelEndpoint, EmbeddingModelEndpoint, GeneralModelEndpoint, ModelAuthKind,
    ModelCapability, ModelProvider, ModelReasoningEffort, ModelSwitchboard, ModelSwitchboardConfig,
};
pub use openai_codex_device_auth::{
    OPENAI_CODEX_AUTH_ISSUER, OPENAI_CODEX_DEVICE_LOGIN_TIMEOUT_SECS, OPENAI_CODEX_OAUTH_CLIENT_ID,
    OpenAiCodexDeviceAuth, OpenAiCodexDeviceAuthError, OpenAiCodexDeviceCode,
};
pub use openai_codex_general::{
    OPENAI_CODEX_COMPAT_VERSION, OPENAI_CODEX_RESPONSES_URL, OpenAiCodexGeneralEndpoint,
};
pub use openai_ready_embedding::{
    DEFAULT_REMOTE_EMBEDDING_BATCH_SIZE, DEFAULT_REMOTE_EMBEDDING_CONCURRENCY,
    OpenAiReadyEmbeddingEndpoint,
};
pub use openai_ready_general::OpenAiReadyGeneralEndpoint;
pub use packed_vector_error::PackedVectorError;
pub use packed_vector_model::{PackedVectorId, PackedVectorInfo, PackedVectorStats};
pub use phylactery::Phylactery;
pub use phylactery_error::PhylacteryError;
pub use project_environment::{ProjectAdoptionSource, ProjectRepositoryMode};
pub use project_history_model::{
    ProjectFileRef, ProjectRepositoryKind, ProjectRepositoryManagement, ProjectRepositoryRef,
    ProjectRevisionCorrelation, ProjectRevisionRef, RelSemanticCut,
};
pub use rel_metadata_model::{
    MAX_REL_DEPENDENCIES, MAX_REL_DEPENDENCY_ID_BYTES, MAX_REL_TYPE_LABEL_BYTES, RelMetadata,
};
pub use relationship_error::RelationshipError;
pub use relationship_model::{
    MAX_RELATIONSHIP_EVIDENCE, MAX_RELATIONSHIP_KIND_BYTES, MAX_RELATIONSHIP_OWNER_ID_BYTES,
    MAX_RELATIONSHIP_PARTICIPANTS, MAX_RELATIONSHIP_ROLE_BYTES, MAX_RELATIONSHIP_SUMMARY_BYTES,
    Relationship, RelationshipDraft, RelationshipId, RelationshipParticipant, RelationshipStats,
};
pub use runtime_host::memory_search::{
    DEFAULT_MEMORY_SEARCH_DEPTH, DEFAULT_MEMORY_SEARCH_RESULTS, MAX_MEMORY_SEARCH_DEPTH,
    MAX_MEMORY_SEARCH_QUERY_BYTES, MAX_MEMORY_SEARCH_RESULTS, MemorySearchItem, MemorySearchLane,
    MemorySearchResult, VisibleMemorySearchOwner, VisibleMemorySearchResult,
};
pub use runtime_host::status::RuntimeBackgroundStatus;
pub use runtime_host::{
    DEFAULT_ARCHIVE_SEARCH_RESULTS, RelReconciliationCandidateReport, RelReconciliationReport,
    ReliquaryRuntimeHost, ReliquaryRuntimeHostError, ReliquaryRuntimeRoutes, RuntimeEchoSource,
    RuntimeEchoTurn, RuntimeKnowledgeProvenanceResolution, RuntimeKnowledgeState,
    RuntimeSemanticOwner, RuntimeSemanticOwnerKind, VisibleArchiveSearchOwner,
    VisibleArchiveSearchResult,
};
pub use search_error::SearchError;
pub use search_model::{
    DEFAULT_LEXICAL_WEIGHT, DEFAULT_SEARCH_CANDIDATE_LIMIT, DEFAULT_SEARCH_RESULT_LIMIT,
    DEFAULT_SEMANTIC_WEIGHT, RetrievalConfig, SearchCandidate,
};
pub use semantic_node_model::{SemanticNodeKind, SemanticNodeRef};
pub use semantic_search_error::SemanticSearchError;
pub use semantic_search_model::{MAX_SEMANTIC_SEARCH_LIMIT, SemanticSearchHit};
pub use turn_ingest_model::{IncomingAttachment, IncomingTurn, IngestedTurn};
pub use vector_generation_error::VectorGenerationError;
pub use vector_generation_model::{VectorGeneration, VectorGenerationId, VectorGenerationStats};

#[cfg(test)]
#[path = "archive/archive_inventory_tests.rs"]
mod archive_inventory_tests;
#[cfg(test)]
#[path = "archive/archive_search_tests.rs"]
mod archive_search_tests;
#[cfg(test)]
#[path = "archive/archive_tests.rs"]
mod archive_tests;
#[cfg(test)]
#[path = "retrieval/archive_vector_tests.rs"]
mod archive_vector_tests;
#[cfg(test)]
#[path = "chronos/chronos_boundary_tests.rs"]
mod chronos_boundary_tests;
#[cfg(test)]
#[path = "chronos/chronos_calendar_language_tests.rs"]
mod chronos_calendar_language_tests;
#[cfg(test)]
#[path = "chronos/chronos_detection_tests.rs"]
mod chronos_detection_tests;
#[cfg(test)]
#[path = "chronos/chronos_duration_recurrence_tests.rs"]
mod chronos_duration_recurrence_tests;
#[cfg(test)]
#[path = "chronos/chronos_duration_uncertainty_tests.rs"]
mod chronos_duration_uncertainty_tests;
#[cfg(test)]
#[path = "chronos/chronos_event_relative_tests.rs"]
mod chronos_event_relative_tests;
#[cfg(test)]
#[path = "chronos/chronos_inference_tests.rs"]
mod chronos_inference_tests;
#[cfg(test)]
#[path = "chronos/chronos_processing_epoch_tests.rs"]
mod chronos_processing_epoch_tests;
#[cfg(test)]
#[path = "chronos/chronos_relative_tests.rs"]
mod chronos_relative_tests;
#[cfg(test)]
#[path = "chronos/chronos_resolution_tests.rs"]
mod chronos_resolution_tests;
#[cfg(test)]
#[path = "chronos/chronos_tests.rs"]
mod chronos_tests;
#[cfg(test)]
#[path = "chronos/chronos_time_of_day_tests.rs"]
mod chronos_time_of_day_tests;
#[cfg(all(test, feature = "community-benchmarks"))]
#[path = "semantic_graph/community_end_to_end_bench.rs"]
mod community_end_to_end_bench;
#[cfg(all(test, feature = "community-benchmarks"))]
#[path = "semantic_graph/community_end_to_end_grid_bench.rs"]
mod community_end_to_end_grid_bench;
#[cfg(all(test, feature = "community-benchmarks"))]
#[path = "semantic_graph/community_lexical_escape_bench.rs"]
mod community_lexical_escape_bench;
#[cfg(test)]
#[path = "semantic_graph/community_lineage_tests.rs"]
mod community_lineage_tests;
#[cfg(all(test, feature = "community-benchmarks"))]
#[path = "semantic_graph/community_retrieval_bench.rs"]
mod community_retrieval_bench;
#[cfg(all(test, feature = "community-benchmarks"))]
#[path = "semantic_graph/community_routing_bench.rs"]
mod community_routing_bench;
#[cfg(all(test, feature = "community-benchmarks"))]
#[path = "semantic_graph/community_routing_bench_fixture.rs"]
mod community_routing_bench_fixture;
#[cfg(all(test, feature = "community-benchmarks"))]
#[path = "semantic_graph/community_routing_bench_support.rs"]
mod community_routing_bench_support;
#[cfg(all(test, feature = "community-benchmarks"))]
#[path = "semantic_graph/community_routing_cached_bench.rs"]
mod community_routing_cached_bench;
#[cfg(all(test, feature = "community-benchmarks"))]
#[path = "semantic_graph/community_routing_holdout_bench.rs"]
mod community_routing_holdout_bench;
#[cfg(all(test, feature = "community-benchmarks"))]
#[path = "semantic_graph/community_routing_scale_bench.rs"]
mod community_routing_scale_bench;
#[cfg(all(test, feature = "community-benchmarks"))]
#[path = "semantic_graph/community_routing_score_bench.rs"]
mod community_routing_score_bench;
#[cfg(test)]
#[path = "semantic_graph/community_routing_tests.rs"]
mod community_routing_tests;
#[cfg(all(test, feature = "community-benchmarks"))]
#[path = "semantic_graph/community_scan_merge_bench.rs"]
mod community_scan_merge_bench;
#[cfg(test)]
#[path = "semantic_graph/community_scan_merge_tests.rs"]
mod community_scan_merge_tests;
#[cfg(all(test, feature = "community-benchmarks"))]
#[path = "semantic_graph/community_subcentroid_routing.rs"]
mod community_subcentroid_routing;
#[cfg(test)]
#[path = "semantic_graph/community_test_support.rs"]
mod community_test_support;
#[cfg(test)]
#[path = "semantic_graph/community_tests.rs"]
mod community_tests;
#[cfg(all(test, feature = "community-benchmarks"))]
#[path = "semantic_graph/community_traversal_bench.rs"]
mod community_traversal_bench;
#[cfg(all(test, feature = "community-benchmarks"))]
#[path = "semantic_graph/community_traversal_bench_fixture.rs"]
mod community_traversal_bench_fixture;
#[cfg(all(test, feature = "community-benchmarks"))]
#[path = "semantic_graph/community_traversal_bench_support.rs"]
mod community_traversal_bench_support;
#[cfg(test)]
#[path = "retrieval/compatibility_profile_tests.rs"]
mod compatibility_profile_tests;
#[cfg(test)]
#[path = "config_security/config_tests.rs"]
mod config_tests;
#[cfg(test)]
#[path = "container/container_tests.rs"]
mod container_tests;
#[cfg(test)]
#[path = "archive/conversation_compaction_tests.rs"]
mod conversation_compaction_tests;
#[cfg(test)]
#[path = "retrieval/conversation_search_tests.rs"]
mod conversation_search_tests;
#[cfg(test)]
#[path = "archive/conversation_tests.rs"]
mod conversation_tests;
#[cfg(test)]
#[path = "config_security/credential_tests.rs"]
mod credential_tests;
#[cfg(test)]
#[path = "facade/cva_reconcile_conflict_tests.rs"]
mod cva_reconcile_conflict_tests;
#[cfg(test)]
#[path = "facade/cva_reconcile_derived_tests.rs"]
mod cva_reconcile_derived_tests;
#[cfg(test)]
#[path = "facade/cva_reconcile_graph_policy_tests.rs"]
mod cva_reconcile_graph_policy_tests;
#[cfg(test)]
#[path = "facade/cva_reconcile_graph_test_support.rs"]
mod cva_reconcile_graph_test_support;
#[cfg(test)]
#[path = "facade/cva_reconcile_graph_tests.rs"]
mod cva_reconcile_graph_tests;
#[cfg(test)]
#[path = "facade/cva_reconcile_grouped_tests.rs"]
mod cva_reconcile_grouped_tests;
#[cfg(test)]
#[path = "facade/cva_reconcile_guard_tests.rs"]
mod cva_reconcile_guard_tests;
#[cfg(test)]
#[path = "facade/cva_reconcile_link_tests.rs"]
mod cva_reconcile_link_tests;
#[cfg(test)]
#[path = "facade/cva_reconcile_memory_tests.rs"]
mod cva_reconcile_memory_tests;
#[cfg(test)]
#[path = "facade/cva_reconcile_merge_tests.rs"]
mod cva_reconcile_merge_tests;
#[cfg(test)]
#[path = "facade/cva_reconcile_promotion_tests.rs"]
mod cva_reconcile_promotion_tests;
#[cfg(test)]
#[path = "facade/cva_reconcile_tests.rs"]
mod cva_reconcile_tests;
#[cfg(test)]
#[path = "runtime_inference/dream_candidate_test_support.rs"]
mod dream_candidate_test_support;
#[cfg(test)]
#[path = "runtime_inference/dream_candidate_tests.rs"]
mod dream_candidate_tests;
#[cfg(test)]
#[path = "runtime_inference/dream_canonical_test_support.rs"]
mod dream_canonical_test_support;
#[cfg(test)]
#[path = "runtime_inference/dream_canonical_tests.rs"]
mod dream_canonical_tests;
#[cfg(test)]
#[path = "runtime_inference/dream_classifier_tests.rs"]
mod dream_classifier_tests;
#[cfg(test)]
#[path = "runtime_inference/dream_community_naming_tests.rs"]
mod dream_community_naming_tests;
#[cfg(test)]
#[path = "runtime_inference/dream_cooldown_tests.rs"]
mod dream_cooldown_tests;
#[cfg(test)]
#[path = "runtime_inference/dream_duplicate_tests.rs"]
mod dream_duplicate_tests;
#[cfg(test)]
#[path = "runtime_inference/dream_lifecycle_tests.rs"]
mod dream_lifecycle_tests;
#[cfg(test)]
#[path = "runtime_inference/dream_processor_tests.rs"]
mod dream_processor_tests;
#[cfg(test)]
#[path = "runtime_inference/dream_publisher_tests.rs"]
mod dream_publisher_tests;
#[cfg(test)]
#[path = "runtime_inference/dream_temporal_tests.rs"]
mod dream_temporal_tests;
#[cfg(test)]
#[path = "runtime_inference/dream_verifier_tests.rs"]
mod dream_verifier_tests;
#[cfg(test)]
#[path = "archive/echo_tests.rs"]
mod echo_tests;
#[cfg(test)]
#[path = "ego/ego_tests.rs"]
mod ego_tests;
#[cfg(test)]
#[path = "perception/entity_resolution_wake_tests.rs"]
mod entity_resolution_wake_tests;
#[cfg(test)]
#[path = "perception/entity_tests.rs"]
mod entity_tests;
#[cfg(test)]
#[path = "archive/episode_tests.rs"]
mod episode_tests;
#[cfg(test)]
#[path = "archive/file_memory_link_tests.rs"]
mod file_memory_link_tests;
#[cfg(test)]
#[path = "archive/file_tests.rs"]
mod file_tests;
#[cfg(test)]
#[path = "archive/fragment_tests.rs"]
mod fragment_tests;
#[cfg(test)]
#[path = "semantic_graph/graph_codec_tests.rs"]
mod graph_codec_tests;
#[cfg(test)]
#[path = "semantic_graph/graph_tests.rs"]
mod graph_tests;
#[cfg(test)]
#[path = "archive/history_tests.rs"]
mod history_tests;
#[cfg(test)]
#[path = "runtime_inference/interaction_background_tests.rs"]
mod interaction_background_tests;
#[cfg(test)]
#[path = "runtime_inference/interaction_runtime_tests.rs"]
mod interaction_runtime_tests;
#[cfg(test)]
#[path = "runtime_inference/interaction_session_tests.rs"]
mod interaction_session_tests;
#[cfg(test)]
#[path = "retrieval/lexical_index_tests.rs"]
mod lexical_index_tests;
#[cfg(test)]
#[path = "config_security/master_key_tests.rs"]
mod master_key_tests;
#[cfg(test)]
#[path = "memory/memory_codec_tests.rs"]
mod memory_codec_tests;
#[cfg(test)]
#[path = "retrieval/memory_retrieval_stale_tests.rs"]
mod memory_retrieval_stale_tests;
#[cfg(test)]
#[path = "retrieval/memory_retrieval_tests.rs"]
mod memory_retrieval_tests;
#[cfg(test)]
#[path = "memory/memory_routing_tests.rs"]
mod memory_routing_tests;
#[cfg(test)]
#[path = "memory/memory_source_time_tests.rs"]
mod memory_source_time_tests;
#[cfg(test)]
#[path = "memory/memory_temporal_inference_tests.rs"]
mod memory_temporal_inference_tests;
#[cfg(test)]
#[path = "memory/memory_tests.rs"]
mod memory_tests;
#[cfg(test)]
#[path = "retrieval/memory_vector_tests.rs"]
mod memory_vector_tests;
#[cfg(test)]
#[path = "facade/migration_tests.rs"]
mod migration_tests;
#[cfg(test)]
#[path = "runtime_inference/model_switchboard_tests.rs"]
mod model_switchboard_tests;
#[cfg(test)]
#[path = "runtime_inference/openai_codex_device_auth_tests.rs"]
mod openai_codex_device_auth_tests;
#[cfg(test)]
#[path = "runtime_inference/openai_codex_general_tests.rs"]
mod openai_codex_general_tests;
#[cfg(test)]
#[path = "runtime_inference/openai_ready_embedding_tests.rs"]
mod openai_ready_embedding_tests;
#[cfg(test)]
#[path = "runtime_inference/openai_ready_general_tests.rs"]
mod openai_ready_general_tests;
#[cfg(test)]
#[path = "retrieval/packed_vector_tests.rs"]
mod packed_vector_tests;
#[cfg(test)]
#[path = "facade/phylactery_dream_tests.rs"]
mod phylactery_dream_tests;
#[cfg(test)]
#[path = "facade/phylactery_tests.rs"]
mod phylactery_tests;
#[cfg(test)]
#[path = "memory/processing_epoch_model_tests.rs"]
mod processing_epoch_model_tests;
#[cfg(test)]
#[path = "memory/processing_epoch_store_tests.rs"]
mod processing_epoch_store_tests;
#[cfg(test)]
#[path = "archive/project_file_attachment_tests.rs"]
mod project_file_attachment_tests;
#[cfg(test)]
#[path = "archive/project_history_reconcile_tests.rs"]
mod project_history_reconcile_tests;
#[cfg(test)]
#[path = "archive/project_history_tests.rs"]
mod project_history_tests;
#[cfg(test)]
#[path = "perception/relationship_maintenance_tests.rs"]
mod relationship_maintenance_tests;
#[cfg(test)]
#[path = "perception/relationship_test_support.rs"]
mod relationship_test_support;
#[cfg(test)]
#[path = "perception/relationship_tests.rs"]
mod relationship_tests;
#[cfg(test)]
#[path = "facade/reliquary_tests.rs"]
mod reliquary_tests;
#[cfg(test)]
#[path = "runtime_inference/runtime_host_backpressure_tests.rs"]
mod runtime_host_backpressure_tests;
#[cfg(test)]
#[path = "runtime_inference/runtime_host_knowledge_tests.rs"]
mod runtime_host_knowledge_tests;
#[cfg(test)]
#[path = "runtime_inference/runtime_host_perception_test_support.rs"]
mod runtime_host_perception_test_support;
#[cfg(test)]
#[path = "runtime_inference/runtime_host_perception_tests.rs"]
mod runtime_host_perception_tests;
#[cfg(test)]
#[path = "runtime_inference/runtime_host_reconciliation_tests.rs"]
mod runtime_host_reconciliation_tests;
#[cfg(test)]
#[path = "runtime_inference/runtime_host_route_tests.rs"]
mod runtime_host_route_tests;
#[cfg(test)]
#[path = "runtime_inference/runtime_host_semantic_access_tests.rs"]
mod runtime_host_semantic_access_tests;
#[cfg(test)]
#[path = "runtime_inference/runtime_host_session_tests.rs"]
mod runtime_host_session_tests;
#[cfg(test)]
#[path = "runtime_inference/runtime_host_test_support.rs"]
mod runtime_host_test_support;
#[cfg(test)]
#[path = "runtime_inference/runtime_host_tests.rs"]
mod runtime_host_tests;
#[cfg(test)]
#[path = "runtime_inference/runtime_host_topology_tests.rs"]
mod runtime_host_topology_tests;
#[cfg(test)]
#[path = "runtime_inference/runtime_host_vector_tests.rs"]
mod runtime_host_vector_tests;
#[cfg(test)]
#[path = "retrieval/search_policy_tests.rs"]
mod search_policy_tests;
#[cfg(test)]
#[path = "retrieval/search_tests.rs"]
mod search_tests;
#[cfg(test)]
#[path = "semantic_graph/semantic_graph_reconcile_tests.rs"]
mod semantic_graph_reconcile_tests;
#[cfg(test)]
#[path = "semantic_graph/semantic_graph_runtime_host_tests.rs"]
mod semantic_graph_runtime_host_tests;
#[cfg(test)]
#[path = "semantic_graph/semantic_graph_tests.rs"]
mod semantic_graph_tests;
#[cfg(test)]
#[path = "retrieval/semantic_search_tests.rs"]
mod semantic_search_tests;
#[cfg(test)]
#[path = "container/transaction_time_tests.rs"]
mod transaction_time_tests;
#[cfg(test)]
#[path = "archive/turn_ingest_tests.rs"]
mod turn_ingest_tests;
#[cfg(test)]
#[path = "retrieval/vector_generation_tests.rs"]
mod vector_generation_tests;
#[cfg(test)]
#[path = "retrieval/vector_generation_validation_tests.rs"]
mod vector_generation_validation_tests;
