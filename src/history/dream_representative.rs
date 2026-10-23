//! Read-only retained Memory/Dream representative authority.
use crate::graph_relation_sets::{duplicate_component, unique_superseder};
use crate::memory_source_time_admission::reliquary_memory_source_timestamp_ns;
use crate::{GraphRelation, Memory, MemoryId};
use std::collections::{HashSet, VecDeque};
pub(crate) fn historical_duplicate_representative(
    source: &crate::owner_lifecycle::HistoryReadAccess<'_>,
    archive: &crate::Archive,
    memories: &crate::memory_store::MemoryStore,
    births: &crate::freshness_dream_journal::DreamFreshnessJournal,
    root_id: MemoryId,
    relations: &[GraphRelation],
) -> Result<Option<MemoryId>, crate::MemoryError> {
    let mut visited = HashSet::new();
    let mut queue = VecDeque::from([root_id]);
    let mut active = Vec::new();
    while let Some(id) = queue.pop_front() {
        if !visited.insert(id) {
            continue;
        }
        let Some(current) =
            historical_supersession_terminal(source, archive, memories, births, id, relations)?
        else {
            return Ok(None);
        };
        queue.extend(duplicate_component(id, relations));
        if current != id {
            queue.push_back(current);
            continue;
        }
        let memory = source.current_memory(memories, current)?;
        if !memory.archived && births.is_publication_birth(current) {
            active.push(memory);
        }
    }
    active.sort_by_key(|memory| representative_key(archive, memory));
    Ok(active.first().map(|memory| memory.id))
}

fn historical_supersession_terminal(
    source: &crate::owner_lifecycle::HistoryReadAccess<'_>,
    archive: &crate::Archive,
    memories: &crate::memory_store::MemoryStore,
    births: &crate::freshness_dream_journal::DreamFreshnessJournal,
    root_id: MemoryId,
    relations: &[GraphRelation],
) -> Result<Option<MemoryId>, crate::MemoryError> {
    let mut current = root_id;
    let mut visited = HashSet::new();
    loop {
        if !visited.insert(current) {
            return Ok(None);
        }
        let memory = source.current_memory(memories, current)?;
        let Some(superseder) = memory.superseded_by else {
            break;
        };
        // Both existing owners must agree. A bare pointer, conflicting active
        // edge, cycle or unproved publication cannot confer representation.
        if !memory.archived
            || unique_superseder(relations, current) != Some(superseder)
            || !births.is_publication_birth(superseder)
        {
            return Ok(None);
        }
        current = superseder;
    }
    Ok(Some(current))
}

pub(crate) fn representative_key(archive: &crate::Archive, memory: &Memory) -> (u8, i64, [u8; 32]) {
    let state_rank = match memory.lifecycle_state.as_str() {
        "canonical" => 0,
        "knowledge" => 1,
        "extracted" => 2,
        _ => 3,
    };
    (
        state_rank,
        reliquary_memory_source_timestamp_ns(archive, memory).unwrap_or(i64::MAX),
        memory.id.0,
    )
}

/// Resolve retained first-cycle roots using the accepted duplicate/supersession proof.
pub(crate) fn historical_dream_representatives(
    source: &crate::owner_lifecycle::HistoryReadAccess<'_>,
    archive: &crate::Archive,
    memories: &crate::memory_store::MemoryStore,
    births: &crate::freshness_dream_journal::DreamFreshnessJournal,
    relations: &[GraphRelation],
    source_id: MemoryId,
    roots: &std::collections::BTreeSet<MemoryId>,
) -> Result<std::collections::BTreeMap<MemoryId, MemoryId>, crate::owner_lifecycle::OwnerError> {
    source.current_memory(memories, source_id)?;
    let relations = relations.to_vec();
    let mut result = std::collections::BTreeMap::new();
    for &root in roots {
        source.current_memory(memories, root)?;
        let duplicate = relations.iter().any(|relation| {
            relation.kind == crate::GraphRelationKind::DuplicateOf
                && (relation.source == root || relation.target == root)
        });
        let superseded = source
            .current_memory(memories, root)?
            .superseded_by
            .is_some();
        let representative = if duplicate || superseded {
            historical_duplicate_representative(source, archive, memories, births, root, &relations)
                .map_err(|error| crate::owner_lifecycle::OwnerError::Freshness(error.to_string()))?
                .ok_or_else(|| {
                    crate::owner_lifecycle::OwnerError::Freshness(
                        "historical duplicate root has no verified representative".into(),
                    )
                })?
        } else {
            root
        };
        result.insert(root, representative);
    }
    Ok(result)
}
