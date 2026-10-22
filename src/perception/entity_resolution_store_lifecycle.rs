use super::EntityResolutionStore;
use crate::{
    Container, DEFAULT_ENTITY_RESOLUTION_DORMANT_TTL_NS, DEFAULT_ENTITY_RESOLUTION_PENDING_TTL_NS,
    EntityResolutionCompaction, EntityResolutionDormant, MemoryEntityResolutionStatus, MemoryError,
    MemoryId,
};
use std::collections::HashSet;

impl EntityResolutionStore {
    pub(crate) fn compact(
        &mut self,
        container: &mut Container,
        inactive_memory_ids: &HashSet<MemoryId>,
        now_ns: i64,
    ) -> Result<EntityResolutionCompaction, MemoryError> {
        let snapshot: Vec<_> = self
            .maintenance
            .iter()
            .filter_map(|key| self.current.get(key).cloned())
            .collect();
        let mut result = EntityResolutionCompaction::default();

        for value in snapshot {
            let inactive = inactive_memory_ids.contains(&value.key.memory_id);
            match value.status {
                MemoryEntityResolutionStatus::Resolved { .. } => {}
                MemoryEntityResolutionStatus::Rejected { .. } if inactive => {
                    if self.tombstone(container, value.key, now_ns)? {
                        result.rejected_purged += 1;
                    }
                }
                MemoryEntityResolutionStatus::Rejected { .. } => {}
                MemoryEntityResolutionStatus::Pending(_) if inactive => {
                    if self.tombstone(container, value.key, now_ns)? {
                        result.purged += 1;
                    }
                }
                MemoryEntityResolutionStatus::Dormant(_) if inactive => {
                    if self.tombstone(container, value.key, now_ns)? {
                        result.purged += 1;
                    }
                }
                MemoryEntityResolutionStatus::Pending(pending)
                    if elapsed_at_least(
                        now_ns,
                        pending.last_attempt_at_ns,
                        DEFAULT_ENTITY_RESOLUTION_PENDING_TTL_NS,
                    ) =>
                {
                    self.append(
                        container,
                        value.key,
                        now_ns,
                        MemoryEntityResolutionStatus::Dormant(EntityResolutionDormant {
                            candidate_entity_ids: pending.candidate_entity_ids,
                            reason: pending.reason,
                            candidate_set_fingerprint: pending.candidate_set_fingerprint,
                            context_fingerprint: pending.context_fingerprint,
                            first_seen_at_ns: pending.first_seen_at_ns,
                            last_attempt_at_ns: pending.last_attempt_at_ns,
                            attempt_count: pending.attempt_count,
                            dormant_at_ns: now_ns,
                        }),
                    )?;
                    result.dormant += 1;
                }
                MemoryEntityResolutionStatus::Dormant(dormant)
                    if elapsed_at_least(
                        now_ns,
                        dormant.dormant_at_ns,
                        DEFAULT_ENTITY_RESOLUTION_DORMANT_TTL_NS,
                    ) =>
                {
                    if self.tombstone(container, value.key, now_ns)? {
                        result.purged += 1;
                    }
                }
                MemoryEntityResolutionStatus::Pending(_)
                | MemoryEntityResolutionStatus::Dormant(_) => {}
            }
        }

        Ok(result)
    }
}

fn elapsed_at_least(now_ns: i64, then_ns: i64, threshold_ns: i64) -> bool {
    now_ns
        .checked_sub(then_ns)
        .is_some_and(|elapsed| elapsed >= threshold_ns)
}
