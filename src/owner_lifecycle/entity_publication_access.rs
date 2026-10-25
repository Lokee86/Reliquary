use crate::container::Container;
use crate::entity_resolution_store::EntityResolutionStore;
use crate::entity_store::EntityStore;
use crate::owner_lifecycle::TombstoneOutcome;
use crate::{
    Entity, EntityDraft, EntityError, EntityGlobalId, EntityId, EntityRetentionDescriptor,
    EntityStats, MemoryEntityMentionKey, MemoryError,
};

/// Validates and publishes an Entity through the durable owner boundary.
pub(crate) fn publish(
    container: &mut Container,
    entities: &mut EntityStore,
    id: Option<EntityId>,
    expected_revision: u64,
    draft: EntityDraft,
    reject_principal: bool,
) -> Result<(Entity, bool), EntityError> {
    crate::owner_lifecycle::entity_publication_admission::validate_entity_publication(
        &draft,
        reject_principal,
    )?;
    entities.publish(container, id, expected_revision, draft)
}

pub(crate) fn publish_with_global_id(
    container: &mut Container,
    entities: &mut EntityStore,
    id: Option<EntityId>,
    expected_revision: u64,
    draft: EntityDraft,
    global_id: Option<EntityGlobalId>,
) -> Result<(Entity, bool), EntityError> {
    entities.publish_with_global_id(container, id, expected_revision, draft, global_id)
}

pub(crate) fn tombstone_direct(
    container: &mut Container,
    entities: &mut EntityStore,
    id: EntityId,
    expected_revision: u64,
) -> Result<TombstoneOutcome, EntityError> {
    entities.tombstone_direct(container, id, expected_revision)
}

pub(crate) fn retarget_resolution(
    container: &mut Container,
    resolutions: &mut EntityResolutionStore,
    key: MemoryEntityMentionKey,
    from: EntityId,
    to: EntityId,
    now_ns: i64,
) -> Result<bool, MemoryError> {
    resolutions.retarget_resolved(container, key, from, to, now_ns)
}

pub(crate) fn entity(entities: &EntityStore, id: EntityId) -> Result<Entity, EntityError> {
    entities.entity(id)
}

pub(crate) fn entity_revision(
    entities: &EntityStore,
    id: EntityId,
    revision: u64,
) -> Result<Entity, EntityError> {
    entities.entity_revision(id, revision)
}

pub(crate) fn lifecycle_state(
    entities: &EntityStore,
    id: EntityId,
) -> Result<crate::owner_lifecycle::LifecycleState, EntityError> {
    entities.lifecycle_state(id)
}

pub(crate) fn retention_descriptor(
    entities: &EntityStore,
    id: EntityId,
) -> Result<EntityRetentionDescriptor, EntityError> {
    entities.retention_descriptor(id)
}

pub(crate) fn candidates_for_surface(
    entities: &EntityStore,
    surface: &str,
    limit: usize,
) -> Vec<Entity> {
    entities.candidates_for_surface(surface, limit)
}

pub(crate) fn candidates_for_normalized_surface(
    entities: &EntityStore,
    surface: &str,
    limit: usize,
) -> Vec<Entity> {
    entities.candidates_for_normalized_surface(surface, limit)
}

pub(crate) fn candidates_for_alias_surface(
    entities: &EntityStore,
    surface: &str,
    limit: usize,
) -> Vec<Entity> {
    entities.candidates_for_alias_surface(surface, limit)
}

pub(crate) fn creation_conflicts(
    entities: &EntityStore,
    surface: &str,
    kind: &str,
    limit: usize,
) -> Vec<Entity> {
    entities.creation_conflicts(surface, kind, limit)
}

pub(crate) fn retired_replacement(entities: &EntityStore, id: EntityId) -> Option<EntityId> {
    entities.retired_replacement(id)
}

pub(crate) fn entity_version(entities: &EntityStore) -> u64 {
    entities.entity_version()
}

pub(crate) fn current_entities(entities: &EntityStore) -> Vec<Entity> {
    entities.current()
}

pub(crate) fn stats(entities: &EntityStore) -> EntityStats {
    entities.stats()
}
