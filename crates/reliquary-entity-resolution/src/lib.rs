//! Entity identity-resolution policy over prepared evidence.
mod entity_identity_guard;
pub use entity_identity_guard::creation_conflicts;

mod entity_candidate_model;
mod entity_candidate_rank;
pub use entity_candidate_model::*;
pub use entity_candidate_rank::{
    CandidateEvidence, candidate_order, finalize_candidate, push_unique,
};
mod entity_candidate_routing;
pub use entity_candidate_routing::{context_query, memory_contains_surface};

mod entity_fingerprint;
pub use entity_fingerprint::{candidate_fingerprint, context_fingerprint};
use reliquary_memory_core::Memory;

mod entity_admission;
mod entity_resolution_decision;
mod entity_resolution_engine;
mod entity_resolution_evaluate;
mod entity_resolver;
mod entity_resolver_model;
mod entity_resolver_schema;
pub use entity_admission::MAX_ENTITY_ADMISSION_CONTEXT_MEMORIES;
pub use entity_resolution_engine::EntityResolutionEngine;
pub use entity_resolver::{
    EntityResolver, MAX_ENTITY_RESOLVER_EVIDENCE_MEMORIES, MAX_ENTITY_RESOLVER_EVIDENCE_TEXT_BYTES,
    parse_reason,
};
pub use entity_resolver_model::*;
pub use entity_resolver_schema::*;
use reliquary_memory_core::{MemoryEntityMentionKey, MemoryTextField};
use reliquary_perception_store::{
    EntityId, EntityResolutionReason, MAX_ENTITY_KIND_BYTES, MAX_ENTITY_SUMMARY_BYTES,
};
use reliquary_switchboard::{
    DecisionEndpoint, DecisionEndpointError, GeneralEndpoint, GeneralEndpointError,
};

#[derive(Debug)]
pub enum EntityResolverError {
    Endpoint(GeneralEndpointError),
    InvalidOutput(String),
}
impl From<GeneralEndpointError> for EntityResolverError {
    fn from(e: GeneralEndpointError) -> Self {
        Self::Endpoint(e)
    }
}
impl std::fmt::Display for EntityResolverError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Endpoint(e) => e.fmt(f),
            Self::InvalidOutput(s) => write!(f, "Entity resolver output: {s}"),
        }
    }
}
impl std::error::Error for EntityResolverError {}

pub struct EntityResolutionPrepared<'a> {
    pub memory: &'a Memory,
    pub candidates: &'a EntityCandidateSet,
    pub evidence: &'a [Vec<Memory>],
    pub admission_context: &'a [Memory],
}
pub struct EntityResolutionEvaluation {
    pub output: EntityResolverOutput,
    pub materialization: Option<EntityMaterialization>,
}

mod entity_audit_collisions;
mod entity_audit_model;
mod entity_reconciliation_candidates;
mod entity_reconciliation_inference;
mod entity_reconciliation_model;
mod entity_reconciliation_schema;
pub use entity_audit_collisions::{
    alias_collisions, cross_kind_alias_shadows, exact_surface_collisions,
    normalized_surface_collisions,
};
pub use entity_audit_model::*;
pub use entity_reconciliation_candidates::reconciliation_candidates;
pub use entity_reconciliation_inference::classify_pair;
pub use entity_reconciliation_model::*;
use reliquary_perception_store::Entity;
#[derive(Debug)]
pub enum ReconciliationError {
    Endpoint(GeneralEndpointError),
    InvalidOutput(String),
}
impl From<GeneralEndpointError> for ReconciliationError {
    fn from(e: GeneralEndpointError) -> Self {
        Self::Endpoint(e)
    }
}
impl std::fmt::Display for ReconciliationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Endpoint(e) => e.fmt(f),
            Self::InvalidOutput(s) => write!(f, "Entity reconciliation output: {s}"),
        }
    }
}
impl std::error::Error for ReconciliationError {}
