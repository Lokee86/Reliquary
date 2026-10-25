// Canonical Graph operations; public signatures remain facade bindings.
mod rel {
    use crate::{
        EntityId, GraphDirection, GraphError, GraphNeighbor, GraphRelation, GraphRelationChange,
        GraphRelationOrigin, GraphStats, MemoryGraphPath, MemoryId, SemanticGraphNeighbor,
        SemanticGraphRelation, SemanticGraphRelationChange, SemanticNodeRef,
    };

    impl crate::owner_lifecycle::ReliquaryOwner {
        pub(crate) fn native_graph_set_entity_association(
            &mut self,
            memory_id: MemoryId,
            entity_id: EntityId,
            active: bool,
            expected_graph_version: u64,
        ) -> Result<Option<SemanticGraphRelation>, GraphError> {
            let change =
                SemanticGraphRelationChange::entity_association(memory_id, entity_id, active);
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
        pub(crate) fn native_graph_set_principal_association(
            &mut self,
            memory_id: MemoryId,
            entity_id: EntityId,
            active: bool,
            expected_graph_version: u64,
        ) -> Result<Option<SemanticGraphRelation>, GraphError> {
            let change =
                SemanticGraphRelationChange::principal_association(memory_id, entity_id, active);
            crate::graph_endpoint_admission::validate_semantic_changes(
                &self.memories,
                &self.entities,
                &[change],
                GraphRelationOrigin::Perception,
            )?;
            self.graph.set_principal_association(
                &mut self.container,
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
        pub(crate) fn native_graph_graph_stats(&self) -> GraphStats {
            self.graph.stats()
        }
        pub(crate) fn native_graph_entity_associations_for_memory(
            &self,
            memory_id: MemoryId,
        ) -> Vec<EntityId> {
            self.graph.entity_associations_for_memory(memory_id)
        }
        pub(crate) fn native_graph_memories_for_entity(
            &self,
            entity_id: EntityId,
        ) -> Vec<MemoryId> {
            self.graph.memories_for_entity(entity_id)
        }
        pub(crate) fn native_graph_principal_associations_for_memory(
            &self,
            memory_id: MemoryId,
        ) -> Vec<EntityId> {
            self.graph.principal_associations_for_memory(memory_id)
        }
        pub(crate) fn native_graph_memories_for_principal(
            &self,
            entity_id: EntityId,
        ) -> Vec<MemoryId> {
            self.graph.memories_for_principal(entity_id)
        }
        pub(crate) fn native_graph_semantic_graph_neighbors(
            &self,
            node: SemanticNodeRef,
            direction: GraphDirection,
        ) -> Result<Vec<SemanticGraphNeighbor>, GraphError> {
            self.graph.semantic_neighbors(node, direction)
        }
        pub(crate) fn native_graph_graph_neighbors(
            &self,
            memory_id: MemoryId,
            direction: GraphDirection,
        ) -> Result<Vec<GraphNeighbor>, GraphError> {
            self.graph.neighbors(memory_id, direction)
        }
        pub(crate) fn native_graph_shortest_memory_path(
            &self,
            source: MemoryId,
            target: MemoryId,
            max_depth: usize,
        ) -> Result<Option<MemoryGraphPath>, GraphError> {
            self.graph.shortest_path(source, target, max_depth)
        }
    }
}
mod phy {
    use crate::{
        EntityId, GraphDirection, GraphError, GraphNeighbor, GraphRelation, GraphRelationChange,
        GraphRelationOrigin, GraphStats, MemoryGraphPath, MemoryId, SemanticGraphNeighbor,
        SemanticGraphRelation, SemanticGraphRelationChange, SemanticNodeRef,
    };

    impl crate::owner_lifecycle::PhylacteryOwner {
        pub(crate) fn native_graph_set_entity_association(
            &mut self,
            memory_id: MemoryId,
            entity_id: EntityId,
            active: bool,
            expected_graph_version: u64,
        ) -> Result<Option<SemanticGraphRelation>, GraphError> {
            let change =
                SemanticGraphRelationChange::entity_association(memory_id, entity_id, active);
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
        pub(crate) fn native_graph_set_semantic_relations_with_origin(
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
        pub(crate) fn native_graph_memory_graph_version(&self) -> u64 {
            self.graph.memory_graph_version()
        }
        pub(crate) fn native_graph_graph_stats(&self) -> GraphStats {
            self.graph.stats()
        }
        pub(crate) fn native_graph_graph_relations(&self) -> Vec<GraphRelation> {
            self.graph.active_relations()
        }
        pub(crate) fn native_graph_entity_associations_for_memory(
            &self,
            memory_id: MemoryId,
        ) -> Vec<EntityId> {
            self.graph.entity_associations_for_memory(memory_id)
        }
        pub(crate) fn native_graph_memories_for_entity(
            &self,
            entity_id: EntityId,
        ) -> Vec<MemoryId> {
            self.graph.memories_for_entity(entity_id)
        }
        pub(crate) fn native_graph_semantic_graph_neighbors(
            &self,
            node: SemanticNodeRef,
            direction: GraphDirection,
        ) -> Result<Vec<SemanticGraphNeighbor>, GraphError> {
            self.graph.semantic_neighbors(node, direction)
        }
        pub(crate) fn native_graph_graph_neighbors(
            &self,
            memory_id: MemoryId,
            direction: GraphDirection,
        ) -> Result<Vec<GraphNeighbor>, GraphError> {
            self.graph.neighbors(memory_id, direction)
        }
        pub(crate) fn native_graph_shortest_memory_path(
            &self,
            source: MemoryId,
            target: MemoryId,
            max_depth: usize,
        ) -> Result<Option<MemoryGraphPath>, GraphError> {
            self.graph.shortest_path(source, target, max_depth)
        }
    }
}
use crate::{
    CommunityError, CommunityId, CommunitySemanticName, CommunitySemanticNameSource, GraphError,
    GraphRelationChange, GraphRelationOrigin,
};
impl crate::owner_lifecycle::ReliquaryOwner {
    pub(crate) fn owner_runtime_set_knowledge_relations(
        &mut self,
        changes: &[GraphRelationChange],
        origin: GraphRelationOrigin,
        expected_graph_version: u64,
    ) -> Result<Vec<crate::GraphRelation>, GraphError> {
        crate::graph_endpoint_admission::validate_memory_changes(&self.memories, changes, origin)?;
        self.graph.set_relations_with_origin(
            &mut self.container,
            changes,
            origin,
            expected_graph_version,
        )
    }
}
impl crate::owner_lifecycle::ReliquaryOwner {
    pub(crate) fn owner_runtime_set_knowledge_community_name(
        &mut self,
        community_id: CommunityId,
        name: String,
    ) -> Result<bool, CommunityError> {
        self.communities.publish_semantic_name(
            &mut self.container,
            &self.graph,
            CommunitySemanticName {
                community_id,
                baseline_community_id: community_id,
                contract_version: 0,
                source: CommunitySemanticNameSource::User,
                name: name.trim().to_owned(),
                representative_memories: Vec::new(),
            },
        )
    }
}
impl crate::owner_lifecycle::PhylacteryOwner {
    pub(crate) fn owner_runtime_set_knowledge_relations(
        &mut self,
        changes: &[GraphRelationChange],
        origin: GraphRelationOrigin,
        expected_graph_version: u64,
    ) -> Result<Vec<crate::GraphRelation>, GraphError> {
        crate::graph_endpoint_admission::validate_memory_changes(&self.memories, changes, origin)?;
        self.graph.set_relations_with_origin(
            &mut self.container,
            changes,
            origin,
            expected_graph_version,
        )
    }
}
impl crate::owner_lifecycle::PhylacteryOwner {
    pub(crate) fn owner_runtime_set_knowledge_community_name(
        &mut self,
        community_id: CommunityId,
        name: String,
    ) -> Result<bool, CommunityError> {
        self.communities.publish_semantic_name(
            &mut self.container,
            &self.graph,
            CommunitySemanticName {
                community_id,
                baseline_community_id: community_id,
                contract_version: 0,
                source: CommunitySemanticNameSource::User,
                name: name.trim().to_owned(),
                representative_memories: Vec::new(),
            },
        )
    }
}
macro_rules! graph_runtime_reads {
    ($owner:ty) => {
        impl $owner {
            pub(crate) fn owner_graph_version(&self) -> u64 {
                self.graph.graph_version()
            }
            pub(crate) fn owner_graph_relations(&self) -> Vec<crate::GraphRelation> {
                self.graph.active_relations()
            }
            pub(crate) fn owner_semantic_graph_relations(
                &self,
            ) -> Vec<crate::SemanticGraphRelation> {
                self.graph.active_semantic_relations()
            }
        }
    };
}
graph_runtime_reads!(crate::owner_lifecycle::ReliquaryOwner);
graph_runtime_reads!(crate::owner_lifecycle::PhylacteryOwner);

macro_rules! perception_graph_writes {
    ($owner:ty) => {
        impl $owner {
            pub(crate) fn exclude_entity_from_graph(
                &mut self,
                id: crate::EntityId,
            ) -> Result<(), crate::GraphError> {
                self.graph
                    .exclude_from_current(crate::SemanticNodeRef::entity(id))
            }
            pub(crate) fn apply_validated_entity_association(
                &mut self,
                memory: crate::MemoryId,
                entity: crate::EntityId,
                active: bool,
                expected: u64,
            ) -> Result<Option<crate::SemanticGraphRelation>, crate::GraphError> {
                self.graph.set_entity_association(
                    &mut self.container,
                    memory,
                    entity,
                    active,
                    expected,
                )
            }
        }
    };
}
perception_graph_writes!(crate::owner_lifecycle::ReliquaryOwner);
perception_graph_writes!(crate::owner_lifecycle::PhylacteryOwner);

macro_rules! community_lifecycle_operations {
    ($owner:ty) => {
        impl $owner {
            pub(crate) fn native_community_stats(&self) -> crate::CommunityStats {
                self.communities.stats(&self.graph)
            }

            pub(crate) fn native_community_semantic_name(
                &self,
                community_id: crate::CommunityId,
            ) -> Option<crate::CommunitySemanticName> {
                self.communities.latest_if_live_members(&self.graph)?;
                self.communities.semantic_name(community_id)
            }

            pub(crate) fn native_community_lineage(
                &self,
            ) -> Option<crate::CommunityLineageTransition> {
                self.communities.latest_lineage()
            }

            pub(crate) fn native_publish_community_semantic_name(
                &mut self,
                record: crate::CommunitySemanticName,
            ) -> Result<bool, crate::CommunityError> {
                self.communities
                    .publish_semantic_name(&mut self.container, &self.graph, record)
            }

            pub(crate) fn native_refresh_communities_leiden(
                &mut self,
            ) -> Result<crate::CommunitySnapshot, crate::CommunityError> {
                if let Some(snapshot) = self.communities.latest()
                    && snapshot.derived_graph_version == self.graph.memory_graph_version()
                    && self
                        .communities
                        .latest_if_current_graph(&self.graph)
                        .is_some()
                    && snapshot.algorithm_version == crate::COMMUNITY_ALGORITHM_VERSION
                {
                    return Ok(snapshot.clone());
                }
                let owner_uuid = self
                    .container
                    .owner_uuid()
                    .ok_or(crate::CommunityError::MissingOwnerIdentity)?;
                let generation = self.communities.next_generation()?;
                let snapshot = crate::community_scan_merge::scan_merge_snapshot(
                    &self.graph,
                    owner_uuid,
                    generation,
                )?;
                self.communities.publish(
                    &mut self.container,
                    &self.graph,
                    owner_uuid,
                    snapshot.clone(),
                )?;
                Ok(snapshot)
            }
        }
    };
}
community_lifecycle_operations!(crate::owner_lifecycle::ReliquaryOwner);
community_lifecycle_operations!(crate::owner_lifecycle::PhylacteryOwner);
