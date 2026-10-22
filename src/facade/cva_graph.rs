use crate::{
    Cva, Entity, EntityId, GraphDirection, GraphError, GraphNeighbor, GraphRelation,
    GraphRelationChange, GraphRelationKind, GraphRelationOrigin, GraphStats, MemoryGraphPath,
    MemoryId, SemanticGraphNeighbor, SemanticGraphRelation, SemanticGraphRelationChange,
    SemanticNodeRef,
};

impl Cva {
    pub fn set_memory_relation(
        &mut self,
        source: MemoryId,
        target: MemoryId,
        kind: GraphRelationKind,
        active: bool,
        expected_graph_version: u64,
    ) -> Result<Option<GraphRelation>, GraphError> {
        self.graph.set_relation(
            &mut self.container,
            &self.memories,
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
        self.graph.set_relation_with_origin(
            &mut self.container,
            &self.memories,
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
        self.graph.set_relations(
            &mut self.container,
            &self.memories,
            changes,
            expected_graph_version,
        )
    }

    pub fn set_memory_relations_with_origin(
        &mut self,
        changes: &[GraphRelationChange],
        origin: GraphRelationOrigin,
        expected_graph_version: u64,
    ) -> Result<Vec<GraphRelation>, GraphError> {
        self.graph.set_relations_with_origin(
            &mut self.container,
            &self.memories,
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
        self.graph.set_entity_association(
            &mut self.container,
            &self.memories,
            &self.entities,
            memory_id,
            entity_id,
            active,
            expected_graph_version,
        )
    }

    pub(crate) fn set_principal_association(
        &mut self,
        memory_id: MemoryId,
        entity_id: EntityId,
        active: bool,
        expected_graph_version: u64,
    ) -> Result<Option<SemanticGraphRelation>, GraphError> {
        self.graph.set_principal_association(
            &mut self.container,
            &self.memories,
            &self.entities,
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
        self.graph.set_semantic_relations_with_origin(
            &mut self.container,
            &self.memories,
            &self.entities,
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

    pub fn principal_associations_for_memory(&self, memory_id: MemoryId) -> Vec<EntityId> {
        self.graph.principal_associations_for_memory(memory_id)
    }

    pub fn memories_for_principal(&self, entity_id: EntityId) -> Vec<MemoryId> {
        self.graph.memories_for_principal(entity_id)
    }

    pub fn principal_entity(&self, principal_id: &str) -> Option<Entity> {
        if !crate::entity_principal::is_phy_principal_id(principal_id) {
            return None;
        }
        self.entity(crate::entity_principal::principal_entity_id(principal_id))
            .ok()
            .filter(|entity| entity.kind == crate::entity_principal::PRINCIPAL_ENTITY_KIND)
    }

    pub fn memories_for_phy_principal(&self, principal_id: &str) -> Vec<MemoryId> {
        let Some(entity) = self.principal_entity(principal_id) else {
            return Vec::new();
        };
        self.graph.memories_for_principal(entity.id)
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
