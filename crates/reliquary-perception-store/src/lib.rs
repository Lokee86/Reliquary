//! Durable Perception owners; product authorization and inference stay above this crate.
pub mod entity_resolution_codec;
pub mod entity_resolution_model;
pub mod entity_resolution_store;
pub use entity_resolution_model::*;
pub use resolution_error::ResolutionError;
mod resolution_error;

pub mod relationship_model;
pub mod relationship_error;
pub mod relationship_codec;
pub mod relationship_publication_codec;
pub mod relationship_compact_codec;
pub mod relationship_compact_contract;
pub mod relationship_compact_values;
pub mod relationship_compact_apply;
pub mod relationship_compact_publish;
pub mod relationship_rebuild;
pub mod relationship_replay;
pub mod relationship_store;
pub mod relationship_store_read;
pub mod relationship_store_validation;
pub mod relationship_retained_structure;
pub mod relationship_description_proof;
pub mod relationship_description_prune;

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

// Entity durable owner.

pub mod entity_codec;
pub mod entity_compact_apply;
pub mod entity_compact_codec;
pub mod entity_compact_contract;
pub mod entity_compact_values;
pub mod entity_description_proof;
pub mod entity_description_prune;
mod entity_error;
mod entity_identity;
mod entity_identity_guard;
pub mod entity_model;
mod entity_rebuild;
pub mod entity_retained_structure;
mod entity_store;

pub use entity_error::{EntityError, EntityPayloadUnavailable};
pub use entity_model::{
    Entity, EntityDraft, EntityGlobalId, EntityId, EntityMergeOutcome,
    EntityRetainedRevision, EntityRetentionDescriptor, EntityStats,
    EntityTerminalCoordinates, EntityTerminalOutcome, EntityLifecycleState, MAX_ENTITY_ALIASES,
    MAX_ENTITY_KIND_BYTES, MAX_ENTITY_NAME_BYTES, MAX_ENTITY_SUMMARY_BYTES,
};
pub use entity_rebuild::EntityOpenState;
pub use entity_store::{EntityReplayPublication, EntityStore};
