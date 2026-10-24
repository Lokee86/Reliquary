use crate::entity_resolution_store::EntityResolutionStore;
use crate::entity_store::EntityStore;
use crate::memory_store::MemoryStore;
use crate::{
    Container, EntityId, EntityResolutionCompaction, EntityResolutionReason, MemoryEntityMentionKey,
    MemoryEntityResolution, MemoryEntityResolutionStatus, MemoryError, MemoryId,
};
use std::collections::HashSet;

/// Owner-local admission facts and write intent for Entity resolution records.
pub(crate) struct ResolutionWriteContext<'a> {
    pub(crate) memories: &'a MemoryStore,
    pub(crate) entities: &'a EntityStore,
    pub(crate) entity_resolutions: &'a EntityResolutionStore,
}

#[derive(Debug)]
pub(crate) enum ResolutionWritePlan {
    Resolved {
        key: MemoryEntityMentionKey,
        expected_revision: u32,
        entity_id: EntityId,
        reason: EntityResolutionReason,
        now_ns: i64,
    },
    Rejected {
        key: MemoryEntityMentionKey,
        expected_revision: u32,
        reason: EntityResolutionReason,
        now_ns: i64,
    },
    Unresolved {
        key: MemoryEntityMentionKey,
        expected_revision: u32,
        candidate_entity_ids: Vec<EntityId>,
        reason: EntityResolutionReason,
        candidate_set_fingerprint: [u8; 32],
        context_fingerprint: [u8; 32],
        now_ns: i64,
    },
    Reset {
        key: MemoryEntityMentionKey,
        now_ns: i64,
    },
    Compact {
        inactive_memory_ids: HashSet<MemoryId>,
        now_ns: i64,
    },
    Import(MemoryEntityResolution),
}

#[derive(Debug)]
pub(crate) enum ResolutionWriteResult {
    Changed(bool),
    Compacted(EntityResolutionCompaction),
    Imported,
}

impl ResolutionWriteContext<'_> {
    pub(crate) fn put_entity_resolved(
        &self,
        key: MemoryEntityMentionKey,
        expected_revision: u32,
        entity_id: EntityId,
        reason: EntityResolutionReason,
        now_ns: i64,
    ) -> Result<ResolutionWritePlan, MemoryError> {
        self.memories.validate_entity_mention_key(key)?;
        self.entity_resolutions
            .validate_expected_revision(key, expected_revision)?;
        if !self.entities.contains(entity_id) {
            return Err(MemoryError::InvalidField(
                "Entity resolution Entity reference",
            ));
        }
        Ok(ResolutionWritePlan::Resolved {
            key,
            expected_revision,
            entity_id,
            reason,
            now_ns,
        })
    }

    pub(crate) fn put_entity_rejected(
        &self,
        key: MemoryEntityMentionKey,
        expected_revision: u32,
        reason: EntityResolutionReason,
        now_ns: i64,
    ) -> Result<ResolutionWritePlan, MemoryError> {
        self.memories.validate_entity_mention_key(key)?;
        self.entity_resolutions
            .validate_expected_revision(key, expected_revision)?;
        Ok(ResolutionWritePlan::Rejected {
            key,
            expected_revision,
            reason,
            now_ns,
        })
    }

    pub(crate) fn put_entity_unresolved(
        &self,
        key: MemoryEntityMentionKey,
        expected_revision: u32,
        mut candidate_entity_ids: Vec<EntityId>,
        reason: EntityResolutionReason,
        candidate_set_fingerprint: [u8; 32],
        context_fingerprint: [u8; 32],
        now_ns: i64,
    ) -> Result<ResolutionWritePlan, MemoryError> {
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
        Ok(ResolutionWritePlan::Unresolved {
            key,
            expected_revision,
            candidate_entity_ids,
            reason,
            candidate_set_fingerprint,
            context_fingerprint,
            now_ns,
        })
    }

    pub(crate) fn reset_entity_resolution(
        &self,
        key: MemoryEntityMentionKey,
        now_ns: i64,
    ) -> Result<ResolutionWritePlan, MemoryError> {
        self.memories.validate_entity_mention_key(key)?;
        Ok(ResolutionWritePlan::Reset { key, now_ns })
    }

    pub(crate) fn compact_entity_resolutions(
        &self,
        now_ns: i64,
    ) -> Result<ResolutionWritePlan, MemoryError> {
        let inactive_memory_ids = self
            .entity_resolutions
            .records()
            .into_iter()
            .filter_map(|value| {
                self.memories
                    .resolution_source_inactive(value.key.memory_id)
                    .then_some(value.key.memory_id)
            })
            .collect();
        Ok(ResolutionWritePlan::Compact {
            inactive_memory_ids,
            now_ns,
        })
    }

    pub(crate) fn import_entity_resolution(
        &self,
        value: MemoryEntityResolution,
    ) -> Result<ResolutionWritePlan, MemoryError> {
        self.memories.validate_entity_mention_key(value.key)?;
        validate_live_entity_references(&value.status, self.entities)?;
        Ok(ResolutionWritePlan::Import(value))
    }
}

pub(crate) fn apply_resolution_write_plan(
    container: &mut Container,
    resolutions: &mut EntityResolutionStore,
    plan: ResolutionWritePlan,
) -> Result<ResolutionWriteResult, MemoryError> {
    match plan {
        ResolutionWritePlan::Resolved {
            key,
            expected_revision,
            entity_id,
            reason,
            now_ns,
        } => resolutions
            .put_resolved(container, key, expected_revision, entity_id, reason, now_ns)
            .map(ResolutionWriteResult::Changed),
        ResolutionWritePlan::Rejected {
            key,
            expected_revision,
            reason,
            now_ns,
        } => resolutions
            .put_rejected(container, key, expected_revision, reason, now_ns)
            .map(ResolutionWriteResult::Changed),
        ResolutionWritePlan::Unresolved {
            key,
            expected_revision,
            candidate_entity_ids,
            reason,
            candidate_set_fingerprint,
            context_fingerprint,
            now_ns,
        } => resolutions
            .put_unresolved(
                container,
                key,
                expected_revision,
                candidate_entity_ids,
                reason,
                candidate_set_fingerprint,
                context_fingerprint,
                now_ns,
            )
            .map(ResolutionWriteResult::Changed),
        ResolutionWritePlan::Reset { key, now_ns } => resolutions
            .tombstone(container, key, now_ns)
            .map(ResolutionWriteResult::Changed),
        ResolutionWritePlan::Compact {
            inactive_memory_ids,
            now_ns,
        } => resolutions
            .compact(container, &inactive_memory_ids, now_ns)
            .map(ResolutionWriteResult::Compacted),
        ResolutionWritePlan::Import(value) => resolutions
            .import_current(container, value)
            .map(|()| ResolutionWriteResult::Imported),
    }
}

fn validate_live_entity_references(
    status: &MemoryEntityResolutionStatus,
    entities: &EntityStore,
) -> Result<(), MemoryError> {
    let valid = match status {
        MemoryEntityResolutionStatus::Resolved { entity_id, .. } => entities.contains(*entity_id),
        MemoryEntityResolutionStatus::Pending(value) => value
            .candidate_entity_ids
            .iter()
            .copied()
            .all(|id| entities.contains(id)),
        MemoryEntityResolutionStatus::Dormant(value) => value
            .candidate_entity_ids
            .iter()
            .copied()
            .all(|id| entities.contains(id)),
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
