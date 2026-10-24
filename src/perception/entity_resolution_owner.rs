use crate::Container;
use crate::entity_resolution_store::EntityResolutionStore;
use crate::entity_store::EntityStore;
use crate::memory_store::MemoryStore;
use crate::{
    EntityId, EntityResolutionCompaction, EntityResolutionReason, MemoryEntityMentionKey,
    MemoryEntityResolution, MemoryEntityResolutionStatus, MemoryError, MemoryId,
};

pub(crate) struct ResolutionReadContext<'a> {
    pub(crate) memories: &'a MemoryStore,
    pub(crate) entity_resolutions: &'a EntityResolutionStore,
}
pub(crate) struct ResolutionWriteContext<'a> {
    pub(crate) container: &'a mut Container,
    pub(crate) memories: &'a MemoryStore,
    pub(crate) entities: &'a EntityStore,
    pub(crate) entity_resolutions: &'a mut EntityResolutionStore,
}

impl ResolutionReadContext<'_> {
    pub(crate) fn entity_mention_keys_for_memory(
        &self,
        memory_id: MemoryId,
    ) -> Vec<MemoryEntityMentionKey> {
        self.memories
            .routing_metadata(memory_id)
            .map(|metadata| {
                metadata
                    .entity_mentions
                    .iter()
                    .map(|mention| MemoryEntityMentionKey::new(memory_id, mention))
                    .collect()
            })
            .unwrap_or_default()
    }
    pub(crate) fn pending_entity_mentions_for_candidate(
        &self,
        entity_id: EntityId,
    ) -> Vec<MemoryEntityMentionKey> {
        self.entity_resolutions.pending_for_entity(entity_id)
    }
    pub(crate) fn pending_entity_mentions_for_surface(
        &self,
        surface: &str,
    ) -> Vec<MemoryEntityMentionKey> {
        self.memories
            .entity_mention_keys_for_surface(surface)
            .into_iter()
            .filter(|key| {
                self.entity_resolutions.get(*key).is_some_and(|value| {
                    matches!(
                        value.status,
                        MemoryEntityResolutionStatus::Pending(_)
                            | MemoryEntityResolutionStatus::Dormant(_)
                    )
                })
            })
            .collect()
    }
    pub(crate) fn schedulable_entity_mentions_for_memory(
        &self,
        memory_id: MemoryId,
    ) -> Vec<MemoryEntityMentionKey> {
        self.entity_mention_keys_for_memory(memory_id)
            .into_iter()
            .filter(|key| {
                self.entity_resolutions.get(*key).is_none_or(|value| {
                    matches!(
                        value.status,
                        MemoryEntityResolutionStatus::Pending(_)
                            | MemoryEntityResolutionStatus::Dormant(_)
                    )
                })
            })
            .collect()
    }
    pub fn entity_resolution(
        &self,
        key: MemoryEntityMentionKey,
    ) -> Option<&MemoryEntityResolution> {
        self.entity_resolutions.get(key)
    }
    pub fn entity_resolutions_for_memory(
        &self,
        memory_id: MemoryId,
    ) -> Vec<MemoryEntityResolution> {
        self.entity_resolutions.for_memory(memory_id)
    }
    pub fn entity_resolution_retry_needed(
        &self,
        key: MemoryEntityMentionKey,
        candidate_set_fingerprint: [u8; 32],
        context_fingerprint: [u8; 32],
    ) -> bool {
        self.entity_resolutions
            .retry_needed(key, candidate_set_fingerprint, context_fingerprint)
    }
    pub fn entity_resolutions(&self) -> Vec<MemoryEntityResolution> {
        self.entity_resolutions.records()
    }
    pub(crate) fn entity_resolution_records(&self) -> Vec<MemoryEntityResolution> {
        self.entity_resolutions.records()
    }
}
impl ResolutionWriteContext<'_> {
    pub fn put_entity_resolved(
        &mut self,
        key: MemoryEntityMentionKey,
        expected_revision: u32,
        entity_id: EntityId,
        reason: EntityResolutionReason,
        now_ns: i64,
    ) -> Result<bool, MemoryError> {
        self.memories.validate_entity_mention_key(key)?;
        self.entity_resolutions
            .validate_expected_revision(key, expected_revision)?;
        if !self.entities.contains(entity_id) {
            return Err(MemoryError::InvalidField(
                "Entity resolution Entity reference",
            ));
        }
        self.entity_resolutions.put_resolved(
            &mut self.container,
            key,
            expected_revision,
            entity_id,
            reason,
            now_ns,
        )
    }
    pub fn put_entity_rejected(
        &mut self,
        key: MemoryEntityMentionKey,
        expected_revision: u32,
        reason: EntityResolutionReason,
        now_ns: i64,
    ) -> Result<bool, MemoryError> {
        self.memories.validate_entity_mention_key(key)?;
        self.entity_resolutions
            .validate_expected_revision(key, expected_revision)?;
        self.entity_resolutions.put_rejected(
            &mut self.container,
            key,
            expected_revision,
            reason,
            now_ns,
        )
    }
    pub fn put_entity_unresolved(
        &mut self,
        key: MemoryEntityMentionKey,
        expected_revision: u32,
        mut candidate_entity_ids: Vec<EntityId>,
        reason: EntityResolutionReason,
        candidate_set_fingerprint: [u8; 32],
        context_fingerprint: [u8; 32],
        now_ns: i64,
    ) -> Result<bool, MemoryError> {
        self.memories.validate_entity_mention_key(key)?;
        self.entity_resolutions
            .validate_expected_revision(key, expected_revision)?;
        self.entity_resolutions
            .normalize_candidate_ids(&mut candidate_entity_ids)?;
        if candidate_entity_ids
            .iter()
            .any(|id| !self.entities.contains(*id))
        {
            return Err(MemoryError::InvalidField(
                "Entity resolution Entity reference",
            ));
        }
        self.entity_resolutions.put_unresolved(
            &mut self.container,
            key,
            expected_revision,
            candidate_entity_ids,
            reason,
            candidate_set_fingerprint,
            context_fingerprint,
            now_ns,
        )
    }
    pub fn reset_entity_resolution(
        &mut self,
        key: MemoryEntityMentionKey,
        now_ns: i64,
    ) -> Result<bool, MemoryError> {
        self.memories.validate_entity_mention_key(key)?;
        self.entity_resolutions
            .tombstone(&mut self.container, key, now_ns)
    }
    pub fn compact_entity_resolutions(
        &mut self,
        now_ns: i64,
    ) -> Result<EntityResolutionCompaction, MemoryError> {
        let inactive_memory_ids = self
            .entity_resolutions
            .records()
            .into_iter()
            .filter_map(|value| {
                self.memories
                    .resolution_source_inactive(value.key.memory_id)
                    .then_some(value.key.memory_id)
            })
            .collect::<std::collections::HashSet<_>>();
        self.entity_resolutions
            .compact(&mut self.container, &inactive_memory_ids, now_ns)
    }
    pub fn import_entity_resolution(
        &mut self,
        value: MemoryEntityResolution,
    ) -> Result<(), MemoryError> {
        self.memories.validate_entity_mention_key(value.key)?;
        validate_live_entity_references(&value.status, &self.entities)?;
        self.entity_resolutions
            .import_current(&mut self.container, value)
    }
}
fn validate_live_entity_references(
    status: &MemoryEntityResolutionStatus,
    entities: &EntityStore,
) -> Result<(), MemoryError> {
    validate_entity_references(status, entities, false)
}

fn validate_entity_references(
    status: &MemoryEntityResolutionStatus,
    entities: &EntityStore,
    historical: bool,
) -> Result<(), MemoryError> {
    let contains = |id| {
        if historical {
            entities.contains_historical(id)
        } else {
            entities.contains(id)
        }
    };
    let valid = match status {
        MemoryEntityResolutionStatus::Resolved { entity_id, .. } => contains(*entity_id),
        MemoryEntityResolutionStatus::Pending(value) => value
            .candidate_entity_ids
            .iter()
            .copied()
            .all(|id| contains(id)),
        MemoryEntityResolutionStatus::Dormant(value) => value
            .candidate_entity_ids
            .iter()
            .copied()
            .all(|id| contains(id)),
        MemoryEntityResolutionStatus::Rejected { .. } => true,
    };
    if valid {
        Ok(())
    } else {
        Err(MemoryError::InvalidField(
            "Entity resolution Entity reference",
        ))
    }
}
