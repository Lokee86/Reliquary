use super::{GraphStore, RelationKey, validation};
use crate::graph_codec::{GraphNodePayload, encode_format, encode_node};
use crate::graph_compact_codec::publication::{self, GraphPublication, GraphTransaction};
use crate::graph_compact_contract::GraphOrdinalMutation;
use crate::{
    EntityId, GraphAdmission, GraphError, GraphRelation, GraphRelationChange, GraphRelationOrigin,
    MemoryId, SemanticGraphRelation, SemanticGraphRelationChange, SemanticGraphRelationKind,
    SemanticNodeRef,
};
use arcana::NodeId;
use reliquary_container::Container;
use std::collections::{HashMap, HashSet};

impl GraphStore {
    pub fn set_relation(
        &mut self,
        container: &mut Container,
        admission: &GraphAdmission,
        source: MemoryId,
        target: MemoryId,
        kind: crate::GraphRelationKind,
        active: bool,
        expected_graph_version: u64,
    ) -> Result<Option<GraphRelation>, GraphError> {
        self.set_relation_with_origin(
            container,
            admission,
            source,
            target,
            kind,
            active,
            GraphRelationOrigin::Dream,
            expected_graph_version,
        )
    }

    pub fn set_relation_with_origin(
        &mut self,
        container: &mut Container,
        admission: &GraphAdmission,
        source: MemoryId,
        target: MemoryId,
        kind: crate::GraphRelationKind,
        active: bool,
        origin: GraphRelationOrigin,
        expected_graph_version: u64,
    ) -> Result<Option<GraphRelation>, GraphError> {
        Ok(self
            .set_relations_with_origin(
                container,
                admission,
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

    pub fn set_relations(
        &mut self,
        container: &mut Container,
        admission: &GraphAdmission,
        changes: &[GraphRelationChange],
        expected_graph_version: u64,
    ) -> Result<Vec<GraphRelation>, GraphError> {
        self.set_relations_with_origin(
            container,
            admission,
            changes,
            GraphRelationOrigin::Dream,
            expected_graph_version,
        )
    }

    pub fn set_relations_with_origin(
        &mut self,
        container: &mut Container,
        admission: &GraphAdmission,
        changes: &[GraphRelationChange],
        origin: GraphRelationOrigin,
        expected_graph_version: u64,
    ) -> Result<Vec<GraphRelation>, GraphError> {
        for change in changes {
            validation::validate_memory_change(admission, *change, origin)?;
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

    pub fn set_entity_association(
        &mut self,
        container: &mut Container,
        admission: &GraphAdmission,

        memory_id: MemoryId,
        entity_id: EntityId,
        active: bool,
        expected_graph_version: u64,
    ) -> Result<Option<SemanticGraphRelation>, GraphError> {
        Ok(self
            .set_semantic_relations_with_origin(
                container,
                admission,
                &[SemanticGraphRelationChange::entity_association(
                    memory_id, entity_id, active,
                )],
                GraphRelationOrigin::Perception,
                expected_graph_version,
            )?
            .into_iter()
            .next())
    }

    pub fn set_principal_association(
        &mut self,
        container: &mut Container,
        admission: &GraphAdmission,

        memory_id: MemoryId,
        entity_id: EntityId,
        active: bool,
        expected_graph_version: u64,
    ) -> Result<Option<SemanticGraphRelation>, GraphError> {
        Ok(self
            .set_semantic_relations_with_origin(
                container,
                admission,
                &[SemanticGraphRelationChange::principal_association(
                    memory_id, entity_id, active,
                )],
                GraphRelationOrigin::Perception,
                expected_graph_version,
            )?
            .into_iter()
            .next())
    }

    pub fn set_semantic_relations_with_origin(
        &mut self,
        container: &mut Container,
        admission: &GraphAdmission,

        changes: &[SemanticGraphRelationChange],
        origin: GraphRelationOrigin,
        expected_graph_version: u64,
    ) -> Result<Vec<SemanticGraphRelation>, GraphError> {
        for change in changes {
            validation::validate_change(admission, *change, origin)?;
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
        self.publish_semantic_changes_with_shape(
            container,
            changes,
            origin,
            expected_graph_version,
            None,
            false,
        )
    }

    pub fn replay_semantic_transaction(
        &mut self,
        container: &mut Container,
        admission: &GraphAdmission,

        changes: &[SemanticGraphRelationChange],
        origin: GraphRelationOrigin,
        expected_graph_version: u64,
        single: bool,
    ) -> Result<Vec<SemanticGraphRelation>, GraphError> {
        self.replay_semantic_transaction_with_postimages(
            container,
            admission,
            changes,
            origin,
            expected_graph_version,
            single,
            false,
        )
    }

    pub fn replay_import_semantic_transaction(
        &mut self,
        container: &mut Container,
        admission: &GraphAdmission,

        changes: &[SemanticGraphRelationChange],
        origin: GraphRelationOrigin,
        expected_graph_version: u64,
        single: bool,
    ) -> Result<Vec<SemanticGraphRelation>, GraphError> {
        self.replay_semantic_transaction_with_postimages(
            container,
            admission,
            changes,
            origin,
            expected_graph_version,
            single,
            true,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn replay_semantic_transaction_with_postimages(
        &mut self,
        container: &mut Container,
        admission: &GraphAdmission,

        changes: &[SemanticGraphRelationChange],
        origin: GraphRelationOrigin,
        expected_graph_version: u64,
        single: bool,
        preserve_postimages: bool,
    ) -> Result<Vec<SemanticGraphRelation>, GraphError> {
        for change in changes {
            validation::validate_change(admission, *change, origin)?;
        }
        self.publish_semantic_changes_with_shape(
            container,
            changes,
            origin,
            expected_graph_version,
            Some(single),
            preserve_postimages,
        )
    }

    fn publish_semantic_changes_with_shape(
        &mut self,
        container: &mut Container,
        changes: &[SemanticGraphRelationChange],
        origin: GraphRelationOrigin,
        expected_graph_version: u64,
        source_single: Option<bool>,
        preserve_postimages: bool,
    ) -> Result<Vec<SemanticGraphRelation>, GraphError> {
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
            if state_changed || preserve_postimages {
                effective.push(*change);
            }
        }
        if effective.is_empty() {
            return Ok(Vec::new());
        }

        if source_single.is_none() {
            effective.sort_by_key(|change| {
                (
                    change.source.kind.code(),
                    change.source.id,
                    change.target.kind.code(),
                    change.target.id,
                    change.kind.code(),
                )
            });
        }
        let single = source_single.unwrap_or(effective.len() == 1);
        if single && effective.len() != 1 {
            return Err(GraphError::InvalidGraphVersion);
        }
        let graph_version = self.next_graph_version;
        let next_graph_version = graph_version
            .checked_add(1)
            .ok_or(GraphError::GraphVersionExhausted)?;
        let global = container
            .latest_version()
            .checked_add(1)
            .ok_or(reliquary_container::ContainerError::VersionExhausted)?;
        global
            .checked_add(1)
            .ok_or(reliquary_container::ContainerError::VersionExhausted)?;
        let new_nodes = effective
            .iter()
            .flat_map(|change| [change.source, change.target])
            .filter(|node| !self.node_by_semantic.contains_key(node))
            .collect::<HashSet<_>>();
        if !new_nodes.is_empty() {
            let last = self
                .nodes
                .len()
                .checked_add(new_nodes.len())
                .and_then(|count| count.checked_sub(1))
                .ok_or(GraphError::NodeIdExhausted)?;
            u32::try_from(last).map_err(|_| GraphError::NodeIdExhausted)?;
        }
        // Prepare ordinal assignments without installing them into the live
        // store. A failed transaction-envelope append must not publish the
        // staged Graph mappings or advance in-memory semantic state.
        let mut staged_nodes = Vec::with_capacity(new_nodes.len());
        let mut staged_ids = HashMap::with_capacity(new_nodes.len());
        let mut ordinals = Vec::with_capacity(effective.len());
        for change in &effective {
            let source = planned_node_id(self, change.source, &mut staged_nodes, &mut staged_ids)?;
            let target = planned_node_id(self, change.target, &mut staged_nodes, &mut staged_ids)?;
            ordinals.push(GraphOrdinalMutation {
                source: source.0,
                target: target.0,
                kind: change.kind,
                active: change.active,
                origin,
            });
        }
        let transaction = if single {
            if ordinals.len() != 1 {
                return Err(GraphError::InvalidGraphVersion);
            }
            GraphTransaction::Single(ordinals[0])
        } else {
            GraphTransaction::Batch(ordinals)
        };
        let body = publication::encode(&GraphPublication {
            graph_version,
            transaction,
        })?;

        if !self.format_initialized {
            container.append(&encode_format())?;
        }
        for node in &staged_nodes {
            container.append(&encode_node(GraphNodePayload {
                semantic_node: node.semantic_node,
                node_id: node.node_id,
            }))?;
        }
        let (global_version, _) = container.append_publication_envelope(&body)?;

        // Commit staged mappings only after the sole transaction publication
        // has appended successfully.
        self.format_initialized = true;
        for node in staged_nodes {
            self.node_by_semantic
                .insert(node.semantic_node, node.node_id);
            self.nodes.push(node);
        }

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
        self.next_graph_version = next_graph_version;
        self.rebuild_topologies()?;
        Ok(relations)
    }
}

fn planned_node_id(
    store: &GraphStore,
    semantic_node: SemanticNodeRef,
    staged_nodes: &mut Vec<crate::GraphNodeRecord>,
    staged_ids: &mut HashMap<SemanticNodeRef, NodeId>,
) -> Result<NodeId, GraphError> {
    if let Some(node_id) = store.node_by_semantic.get(&semantic_node) {
        return Ok(*node_id);
    }
    if let Some(node_id) = staged_ids.get(&semantic_node) {
        return Ok(*node_id);
    }
    let index = store
        .nodes
        .len()
        .checked_add(staged_nodes.len())
        .ok_or(GraphError::NodeIdExhausted)?;
    let node_id = NodeId(u32::try_from(index).map_err(|_| GraphError::NodeIdExhausted)?);
    staged_nodes.push(crate::GraphNodeRecord {
        semantic_node,
        node_id,
    });
    staged_ids.insert(semantic_node, node_id);
    Ok(node_id)
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
