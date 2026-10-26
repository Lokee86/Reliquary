//! Product admission and typed identity conversion for the canonical Graph owner.
use crate::memory_store::MemoryStore;
use crate::*;
use arcana::NodeId;
use reliquary_container::Container;
use reliquary_perception_store::EntityStore;

pub(crate) struct GraphOwner {
    pub(crate) inner: reliquary_graph::GraphStore,
}
impl GraphOwner {
    pub(crate) fn empty() -> Self {
        Self {
            inner: reliquary_graph::GraphStore::empty(),
        }
    }
    pub(crate) fn initialize(&mut self, container: &mut Container) -> Result<(), GraphError> {
        self.inner.initialize(container).map_err(Into::into)
    }
    pub(crate) fn graph_version(&self) -> u64 {
        self.inner.graph_version()
    }
    pub(crate) fn memory_graph_version(&self) -> u64 {
        self.inner.memory_graph_version()
    }
    pub(crate) fn stats(&self) -> GraphStats {
        self.inner.stats()
    }
    pub(crate) fn transaction_global_versions(&self) -> &[u64] {
        self.inner.transaction_global_versions()
    }
    pub(crate) fn memory_node_count(&self) -> usize {
        self.inner.memory_node_count()
    }
    pub(crate) fn topology(&self) -> &arcana::storage::InMemoryGraph {
        self.inner.topology()
    }
    pub(crate) fn memory_topology(&self) -> &arcana::storage::InMemoryGraph {
        self.inner.memory_topology()
    }
    pub(crate) fn active_relations(&self) -> Vec<GraphRelation> {
        self.inner
            .active_relations()
            .into_iter()
            .map(Into::into)
            .collect()
    }
    pub(crate) fn active_semantic_relations(&self) -> Vec<SemanticGraphRelation> {
        self.inner
            .active_semantic_relations()
            .into_iter()
            .map(Into::into)
            .collect()
    }
    pub(crate) fn node_memory_ids(&self) -> Vec<MemoryId> {
        self.inner
            .node_memory_ids()
            .into_iter()
            .map(MemoryId)
            .collect()
    }
    pub(crate) fn current_memory_node_ids(&self) -> Vec<MemoryId> {
        self.inner
            .current_memory_node_ids()
            .iter()
            .copied()
            .map(MemoryId)
            .collect()
    }
    pub(crate) fn semantic_nodes(&self) -> Vec<SemanticNodeRef> {
        self.inner
            .semantic_nodes()
            .into_iter()
            .map(Into::into)
            .collect()
    }
    pub(crate) fn node_id(&self, id: MemoryId) -> Result<NodeId, GraphError> {
        self.inner.node_id(id.0).map_err(Into::into)
    }
    pub(crate) fn historical_memory_node_id(&self, id: MemoryId) -> Result<NodeId, GraphError> {
        self.inner
            .historical_memory_node_id(id.0)
            .map_err(Into::into)
    }
    pub(crate) fn semantic_node_id(&self, node: SemanticNodeRef) -> Result<NodeId, GraphError> {
        self.inner.semantic_node_id(node.into()).map_err(Into::into)
    }
    pub(crate) fn memory_projection_node_id(&self, id: MemoryId) -> Result<NodeId, GraphError> {
        self.inner
            .memory_projection_node_id(id.0)
            .map_err(Into::into)
    }
    pub(crate) fn memory_projection_memory_id(&self, id: NodeId) -> Result<MemoryId, GraphError> {
        self.inner
            .memory_projection_memory_id(id)
            .map(MemoryId)
            .map_err(Into::into)
    }
    pub(crate) fn semantic_node(&self, id: NodeId) -> Result<SemanticNodeRef, GraphError> {
        self.inner
            .semantic_node(id)
            .map(Into::into)
            .map_err(Into::into)
    }
    pub(crate) fn exclude_from_current(&mut self, node: SemanticNodeRef) -> Result<(), GraphError> {
        self.inner
            .exclude_from_current(node.into())
            .map_err(Into::into)
    }
    pub(crate) fn normalized_import_evidence(&self, bytes: &[u8]) -> Result<Vec<u8>, GraphError> {
        self.inner
            .normalized_import_evidence(bytes)
            .map_err(Into::into)
    }
    pub(crate) fn compact_carrier_digest(
        &self,
        bytes: &[u8],
    ) -> Result<Option<[u8; 32]>, GraphError> {
        self.inner.compact_carrier_digest(bytes).map_err(Into::into)
    }
    pub(crate) fn decode_stored_mutations(
        &self,
        bytes: &[u8],
    ) -> Result<Option<Vec<reliquary_graph::graph_codec::GraphMutationPayload>>, GraphError> {
        self.inner
            .decode_stored_mutations(bytes)
            .map_err(Into::into)
    }
    pub(crate) fn decode_stored_transaction(
        &self,
        bytes: &[u8],
    ) -> Result<
        Option<(
            bool,
            Vec<reliquary_graph::graph_codec::GraphMutationPayload>,
        )>,
        GraphError,
    > {
        self.inner
            .decode_stored_transaction(bytes)
            .map_err(Into::into)
    }
    pub(crate) fn relation_state(
        &self,
        source: MemoryId,
        target: MemoryId,
        kind: GraphRelationKind,
    ) -> Option<GraphRelation> {
        self.inner
            .relation_state(source.0, target.0, kind)
            .map(Into::into)
    }
    pub(crate) fn neighbors(
        &self,
        id: MemoryId,
        direction: GraphDirection,
    ) -> Result<Vec<GraphNeighbor>, GraphError> {
        self.inner
            .neighbors(id.0, direction)
            .map(|v| v.into_iter().map(Into::into).collect())
            .map_err(Into::into)
    }
    pub(crate) fn semantic_neighbors(
        &self,
        node: SemanticNodeRef,
        direction: GraphDirection,
    ) -> Result<Vec<SemanticGraphNeighbor>, GraphError> {
        self.inner
            .semantic_neighbors(node.into(), direction)
            .map(|v| v.into_iter().map(Into::into).collect())
            .map_err(Into::into)
    }
    pub(crate) fn shortest_path(
        &self,
        source: MemoryId,
        target: MemoryId,
        max_depth: usize,
    ) -> Result<Option<MemoryGraphPath>, GraphError> {
        self.inner
            .shortest_path(source.0, target.0, max_depth)
            .map(|v| v.map(Into::into))
            .map_err(Into::into)
    }
    pub(crate) fn entity_associations_for_memory(&self, id: MemoryId) -> Vec<EntityId> {
        self.inner
            .entity_associations_for_memory(id.0)
            .into_iter()
            .map(EntityId)
            .collect()
    }
    pub(crate) fn principal_associations_for_memory(&self, id: MemoryId) -> Vec<EntityId> {
        self.inner
            .principal_associations_for_memory(id.0)
            .into_iter()
            .map(EntityId)
            .collect()
    }
    pub(crate) fn memories_for_entity(&self, id: EntityId) -> Vec<MemoryId> {
        self.inner
            .memories_for_entity(id.0)
            .into_iter()
            .map(MemoryId)
            .collect()
    }
    pub(crate) fn memories_for_principal(&self, id: EntityId) -> Vec<MemoryId> {
        self.inner
            .memories_for_principal(id.0)
            .into_iter()
            .map(MemoryId)
            .collect()
    }

