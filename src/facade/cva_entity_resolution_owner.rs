use crate::entity_resolution_owner::ResolutionReadContext;
use crate::owner_lifecycle::entity_resolution_admission::{
    ResolutionWriteContext, ResolutionWriteResult, apply_resolution_write_plan,
};
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
                ResolutionReadContext {
                    memories: &self.memories,
                    entity_resolutions: &self.entity_resolutions,
                }
                .entity_mention_keys_for_memory(memory_id)
            }
            pub(crate) fn pending_entity_mentions_for_candidate(
                &self,
                entity_id: EntityId,
            ) -> Vec<MemoryEntityMentionKey> {
                ResolutionReadContext {
                    memories: &self.memories,
                    entity_resolutions: &self.entity_resolutions,
                }
                .pending_entity_mentions_for_candidate(entity_id)
            }
            pub(crate) fn pending_entity_mentions_for_surface(
                &self,
                surface: &str,
            ) -> Vec<MemoryEntityMentionKey> {
                ResolutionReadContext {
                    memories: &self.memories,
                    entity_resolutions: &self.entity_resolutions,
                }
                .pending_entity_mentions_for_surface(surface)
            }
            pub(crate) fn schedulable_entity_mentions_for_memory(
                &self,
                memory_id: MemoryId,
            ) -> Vec<MemoryEntityMentionKey> {
                ResolutionReadContext {
                    memories: &self.memories,
                    entity_resolutions: &self.entity_resolutions,
                }
                .schedulable_entity_mentions_for_memory(memory_id)
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
                ResolutionReadContext {
                    memories: &self.memories,
                    entity_resolutions: &self.entity_resolutions,
                }
                .entity_resolutions_for_memory(memory_id)
            }
            pub fn entity_resolution_retry_needed(
                &self,
                key: MemoryEntityMentionKey,
                candidate_set_fingerprint: [u8; 32],
                context_fingerprint: [u8; 32],
            ) -> bool {
                ResolutionReadContext {
                    memories: &self.memories,
                    entity_resolutions: &self.entity_resolutions,
                }
                .entity_resolution_retry_needed(
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
                let plan = ResolutionWriteContext {
                    memories: &self.memories,
                    entities: &self.entities,
                    entity_resolutions: &self.entity_resolutions,
                }
                .put_entity_resolved(key, expected_revision, entity_id, reason, now_ns)?;
                match apply_resolution_write_plan(
                    &mut self.container,
                    &mut self.entity_resolutions,
                    plan,
                )? {
                    ResolutionWriteResult::Changed(value) => Ok(value),
                    _ => unreachable!(),
                }
            }
            pub fn put_entity_rejected(
                &mut self,
                key: MemoryEntityMentionKey,
                expected_revision: u32,
                reason: EntityResolutionReason,
                now_ns: i64,
            ) -> Result<bool, MemoryError> {
                let plan = ResolutionWriteContext {
                    memories: &self.memories,
                    entities: &self.entities,
                    entity_resolutions: &self.entity_resolutions,
                }
                .put_entity_rejected(key, expected_revision, reason, now_ns)?;
                match apply_resolution_write_plan(
                    &mut self.container,
                    &mut self.entity_resolutions,
                    plan,
                )? {
                    ResolutionWriteResult::Changed(value) => Ok(value),
                    _ => unreachable!(),
                }
            }
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
                let plan = ResolutionWriteContext {
                    memories: &self.memories,
                    entities: &self.entities,
                    entity_resolutions: &self.entity_resolutions,
                }
                .put_entity_unresolved(
                    key,
                    expected_revision,
                    candidate_entity_ids,
                    reason,
                    candidate_set_fingerprint,
                    context_fingerprint,
                    now_ns,
                )?;
                match apply_resolution_write_plan(
                    &mut self.container,
                    &mut self.entity_resolutions,
                    plan,
                )? {
                    ResolutionWriteResult::Changed(value) => Ok(value),
                    _ => unreachable!(),
                }
            }
            pub fn reset_entity_resolution(
                &mut self,
                key: MemoryEntityMentionKey,
                now_ns: i64,
            ) -> Result<bool, MemoryError> {
                let plan = ResolutionWriteContext {
                    memories: &self.memories,
                    entities: &self.entities,
                    entity_resolutions: &self.entity_resolutions,
                }
                .reset_entity_resolution(key, now_ns)?;
                match apply_resolution_write_plan(
                    &mut self.container,
                    &mut self.entity_resolutions,
                    plan,
                )? {
                    ResolutionWriteResult::Changed(value) => Ok(value),
                    _ => unreachable!(),
                }
            }
            pub fn compact_entity_resolutions(
                &mut self,
                now_ns: i64,
            ) -> Result<EntityResolutionCompaction, MemoryError> {
                let plan = ResolutionWriteContext {
                    memories: &self.memories,
                    entities: &self.entities,
                    entity_resolutions: &self.entity_resolutions,
                }
                .compact_entity_resolutions(now_ns)?;
                match apply_resolution_write_plan(
                    &mut self.container,
                    &mut self.entity_resolutions,
                    plan,
                )? {
                    ResolutionWriteResult::Compacted(value) => Ok(value),
                    _ => unreachable!(),
                }
            }
            pub fn entity_resolutions(&self) -> Vec<MemoryEntityResolution> {
                ResolutionReadContext {
                    memories: &self.memories,
                    entity_resolutions: &self.entity_resolutions,
                }
                .entity_resolutions()
            }
            pub(crate) fn entity_resolution_records(&self) -> Vec<MemoryEntityResolution> {
                ResolutionReadContext {
                    memories: &self.memories,
                    entity_resolutions: &self.entity_resolutions,
                }
                .entity_resolution_records()
            }
            pub fn import_entity_resolution(
                &mut self,
                value: MemoryEntityResolution,
            ) -> Result<(), MemoryError> {
                let plan = ResolutionWriteContext {
                    memories: &self.memories,
                    entities: &self.entities,
                    entity_resolutions: &self.entity_resolutions,
                }
                .import_entity_resolution(value)?;
                match apply_resolution_write_plan(
                    &mut self.container,
                    &mut self.entity_resolutions,
                    plan,
                )? {
                    ResolutionWriteResult::Imported => Ok(()),
                    _ => unreachable!(),
                }
            }
        }
    };
}
impl_owner!(Cva);
impl_owner!(Phylactery);
