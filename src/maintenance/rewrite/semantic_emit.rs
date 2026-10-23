//! Canonical selected semantic-state emission for fresh-history rewrite.
use super::{RewriteError, closure::SemanticClosure};
use crate::{
    Entity, EntityDraft, GraphRelationOrigin, Relationship, RelationshipDraft,
    SemanticGraphRelation, SemanticGraphRelationChange,
};

pub(crate) fn copy_semantic_state_rel(
    source_owner: Option<&str>,
    source_ego: &crate::ego_store::EgoStore,
    destination: &mut SemanticRewriteDestination<'_>,
    closure: &SemanticClosure,
    include_rel_ego_synthesis: bool,
) -> Result<(), RewriteError> {
    copy_entities(source_owner, destination, &closure.entities)?;
    copy_relationships(source_owner, destination, &closure.relationships)?;
    copy_graph(destination, &closure.graph_relations)?;
    copy_rel_ego(source_ego, destination, include_rel_ego_synthesis)?;
    Ok(())
}

pub(crate) fn copy_semantic_state_phy(
    source_owner: Option<&str>,
    source_ego: &crate::ego_store::EgoStore,
    destination: &mut SemanticRewriteDestination<'_>,
    closure: &SemanticClosure,
) -> Result<(), RewriteError> {
    copy_entities(source_owner, destination, &closure.entities)?;
    copy_relationships(source_owner, destination, &closure.relationships)?;
    copy_graph(destination, &closure.graph_relations)?;
    copy_phy_ego(source_ego, destination)?;
    Ok(())
}

pub(crate) struct SemanticRewriteDestination<'a> {
    pub(crate) container: &'a mut crate::Container,
    pub(crate) memories: &'a crate::memory_store::MemoryStore,
    pub(crate) entities: &'a mut crate::entity_store::EntityStore,
    pub(crate) relationships: &'a mut crate::relationship_store::RelationshipStore,
    pub(crate) graph: &'a mut crate::graph_store::GraphStore,
    pub(crate) ego: &'a mut crate::ego_store::EgoStore,
}
impl SemanticRewriteDestination<'_> {
    fn owner_id(&self) -> Option<String> {
        self.container.owner_id()
    }
    fn memory_version(&self) -> u64 {
        self.memories.memory_version()
    }
    fn replay_entity(&mut self, entity: &Entity) -> Result<(), RewriteError> {
        self.entities
            .publish_with_global_id(
                self.container,
                Some(entity.id),
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
        self.relationships
            .publish_replayed(
                self.container,
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
        crate::graph_endpoint_admission::validate_semantic_changes(
            self.memories,
            self.entities,
            changes,
            origin,
        )
        .map_err(|error| RewriteError(error.to_string()))?;
        self.graph
            .set_semantic_relations_with_origin(
                self.container,
                changes,
                origin,
                self.graph.graph_version(),
            )
            .map(|_| ())
            .map_err(|error| RewriteError(error.to_string()))
    }
    fn append_ego_record(&mut self, bytes: &[u8]) -> Result<(), RewriteError> {
        self.container
            .append(bytes)
            .map_err(|error| RewriteError(error.to_string()))?;
        self.ego
            .ingest(bytes)
            .map_err(|error| RewriteError(error.to_string()))
    }
}

fn copy_entities(
    _source_owner_id: Option<&str>,
    destination: &mut SemanticRewriteDestination<'_>,
    entities: &[Entity],
) -> Result<(), RewriteError> {
    for entity in entities {
        destination.replay_entity(entity)?;
    }
    Ok(())
}

fn copy_relationships(
    source_owner: Option<&str>,
    destination: &mut SemanticRewriteDestination<'_>,
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

fn copy_graph(
    destination: &mut SemanticRewriteDestination<'_>,
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
    source: &crate::ego_store::EgoStore,
    destination: &mut SemanticRewriteDestination<'_>,
    include_synthesis: bool,
) -> Result<(), RewriteError> {
    let mut ego_version = destination.ego.version();
    for anchor in source.anchors() {
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
    if include_synthesis && let Some(synthesis) = source.synthesis() {
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

fn copy_phy_ego(
    source: &crate::ego_store::EgoStore,
    destination: &mut SemanticRewriteDestination<'_>,
) -> Result<(), RewriteError> {
    let mut ego_version = destination.ego.version();
    let identities = source.identities();
    let active_id = source.active_identity_id();
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
    if let Some(personality) = source.personality() {
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
    for anchor in source.anchors() {
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

fn append_ego_record(
    destination: &mut SemanticRewriteDestination<'_>,
    record: crate::ego_codec::EgoRecord,
) -> Result<(), RewriteError> {
    let bytes = crate::ego_codec::compact::encode_initial(&record)
        .map_err(|error| RewriteError(error.to_string()))?;
    destination.append_ego_record(&bytes)
}
