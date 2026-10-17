use super::GraphStore;
use crate::{
    EntityId, GraphError, GraphRelation, GraphStats, MemoryId, SemanticGraphRelation,
    SemanticGraphRelationKind, SemanticNodeRef,
};
use arcana::storage::InMemoryGraph;
use arcana::{Edge, GraphDataset, NodeId};

impl GraphStore {
    pub(crate) fn graph_version(&self) -> u64 {
        self.next_graph_version.saturating_sub(1)
    }

    pub(crate) fn memory_graph_version(&self) -> u64 {
        self.memory_graph_version
    }

    pub(crate) fn stats(&self) -> GraphStats {
        GraphStats {
            nodes: self
                .nodes
                .iter()
                .filter(|node| !self.excluded_nodes.contains(&node.semantic_node))
                .count(),
            memory_nodes: self.memory_nodes.len(),
            active_relations: self
                .states
                .values()
                .filter(|relation| self.is_current_relation(relation))
                .count(),
            memory_active_relations: self
                .states
                .values()
                .filter(|relation| {
                    self.is_current_relation(relation)
                        && matches!(relation.kind, SemanticGraphRelationKind::Memory(_))
                })
                .count(),
            relation_mutations: self.relation_mutations,
            graph_version: self.graph_version(),
            memory_graph_version: self.memory_graph_version(),
        }
    }

    pub(crate) fn active_semantic_relations(&self) -> Vec<SemanticGraphRelation> {
        let mut relations: Vec<_> = self
            .states
            .values()
            .filter(|relation| self.is_current_relation(relation))
            .copied()
            .collect();
        relations.sort_by_key(semantic_relation_sort_key);
        relations
    }

    pub(crate) fn active_relations(&self) -> Vec<GraphRelation> {
        self.active_semantic_relations()
            .into_iter()
            .filter_map(memory_relation)
            .collect()
    }

    pub(crate) fn entity_associations_for_memory(&self, memory_id: MemoryId) -> Vec<EntityId> {
        self.entities_by_memory
            .get(&memory_id)
            .map(|ids| ids.iter().copied().collect())
            .unwrap_or_default()
    }

    pub(crate) fn memories_for_entity(&self, entity_id: EntityId) -> Vec<MemoryId> {
        let mut ids: Vec<_> = self
            .memories_by_entity
            .get(&entity_id)
            .into_iter()
            .flat_map(|ids| ids.iter().copied())
            .collect();
        ids.sort_by_key(|id| id.0);
        ids
    }

    pub(crate) fn principal_associations_for_memory(&self, memory_id: MemoryId) -> Vec<EntityId> {
        self.principals_by_memory
            .get(&memory_id)
            .map(|ids| ids.iter().copied().collect())
            .unwrap_or_default()
    }

    pub(crate) fn memories_for_principal(&self, entity_id: EntityId) -> Vec<MemoryId> {
        let mut ids: Vec<_> = self
            .memories_by_principal
            .get(&entity_id)
            .into_iter()
            .flat_map(|ids| ids.iter().copied())
            .collect();
        ids.sort_by_key(|id| id.0);
        ids
    }

    pub(crate) fn relation_state(
        &self,
        source: MemoryId,
        target: MemoryId,
        kind: crate::GraphRelationKind,
    ) -> Option<GraphRelation> {
        self.states.values().find_map(|relation| {
            let mapped = memory_relation(*relation)?;
            (mapped.source == source && mapped.target == target && mapped.kind == kind)
                .then_some(mapped)
        })
    }

    pub(crate) fn transaction_global_versions(&self) -> &[u64] {
        &self.transaction_global_versions
    }

    pub(crate) fn node_memory_ids(&self) -> Vec<MemoryId> {
        self.memory_nodes.clone()
    }

    pub(crate) fn memory_node_count(&self) -> usize {
        self.memory_nodes.len()
    }

    pub(crate) fn current_memory_node_ids(&self) -> &[MemoryId] {
        &self.memory_nodes
    }

    #[cfg(test)]
    pub(crate) fn semantic_nodes(&self) -> Vec<SemanticNodeRef> {
        self.nodes.iter().map(|node| node.semantic_node).collect()
    }

    pub(crate) fn node_id(&self, memory_id: MemoryId) -> Result<NodeId, GraphError> {
        self.semantic_node_id(SemanticNodeRef::memory(memory_id))
            .map_err(|error| match error {
                GraphError::UnsupportedSemanticNode(_) => GraphError::MissingNode(memory_id),
                other => other,
            })
    }

    pub(crate) fn historical_memory_node_id(&self, id: MemoryId) -> Result<NodeId, GraphError> {
        self.node_by_semantic
            .get(&SemanticNodeRef::memory(id))
            .copied()
            .ok_or(GraphError::MissingNode(id))
    }

    pub(crate) fn semantic_node_id(
        &self,
        semantic_node: SemanticNodeRef,
    ) -> Result<NodeId, GraphError> {
        if self.excluded_nodes.contains(&semantic_node) {
            return Err(GraphError::UnsupportedSemanticNode(semantic_node));
        }
        self.node_by_semantic
            .get(&semantic_node)
            .copied()
            .ok_or(GraphError::UnsupportedSemanticNode(semantic_node))
    }

    pub(crate) fn memory_projection_node_id(
        &self,
        memory_id: MemoryId,
    ) -> Result<NodeId, GraphError> {
        self.memory_node_by_id
            .get(&memory_id)
            .copied()
            .ok_or(GraphError::MissingNode(memory_id))
    }

