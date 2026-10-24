use crate::entity_resolution_store::EntityResolutionStore;
use crate::memory_store::MemoryStore;
use crate::{
    EntityId, MemoryEntityMentionKey, MemoryEntityResolution, MemoryEntityResolutionStatus,
    MemoryId,
};

pub(crate) struct ResolutionReadContext<'a> {
    pub(crate) memories: &'a MemoryStore,
    pub(crate) entity_resolutions: &'a EntityResolutionStore,
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
