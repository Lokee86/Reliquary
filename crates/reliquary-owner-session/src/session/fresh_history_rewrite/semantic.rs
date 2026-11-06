//! Owner-local semantic closure for fresh-history reconstruction.

use super::*;
use reliquary_perception_store::{MemoryEntityResolutionStatus, RelationshipId};

mod replay;
mod support;
mod verify;
use support::*;

pub(super) struct SemanticClosure {
    pub(super) memories: BTreeSet<MemoryId>,
    pub(super) entity_ids: BTreeSet<EntityId>,
    pub(super) relationship_ids: BTreeSet<RelationshipId>,
    pub(super) resolutions: Vec<MemoryEntityResolution>,
    pub(super) relations: Vec<Relation>,
}

impl SemanticClosure {
    pub(super) fn build(
        source: &CommonState,
        source_owner: [u8; 16],
        memories: BTreeSet<MemoryId>,
    ) -> Result<Self, String> {
        let mut closure = Self {
            memories,
            entity_ids: BTreeSet::new(),
            relationship_ids: BTreeSet::new(),
            resolutions: Vec::new(),
            relations: Vec::new(),
        };
        let relationship_ids = all_relationship_ids(&source.perception);
        let graph_relations = source.graph.active_relations().copied().collect::<Vec<_>>();

        loop {
            let before = (
                closure.memories.len(),
                closure.entity_ids.len(),
                closure.relationship_ids.len(),
            );

            for resolution in source
                .perception
                .mentions()
                .filter(|value| closure.memories.contains(&MemoryId(value.key.memory_id)))
            {
                match &resolution.status {
                    MemoryEntityResolutionStatus::Resolved { entity_id, .. } => {
                        require_entity(source, *entity_id)?;
                        closure.entity_ids.insert(*entity_id);
                    }
                    MemoryEntityResolutionStatus::Ambiguous { candidates } => {
                        for id in candidates {
                            require_entity(source, *id)?;
                            closure.entity_ids.insert(*id);
                        }
                    }
                    MemoryEntityResolutionStatus::Unresolved { .. } => {}
                }
            }

            for relation in &graph_relations {
                if !relation_touches(&closure, relation) {
                    continue;
                }
                for node in [relation.source, relation.target] {
                    match node {
                        NodeRef::Memory(id) => {
                            let id = MemoryId(id);
                            require_memory(source, id)?;
                            closure.memories.insert(id);
                        }
                        NodeRef::Entity(id) => {
                            let id = EntityId(id);
                            require_entity(source, id)?;
                            closure.entity_ids.insert(id);
                        }
                    }
                }
            }

            for id in &relationship_ids {
                let relationship = latest_relationship(&source.perception, *id)?;
                let selected_by_memory = relationship.evidence.iter().any(|reference| {
                    owner_ref_matches(&reference.owner_id, source_owner)
                        && closure.memories.contains(&MemoryId(reference.memory_id))
                });
                let selected_by_entity = relationship.participants.iter().any(|participant| {
                    owner_ref_matches(&participant.entity.owner_id, source_owner)
                        && closure.entity_ids.contains(&participant.entity.entity_id)
                });
                if !selected_by_memory && !selected_by_entity {
                    continue;
                }
                closure.relationship_ids.insert(*id);
                for reference in &relationship.evidence {
                    if owner_ref_matches(&reference.owner_id, source_owner) {
                        let memory = MemoryId(reference.memory_id);
                        require_memory(source, memory)?;
                        closure.memories.insert(memory);
                    }
                }
                for participant in &relationship.participants {
                    if owner_ref_matches(&participant.entity.owner_id, source_owner) {
                        require_entity(source, participant.entity.entity_id)?;
                        closure.entity_ids.insert(participant.entity.entity_id);
                    }
                }
            }

            let after = (
                closure.memories.len(),
                closure.entity_ids.len(),
                closure.relationship_ids.len(),
            );
            if before == after {
                break;
            }
        }

        closure.resolutions = source
            .perception
            .mentions()
            .filter(|value| closure.memories.contains(&MemoryId(value.key.memory_id)))
            .cloned()
            .collect();
        closure.relations = graph_relations
            .into_iter()
            .filter(|relation| {
                [relation.source, relation.target]
                    .into_iter()
                    .all(|node| closure.contains_node(node))
            })
            .collect();
        closure.reject_pruned_descriptions(source)?;
        Ok(closure)
    }

    fn contains_node(&self, node: NodeRef) -> bool {
        match node {
            NodeRef::Memory(id) => self.memories.contains(&MemoryId(id)),
            NodeRef::Entity(id) => self.entity_ids.contains(&EntityId(id)),
        }
    }

    fn reject_pruned_descriptions(&self, source: &CommonState) -> Result<(), String> {
        for id in &self.entity_ids {
            let revisions = entity_revision_count(&source.perception, *id)?;
            if (1..=revisions)
                .any(|revision| source.perception.description_prune(*id, revision).is_some())
            {
                return Err(
                    "fresh-history source contains pruned Entity evidence that cannot be safely replayed"
                        .into(),
                );
            }
        }
        for id in &self.relationship_ids {
            let revisions = relationship_revision_count(&source.perception, *id)?;
            if (1..=revisions).any(|revision| {
                source
                    .perception
                    .relationship_description_prune(*id, revision)
                    .is_some()
            }) {
                return Err(
                    "fresh-history source contains pruned Relationship evidence that cannot be safely replayed"
                        .into(),
                );
            }
        }
        Ok(())
    }
}
