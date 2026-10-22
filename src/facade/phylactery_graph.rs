use crate::{
    EntityId, GraphDirection, GraphError, GraphNeighbor, GraphRelation, GraphRelationChange,
    GraphRelationKind, GraphRelationOrigin, GraphStats, MemoryGraphPath, MemoryId, Phylactery,
    SemanticGraphNeighbor, SemanticGraphRelation, SemanticGraphRelationChange, SemanticNodeRef,
};

impl Phylactery {
    pub fn set_memory_relation(
        &mut self,
        source: MemoryId,
        target: MemoryId,
        kind: GraphRelationKind,
        active: bool,
        expected_graph_version: u64,
    ) -> Result<Option<GraphRelation>, GraphError> {
        crate::graph_endpoint_admission::validate_memory_changes(
            &self.memories,
            &[GraphRelationChange {
                source,
                target,
                kind,
                active,
            }],
            GraphRelationOrigin::Dream,
        )?;
        self.graph.set_relation(
            &mut self.container,
            source,
            target,
            kind,
            active,
            expected_graph_version,
        )
    }

    pub fn set_memory_relation_with_origin(
        &mut self,
        source: MemoryId,
        target: MemoryId,
        kind: GraphRelationKind,
        active: bool,
        origin: GraphRelationOrigin,
        expected_graph_version: u64,
    ) -> Result<Option<GraphRelation>, GraphError> {
        crate::graph_endpoint_admission::validate_memory_changes(
            &self.memories,
            &[GraphRelationChange {
                source,
                target,
                kind,
                active,
            }],
            origin,
        )?;
        self.graph.set_relation_with_origin(
            &mut self.container,
            source,
            target,
            kind,
            active,
            origin,
            expected_graph_version,
        )
    }

    pub fn set_memory_relations(
        &mut self,
        changes: &[GraphRelationChange],
        expected_graph_version: u64,
    ) -> Result<Vec<GraphRelation>, GraphError> {
        crate::graph_endpoint_admission::validate_memory_changes(
            &self.memories,
            changes,
            GraphRelationOrigin::Dream,
        )?;
        self.graph
            .set_relations(&mut self.container, changes, expected_graph_version)
    }

    pub fn set_memory_relations_with_origin(
        &mut self,
        changes: &[GraphRelationChange],
        origin: GraphRelationOrigin,
        expected_graph_version: u64,
    ) -> Result<Vec<GraphRelation>, GraphError> {
        crate::graph_endpoint_admission::validate_memory_changes(&self.memories, changes, origin)?;
        self.graph.set_relations_with_origin(
            &mut self.container,
            changes,
            origin,
            expected_graph_version,
        )
    }

    pub fn set_entity_association(
        &mut self,
        memory_id: MemoryId,
        entity_id: EntityId,
        active: bool,
        expected_graph_version: u64,
    ) -> Result<Option<SemanticGraphRelation>, GraphError> {
        let change = SemanticGraphRelationChange::entity_association(memory_id, entity_id, active);
        crate::graph_endpoint_admission::validate_semantic_changes(
            &self.memories,
            &self.entities,
            &[change],
            GraphRelationOrigin::Perception,
        )?;
        self.graph.set_entity_association(
            &mut self.container,
            memory_id,
            entity_id,
            active,
            expected_graph_version,
        )
    }

    pub(crate) fn set_semantic_relations_with_origin(
        &mut self,
        changes: &[SemanticGraphRelationChange],
        origin: GraphRelationOrigin,
        expected_graph_version: u64,
    ) -> Result<Vec<SemanticGraphRelation>, GraphError> {
        crate::graph_endpoint_admission::validate_semantic_changes(
            &self.memories,
            &self.entities,
            changes,
            origin,
        )?;
        self.graph.set_semantic_relations_with_origin(
            &mut self.container,
            changes,
            origin,
            expected_graph_version,
        )
    }

    pub fn graph_version(&self) -> u64 {
        self.graph.graph_version()
    }

    pub fn memory_graph_version(&self) -> u64 {
        self.graph.memory_graph_version()
    }

    pub fn graph_stats(&self) -> GraphStats {
        self.graph.stats()
    }

    pub fn graph_relations(&self) -> Vec<GraphRelation> {
        self.graph.active_relations()
    }

    pub fn semantic_graph_relations(&self) -> Vec<SemanticGraphRelation> {
        self.graph
            .active_semantic_relations()
            .into_iter()
            .filter(|relation| {
                relation.kind != crate::SemanticGraphRelationKind::PrincipalAssociation
            })
            .collect()
    }

    pub fn entity_associations_for_memory(&self, memory_id: MemoryId) -> Vec<EntityId> {
        self.graph.entity_associations_for_memory(memory_id)
    }

    pub fn memories_for_entity(&self, entity_id: EntityId) -> Vec<MemoryId> {
        self.graph.memories_for_entity(entity_id)
    }

    pub fn semantic_graph_neighbors(
        &self,
        node: SemanticNodeRef,
        direction: GraphDirection,
    ) -> Result<Vec<SemanticGraphNeighbor>, GraphError> {
        self.graph.semantic_neighbors(node, direction)
    }

    pub fn graph_neighbors(
        &self,
        memory_id: MemoryId,
        direction: GraphDirection,
    ) -> Result<Vec<GraphNeighbor>, GraphError> {
        self.graph.neighbors(memory_id, direction)
    }

    pub fn shortest_memory_path(
        &self,
        source: MemoryId,
        target: MemoryId,
        max_depth: usize,
    ) -> Result<Option<MemoryGraphPath>, GraphError> {
        self.graph.shortest_path(source, target, max_depth)
    }
}