    pub(crate) fn memory_projection_memory_id(
        &self,
        node_id: NodeId,
    ) -> Result<MemoryId, GraphError> {
        self.memory_nodes
            .get(node_id.0 as usize)
            .copied()
            .ok_or_else(|| {
                GraphError::Topology(format!("unknown Memory projection node {}", node_id.0))
            })
    }

    pub(crate) fn semantic_node(&self, node_id: NodeId) -> Result<SemanticNodeRef, GraphError> {
        self.nodes
            .get(node_id.0 as usize)
            .map(|node| node.semantic_node)
            .ok_or_else(|| GraphError::Topology(format!("unknown dense node {}", node_id.0)))
    }

    pub(crate) fn topology(&self) -> &InMemoryGraph {
        &self.topology
    }

    pub(crate) fn memory_topology(&self) -> &InMemoryGraph {
        &self.memory_topology
    }

    pub(super) fn rebuild_topologies(&mut self) -> Result<(), GraphError> {
        self.rebuild_entity_association_indexes()?;
        self.rebuild_memory_projection()?;

        let mut edges = Vec::new();
        for relation in self.states.values().filter(|relation| {
            self.is_current_relation(relation)
                && relation.kind != SemanticGraphRelationKind::PrincipalAssociation
        }) {
            edges.push(Edge {
                source: self.semantic_node_id(relation.source)?,
                target: self.semantic_node_id(relation.target)?,
                kind: relation.kind.edge_kind(),
            });
        }
        edges.sort_unstable();
        self.topology = InMemoryGraph::new(&GraphDataset {
            node_count: u32::try_from(self.nodes.len()).map_err(|_| GraphError::NodeIdExhausted)?,
            edges,
        })
        .map_err(|error| GraphError::Topology(error.to_string()))?;
        Ok(())
    }

    fn rebuild_entity_association_indexes(&mut self) -> Result<(), GraphError> {
        self.entities_by_memory.clear();
        self.memories_by_entity.clear();
        self.principals_by_memory.clear();
        self.memories_by_principal.clear();
        let excluded = &self.excluded_nodes;
        for relation in self.states.values().filter(|relation| {
            relation.active
                && !excluded.contains(&relation.source)
                && !excluded.contains(&relation.target)
                && matches!(
                    relation.kind,
                    SemanticGraphRelationKind::EntityAssociation
                        | SemanticGraphRelationKind::PrincipalAssociation
                )
        }) {
            let memory_id = relation
                .source
                .as_memory()
                .ok_or(GraphError::InvalidRelationShape)?;
            let entity_id = relation
                .target
                .as_entity()
                .ok_or(GraphError::InvalidRelationShape)?;
            match relation.kind {
                SemanticGraphRelationKind::EntityAssociation => {
                    self.entities_by_memory
                        .entry(memory_id)
                        .or_default()
                        .insert(entity_id);
                    self.memories_by_entity
                        .entry(entity_id)
                        .or_default()
                        .insert(memory_id);
                }
                SemanticGraphRelationKind::PrincipalAssociation => {
                    self.principals_by_memory
                        .entry(memory_id)
                        .or_default()
                        .insert(entity_id);
                    self.memories_by_principal
                        .entry(entity_id)
                        .or_default()
                        .insert(memory_id);
                }
                SemanticGraphRelationKind::Memory(_) => unreachable!(),
            }
        }
        Ok(())
    }

    fn rebuild_memory_projection(&mut self) -> Result<(), GraphError> {
        self.memory_nodes = self
            .nodes
            .iter()
            .filter_map(|node| node.semantic_node.as_memory())
            .filter(|memory_id| {
                self.memory_relation_nodes.contains(memory_id)
                    && !self
                        .excluded_nodes
                        .contains(&SemanticNodeRef::memory(*memory_id))
            })
            .collect();
        self.memory_node_by_id.clear();
        for (index, memory_id) in self.memory_nodes.iter().copied().enumerate() {
            let node_id = NodeId(u32::try_from(index).map_err(|_| GraphError::NodeIdExhausted)?);
            self.memory_node_by_id.insert(memory_id, node_id);
        }

        let mut edges = Vec::new();
        for relation in self
            .states
            .values()
            .filter(|relation| self.is_current_relation(relation))
        {
            let SemanticGraphRelationKind::Memory(kind) = relation.kind else {
                continue;
            };
            let Some(source) = relation.source.as_memory() else {
                return Err(GraphError::InvalidRelationShape);
            };
            let Some(target) = relation.target.as_memory() else {
                return Err(GraphError::InvalidRelationShape);
            };
            edges.push(Edge {
                source: self.memory_projection_node_id(source)?,
                target: self.memory_projection_node_id(target)?,
                kind: kind.into(),
            });
        }
        edges.sort_unstable();
        self.memory_topology = InMemoryGraph::new(&GraphDataset {
            node_count: u32::try_from(self.memory_nodes.len())
                .map_err(|_| GraphError::NodeIdExhausted)?,
            edges,
        })
        .map_err(|error| GraphError::Topology(error.to_string()))?;
        Ok(())
    }
}

impl From<crate::GraphRelationKind> for arcana::EdgeKind {
    fn from(value: crate::GraphRelationKind) -> Self {
        arcana::EdgeKind(value.code())
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

fn semantic_relation_sort_key(
    relation: &SemanticGraphRelation,
) -> (u8, [u8; 32], u8, [u8; 32], u16) {
    (
        relation.source.kind.code(),
        relation.source.id,
        relation.target.kind.code(),
        relation.target.id,
        relation.kind.code(),
    )
}