    pub(crate) fn set_relation(
        &mut self,
        c: &mut Container,
        m: &MemoryStore,
        source: MemoryId,
        target: MemoryId,
        kind: GraphRelationKind,
        active: bool,
        expected: u64,
    ) -> Result<Option<GraphRelation>, GraphError> {
        self.set_relation_with_origin(
            c,
            m,
            source,
            target,
            kind,
            active,
            GraphRelationOrigin::Dream,
            expected,
        )
    }
    pub(crate) fn set_relation_with_origin(
        &mut self,
        c: &mut Container,
        m: &MemoryStore,
        source: MemoryId,
        target: MemoryId,
        kind: GraphRelationKind,
        active: bool,
        origin: GraphRelationOrigin,
        expected: u64,
    ) -> Result<Option<GraphRelation>, GraphError> {
        Ok(self
            .set_relations_with_origin(
                c,
                m,
                &[GraphRelationChange {
                    source,
                    target,
                    kind,
                    active,
                }],
                origin,
                expected,
            )?
            .into_iter()
            .next())
    }
    pub(crate) fn set_relations(
        &mut self,
        c: &mut Container,
        m: &MemoryStore,
        changes: &[GraphRelationChange],
        expected: u64,
    ) -> Result<Vec<GraphRelation>, GraphError> {
        self.set_relations_with_origin(c, m, changes, GraphRelationOrigin::Dream, expected)
    }
    pub(crate) fn set_relations_with_origin(
        &mut self,
        c: &mut Container,
        m: &MemoryStore,
        changes: &[GraphRelationChange],
        origin: GraphRelationOrigin,
        expected: u64,
    ) -> Result<Vec<GraphRelation>, GraphError> {
        let mut ids = Vec::with_capacity(changes.len() * 2);
        for change in changes {
            if change.source == change.target {
                return Err(GraphError::SelfRelation);
            }
            for id in [change.source, change.target] {
                if !m.contains_memory(id) {
                    return Err(GraphError::MissingMemory(id));
                }
                ids.push(id.0);
            }
            if origin == GraphRelationOrigin::Perception {
                return Err(GraphError::InvalidRelationOrigin);
            }
        }
        let facts = reliquary_graph::GraphAdmission::for_changes(ids, []);
        let values: Vec<_> = changes.iter().copied().map(Into::into).collect();
        self.inner
            .set_relations_with_origin(c, &facts, &values, origin, expected)
            .map(|v| v.into_iter().map(Into::into).collect())
            .map_err(Into::into)
    }
    pub(crate) fn set_entity_association(
        &mut self,
        c: &mut Container,
        m: &MemoryStore,
        e: &EntityStore,
        memory: MemoryId,
        entity: EntityId,
        active: bool,
        expected: u64,
    ) -> Result<Option<SemanticGraphRelation>, GraphError> {
        Ok(self
            .set_semantic_relations_with_origin(
                c,
                m,
                e,
                &[SemanticGraphRelationChange::entity_association(
                    memory, entity, active,
                )],
                GraphRelationOrigin::Perception,
                expected,
            )?
            .into_iter()
            .next())
    }
    pub(crate) fn set_principal_association(
        &mut self,
        c: &mut Container,
        m: &MemoryStore,
        e: &EntityStore,
        memory: MemoryId,
        entity: EntityId,
        active: bool,
        expected: u64,
    ) -> Result<Option<SemanticGraphRelation>, GraphError> {
        Ok(self
            .set_semantic_relations_with_origin(
                c,
                m,
                e,
                &[SemanticGraphRelationChange::principal_association(
                    memory, entity, active,
                )],
                GraphRelationOrigin::Perception,
                expected,
            )?
            .into_iter()
            .next())
    }
    pub(crate) fn set_semantic_relations_with_origin(
        &mut self,
        c: &mut Container,
        m: &MemoryStore,
        e: &EntityStore,
        changes: &[SemanticGraphRelationChange],
        origin: GraphRelationOrigin,
        expected: u64,
    ) -> Result<Vec<SemanticGraphRelation>, GraphError> {
        let facts = admit_changes(m, e, changes, origin)?;
        let values: Vec<_> = changes.iter().copied().map(Into::into).collect();
        self.inner
            .set_semantic_relations_with_origin(c, &facts, &values, origin, expected)
            .map(|v| v.into_iter().map(Into::into).collect())
            .map_err(Into::into)
    }
    pub(crate) fn replay_semantic_transaction(
        &mut self,
        c: &mut Container,
        m: &MemoryStore,
        e: &EntityStore,
        changes: &[SemanticGraphRelationChange],
        origin: GraphRelationOrigin,
        expected: u64,
        single: bool,
    ) -> Result<Vec<SemanticGraphRelation>, GraphError> {
        let facts = admit_changes(m, e, changes, origin)?;
        let values: Vec<_> = changes.iter().copied().map(Into::into).collect();
        self.inner
            .replay_semantic_transaction(c, &facts, &values, origin, expected, single)
            .map(|v| v.into_iter().map(Into::into).collect())
            .map_err(Into::into)
    }
    pub(crate) fn replay_import_semantic_transaction(
        &mut self,
        c: &mut Container,
        m: &MemoryStore,
        e: &EntityStore,
        changes: &[SemanticGraphRelationChange],
        origin: GraphRelationOrigin,
        expected: u64,
        single: bool,
    ) -> Result<Vec<SemanticGraphRelation>, GraphError> {
        let facts = admit_changes(m, e, changes, origin)?;
        let values: Vec<_> = changes.iter().copied().map(Into::into).collect();
        self.inner
            .replay_import_semantic_transaction(c, &facts, &values, origin, expected, single)
            .map(|v| v.into_iter().map(Into::into).collect())
            .map_err(Into::into)
    }

