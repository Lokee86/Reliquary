//! Referential closure policy for an explicitly selected fresh-history rewrite.
use super::RewriteError;
use crate::{
    Entity, EntityId, MemoryId, Relationship, SemanticGraphRelation, SemanticNodeKind,
    SemanticNodeRef,
};
use std::collections::{BTreeSet, HashSet};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct SemanticClosure {
    /// Locally referenced Memory IDs added beyond the caller's initial selection.
    pub(crate) expanded_memory_ids: BTreeSet<MemoryId>,
    pub(crate) entities: Vec<Entity>,
    pub(crate) relationships: Vec<Relationship>,
    /// Active current relations only; old graph mutation history is not replayed.
    pub(crate) graph_relations: Vec<SemanticGraphRelation>,
}

pub(crate) fn semantic_closure(
    owner_id: Option<&str>,
    initially_retained: &BTreeSet<MemoryId>,
    available_memories: HashSet<MemoryId>,
    mut entities: Vec<Entity>,
    relationships: Vec<Relationship>,
    graph_relations: Vec<SemanticGraphRelation>,
) -> Result<SemanticClosure, RewriteError> {
    entities.sort_by_key(|entity| entity.id);
    let entity_by_id = entities
        .into_iter()
        .map(|entity| (entity.id, entity))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut retained_memories = initially_retained.clone();
    let mut retained_entities = BTreeSet::<EntityId>::new();
    let mut retained_relationships = BTreeSet::new();

    loop {
        let before = (
            retained_memories.len(),
            retained_entities.len(),
            retained_relationships.len(),
        );

        // A selected current graph edge is a semantic reference in both
        // directions: retain its local Memory and Entity endpoints.
        for relation in &graph_relations {
            if !relation_touches_closure(relation, &retained_memories, &retained_entities) {
                continue;
            }
            for node in [relation.source, relation.target] {
                match node.kind {
                    SemanticNodeKind::Memory => {
                        let memory = MemoryId(node.id);
                        if !available_memories.contains(&memory) {
                            return Err(RewriteError(format!(
                                "selected Graph relation references missing Memory {}",
                                hex32(&memory.0)
                            )));
                        }
                        retained_memories.insert(memory);
                    }
                    SemanticNodeKind::Entity => {
                        let entity = EntityId(node.id);
                        if entity_by_id.contains_key(&entity) {
                            retained_entities.insert(entity);
                        } else {
                            return Err(RewriteError(format!(
                                "selected Graph relation references missing Entity {}",
                                hex32(&entity.0)
                            )));
                        }
                    }
                    SemanticNodeKind::Observation => {
                        return Err(RewriteError(
                            "current Graph relation references an unsupported Observation node"
                                .into(),
                        ));
                    }
                }
            }
        }

        // A Relationship is included when its local evidence or a local Entity
        // participant intersects retained state. Its local evidence then joins
        // the Memory closure, rather than becoming an implicit dangling ref.
        for relationship in &relationships {
            let selected_by_memory = relationship.evidence.iter().any(|reference| {
                owner_id.is_some_and(|owner| reference.owner_id == owner)
                    && retained_memories.contains(&reference.memory_id)
            });
            let selected_by_entity = relationship.participants.iter().any(|participant| {
                owner_id.is_some_and(|owner| participant.entity.owner_id == owner)
                    && retained_entities.contains(&participant.entity.entity_id)
            });
            if !selected_by_memory && !selected_by_entity {
                continue;
            }
            retained_relationships.insert(relationship.id);
            for reference in &relationship.evidence {
                if owner_id.is_some_and(|owner| reference.owner_id == owner) {
                    if !available_memories.contains(&reference.memory_id) {
                        return Err(RewriteError(format!(
                            "selected Relationship {} references missing local Memory {}",
                            hex32(&relationship.id.0),
                            hex32(&reference.memory_id.0)
                        )));
                    }
                    retained_memories.insert(reference.memory_id);
                }
            }
            for participant in &relationship.participants {
                if owner_id.is_some_and(|owner| participant.entity.owner_id == owner) {
                    if !entity_by_id.contains_key(&participant.entity.entity_id) {
                        return Err(RewriteError(format!(
                            "selected Relationship {} references missing local Entity {}",
                            hex32(&relationship.id.0),
                            hex32(&participant.entity.entity_id.0)
                        )));
                    }
                    retained_entities.insert(participant.entity.entity_id);
                }
            }
        }

        let after = (
            retained_memories.len(),
            retained_entities.len(),
            retained_relationships.len(),
        );
        if before == after {
            break;
        }
    }

    let selected_graph = graph_relations
        .into_iter()
        .filter(|relation| {
            relation.source.kind != SemanticNodeKind::Observation
                && relation.target.kind != SemanticNodeKind::Observation
                && relation_node_retained(relation.source, &retained_memories, &retained_entities)
                && relation_node_retained(relation.target, &retained_memories, &retained_entities)
        })
        .collect();
    let selected_relationships = relationships
        .into_iter()
        .filter(|relationship| retained_relationships.contains(&relationship.id))
        .collect();
    let selected_entities = retained_entities
        .into_iter()
        .filter_map(|id| entity_by_id.get(&id).cloned())
        .collect();
    let expanded_memory_ids = retained_memories
        .difference(initially_retained)
        .copied()
        .collect();

    Ok(SemanticClosure {
        expanded_memory_ids,
        entities: selected_entities,
        relationships: selected_relationships,
        graph_relations: selected_graph,
    })
}

fn relation_touches_closure(
    relation: &SemanticGraphRelation,
    memories: &BTreeSet<MemoryId>,
    entities: &BTreeSet<EntityId>,
) -> bool {
    [relation.source, relation.target]
        .into_iter()
        .any(|node| match node.kind {
            SemanticNodeKind::Memory => memories.contains(&MemoryId(node.id)),
            SemanticNodeKind::Entity => entities.contains(&EntityId(node.id)),
            SemanticNodeKind::Observation => false,
        })
}

fn relation_node_retained(
    node: SemanticNodeRef,
    memories: &BTreeSet<MemoryId>,
    entities: &BTreeSet<EntityId>,
) -> bool {
    match node.kind {
        SemanticNodeKind::Memory => memories.contains(&MemoryId(node.id)),
        SemanticNodeKind::Entity => entities.contains(&EntityId(node.id)),
        SemanticNodeKind::Observation => false,
    }
}

fn hex32(bytes: &[u8; 32]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
