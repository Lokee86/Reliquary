use super::*;
use reliquary_graph::{CommunitySnapshot, RelationChange};
use reliquary_perception_store::{EntityDraft, PRINCIPAL_ENTITY_KIND};

impl SemanticClosure {
    pub(in crate::session::fresh_history_rewrite) fn copy_to(
        &self,
        source: &CommonState,
        source_owner: [u8; 16],
        target: &mut Container,
        target_memories: &mut MemoryStore,
    ) -> Result<(), String> {
        let destination_owner = target.owner_id();
        let mut perception = PerceptionStore::open(target).map_err(|error| error.to_string())?;

        for id in &self.entity_ids {
            let revisions = entity_revision_count(&source.perception, *id)?;
            for revision in 1..=revisions {
                let entity = source
                    .perception
                    .entity_revision(*id, revision)
                    .ok_or("selected Entity history is incomplete")?;
                let draft = EntityDraft {
                    canonical_name: entity.canonical_name.clone(),
                    aliases: entity.aliases.clone(),
                    kind: entity.kind.clone(),
                    summary: entity.summary.clone(),
                    mutation_id: entity.mutation_id.clone(),
                    created_at_ns: entity.created_at_ns,
                    updated_at_ns: entity.updated_at_ns,
                };
                let published = if revision == 1 {
                    perception.publish_entity(target, draft)
                } else {
                    perception.update_entity(target, *id, revision - 1, draft)
                }
                .map_err(|error| error.to_string())?;
                if published.id != *id || published.revision != revision {
                    return Err("fresh-history replay changed stable Entity history".into());
                }
            }
        }

        for current in &self.resolutions {
            for revision in 1..=current.revision {
                let value = source
                    .perception
                    .mention_revision(current.key, revision)
                    .ok_or("selected mention-resolution history is incomplete")?;
                perception
                    .put_mention_resolution(
                        target,
                        value.key,
                        revision - 1,
                        value.updated_at_ns,
                        value.status.clone(),
                    )
                    .map_err(|error| error.to_string())?;
            }
        }

        for id in &self.relationship_ids {
            let revisions = relationship_revision_count(&source.perception, *id)?;
            for revision in 1..=revisions {
                let relationship = source
                    .perception
                    .relationship_revision(*id, revision)
                    .ok_or("selected Relationship history is incomplete")?;
                let draft = relationship_draft(relationship, source_owner, destination_owner);
                let published = if revision == 1 {
                    perception.publish_relationship(target, draft)
                } else {
                    perception.update_relationship(target, *id, revision - 1, draft)
                }
                .map_err(|error| error.to_string())?;
                if published.id != *id || published.revision != revision {
                    return Err("fresh-history replay changed stable Relationship history".into());
                }
            }
        }

        for id in &self.entity_ids {
            if let Some(tombstone) = source.perception.entity_tombstone(*id) {
                perception
                    .tombstone_entity(target, *id, tombstone.revision)
                    .map_err(|error| error.to_string())?;
            }
        }
        for id in &self.relationship_ids {
            if let Some(tombstone) = source.perception.relationship_tombstone(*id) {
                perception
                    .tombstone_relationship(target, *id, tombstone.revision)
                    .map_err(|error| error.to_string())?;
            }
        }

        let admission = GraphAdmission::for_changes(
            destination_owner,
            self.memories.iter().map(|id| id.0),
            self.entity_ids.iter().map(|id| {
                let principal = source
                    .perception
                    .entity_revision(*id, 1)
                    .is_some_and(|entity| entity.kind == PRINCIPAL_ENTITY_KIND);
                (id.0, principal)
            }),
        );
        let mut graph = GraphStore::open(target, &admission).map_err(|error| error.to_string())?;
        if !self.relations.is_empty() {
            let changes = self
                .relations
                .iter()
                .map(|relation| RelationChange {
                    source: relation.source,
                    target: relation.target,
                    kind: relation.kind,
                    active: true,
                    origin: relation.origin,
                })
                .collect::<Vec<_>>();
            graph
                .put(target, &admission, &changes)
                .map_err(|error| error.to_string())?;
        }

        if let Some(current) = source.communities.current() {
            let communities = current
                .communities
                .iter()
                .filter_map(|community| {
                    let members = community
                        .members
                        .iter()
                        .copied()
                        .filter(|id| self.memories.contains(&MemoryId(*id)))
                        .collect::<Vec<_>>();
                    (!members.is_empty()).then_some(reliquary_graph::Community {
                        id: community.id,
                        members,
                    })
                })
                .collect::<Vec<_>>();
            if !communities.is_empty() {
                let mut store =
                    CommunityStore::open(target, &graph).map_err(|error| error.to_string())?;
                store
                    .publish(
                        target,
                        &graph,
                        CommunitySnapshot {
                            generation: 1,
                            graph_version: graph.graph_version(),
                            communities,
                        },
                    )
                    .map_err(|error| error.to_string())?;
            }
        }
        copy_ego(&source.ego, target, target_memories.memory_version())
    }

    pub(in crate::session::fresh_history_rewrite) fn replay_admission(
        &self,
        source: &CommonState,
        destination_owner: [u8; 16],
    ) -> Result<GraphAdmission, String> {
        let mut memories = Vec::new();
        let mut historical_memories = Vec::new();
        for id in &self.memories {
            let history = source
                .memories
                .history(*id)
                .map_err(|error| error.to_string())?;
            if history.tombstone.is_some() {
                historical_memories.push(id.0);
            } else {
                memories.push(id.0);
            }
        }
        let mut entities = Vec::new();
        let mut historical_entities = Vec::new();
        let mut historical_principals = Vec::new();
        for id in &self.entity_ids {
            let first = source
                .perception
                .entity_revision(*id, 1)
                .ok_or("selected Entity history is incomplete")?;
            let principal = first.kind == PRINCIPAL_ENTITY_KIND;
            if source.perception.entity_tombstone(*id).is_some() {
                historical_entities.push(id.0);
                if principal {
                    historical_principals.push(id.0);
                }
            } else {
                entities.push((id.0, principal));
            }
        }
        Ok(GraphAdmission::for_replay(
            destination_owner,
            memories,
            historical_memories,
            entities,
            historical_entities,
            historical_principals,
        ))
    }
}
