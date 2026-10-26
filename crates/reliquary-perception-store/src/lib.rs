//! Durable Perception owners; product authorization and inference stay above this crate.
pub mod entity_resolution_codec;
pub mod entity_resolution_model;
pub mod entity_resolution_store;
pub use entity_resolution_model::*;
pub use resolution_error::ResolutionError;
mod resolution_error;

// Transitional Relationship durable owner.
mod relationship_model;
mod relationship_error;
mod relationship_codec;
mod relationship_publication_codec;
mod relationship_compact_codec;
mod relationship_compact_contract;
mod relationship_compact_values;
mod relationship_compact_apply;
mod relationship_compact_publish;
mod relationship_rebuild;
mod relationship_replay;
mod relationship_store;
mod relationship_store_read;
mod relationship_store_validation;
mod relationship_retained_structure;
mod relationship_description_proof;
mod relationship_description_prune;

pub use relationship_model::{
    EntityRef, Relationship, RelationshipDraft, RelationshipId, RelationshipLifecycleState,
    RelationshipMemoryRef, RelationshipParticipant, RelationshipPayloadUnavailable,
    RelationshipRetainedRevision, RelationshipRetentionDescriptor, RelationshipStats,
    RelationshipTombstoneCoordinates, RelationshipTombstoneOutcome,
    MAX_RELATIONSHIP_EVIDENCE, MAX_RELATIONSHIP_KIND_BYTES, MAX_RELATIONSHIP_OWNER_ID_BYTES,
    MAX_RELATIONSHIP_PARTICIPANTS, MAX_RELATIONSHIP_ROLE_BYTES, MAX_RELATIONSHIP_SUMMARY_BYTES,
};
pub use relationship_error::RelationshipError;
pub use relationship_store::RelationshipStore;

// EntityId is owned and exported by this same crate through the Entity extraction.
pub use entity_model::EntityId;
mod entity_model;
