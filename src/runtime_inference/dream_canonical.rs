use crate::graph_relation_sets::{duplicate_component, unique_superseder};
use crate::memory_source_time_admission::reliquary_memory_source_timestamp_ns;
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
    active.sort_by_key(|memory| {
        crate::history::dream_representative::representative_key(&cva.archive, memory)
    });
    Ok(active.first().map(|memory| memory.id))
}

pub(crate) fn existing_duplicate_representative(
    cva: &mut Cva,
    root_id: MemoryId,
    source_birth_version: u64,
    source_id: MemoryId,
    relations: &[GraphRelation],
) -> Result<Option<MemoryId>, DreamLifecycleError> {
    duplicate_representative(
        cva,
        root_id,
        source_birth_version,
        Some(source_id),
        relations,
    )
}

/// Historical authority reads are implemented by the read-only History service.
pub(crate) fn historical_duplicate_representative(
    cva: &mut Cva,
    root_id: MemoryId,
    relations: &[GraphRelation],
) -> Result<Option<MemoryId>, DreamLifecycleError> {
    crate::history::dream_representative::historical_duplicate_representative(
        &crate::owner_lifecycle::HistoryReadAccess::new(&cva.container),
        &cva.archive,
        &cva.memories,
        &cva.dream_freshness,
        root_id,
        relations,
    )
    .map_err(Into::into)
}

fn duplicate_representative(
    cva: &mut Cva,
    root_id: MemoryId,
    source_birth_version: u64,
    source_id: Option<MemoryId>,
    relations: &[GraphRelation],
) -> Result<Option<MemoryId>, DreamLifecycleError> {
    let mut active = Vec::new();
    for id in duplicate_component(root_id, relations) {
        // Only owner-local Memories with explicit durable birth enrollment can
        // represent an existing target; legacy rows and same-batch births are
        // excluded without recreating Dream's classifier or chronology key.
        if !cva.freshness_is_publication_birth(id) {
            continue;
        }
        let first = cva.memory_revision(id, 1)?;
        if first.global_version >= source_birth_version {
            continue;
        }
        if let Some(source_id) = source_id
            && cva
                .freshness_same_publication_cohort(source_id, id)
                .map_err(|error| DreamLifecycleError::Freshness(error.to_string()))?
        {
            continue;
        }
        let memory = cva.memory(id)?;
        if !memory.archived {
            active.push(memory);
        }
    }
    active.sort_by_key(|memory| {
        crate::history::dream_representative::representative_key(&cva.archive, memory)
    });
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

fn authority_anchor(cva: &Cva, memory: &Memory) -> Option<(String, String)> {
    let node_id = memory.source_node_id.clone()?;
    let conversation_id = memory
        .source_episode_id
        .and_then(|id| cva.episode(id))
        .map(|episode| episode.conversation_id.clone())?;
    Some((conversation_id, node_id))
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
