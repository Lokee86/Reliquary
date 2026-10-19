//! Current semantic-state closure and fresh-baseline emission for rewrites.
//!
//! Observation is not a separately persisted owner store in the current format.
//! Relationship evidence is its durable local-memory reference surface, so this
//! module closes those references before deciding which semantic records survive.

use super::RewriteError;
use crate::{
    Cva, Entity, EntityDraft, EntityId, GraphRelationOrigin, MemoryId, Phylactery, Relationship,
    RelationshipDraft, SemanticGraphRelation, SemanticGraphRelationChange,
    SemanticGraphRelationKind, SemanticNodeKind, SemanticNodeRef,
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

pub(crate) fn semantic_closure_rel(
    source: &Cva,
    retained: &BTreeSet<MemoryId>,
) -> Result<SemanticClosure, RewriteError> {
    semantic_closure(
        source.owner_id().as_deref(),
        retained,
        source.memories.current_ids().into_iter().collect(),
        source.entities(),
        source.relationships(),
        source.graph.active_semantic_relations(),
    )
}

pub(crate) fn semantic_closure_phy(
    source: &Phylactery,
    retained: &BTreeSet<MemoryId>,
) -> Result<SemanticClosure, RewriteError> {
    semantic_closure(
        source.owner_id().as_deref(),
        retained,
        source.memories.current_ids().into_iter().collect(),
        source.entities(),
        source.relationships(),
        source
            .graph
            .active_semantic_relations()
            .into_iter()
            .filter(|relation| relation.kind != SemanticGraphRelationKind::PrincipalAssociation)
            .collect(),
    )
}

fn semantic_closure(
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

pub(crate) fn copy_semantic_state_rel(
    source: &Cva,
    destination: &mut Cva,
    closure: &SemanticClosure,
    include_rel_ego_synthesis: bool,
) -> Result<(), RewriteError> {
    copy_entities(source.owner_id().as_deref(), destination, &closure.entities)?;
    copy_relationships_rel(
        source.owner_id().as_deref(),
        destination,
        &closure.relationships,
    )?;
    copy_graph(destination, &closure.graph_relations)?;
    copy_rel_ego(source, destination, include_rel_ego_synthesis)?;
    Ok(())
}

pub(crate) fn copy_semantic_state_phy(
    source: &Phylactery,
    destination: &mut Phylactery,
    closure: &SemanticClosure,
) -> Result<(), RewriteError> {
    copy_entities(source.owner_id().as_deref(), destination, &closure.entities)?;
    copy_relationships_phy(
        source.owner_id().as_deref(),
        destination,
        &closure.relationships,
    )?;
    copy_graph(destination, &closure.graph_relations)?;
    copy_phy_ego(source, destination)?;
    Ok(())
}

trait SemanticDestination {
    fn owner_id(&self) -> Option<String>;
    fn replay_entity(&mut self, entity: &Entity) -> Result<(), RewriteError>;
    fn replay_relationship(&mut self, relationship: &Relationship) -> Result<(), RewriteError>;
    fn write_graph_baseline(
        &mut self,
        changes: &[SemanticGraphRelationChange],
        origin: GraphRelationOrigin,
    ) -> Result<(), RewriteError>;
}

impl SemanticDestination for Cva {
    fn owner_id(&self) -> Option<String> {
        Cva::owner_id(self)
    }

    fn replay_entity(&mut self, entity: &Entity) -> Result<(), RewriteError> {
        self.replay_entity_revision(
            entity.id,
            0,
            EntityDraft {
                canonical_name: entity.canonical_name.clone(),
                aliases: entity.aliases.clone(),
                kind: entity.kind.clone(),
                summary: entity.summary.clone(),
                mutation_id: entity.mutation_id.clone(),
                created_at_ns: entity.created_at_ns,
                updated_at_ns: entity.updated_at_ns,
            },
            entity.global_id,
        )
        .map(|_| ())
        .map_err(|error| RewriteError(error.to_string()))
    }

    fn replay_relationship(&mut self, relationship: &Relationship) -> Result<(), RewriteError> {
        self.replay_relationship_revision(
            Some(relationship.id),
            0,
            relationship_draft(relationship),
        )
        .map(|_| ())
        .map_err(|error| RewriteError(error.to_string()))
    }

    fn write_graph_baseline(
        &mut self,
        changes: &[SemanticGraphRelationChange],
        origin: GraphRelationOrigin,
    ) -> Result<(), RewriteError> {
        self.set_semantic_relations_with_origin(changes, origin, self.graph_version())
            .map(|_| ())
            .map_err(|error| RewriteError(error.to_string()))
    }
}

impl SemanticDestination for Phylactery {
    fn owner_id(&self) -> Option<String> {
        Phylactery::owner_id(self)
    }

    fn replay_entity(&mut self, entity: &Entity) -> Result<(), RewriteError> {
        self.replay_entity_revision(
            entity.id,
            0,
            EntityDraft {
                canonical_name: entity.canonical_name.clone(),
                aliases: entity.aliases.clone(),
                kind: entity.kind.clone(),
                summary: entity.summary.clone(),
                mutation_id: entity.mutation_id.clone(),
                created_at_ns: entity.created_at_ns,
                updated_at_ns: entity.updated_at_ns,
            },
            entity.global_id,
        )
        .map(|_| ())
        .map_err(|error| RewriteError(error.to_string()))
    }

    fn replay_relationship(&mut self, relationship: &Relationship) -> Result<(), RewriteError> {
        self.replay_relationship_revision(
            Some(relationship.id),
            0,
            relationship_draft(relationship),
        )
        .map(|_| ())
        .map_err(|error| RewriteError(error.to_string()))
    }

    fn write_graph_baseline(
        &mut self,
        changes: &[SemanticGraphRelationChange],
        origin: GraphRelationOrigin,
    ) -> Result<(), RewriteError> {
        self.set_semantic_relations_with_origin(changes, origin, self.graph_version())
            .map(|_| ())
            .map_err(|error| RewriteError(error.to_string()))
    }
}

fn copy_entities<D: SemanticDestination>(
    _source_owner_id: Option<&str>,
    destination: &mut D,
    entities: &[Entity],
) -> Result<(), RewriteError> {
    for entity in entities {
        destination.replay_entity(entity)?;
    }
    Ok(())
}

fn copy_relationships_rel(
    source_owner: Option<&str>,
    destination: &mut Cva,
    relationships: &[Relationship],
) -> Result<(), RewriteError> {
    copy_relationships(source_owner, destination, relationships)
}

fn copy_relationships_phy(
    source_owner: Option<&str>,
    destination: &mut Phylactery,
    relationships: &[Relationship],
) -> Result<(), RewriteError> {
    copy_relationships(source_owner, destination, relationships)
}

fn copy_relationships<D: SemanticDestination>(
    source_owner: Option<&str>,
    destination: &mut D,
    relationships: &[Relationship],
) -> Result<(), RewriteError> {
    let destination_owner = destination
        .owner_id()
        .ok_or_else(|| RewriteError("rewrite destination has no owner identity".into()))?;
    for original in relationships {
        let mut relationship = original.clone();
        for participant in &mut relationship.participants {
            if source_owner.is_some_and(|owner| participant.entity.owner_id == owner) {
                participant.entity.owner_id = destination_owner.clone();
            }
        }
        for reference in &mut relationship.evidence {
            if source_owner.is_some_and(|owner| reference.owner_id == owner) {
                reference.owner_id = destination_owner.clone();
            }
        }
        destination.replay_relationship(&relationship)?;
    }
    Ok(())
}

fn relationship_draft(relationship: &Relationship) -> RelationshipDraft {
    RelationshipDraft {
        kind: relationship.kind.clone(),
        participants: relationship.participants.clone(),
        evidence: relationship.evidence.clone(),
        summary: relationship.summary.clone(),
        mutation_id: relationship.mutation_id.clone(),
        created_at_ns: relationship.created_at_ns,
        updated_at_ns: relationship.updated_at_ns,
    }
}

fn copy_graph<D: SemanticDestination>(
    destination: &mut D,
    relations: &[SemanticGraphRelation],
) -> Result<(), RewriteError> {
    for origin in [
        GraphRelationOrigin::User,
        GraphRelationOrigin::Dream,
        GraphRelationOrigin::Perception,
    ] {
        let changes = relations
            .iter()
            .filter(|relation| relation.origin == origin)
            .map(|relation| SemanticGraphRelationChange {
                source: relation.source,
                target: relation.target,
                kind: relation.kind,
                active: true,
            })
            .collect::<Vec<_>>();
        if !changes.is_empty() {
            destination.write_graph_baseline(&changes, origin)?;
        }
    }
    Ok(())
}

fn copy_rel_ego(
    source: &Cva,
    destination: &mut Cva,
    include_synthesis: bool,
) -> Result<(), RewriteError> {
    let mut ego_version = destination.ego.version();
    for anchor in source.ego.anchors() {
        ego_version = ego_version
            .checked_add(1)
            .ok_or_else(|| RewriteError("Ego version exhausted".into()))?;
        append_ego_record(
            destination,
            crate::ego_codec::EgoRecord::Anchor {
                ego_version,
                id: anchor.id,
                revision: 1,
                deleted: false,
                priority: anchor.priority,
                text: anchor.text,
            },
        )?;
    }
    if include_synthesis && let Some(synthesis) = source.ego.synthesis() {
        ego_version = ego_version
            .checked_add(1)
            .ok_or_else(|| RewriteError("Ego version exhausted".into()))?;
        append_ego_record(
            destination,
            crate::ego_codec::EgoRecord::Synthesis {
                ego_version,
                revision: 1,
                source_memory_version: destination.memory_version(),
                text: synthesis.text.clone(),
            },
        )?;
    }
    Ok(())
}

fn copy_phy_ego(source: &Phylactery, destination: &mut Phylactery) -> Result<(), RewriteError> {
    let mut ego_version = destination.ego.version();
    let identities = source.ego.identities();
    let active_id = source.ego.active_identity_id();
    let first_id = identities.first().map(|identity| identity.id);
    for identity in identities {
        ego_version = ego_version
            .checked_add(1)
            .ok_or_else(|| RewriteError("Ego version exhausted".into()))?;
        append_ego_record(
            destination,
            crate::ego_codec::EgoRecord::Identity {
                ego_version,
                id: identity.id,
                revision: 1,
                deleted: false,
                activate: Some(identity.id) == first_id,
                name: identity.name,
                text: identity.text,
            },
        )?;
    }
    if let (Some(first), Some(active)) = (first_id, active_id)
        && first != active
    {
        ego_version = ego_version
            .checked_add(1)
            .ok_or_else(|| RewriteError("Ego version exhausted".into()))?;
        append_ego_record(
            destination,
            crate::ego_codec::EgoRecord::IdentityActive {
                ego_version,
                id: active,
            },
        )?;
    }
    if let Some(personality) = source.ego.personality() {
        ego_version = ego_version
            .checked_add(1)
            .ok_or_else(|| RewriteError("Ego version exhausted".into()))?;
        append_ego_record(
            destination,
            crate::ego_codec::EgoRecord::Personality {
                ego_version,
                revision: 1,
                source_memory_version: destination.memory_version(),
                text: personality.text.clone(),
            },
        )?;
    }
    for anchor in source.ego.anchors() {
        ego_version = ego_version
            .checked_add(1)
            .ok_or_else(|| RewriteError("Ego version exhausted".into()))?;
        append_ego_record(
            destination,
            crate::ego_codec::EgoRecord::Anchor {
                ego_version,
                id: anchor.id,
                revision: 1,
                deleted: false,
                priority: anchor.priority,
                text: anchor.text,
            },
        )?;
    }
    Ok(())
}

fn append_ego_record<O>(
    destination: &mut O,
    record: crate::ego_codec::EgoRecord,
) -> Result<(), RewriteError>
where
    O: EgoDestination,
{
    let bytes =
        crate::ego_codec::encode(&record).map_err(|error| RewriteError(error.to_string()))?;
    destination.append_ego_record(&bytes)
}

trait EgoDestination {
    fn append_ego_record(&mut self, bytes: &[u8]) -> Result<(), RewriteError>;
}
impl EgoDestination for Cva {
    fn append_ego_record(&mut self, bytes: &[u8]) -> Result<(), RewriteError> {
        self.container
            .append(bytes)
            .map_err(|error| RewriteError(error.to_string()))?;
        self.ego
            .ingest(bytes)
            .map_err(|error| RewriteError(error.to_string()))
    }
}
impl EgoDestination for Phylactery {
    fn append_ego_record(&mut self, bytes: &[u8]) -> Result<(), RewriteError> {
        self.container
            .append(bytes)
            .map_err(|error| RewriteError(error.to_string()))?;
        self.ego
            .ingest(bytes)
            .map_err(|error| RewriteError(error.to_string()))
    }
}

fn hex32(bytes: &[u8; 32]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
#[path = "semantic_state_tests.rs"]
mod tests;
