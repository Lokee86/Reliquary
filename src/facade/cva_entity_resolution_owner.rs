use crate::{
    Cva, EntityId, EntityResolutionCompaction, EntityResolutionReason, MemoryEntityMentionKey,
    MemoryEntityResolution, MemoryError, MemoryId, Phylactery,
};

macro_rules! impl_owner {
    ($owner:ty) => {
        impl $owner {
            pub(crate) fn entity_mention_keys_for_memory(
                &self,
                id: MemoryId,
            ) -> Vec<MemoryEntityMentionKey> {
                self.runtime_entity_mention_keys_for_memory(id)
            }
            pub(crate) fn pending_entity_mentions_for_candidate(
                &self,
                id: EntityId,
            ) -> Vec<MemoryEntityMentionKey> {
                self.runtime_pending_entity_mentions_for_candidate(id)
            }
            pub(crate) fn pending_entity_mentions_for_surface(
                &self,
                surface: &str,
            ) -> Vec<MemoryEntityMentionKey> {
                self.runtime_pending_entity_mentions_for_surface(surface)
            }
            pub(crate) fn schedulable_entity_mentions_for_memory(
                &self,
                id: MemoryId,
            ) -> Vec<MemoryEntityMentionKey> {
                self.runtime_schedulable_entity_mentions_for_memory(id)
            }
            pub fn entity_resolution(
                &self,
                key: MemoryEntityMentionKey,
            ) -> Option<&MemoryEntityResolution> {
                self.runtime_entity_resolution(key)
            }
            pub fn entity_resolutions_for_memory(
                &self,
                id: MemoryId,
            ) -> Vec<MemoryEntityResolution> {
                self.runtime_entity_resolutions_for_memory(id)
            }
            pub fn entity_resolution_retry_needed(
                &self,
                key: MemoryEntityMentionKey,
                candidate: [u8; 32],
                context: [u8; 32],
            ) -> bool {
                self.runtime_entity_resolution_retry_needed(key, candidate, context)
            }
            pub fn put_entity_resolved(
                &mut self,
                key: MemoryEntityMentionKey,
                revision: u32,
                id: EntityId,
                reason: EntityResolutionReason,
                now_ns: i64,
            ) -> Result<bool, MemoryError> {
                self.runtime_put_entity_resolved(key, revision, id, reason, now_ns)
            }
            pub fn put_entity_rejected(
                &mut self,
                key: MemoryEntityMentionKey,
                revision: u32,
                reason: EntityResolutionReason,
                now_ns: i64,
            ) -> Result<bool, MemoryError> {
                self.runtime_put_entity_rejected(key, revision, reason, now_ns)
            }
            pub fn put_entity_unresolved(
                &mut self,
                key: MemoryEntityMentionKey,
                revision: u32,
                ids: Vec<EntityId>,
                reason: EntityResolutionReason,
                candidate: [u8; 32],
                context: [u8; 32],
                now_ns: i64,
            ) -> Result<bool, MemoryError> {
                self.runtime_put_entity_unresolved(
                    key, revision, ids, reason, candidate, context, now_ns,
                )
            }
            pub fn reset_entity_resolution(
                &mut self,
                key: MemoryEntityMentionKey,
                now_ns: i64,
            ) -> Result<bool, MemoryError> {
                self.runtime_reset_entity_resolution(key, now_ns)
            }
            pub fn compact_entity_resolutions(
                &mut self,
                now_ns: i64,
            ) -> Result<EntityResolutionCompaction, MemoryError> {
                self.runtime_compact_entity_resolutions(now_ns)
            }
            pub fn entity_resolutions(&self) -> Vec<MemoryEntityResolution> {
                self.runtime_entity_resolutions()
            }
            pub(crate) fn entity_resolution_records(&self) -> Vec<MemoryEntityResolution> {
                self.runtime_entity_resolution_records()
            }
            pub fn import_entity_resolution(
                &mut self,
                value: MemoryEntityResolution,
            ) -> Result<(), MemoryError> {
                self.runtime_import_entity_resolution(value)
            }
        }
    };
}
impl_owner!(Cva);
impl_owner!(Phylactery);
