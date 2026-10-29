use crate::{DreamLifecycleError, GraphRelation, GraphRelationKind, MemoryId, Phylactery};

pub(crate) fn canonical_superseders(
    phy: &mut Phylactery,
    source_id: MemoryId,
    relations: &[GraphRelation],
) -> Result<Vec<MemoryId>, DreamLifecycleError> {
    let service_relations = service_relations(relations);
    let mut facts = Vec::new();
    for relation in relations.iter().filter(|relation| {
        relation.kind == GraphRelationKind::Supersedes
            && (relation.source == source_id || relation.target == source_id)
    }) {
        if unique_superseder(relations, relation.target) == Some(relation.source) {
            facts.push(reliquary_dream::DreamCanonicalSupersederFact {
                target: relation.target.0,
                superseder: relation.source.0,
                target_had_canonical_authority: target_has_canonical_authority(
                    phy,
                    relation.target,
                    relation.source,
                )?,
            });
        }
    }
    Ok(
        reliquary_dream::canonical_superseders(source_id.0, &service_relations, &facts)
            .into_iter()
            .map(MemoryId)
            .collect(),
    )
}

pub(crate) fn duplicate_component_has_active_peer(
    phy: &mut Phylactery,
    source_id: MemoryId,
    relations: &[GraphRelation],
    will_archive: &[MemoryId],
) -> Result<bool, DreamLifecycleError> {
    let component = duplicate_component(source_id, relations);
    let mut memories = Vec::with_capacity(component.len());
    for id in component {
        let memory = phy.memory(id)?;
        memories.push(reliquary_dream::DreamCanonicalMemory {
            id: id.0,
            lifecycle_state: memory.lifecycle_state,
            archived: memory.archived,
            superseded_by: memory.superseded_by.map(|id| id.0),
            source_timestamp_ns: None,
            authority_anchor: None,
            publication_birth: false,
        });
    }
    Ok(reliquary_dream::has_active_duplicate_peer(
        source_id.0,
        &service_relations(relations),
        &memories,
        &will_archive.iter().map(|id| id.0).collect::<Vec<_>>(),
    ))
}

pub(crate) fn unique_superseder(relations: &[GraphRelation], target: MemoryId) -> Option<MemoryId> {
    reliquary_dream::unique_superseder(&service_relations(relations), target.0).map(MemoryId)
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

fn target_has_canonical_authority(
    phy: &mut Phylactery,
    target: MemoryId,
    superseder: MemoryId,
) -> Result<bool, DreamLifecycleError> {
    let current = phy.memory(target)?;
    if !current.archived {
        return Ok(current.lifecycle_state == "canonical");
    }
    if current.superseded_by != Some(superseder) {
        return Ok(false);
    }
    for revision in (1..current.revision).rev() {
        let previous = phy.memory_revision(target, revision)?;
        if !previous.archived {
            return Ok(previous.lifecycle_state == "canonical");
        }
    }
    Ok(false)
}