    #[cfg(test)]
    pub(crate) fn insert_node_rebuilt(&mut self, node: GraphNodeRecord) -> Result<(), GraphError> {
        self.inner
            .insert_node_rebuilt(node.into())
            .map_err(Into::into)
    }
    #[cfg(test)]
    pub(crate) fn encode_compact_mutations(
        &self,
        values: &[reliquary_graph::graph_codec::GraphMutationPayload],
        single: bool,
    ) -> Result<Vec<u8>, GraphError> {
        self.inner
            .encode_compact_mutations(values, single)
            .map_err(Into::into)
    }
    pub(crate) fn validate_rewrite_baseline(
        &self,
        owner: Option<[u8; 16]>,
        genesis: Option<&RewriteGenesis>,
    ) -> Result<(), GraphError> {
        if let Some((baseline_owner, generation, policy)) =
            self.inner.historical_baseline_identity()
        {
            let Some(genesis) = genesis else {
                return Err(GraphError::InvalidGraphVersion);
            };
            if owner != Some(baseline_owner)
                || genesis.generation.as_bytes() != &generation
                || genesis.policy_digest != policy
            {
                return Err(GraphError::InvalidGraphVersion);
            }
        }
        Ok(())
    }
    pub(crate) fn publish_rewrite_baseline(
        &mut self,
        c: &mut Container,
        m: &MemoryStore,
        owner: [u8; 16],
        generation: HistoryGenerationId,
        policy: [u8; 32],
        memories: &[MemoryId],
    ) -> Result<(), GraphError> {
        for id in memories {
            if !m.contains_historical_memory(*id) {
                return Err(GraphError::MissingMemory(*id));
            }
        }
        let facts = reliquary_graph::GraphAdmission::for_replay(
            memories
                .iter()
                .filter(|id| m.contains_memory(**id))
                .map(|id| id.0),
            memories.iter().map(|id| id.0),
            [],
            [],
            [],
        );
        self.inner
            .publish_rewrite_baseline(
                c,
                &facts,
                owner,
                *generation.as_bytes(),
                policy,
                &memories.iter().map(|id| id.0).collect::<Vec<_>>(),
            )
            .map_err(Into::into)
    }
    pub(crate) fn rewrite_baseline_memories(&self) -> Option<Vec<MemoryId>> {
        self.inner
            .rewrite_baseline_memories()
            .map(|ids| ids.iter().copied().map(MemoryId).collect())
    }
    pub(crate) fn historical_baseline_identity(
        &self,
    ) -> Option<([u8; 16], HistoryGenerationId, [u8; 32])> {
        self.inner
            .historical_baseline_identity()
            .map(|(o, g, p)| (o, HistoryGenerationId::from_bytes(g), p))
    }
}

