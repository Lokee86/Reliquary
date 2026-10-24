use crate::graph_store::GraphStore;
use crate::memory_store::MemoryStore;
use crate::{Container, EntityCandidateSet, EntityId, EntityResolverError, Memory, MemoryId};

pub(crate) struct EvidenceContext<'a> {
    pub(crate) memories: &'a MemoryStore,
    pub(crate) graph: &'a GraphStore,
    pub(crate) container: &'a Container,
}

pub(super) fn hydrate_admission_context(
    owner: &EvidenceContext<'_>,
    set: &EntityCandidateSet,
    source_id: MemoryId,
) -> Result<Vec<Memory>, EntityResolverError> {
    let mut context = Vec::new();
    for id in &set.admission_context_memory_ids {
        if *id == source_id {
            continue;
        }
        let memory = owner.memories.memory(owner.container, *id)?;
        if !memory.archived {
            context.push(memory);
        }
    }
    Ok(context)
}

pub(super) fn hydrate_evidence(
    owner: &EvidenceContext<'_>,
    set: &EntityCandidateSet,
    source_id: MemoryId,
) -> Result<Vec<Vec<Memory>>, EntityResolverError> {
    let mut all = Vec::with_capacity(set.candidates.len());
    for candidate in &set.candidates {
        let mut ids = Vec::new();
        for id in owner.graph.memories_for_entity(candidate.entity.id) {
            if id != source_id && !ids.contains(&id) {
                ids.push(id);
            }
        }
        for id in &candidate.supporting_memory_ids {
            if *id != source_id && !ids.contains(id) {
                ids.push(*id);
            }
        }
        ids.truncate(crate::MAX_ENTITY_RESOLVER_EVIDENCE_MEMORIES);

        let mut evidence = Vec::with_capacity(ids.len());
        for id in ids {
            let memory = owner.memories.memory(owner.container, id)?;
            if !memory.archived {
                evidence.push(memory);
            }
        }
        all.push(evidence);
    }
    Ok(all)
}
