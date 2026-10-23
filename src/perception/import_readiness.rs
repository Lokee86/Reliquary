use crate::entity_resolution_store::EntityResolutionStore;
use crate::memory_store::MemoryStore;
use crate::{MemoryEntityMentionKey, MemoryEntityResolution, MemoryId};

/// Perception-owned read view for exact imported mention completion evidence.
pub(crate) struct ImportReadiness<'a> {
    memories: &'a MemoryStore,
    resolutions: &'a EntityResolutionStore,
}

impl<'a> ImportReadiness<'a> {
    pub(crate) fn new(memories: &'a MemoryStore, resolutions: &'a EntityResolutionStore) -> Self {
        Self {
            memories,
            resolutions,
        }
    }

    pub(crate) fn mention_keys(&self, memory_id: MemoryId) -> Vec<MemoryEntityMentionKey> {
        self.memories.entity_mention_keys_for_memory(memory_id)
    }

    pub(crate) fn resolution(
        &self,
        key: MemoryEntityMentionKey,
    ) -> Option<&MemoryEntityResolution> {
        self.resolutions.get(key)
    }
}
