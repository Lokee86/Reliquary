use crate::memory_source_time::reliquary_memory_source_timestamp_ns;
use crate::{Cva, DreamLifecycleError, GraphRelation, GraphRelationKind, Memory, MemoryId};
use std::collections::{HashSet, VecDeque};

pub(crate) fn explicit_authority_promotes(memory: &Memory) -> bool {
    reliquary_dream::explicit_authority_promotes(
        &memory.authority_kind,
        &memory.category,
        &memory.memory_type,
    )
}

pub(crate) fn canonical_superseders(
    cva: &mut Cva,
    source_id: MemoryId,
    relations: &[GraphRelation],
) -> Result<Vec<MemoryId>, DreamLifecycleError> {
    let service_relations = service_relations(relations);
    let mut authority_facts = Vec::new();
    for relation in relations.iter().filter(|relation| {
        relation.kind == GraphRelationKind::Supersedes
            && (relation.source == source_id || relation.target == source_id)
    }) {
        if unique_superseder(relations, relation.target) == Some(relation.source) {
            authority_facts.push(reliquary_dream::DreamCanonicalSupersederFact {
                target: relation.target.0,
                superseder: relation.source.0,
                target_had_canonical_authority: target_has_canonical_authority(
                    cva,
                    relation.target,
                    relation.source,
                )?,
            });
        }
    }
    Ok(
        reliquary_dream::canonical_superseders(source_id.0, &service_relations, &authority_facts)
            .into_iter()
            .map(MemoryId)
            .collect(),
    )
}

pub(crate) fn corroborated_representative(
    cva: &mut Cva,
    source_id: MemoryId,
    relations: &[GraphRelation],
    will_archive: &[MemoryId],
) -> Result<Option<MemoryId>, DreamLifecycleError> {
    let component = duplicate_component(source_id, relations);
    let mut memories = Vec::with_capacity(component.len());
    for id in component {
        let memory = cva.memory(id)?;
        memories.push(canonical_memory(cva, &memory, false));
    }
    Ok(reliquary_dream::corroborated_representative(
        source_id.0,
        &service_relations(relations),
        &memories,
        &will_archive.iter().map(|id| id.0).collect::<Vec<_>>(),
    )
    .map(MemoryId))
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

/// Historical owner proof pins final authority without using physical publication
/// order or excluding the representative's own birth cohort.
pub(crate) fn historical_duplicate_representative(
    cva: &mut Cva,
    root_id: MemoryId,
    relations: &[GraphRelation],
) -> Result<Option<MemoryId>, DreamLifecycleError> {
    let mut visited = HashSet::new();
    let mut queue = VecDeque::from([root_id]);
    let mut active = Vec::new();
    while let Some(id) = queue.pop_front() {
        if !visited.insert(id) {
            continue;
        }
        let Some(current) = historical_supersession_terminal(cva, id, relations)? else {
            return Ok(None);
        };
        queue.extend(duplicate_component(id, relations));
        if current != id {
            queue.push_back(current);
            continue;
        }
        let memory = cva.memory(current)?;
        if !memory.archived && cva.freshness_is_publication_birth(current) {
            active.push(canonical_memory(
                cva,
                &memory,
                cva.freshness_is_publication_birth(current),
            ));
        }
    }
    Ok(reliquary_dream::select_duplicate_representative(&active).map(MemoryId))
}

fn historical_supersession_terminal(
    cva: &mut Cva,
    root_id: MemoryId,
    relations: &[GraphRelation],
) -> Result<Option<MemoryId>, DreamLifecycleError> {
    let mut current = root_id;
    let mut visited = HashSet::new();
    let mut facts = Vec::new();
    while visited.insert(current) {
        let memory = cva.memory(current)?;
        let superseder = memory.superseded_by;
        let birth = cva.freshness_is_publication_birth(current);
        facts.push(canonical_memory(cva, &memory, birth));
        let Some(superseder) = superseder else {
            break;
        };
        // Root hydrates only owner-proved superseder births. A missing proof
        // remains a pure-policy rejection without causing a Memory lookup error.
        if !memory.archived
            || unique_superseder(relations, current) != Some(superseder)
            || !cva.freshness_is_publication_birth(superseder)
        {
            break;
        }
        current = superseder;
    }
    Ok(reliquary_dream::historical_supersession_terminal(
        root_id.0,
        &service_relations(relations),
        &facts,
    )
    .map(MemoryId))
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
            active.push(canonical_memory(cva, &memory, false));
        }
    }
    Ok(reliquary_dream::select_duplicate_representative(&active).map(MemoryId))
}

pub(crate) fn duplicate_component_has_active_peer(
    cva: &mut Cva,
    source_id: MemoryId,
    relations: &[GraphRelation],
    will_archive: &[MemoryId],
) -> Result<bool, DreamLifecycleError> {
    let component = duplicate_component(source_id, relations);
    let mut memories = Vec::with_capacity(component.len());
    for id in component {
        let memory = cva.memory(id)?;
        memories.push(canonical_memory(cva, &memory, false));
    }
    Ok(reliquary_dream::has_active_duplicate_peer(
        source_id.0,
        &service_relations(relations),
        &memories,
        &will_archive.iter().map(|id| id.0).collect::<Vec<_>>(),
    ))
}

fn duplicate_component(source_id: MemoryId, relations: &[GraphRelation]) -> Vec<MemoryId> {
    reliquary_dream::duplicate_component(source_id.0, &service_relations(relations))
        .into_iter()
        .map(MemoryId)
        .collect()
}

fn service_relations(relations: &[GraphRelation]) -> Vec<reliquary_graph::GraphRelation> {
    relations.iter().copied().map(Into::into).collect()
}

fn authority_anchor(cva: &Cva, memory: &Memory) -> Option<(String, String)> {
    let node_id = memory.source_node_id.clone()?;
    let conversation_id = memory
        .source_episode_id
        .and_then(|id| cva.episode(crate::EpisodeId((id).0)))
        .map(|episode| episode.conversation_id.clone())?;
    Some((conversation_id, node_id))
}

fn canonical_memory(
    cva: &Cva,
    memory: &Memory,
    publication_birth: bool,
) -> reliquary_dream::DreamCanonicalMemory {
    reliquary_dream::DreamCanonicalMemory {
        id: memory.id.0,
        lifecycle_state: memory.lifecycle_state.clone(),
        archived: memory.archived,
        superseded_by: memory.superseded_by.map(|id| id.0),
        source_timestamp_ns: reliquary_memory_source_timestamp_ns(&cva.archive, memory),
        authority_anchor: authority_anchor(cva, memory),
        publication_birth,
    }
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

pub(crate) fn unique_superseder(relations: &[GraphRelation], target: MemoryId) -> Option<MemoryId> {
    reliquary_dream::unique_superseder(&service_relations(relations), target.0).map(MemoryId)
}
