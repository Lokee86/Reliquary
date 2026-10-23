use crate::graph_relation_sets::{duplicate_component, unique_superseder};
use crate::{DreamLifecycleError, GraphRelation, GraphRelationKind, Memory, MemoryId};
use std::collections::HashSet;

/// Read-only owner facts used by canonical decisions; no facade or publication access.
pub(crate) trait DreamOwnerRead {
    fn memory(&self, id: MemoryId) -> Result<Memory, crate::MemoryError>;
    fn memory_revision(&self, id: MemoryId, revision: u64) -> Result<Memory, crate::MemoryError>;
    fn is_publication_birth(&self, id: MemoryId) -> bool;
    fn same_publication_cohort(&self, a: MemoryId, b: MemoryId) -> Result<bool, crate::CvaError>;
    fn authority_anchor(&self, memory: &Memory) -> Option<(String, String)>;
    fn representative_key(&self, memory: &Memory) -> (u8, i64, [u8; 32]);
}

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
    parts: &impl DreamOwnerRead,
    source_id: MemoryId,
    relations: &[GraphRelation],
) -> Result<Vec<MemoryId>, DreamLifecycleError> {
    let mut candidates = Vec::new();
    for relation in relations.iter().filter(|relation| {
        relation.kind == GraphRelationKind::Supersedes
            && (relation.source == source_id || relation.target == source_id)
    }) {
        if unique_superseder(relations, relation.target) == Some(relation.source)
            && target_has_canonical_authority(parts, relation.target, relation.source)?
        {
            candidates.push(relation.source);
        }
    }
    candidates.sort_by_key(|id| id.0);
    candidates.dedup();
    Ok(candidates)
}

pub(crate) fn corroborated_representative(
    parts: &impl DreamOwnerRead,
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
        let memory = parts.memory(id)?;
        if let Some(anchor) = parts.authority_anchor(&memory) {
            anchors.insert(anchor);
        }
        if !memory.archived {
            active.push(memory);
        }
    }
    if anchors.len() < 2 || active.is_empty() {
        return Ok(None);
    }
    active.sort_by_key(|memory| parts.representative_key(memory));
    Ok(active.first().map(|memory| memory.id))
}

pub(crate) fn existing_duplicate_representative(
    parts: &impl DreamOwnerRead,
    root_id: MemoryId,
    source_birth_version: u64,
    source_id: MemoryId,
    relations: &[GraphRelation],
) -> Result<Option<MemoryId>, DreamLifecycleError> {
    duplicate_representative(
        parts,
        root_id,
        source_birth_version,
        Some(source_id),
        relations,
    )
}

fn duplicate_representative(
    parts: &impl DreamOwnerRead,
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
        if !parts.is_publication_birth(id) {
            continue;
        }
        let first = parts.memory_revision(id, 1)?;
        if first.global_version >= source_birth_version {
            continue;
        }
        if let Some(source_id) = source_id
            && parts
                .same_publication_cohort(source_id, id)
                .map_err(|error| DreamLifecycleError::Freshness(error.to_string()))?
        {
            continue;
        }
        let memory = parts.memory(id)?;
        if !memory.archived {
            active.push(memory);
        }
    }
    active.sort_by_key(|memory| parts.representative_key(memory));
    Ok(active.first().map(|memory| memory.id))
}

pub(crate) fn duplicate_component_has_active_peer(
    parts: &impl DreamOwnerRead,
    source_id: MemoryId,
    relations: &[GraphRelation],
) -> Result<bool, DreamLifecycleError> {
    for id in duplicate_component(source_id, relations)
        .into_iter()
        .filter(|id| *id != source_id)
    {
        if !parts.memory(id)?.archived {
            return Ok(true);
        }
    }
    Ok(false)
}

fn target_has_canonical_authority(
    parts: &impl DreamOwnerRead,
    target: MemoryId,
    superseder: MemoryId,
) -> Result<bool, DreamLifecycleError> {
    let current = parts.memory(target)?;
    if !current.archived {
        return Ok(current.lifecycle_state == "canonical");
    }
    if current.superseded_by != Some(superseder) {
        return Ok(false);
    }
    for revision in (1..current.revision).rev() {
        let previous = parts.memory_revision(target, revision)?;
        if !previous.archived {
            return Ok(previous.lifecycle_state == "canonical");
        }
    }
    Ok(false)
}
