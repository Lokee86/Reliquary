use super::{GraphStore, RelationKey, validation};
use crate::entity_store::EntityStore;
use crate::graph_codec::{
    GraphMutationPayload, GraphVersionPayload, encode_batch, encode_mutation, encode_version,
};
use crate::memory_store::MemoryStore;
use crate::{
    Container, EntityId, GraphError, GraphRelation, GraphRelationChange, GraphRelationOrigin,
    MemoryId, SemanticGraphRelation, SemanticGraphRelationChange, SemanticGraphRelationKind,
    SemanticNodeRef,
};
use std::collections::HashSet;

impl GraphStore {
    pub(crate) fn set_relation(
        &mut self,
        container: &mut Container,
        memories: &MemoryStore,
        source: MemoryId,
        target: MemoryId,
        kind: crate::GraphRelationKind,
        active: bool,
        expected_graph_version: u64,
    ) -> Result<Option<GraphRelation>, GraphError> {
        container.require_storage_format_writable()?;
        self.set_relation_with_origin(
            container,
            memories,
            source,
            target,
            kind,
            active,
            GraphRelationOrigin::Dream,
            expected_graph_version,
        )
    }

    pub(crate) fn set_relation_with_origin(
        &mut self,
        container: &mut Container,
        memories: &MemoryStore,
        source: MemoryId,
        target: MemoryId,
        kind: crate::GraphRelationKind,
        active: bool,
        origin: GraphRelationOrigin,
        expected_graph_version: u64,
    ) -> Result<Option<GraphRelation>, GraphError> {
        container.require_storage_format_writable()?;
        Ok(self
            .set_relations_with_origin(
                container,
                memories,
                &[GraphRelationChange {
                    source,
                    target,
                    kind,
                    active,
                }],
                origin,
                expected_graph_version,
            )?
            .into_iter()
            .next())
    }

    pub(crate) fn set_relations(
        &mut self,
        container: &mut Container,
        memories: &MemoryStore,
        changes: &[GraphRelationChange],
        expected_graph_version: u64,
    ) -> Result<Vec<GraphRelation>, GraphError> {
        container.require_storage_format_writable()?;
        self.set_relations_with_origin(
            container,
            memories,
            changes,
            GraphRelationOrigin::Dream,
            expected_graph_version,
        )
    }

    pub(crate) fn set_relations_with_origin(
        &mut self,
        container: &mut Container,
        memories: &MemoryStore,
        changes: &[GraphRelationChange],
        origin: GraphRelationOrigin,
        expected_graph_version: u64,
    ) -> Result<Vec<GraphRelation>, GraphError> {
        container.require_storage_format_writable()?;
        for change in changes {
            validation::validate_memory_change(memories, *change, origin)?;
        }
        let semantic: Vec<_> = changes
            .iter()
            .map(|change| SemanticGraphRelationChange {
                source: SemanticNodeRef::memory(change.source),
                target: SemanticNodeRef::memory(change.target),
                kind: SemanticGraphRelationKind::Memory(change.kind),
                active: change.active,
            })
            .collect();
        Ok(self
            .publish_semantic_changes(container, &semantic, origin, expected_graph_version)?
            .into_iter()
            .filter_map(memory_relation)
            .collect())
    }

    pub(crate) fn set_entity_association(
        &mut self,
        container: &mut Container,
        memories: &MemoryStore,
        entities: &EntityStore,
        memory_id: MemoryId,
        entity_id: EntityId,
        active: bool,
        expected_graph_version: u64,
    ) -> Result<Option<SemanticGraphRelation>, GraphError> {
        container.require_storage_format_writable()?;
        Ok(self
            .set_semantic_relations_with_origin(
                container,
                memories,
                entities,
                &[SemanticGraphRelationChange::entity_association(
                    memory_id, entity_id, active,
                )],
                GraphRelationOrigin::Perception,
                expected_graph_version,
            )?
            .into_iter()
            .next())
    }

