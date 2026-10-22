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
