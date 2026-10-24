use crate::graph_relation_sets::{duplicate_component, unique_superseder};
use crate::memory_store::{MemoryOwnerWriter, MemoryReadView};
use crate::{DreamLifecycleError, GraphRelation, GraphRelationKind, Memory, MemoryId};
use std::collections::{HashMap, HashSet};

/// Current and historical Memory reads owned by the Memory store.
pub(crate) trait DreamMemoryRead {
    fn memory(&self, id: MemoryId) -> Result<Memory, crate::MemoryError>;
    fn memory_revision(&self, id: MemoryId, revision: u64) -> Result<Memory, crate::MemoryError>;
}

impl DreamMemoryRead for MemoryReadView<'_> {
    fn memory(&self, id: MemoryId) -> Result<Memory, crate::MemoryError> {
        MemoryReadView::memory(self, id)
    }
    fn memory_revision(&self, id: MemoryId, revision: u64) -> Result<Memory, crate::MemoryError> {
        MemoryReadView::memory_revision(self, id, revision)
    }
}

impl DreamMemoryRead for MemoryOwnerWriter<'_> {
    fn memory(&self, id: MemoryId) -> Result<Memory, crate::MemoryError> {
        MemoryOwnerWriter::memory(self, id)
    }
    fn memory_revision(&self, id: MemoryId, revision: u64) -> Result<Memory, crate::MemoryError> {
        MemoryOwnerWriter::memory_revision(self, id, revision)
    }
}

/// Additional owner facts for cross-publication representative selection.
pub(crate) struct DreamOwnerFacts {
    pub(crate) publication_births: HashSet<MemoryId>,
    pub(crate) cohorts: HashMap<(MemoryId, MemoryId), Result<bool, String>>,
    pub(crate) anchors: HashMap<MemoryId, Option<(String, String)>>,
    pub(crate) representative_keys: HashMap<MemoryId, (u8, i64, [u8; 32])>,
}

impl DreamOwnerFacts {
    fn is_publication_birth(&self, id: MemoryId) -> bool {
        self.publication_births.contains(&id)
    }

    fn same_publication_cohort(
        &self,
        a: MemoryId,
        b: MemoryId,
    ) -> Result<bool, DreamLifecycleError> {
        self.cohorts
            .get(&(a, b))
            .cloned()
            .ok_or_else(|| {
                DreamLifecycleError::Freshness(format!(
                    "missing prepared publication cohort fact for {:?} -> {:?}",
                    a, b
                ))
            })?
            .map_err(DreamLifecycleError::Freshness)
    }

    fn authority_anchor(
        &self,
        id: MemoryId,
    ) -> Result<Option<(String, String)>, DreamLifecycleError> {
        self.anchors.get(&id).cloned().ok_or_else(|| {
            DreamLifecycleError::Freshness(format!(
                "missing prepared authority anchor fact for {:?}",
                id
            ))
        })
    }

    fn representative_key(&self, id: MemoryId) -> Result<(u8, i64, [u8; 32]), DreamLifecycleError> {
        self.representative_keys.get(&id).copied().ok_or_else(|| {
            DreamLifecycleError::Freshness(format!(
                "missing prepared representative key for {:?}",
                id
            ))
        })
    }
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
    parts: &impl DreamMemoryRead,
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
    parts: &impl DreamMemoryRead,
    facts: &DreamOwnerFacts,
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
        if let Some(anchor) = facts.authority_anchor(memory.id)? {
            anchors.insert(anchor);
        }
        if !memory.archived {
            active.push(memory);
        }
    }
    if anchors.len() < 2 || active.is_empty() {
        return Ok(None);
    }
    let mut active: Vec<_> = active
        .into_iter()
        .map(|memory| Ok((facts.representative_key(memory.id)?, memory)))
        .collect::<Result<_, DreamLifecycleError>>()?;
    active.sort_by_key(|(key, _)| *key);
    Ok(active.first().map(|(_, memory)| memory.id))
}

pub(crate) fn existing_duplicate_representative(
    parts: &impl DreamMemoryRead,
    facts: &DreamOwnerFacts,
    root_id: MemoryId,
    source_birth_version: u64,
    source_id: MemoryId,
    relations: &[GraphRelation],
) -> Result<Option<MemoryId>, DreamLifecycleError> {
    duplicate_representative(
        parts,
        facts,
        root_id,
        source_birth_version,
        Some(source_id),
        relations,
    )
}

fn duplicate_representative(
    parts: &impl DreamMemoryRead,
    facts: &DreamOwnerFacts,
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
        if !facts.is_publication_birth(id) {
            continue;
        }
        let first = parts.memory_revision(id, 1)?;
        if first.global_version >= source_birth_version {
            continue;
        }
        if let Some(source_id) = source_id
            && facts.same_publication_cohort(source_id, id)?
        {
            continue;
        }
        let memory = parts.memory(id)?;
        if !memory.archived {
            active.push(memory);
        }
    }
    let mut active: Vec<_> = active
        .into_iter()
        .map(|memory| Ok((facts.representative_key(memory.id)?, memory)))
        .collect::<Result<_, DreamLifecycleError>>()?;
    active.sort_by_key(|(key, _)| *key);
    Ok(active.first().map(|(_, memory)| memory.id))
}

pub(crate) fn duplicate_component_has_active_peer(
    parts: &impl DreamMemoryRead,
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
    parts: &impl DreamMemoryRead,
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

#[cfg(test)]
mod owner_fact_tests {
    use super::*;

    struct NoMemoryReads;

    impl DreamMemoryRead for NoMemoryReads {
        fn memory(&self, _: MemoryId) -> Result<Memory, crate::MemoryError> {
            panic!("ineligible candidate must not load Memory")
        }
        fn memory_revision(&self, _: MemoryId, _: u64) -> Result<Memory, crate::MemoryError> {
            panic!("ineligible candidate must not load a Memory revision")
        }
    }

    #[test]
    fn deferred_history_error_is_not_surfaced_for_unrequested_cohort() {
        let a = MemoryId([1; 32]);
        let b = MemoryId([2; 32]);
        let unrelated = MemoryId([3; 32]);
        let facts = DreamOwnerFacts {
            publication_births: HashSet::from([a, b]),
            cohorts: HashMap::from([((a, b), Err("history unavailable".into()))]),
            anchors: HashMap::new(),
            representative_keys: HashMap::new(),
        };

        let ineligible = DreamOwnerFacts {
            publication_births: HashSet::new(),
            cohorts: facts.cohorts.clone(),
            anchors: HashMap::new(),
            representative_keys: HashMap::new(),
        };
        assert!(
            existing_duplicate_representative(&NoMemoryReads, &ineligible, b, 1, a, &[],)
                .unwrap()
                .is_none()
        );

        assert!(matches!(
            facts.same_publication_cohort(unrelated, b),
            Err(DreamLifecycleError::Freshness(message)) if message.contains("missing prepared publication cohort fact")
        ));
        assert!(matches!(
            facts.same_publication_cohort(a, b),
            Err(DreamLifecycleError::Freshness(message)) if message == "history unavailable"
        ));
    }
}
