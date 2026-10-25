// Canonical Graph operations; public signatures remain facade bindings.
mod rel {
    use crate::{
        EntityId, GraphDirection, GraphError, GraphNeighbor, GraphRelation, GraphRelationChange,
        GraphRelationOrigin, GraphStats, MemoryGraphPath, MemoryId, SemanticGraphNeighbor,
        SemanticGraphRelation, SemanticGraphRelationChange, SemanticNodeRef,
    };

    impl crate::RelExecutionOwner {
        pub(crate) fn native_graph_set_entity_association(
            &mut self,
            memory_id: MemoryId,
            entity_id: EntityId,
            active: bool,
            expected_graph_version: u64,
        ) -> Result<Option<SemanticGraphRelation>, GraphError> {
            self.owner.native_graph_set_entity_association(
                memory_id,
                entity_id,
                active,
                expected_graph_version,
            )
        }
        pub(crate) fn native_graph_set_principal_association(
            &mut self,
            memory_id: MemoryId,
            entity_id: EntityId,
            active: bool,
            expected_graph_version: u64,
        ) -> Result<Option<SemanticGraphRelation>, GraphError> {
            self.owner.native_graph_set_principal_association(
                memory_id,
                entity_id,
                active,
                expected_graph_version,
            )
        }
        pub(crate) fn native_graph_set_semantic_relations_with_origin(
            &mut self,
            changes: &[SemanticGraphRelationChange],
            origin: GraphRelationOrigin,
            expected_graph_version: u64,
        ) -> Result<Vec<SemanticGraphRelation>, GraphError> {
            self.owner.native_graph_set_semantic_relations_with_origin(
                changes,
                origin,
                expected_graph_version,
            )
        }
        pub(crate) fn native_graph_graph_stats(&self) -> GraphStats {
            self.owner.native_graph_graph_stats()
        }
        pub(crate) fn native_graph_entity_associations_for_memory(
            &self,
            memory_id: MemoryId,
        ) -> Vec<EntityId> {
            self.owner
                .native_graph_entity_associations_for_memory(memory_id)
        }
        pub(crate) fn native_graph_memories_for_entity(
            &self,
            entity_id: EntityId,
        ) -> Vec<MemoryId> {
            self.owner.native_graph_memories_for_entity(entity_id)
        }
        pub(crate) fn native_graph_principal_associations_for_memory(
            &self,
            memory_id: MemoryId,
        ) -> Vec<EntityId> {
            self.owner
                .native_graph_principal_associations_for_memory(memory_id)
        }
        pub(crate) fn native_graph_memories_for_principal(
            &self,
            entity_id: EntityId,
        ) -> Vec<MemoryId> {
            self.owner.native_graph_memories_for_principal(entity_id)
        }
        pub(crate) fn native_graph_semantic_graph_neighbors(
            &self,
            node: SemanticNodeRef,
            direction: GraphDirection,
        ) -> Result<Vec<SemanticGraphNeighbor>, GraphError> {
            self.owner
                .native_graph_semantic_graph_neighbors(node, direction)
        }
        pub(crate) fn native_graph_graph_neighbors(
            &self,
            memory_id: MemoryId,
            direction: GraphDirection,
        ) -> Result<Vec<GraphNeighbor>, GraphError> {
            self.owner
                .native_graph_graph_neighbors(memory_id, direction)
        }
        pub(crate) fn native_graph_shortest_memory_path(
            &self,
            source: MemoryId,
            target: MemoryId,
            max_depth: usize,
        ) -> Result<Option<MemoryGraphPath>, GraphError> {
            self.owner
                .native_graph_shortest_memory_path(source, target, max_depth)
        }
    }
}
mod phy {
    use crate::{
        EntityId, GraphDirection, GraphError, GraphNeighbor, GraphRelation, GraphRelationChange,
        GraphRelationOrigin, GraphStats, MemoryGraphPath, MemoryId, SemanticGraphNeighbor,
        SemanticGraphRelation, SemanticGraphRelationChange, SemanticNodeRef,
    };

    impl crate::phy_execution_owner::PhyExecutionOwner {
        pub(crate) fn native_graph_set_entity_association(
            &mut self,
            memory_id: MemoryId,
            entity_id: EntityId,
            active: bool,
            expected_graph_version: u64,
        ) -> Result<Option<SemanticGraphRelation>, GraphError> {
            self.owner.native_graph_set_entity_association(
                memory_id,
                entity_id,
                active,
                expected_graph_version,
            )
        }
        pub(crate) fn native_graph_set_semantic_relations_with_origin(
            &mut self,
            changes: &[SemanticGraphRelationChange],
            origin: GraphRelationOrigin,
            expected_graph_version: u64,
        ) -> Result<Vec<SemanticGraphRelation>, GraphError> {
            self.owner.native_graph_set_semantic_relations_with_origin(
                changes,
                origin,
                expected_graph_version,
            )
        }
        pub(crate) fn native_graph_memory_graph_version(&self) -> u64 {
            self.owner.native_graph_memory_graph_version()
        }
        pub(crate) fn native_graph_graph_stats(&self) -> GraphStats {
            self.owner.native_graph_graph_stats()
        }
        pub(crate) fn native_graph_graph_relations(&self) -> Vec<GraphRelation> {
            self.owner.native_graph_graph_relations()
        }
        pub(crate) fn native_graph_entity_associations_for_memory(
            &self,
            memory_id: MemoryId,
        ) -> Vec<EntityId> {
            self.owner
                .native_graph_entity_associations_for_memory(memory_id)
        }
        pub(crate) fn native_graph_memories_for_entity(
            &self,
            entity_id: EntityId,
        ) -> Vec<MemoryId> {
            self.owner.native_graph_memories_for_entity(entity_id)
        }
        pub(crate) fn native_graph_semantic_graph_neighbors(
            &self,
            node: SemanticNodeRef,
            direction: GraphDirection,
        ) -> Result<Vec<SemanticGraphNeighbor>, GraphError> {
            self.owner
                .native_graph_semantic_graph_neighbors(node, direction)
        }
        pub(crate) fn native_graph_graph_neighbors(
            &self,
            memory_id: MemoryId,
            direction: GraphDirection,
        ) -> Result<Vec<GraphNeighbor>, GraphError> {
            self.owner
                .native_graph_graph_neighbors(memory_id, direction)
        }
        pub(crate) fn native_graph_shortest_memory_path(
            &self,
            source: MemoryId,
            target: MemoryId,
            max_depth: usize,
        ) -> Result<Option<MemoryGraphPath>, GraphError> {
            self.owner
                .native_graph_shortest_memory_path(source, target, max_depth)
        }
    }
}
