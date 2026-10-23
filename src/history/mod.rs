//! Read-only historical cuts, owner proofs and reconstruction.
mod source_digest;
pub(crate) use source_digest::{owner_history_digest, validate_source_history};
mod frontier;
pub(crate) use frontier::frontier_end;
mod freshness_base;
pub(crate) use freshness_base::{
    ExternalBirthFacts, HistoricalFreshnessBaseCut, HistoricalFreshnessBaseError,
    historical_freshness_base,
};
pub(crate) mod cuts;
mod retained_evidence;
pub(crate) use retained_evidence::historical_owner_evidence_digest;

pub(crate) mod community_proof;
pub(crate) mod graph_proof;
pub(crate) mod manifest_encoding;
pub(crate) mod manifest_proof;
pub(crate) mod source_proof;
#[cfg(test)]
mod source_snapshot_tests;
pub(crate) mod vector_proof;

pub(crate) mod memory_retention;

pub(crate) mod archive_retention;

mod phylactery_prune;
pub(crate) use phylactery_prune::{
    entity_retention as phylactery_entity_retention,
    profile_retention as phylactery_profile_retention,
    relationship_retention as phylactery_relationship_retention,
    validate_phylactery_prune_evidence,
};

mod completion_proof;
pub(crate) mod reliquary_description_retention;
pub(crate) mod reliquary_vector_retention;
pub(crate) use completion_proof::{completion_epochs, validate_import_binding};
mod epoch_scope_proof;
pub(crate) use epoch_scope_proof::{
    PreparedSemanticImportScope, episode_matches_source, prepare_semantic_import_scope,
    validate_semantic_import_births,
};

mod import_epoch_scope;
pub(crate) use import_epoch_scope::{
    ImportEpochScope, import_source_nodes, replay_import_epoch_scope,
    validate_import_settlement_scope,
};

mod settlement_births;
pub(crate) use settlement_births::{legacy_import_births, validate_legacy_singleton_births};

mod publication_cohorts;
pub(crate) use publication_cohorts::{publication_birth_cohort, same_publication_birth_cohort};

mod settlement_scope;
pub(crate) use settlement_scope::validate_historical_settlement;

mod reliquary_open;
pub(crate) use reliquary_open::reconstruct_reliquary;

mod transformed_copy;
pub(crate) use transformed_copy::{HistoricalCopyView, historical_copy_preserves_import_authority};

pub(crate) mod freshness_reconciliation;
pub(crate) mod reconciliation_inventory;

mod settlement_completion_evidence;
pub(crate) use settlement_completion_evidence::SettlementCompletionEvidence;

pub(crate) mod dream_evidence;

pub(crate) mod dream_representative;

pub(crate) mod import_admission;

pub(crate) use import_epoch_scope::read_import_epoch_scope;

pub(crate) use settlement_scope::HistoricalSettlementReadOwners;

pub(crate) mod import_replay_read;
