use crate::memory_source_time::reliquary_memory_source_timestamp_ns;
use crate::{Cva, DreamLifecycleError, GraphRelation, GraphRelationKind, Memory, MemoryId};
use std::collections::{HashSet, VecDeque};

pub(crate) fn explicit_authority_promotes(memory: &Memory) -> bool {
    if !matches!(memory.authority_kind.as_str(), "direct" | "correction") {
        return false;
    }
    matches!(
        memory.category.as_str(),
        "decision" | "preference" | "instruction" | "constraint" | "correction" | "commitment"
    ) || (memory.category == "fact"
        && matches!(
            memory.memory_type.as_str(),
            "project" | "process" | "product" | "schedule"
        ))
}

pub(crate) fn canonical_superseders(
    cva: &mut Cva,
    source_id: MemoryId,
    relations: &[GraphRelation],
) -> Result<Vec<MemoryId>, DreamLifecycleError> {
    let mut candidates = Vec::new();
    for relation in relations.iter().filter(|relation| {
        relation.kind == GraphRelationKind::Supersedes
            && (relation.source == source_id || relation.target == source_id)
    }) {
        if unique_superseder(relations, relation.target) == Some(relation.source)
            && target_has_canonical_authority(cva, relation.target, relation.source)?
        {
            candidates.push(relation.source);
        }
    }
    candidates.sort_by_key(|id| id.0);
    candidates.dedup();
    Ok(candidates)
}

pub(crate) fn corroborated_representative(
    cva: &mut Cva,
    source_id: MemoryId,
    relations: &[GraphRelation],
) -> Result<Option<MemoryId>, DreamLifecycleError> {
    let component = duplicate_component(source_id, relations);
    if component.len() < 2 {
        return Ok(None);
    }
    let mut anchors = HashSet::new();
    let mut active = Vec::new();
    for id in component {
        let memory = cva.memory(id)?;
        if let Some(anchor) = authority_anchor(cva, &memory) {
            anchors.insert(anchor);
        }
        if !memory.archived {
            active.push(memory);
        }
    }
    if anchors.len() < 2 || active.is_empty() {
        return Ok(None);
    }
    active.sort_by_key(|memory| representative_key(cva, memory));
    Ok(active.first().map(|memory| memory.id))
}

pub(crate) fn duplicate_component_has_active_peer(
    cva: &mut Cva,
    source_id: MemoryId,
    relations: &[GraphRelation],
) -> Result<bool, DreamLifecycleError> {
    for id in duplicate_component(source_id, relations)
        .into_iter()
        .filter(|id| *id != source_id)
    {
        if !cva.memory(id)?.archived {
            return Ok(true);
        }
    }
    Ok(false)
}

fn duplicate_component(source_id: MemoryId, relations: &[GraphRelation]) -> Vec<MemoryId> {
    let mut visited = HashSet::from([source_id]);
    let mut queue = VecDeque::from([source_id]);
    while let Some(current) = queue.pop_front() {
        for relation in relations.iter().filter(|relation| {
            relation.kind == GraphRelationKind::DuplicateOf
                && (relation.source == current || relation.target == current)
        }) {
            let neighbor = if relation.source == current {
                relation.target
            } else {
                relation.source
            };
            if visited.insert(neighbor) {
                queue.push_back(neighbor);
            }
        }
    }
    visited.into_iter().collect()
}

fn authority_anchor(cva: &Cva, memory: &Memory) -> Option<(String, String)> {
    let node_id = memory.source_node_id.clone()?;
    let conversation_id = memory
        .source_episode_id
        .and_then(|id| cva.episode(id))
        .map(|episode| episode.conversation_id.clone())?;
    Some((conversation_id, node_id))
}

fn representative_key(cva: &Cva, memory: &Memory) -> (u8, i64, [u8; 32]) {
    let state_rank = match memory.lifecycle_state.as_str() {
        "canonical" => 0,
        "knowledge" => 1,
        "extracted" => 2,
        _ => 3,
    };
    (
        state_rank,
        reliquary_memory_source_timestamp_ns(&cva.archive, memory).unwrap_or(i64::MAX),
        memory.id.0,
    )
}

fn target_has_canonical_authority(
    cva: &mut Cva,
    target: MemoryId,
    superseder: MemoryId,
) -> Result<bool, DreamLifecycleError> {
    let current = cva.memory(target)?;
    if !current.archived {
        return Ok(current.lifecycle_state == "canonical");
    }
    if current.superseded_by != Some(superseder) {
        return Ok(false);
    }
    for revision in (1..current.revision).rev() {
        let previous = cva.memory_revision(target, revision)?;
        if !previous.archived {
            return Ok(previous.lifecycle_state == "canonical");
        }
    }
    Ok(false)
}

fn unique_superseder(relations: &[GraphRelation], target: MemoryId) -> Option<MemoryId> {
    let mut sources: Vec<_> = relations
        .iter()
        .filter(|relation| {
            relation.kind == GraphRelationKind::Supersedes && relation.target == target
        })
        .map(|relation| relation.source)
        .collect();
    sources.sort_by_key(|id| id.0);
    sources.dedup();
    match sources.as_slice() {
        [source] => Some(*source),
        _ => None,
    }
}
