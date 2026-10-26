use super::{EntityResolutionStore, mention_sort_key};
use crate::{EntityId, MemoryEntityMentionKey, MemoryEntityResolutionStatus};
use std::collections::{HashMap, HashSet};

impl EntityResolutionStore {
    pub fn pending_for_entity(&self, entity_id: EntityId) -> Vec<MemoryEntityMentionKey> {
        let mut keys: Vec<_> = self
            .pending_by_entity
            .get(&entity_id)
            .into_iter()
            .flatten()
            .copied()
            .collect();
        keys.sort_by_key(|key| mention_sort_key(*key));
        keys
    }
}

pub(super) fn index_pending(
    index: &mut HashMap<EntityId, HashSet<MemoryEntityMentionKey>>,
    key: MemoryEntityMentionKey,
    status: &MemoryEntityResolutionStatus,
) {
    for entity_id in pending_candidate_ids(status) {
        index.entry(*entity_id).or_default().insert(key);
    }
}

pub(super) fn deindex_pending(
    index: &mut HashMap<EntityId, HashSet<MemoryEntityMentionKey>>,
    key: MemoryEntityMentionKey,
    status: &MemoryEntityResolutionStatus,
) {
    for entity_id in pending_candidate_ids(status) {
        if let Some(keys) = index.get_mut(entity_id) {
            keys.remove(&key);
            if keys.is_empty() {
                index.remove(entity_id);
            }
        }
    }
}

fn pending_candidate_ids(status: &MemoryEntityResolutionStatus) -> &[EntityId] {
    match status {
        MemoryEntityResolutionStatus::Pending(value) => &value.candidate_entity_ids,
        MemoryEntityResolutionStatus::Dormant(value) => &value.candidate_entity_ids,
        MemoryEntityResolutionStatus::Resolved { .. }
        | MemoryEntityResolutionStatus::Rejected { .. } => &[],
    }
}
