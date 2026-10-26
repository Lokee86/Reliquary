use crate::entity_resolution_boundary::ResolutionAdmission;
use crate::{
    Cva, EntityId, EntityResolutionCompaction, EntityResolutionReason, MemoryEntityMentionKey,
    MemoryEntityResolution, MemoryEntityResolutionStatus, MemoryError, MemoryId, Phylactery,
};

macro_rules! impl_owner {
    ($owner:ty) => {
        impl $owner {
            pub(crate) fn entity_mention_keys_for_memory(
                &self,
                memory_id: MemoryId,
            ) -> Vec<MemoryEntityMentionKey> {
                self.memory_routing_metadata(memory_id)
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
                self.entity_resolutions.owner_pending_for_entity(entity_id)
            }

            pub(crate) fn pending_entity_mentions_for_surface(
                &self,
                surface: &str,
            ) -> Vec<MemoryEntityMentionKey> {
                self.memories
                    .entity_mention_keys_for_surface(surface)
                    .into_iter()
                    .filter(|key| {
                        self.entity_resolution(*key).is_some_and(|value| {
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
                        self.entity_resolution(*key).is_none_or(|value| {
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
            ) -> Option<MemoryEntityResolution> {
                self.entity_resolutions.owner_get(key)
            }

            pub fn entity_resolutions_for_memory(
                &self,
                memory_id: MemoryId,
            ) -> Vec<MemoryEntityResolution> {
                self.entity_resolutions.owner_for_memory(memory_id)
            }

            pub fn entity_resolution_retry_needed(
                &self,
                key: MemoryEntityMentionKey,
                candidate_set_fingerprint: [u8; 32],
                context_fingerprint: [u8; 32],
            ) -> bool {
                self.entity_resolutions.owner_retry_needed(
                    key,
                    candidate_set_fingerprint,
                    context_fingerprint,
                )
            }

            pub fn put_entity_resolved(
                &mut self,
                key: MemoryEntityMentionKey,
                expected_revision: u32,
                entity_id: EntityId,
                reason: EntityResolutionReason,
                now_ns: i64,
            ) -> Result<bool, MemoryError> {
                self.entity_resolutions.owner_put_resolved(
                    &mut self.container,
                    &self.memories,
                    &self.entities,
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
                self.entity_resolutions.owner_put_rejected(
                    &mut self.container,
                    &self.memories,
                    key,
                    expected_revision,
                    reason,
                    now_ns,
                )
            }

            #[allow(clippy::too_many_arguments)]
            pub fn put_entity_unresolved(
                &mut self,
                key: MemoryEntityMentionKey,
                expected_revision: u32,
                candidate_entity_ids: Vec<EntityId>,
                reason: EntityResolutionReason,
                candidate_set_fingerprint: [u8; 32],
                context_fingerprint: [u8; 32],
                now_ns: i64,
            ) -> Result<bool, MemoryError> {
                self.entity_resolutions.owner_put_unresolved(
                    &mut self.container,
                    &self.memories,
                    &self.entities,
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
                    .owner_tombstone(&mut self.container, key, now_ns)
            }

            pub fn compact_entity_resolutions(
                &mut self,
                now_ns: i64,
            ) -> Result<EntityResolutionCompaction, MemoryError> {
                self.entity_resolutions
                    .owner_compact(&mut self.container, &self.memories, now_ns)
            }

            pub fn entity_resolutions(&self) -> Vec<MemoryEntityResolution> {
                self.entity_resolutions.owner_records()
            }

            pub(crate) fn entity_resolution_records(&self) -> Vec<MemoryEntityResolution> {
                self.entity_resolutions.owner_records()
            }

            pub fn import_entity_resolution(
                &mut self,
                value: MemoryEntityResolution,
            ) -> Result<(), MemoryError> {
                self.entity_resolutions.owner_import_current(
                    &mut self.container,
                    &self.memories,
                    &self.entities,
                    value,
                )
            }
        }
    };
}

impl_owner!(Cva);
impl_owner!(Phylactery);