    pub(crate) fn set_principal_association(
        &mut self,
        container: &mut Container,
        memories: &MemoryStore,
        entities: &EntityStore,
        memory_id: MemoryId,
        entity_id: EntityId,
        active: bool,
        expected_graph_version: u64,
    ) -> Result<Option<SemanticGraphRelation>, GraphError> {
        container.require_storage_format_writable()?;
        Ok(self
            .set_semantic_relations_with_origin(
                container,
                memories,
                entities,
                &[SemanticGraphRelationChange::principal_association(
                    memory_id, entity_id, active,
                )],
                GraphRelationOrigin::Perception,
                expected_graph_version,
            )?
            .into_iter()
            .next())
    }

    pub(crate) fn set_semantic_relations_with_origin(
        &mut self,
        container: &mut Container,
        memories: &MemoryStore,
        entities: &EntityStore,
        changes: &[SemanticGraphRelationChange],
        origin: GraphRelationOrigin,
        expected_graph_version: u64,
    ) -> Result<Vec<SemanticGraphRelation>, GraphError> {
        container.require_storage_format_writable()?;
        for change in changes {
            validation::validate_change(memories, entities, *change, origin)?;
        }
        self.publish_semantic_changes(container, changes, origin, expected_graph_version)
    }

    fn publish_semantic_changes(
        &mut self,
        container: &mut Container,
        changes: &[SemanticGraphRelationChange],
        origin: GraphRelationOrigin,
        expected_graph_version: u64,
    ) -> Result<Vec<SemanticGraphRelation>, GraphError> {
        container.require_storage_format_writable()?;
        let current_version = self.graph_version();
        if expected_graph_version != current_version {
            return Err(GraphError::RevisionConflict {
                expected: expected_graph_version,
                actual: current_version,
            });
        }

        let mut keys = HashSet::with_capacity(changes.len());
        let mut effective = Vec::new();
        for change in changes {
            let key = RelationKey {
                source: change.source,
                target: change.target,
                kind: change.kind,
            };
            if !keys.insert(key) {
                return Err(GraphError::DuplicateRelationChange);
            }
            let state_changed = match self.states.get(&key) {
                Some(state) => state.active != change.active || state.origin != origin,
                None => change.active,
            };
            if state_changed {
                effective.push(*change);
            }
        }
        if effective.is_empty() {
            return Ok(Vec::new());
        }

        effective.sort_by_key(|change| {
            (
                change.source.kind.code(),
                change.source.id,
                change.target.kind.code(),
                change.target.id,
                change.kind.code(),
            )
        });
        self.ensure_format(container)?;
        for change in &effective {
            self.ensure_semantic_node(container, change.source)?;
            self.ensure_semantic_node(container, change.target)?;
        }

        let payloads: Vec<_> = effective
            .iter()
            .map(|change| GraphMutationPayload {
                source: change.source,
                target: change.target,
                kind: change.kind,
                active: change.active,
                origin,
            })
            .collect();
        let payload = if payloads.len() == 1 {
            encode_mutation(payloads[0])
        } else {
            encode_batch(&payloads)
        };
        let mutation = container.append(&payload)?;
        let graph_version = self.next_graph_version;
        let global_version = container.allocate_version()?;
        container.append(&encode_version(GraphVersionPayload {
            global_version,
            graph_version,
            mutation,
        }))?;

        let relations: Vec<_> = effective
            .into_iter()
            .map(|change| SemanticGraphRelation {
                source: change.source,
                target: change.target,
                kind: change.kind,
                active: change.active,
                origin,
                global_version,
                graph_version,
            })
            .collect();
        self.insert_transaction(&relations)?;
        self.next_graph_version = self
            .next_graph_version
            .checked_add(1)
            .ok_or(GraphError::GraphVersionExhausted)?;
        self.rebuild_topologies()?;
        Ok(relations)
    }
}

fn memory_relation(relation: SemanticGraphRelation) -> Option<GraphRelation> {
    let SemanticGraphRelationKind::Memory(kind) = relation.kind else {
        return None;
    };
    Some(GraphRelation {
        source: relation.source.as_memory()?,
        target: relation.target.as_memory()?,
        kind,
        active: relation.active,
        origin: relation.origin,
        global_version: relation.global_version,
        graph_version: relation.graph_version,
    })
}
