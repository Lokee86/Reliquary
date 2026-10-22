//! Explicit repair of persisted references to retired Entities.
use crate::container::Container;
use crate::entity_resolution_store::EntityResolutionStore;
use crate::entity_store::EntityStore;
use crate::memory_store::MemoryStore;
use crate::{EntityId, MemoryEntityMentionKey, MemoryEntityResolutionStatus, MemoryError};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EntityResolutionMaintenance {
    NothingToRepair,
    RetiredEntityReferences { revisions: usize },
}

pub(crate) fn inspect(
    resolutions: &EntityResolutionStore,
    memories: &MemoryStore,
    entities: &EntityStore,
) -> Result<EntityResolutionMaintenance, MemoryError> {
    let revisions = retired_entity_reference_repairs(resolutions, memories, entities)?.len();
    Ok(if revisions == 0 {
        EntityResolutionMaintenance::NothingToRepair
    } else {
        EntityResolutionMaintenance::RetiredEntityReferences { revisions }
    })
}

pub(crate) fn repair(
    container: &mut Container,
    resolutions: &mut EntityResolutionStore,
    memories: &MemoryStore,
    entities: &EntityStore,
) -> Result<usize, MemoryError> {
    let repairs = retired_entity_reference_repairs(resolutions, memories, entities)?;
    let repaired = repairs.len();
    for (key, updated_at_ns, status) in repairs {
        resolutions.append(container, key, updated_at_ns, status)?;
    }
    Ok(repaired)
}

fn retired_entity_reference_repairs(
    resolutions: &crate::entity_resolution_store::EntityResolutionStore,
    memories: &MemoryStore,
    entities: &EntityStore,
) -> Result<Vec<(MemoryEntityMentionKey, i64, MemoryEntityResolutionStatus)>, MemoryError> {
    let mut repairs = Vec::new();
    for current in resolutions.records() {
        memories.validate_historical_entity_mention_key(current.key)?;
        crate::entity_resolution_store::validate_status(&current.status)?;
        let mut changed = false;
        let status = match current.status {
            MemoryEntityResolutionStatus::Resolved { entity_id, reason } => {
                let canonical = entities
                    .canonical_active_id(entity_id)
                    .or_else(|| entities.contains_historical(entity_id).then_some(entity_id))
                    .ok_or(MemoryError::InvalidField(
                        "Entity resolution Entity reference",
                    ))?;
                changed = canonical != entity_id;
                MemoryEntityResolutionStatus::Resolved {
                    entity_id: canonical,
                    reason,
                }
            }
            MemoryEntityResolutionStatus::Pending(mut value) => {
                canonicalize_candidates(&mut value.candidate_entity_ids, entities, &mut changed)?;
                if changed {
                    value.candidate_set_fingerprint = [0; 32];
                }
                MemoryEntityResolutionStatus::Pending(value)
            }
            MemoryEntityResolutionStatus::Dormant(mut value) => {
                canonicalize_candidates(&mut value.candidate_entity_ids, entities, &mut changed)?;
                if changed {
                    value.candidate_set_fingerprint = [0; 32];
                }
                MemoryEntityResolutionStatus::Dormant(value)
            }
            MemoryEntityResolutionStatus::Rejected { reason } => {
                MemoryEntityResolutionStatus::Rejected { reason }
            }
        };
        if changed {
            repairs.push((current.key, current.updated_at_ns, status));
        }
    }
    Ok(repairs)
}

fn canonicalize_candidates(
    values: &mut Vec<EntityId>,
    entities: &EntityStore,
    changed: &mut bool,
) -> Result<(), MemoryError> {
    for value in values.iter_mut() {
        let canonical = entities
            .canonical_active_id(*value)
            .or_else(|| entities.contains_historical(*value).then_some(*value))
            .ok_or(MemoryError::InvalidField(
                "Entity resolution Entity reference",
            ))?;
        if canonical != *value {
            *value = canonical;
            *changed = true;
        }
    }
    values.sort();
    let original_len = values.len();
    values.dedup();
    *changed |= values.len() != original_len;
    Ok(())
}