fn admit_changes(
    m: &MemoryStore,
    e: &EntityStore,
    changes: &[SemanticGraphRelationChange],
    origin: GraphRelationOrigin,
) -> Result<reliquary_graph::GraphAdmission, GraphError> {
    let mut memories = Vec::new();
    let mut entities = Vec::new();
    for change in changes {
        // Intrinsic shape and origin precede source/target existence.
        crate::graph_admission::validate_relation_contract(
            change.source,
            change.target,
            change.kind,
            origin,
        )?;
        for node in [change.source, change.target] {
            match node.kind {
                SemanticNodeKind::Memory => {
                    let id = MemoryId(node.id);
                    if !m.contains_memory(id) {
                        return Err(GraphError::MissingMemory(id));
                    }
                    memories.push(node.id);
                }
                SemanticNodeKind::Entity => {
                    let id = EntityId(node.id);
                    if !e.contains(id) {
                        return Err(GraphError::MissingEntity(id));
                    }
                }
                SemanticNodeKind::Observation => {
                    return Err(GraphError::UnsupportedSemanticNode(node));
                }
            }
        }
        if let Some(id) = change.target.as_entity() {
            let entity = e.entity(id).map_err(|_| GraphError::MissingEntity(id))?;
            let principal = entity.kind == crate::entity_principal::PRINCIPAL_ENTITY_KIND;
            if (change.kind == SemanticGraphRelationKind::PrincipalAssociation) != principal {
                return Err(GraphError::InvalidRelationShape);
            }
            entities.push((id.0, principal));
        }
    }
    Ok(reliquary_graph::GraphAdmission::for_changes(
        memories, entities,
    ))
}
