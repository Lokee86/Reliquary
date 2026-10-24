use crate::{
    CommunityError, CommunityId, CommunitySemanticName, CommunitySemanticNameSource, GraphError,
    GraphRelationChange, GraphRelationOrigin, Memory, MemoryDraft, MemoryError, MemoryId,
    MemorySourceRef,
};

pub(crate) enum KnowledgeMutationExecutionError {
    Memory(MemoryError),
    Graph(GraphError),
}

impl From<MemoryError> for KnowledgeMutationExecutionError {
    fn from(error: MemoryError) -> Self {
        Self::Memory(error)
    }
}

impl From<GraphError> for KnowledgeMutationExecutionError {
    fn from(error: GraphError) -> Self {
        Self::Graph(error)
    }
}

impl std::fmt::Display for KnowledgeMutationExecutionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Memory(error) => write!(formatter, "{error}"),
            Self::Graph(error) => write!(formatter, "{error}"),
        }
    }
}

impl crate::RelExecutionOwner {
    pub(crate) fn runtime_replace_knowledge_memory(
        &mut self,
        memory_id: MemoryId,
        title: String,
        content: String,
        now_ns: i64,
    ) -> Result<Memory, KnowledgeMutationExecutionError> {
        let current = self
            .owner
            .memories
            .memory(&self.owner.container, memory_id)?;
        let relations = self.owner.graph.active_relations();
        let plan = crate::knowledge_mutation::plan_knowledge_replacement(
            &current, &relations, title, content, now_ns,
        );
        let replacement = self
            .runtime_publish_knowledge_memory(None, 0, plan.replacement_draft)?
            .0;
        for relation in plan.adjacent_relations {
            let source = if relation.source == memory_id {
                replacement.id
            } else {
                relation.source
            };
            let target = if relation.target == memory_id {
                replacement.id
            } else {
                relation.target
            };
            let expected_graph_version = self.owner.graph.graph_version();
            self.runtime_set_knowledge_relations(
                &[GraphRelationChange {
                    source,
                    target,
                    kind: relation.kind,
                    active: true,
                }],
                relation.origin,
                expected_graph_version,
            )?;
        }
        let expected_graph_version = self.owner.graph.graph_version();
        self.runtime_set_knowledge_relations(
            &[GraphRelationChange {
                source: replacement.id,
                target: current.id,
                kind: crate::GraphRelationKind::Supersedes,
                active: true,
            }],
            GraphRelationOrigin::User,
            expected_graph_version,
        )?;
        let retired = crate::knowledge_mutation::knowledge_retirement_draft(
            &plan.current,
            replacement.id,
            now_ns,
        );
        self.runtime_publish_knowledge_memory(Some(current.id), current.revision, retired)?;
        Ok(replacement)
    }

    pub(crate) fn runtime_publish_knowledge_memory(
        &mut self,
        id: Option<MemoryId>,
        expected_revision: u64,
        draft: MemoryDraft,
    ) -> Result<(Memory, bool), MemoryError> {
        let preflight = crate::cva_memory_publish::preflight_memory_parts(
            &self.owner.archive,
            &mut self.owner.memories,
            &mut self.owner.container,
            id,
            expected_revision,
            &draft,
        )?;
        let target_id = match preflight {
            crate::memory_store::PublishPreflight::Retry(memory) => memory.id,
            crate::memory_store::PublishPreflight::New { record, .. } => record.id,
        };
        let first_creation = !self.owner.memories.contains_memory(target_id);
        let active_at_birth =
            !draft.archived && matches!(draft.lifecycle_state.as_str(), "knowledge" | "canonical");
        let accepted_turn = self.owner.archive.rel_turn_count();
        if first_creation && self.owner.container.owner_uuid().is_some() {
            self.runtime_prepare_memory_publication(
                target_id,
                &draft.mutation_id,
                accepted_turn,
                active_at_birth,
            )
            .map_err(|error| MemoryError::Freshness(error.to_string()))?;
        }
        let (memory, changed) = crate::cva_memory_publish::publish_memory_parts(
            &self.owner.archive,
            &mut self.owner.memories,
            &mut self.owner.container,
            id,
            expected_revision,
            draft,
        )?;
        if self.owner.container.owner_uuid().is_some()
            && ((first_creation && changed) || self.runtime_has_pending_publication(memory.id))
        {
            self.runtime_freshness_initialize(memory.id)
                .map_err(|error| MemoryError::Freshness(error.to_string()))?;
        }
        Ok((memory, changed))
    }

    pub(crate) fn runtime_set_knowledge_relations(
        &mut self,
        changes: &[GraphRelationChange],
        origin: GraphRelationOrigin,
        expected_graph_version: u64,
    ) -> Result<Vec<crate::GraphRelation>, GraphError> {
        crate::graph_endpoint_admission::validate_memory_changes(
            &self.owner.memories,
            changes,
            origin,
        )?;
        self.owner.graph.set_relations_with_origin(
            &mut self.owner.container,
            changes,
            origin,
            expected_graph_version,
        )
    }

    pub(crate) fn runtime_set_knowledge_community_name(
        &mut self,
        community_id: CommunityId,
        name: String,
    ) -> Result<bool, CommunityError> {
        self.owner.communities.publish_semantic_name(
            &mut self.owner.container,
            &self.owner.graph,
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

impl crate::phy_execution_owner::PhyExecutionOwner {
    pub(crate) fn runtime_replace_knowledge_memory(
        &mut self,
        memory_id: MemoryId,
        title: String,
        content: String,
        now_ns: i64,
    ) -> Result<Memory, KnowledgeMutationExecutionError> {
        let current = self
            .owner
            .memories
            .memory(&mut self.owner.container, memory_id)?;
        let relations = self.owner.graph.active_relations();
        let plan = crate::knowledge_mutation::plan_knowledge_replacement(
            &current, &relations, title, content, now_ns,
        );
        let replacement = self
            .runtime_publish_knowledge_memory(
                None,
                0,
                plan.replacement_draft,
                current.source_ref.clone(),
            )?
            .0;
        for relation in plan.adjacent_relations {
            let source = if relation.source == memory_id {
                replacement.id
            } else {
                relation.source
            };
            let target = if relation.target == memory_id {
                replacement.id
            } else {
                relation.target
            };
            let expected_graph_version = self.owner.graph.graph_version();
            self.runtime_set_knowledge_relations(
                &[GraphRelationChange {
                    source,
                    target,
                    kind: relation.kind,
                    active: true,
                }],
                relation.origin,
                expected_graph_version,
            )?;
        }
        let expected_graph_version = self.owner.graph.graph_version();
        self.runtime_set_knowledge_relations(
            &[GraphRelationChange {
                source: replacement.id,
                target: current.id,
                kind: crate::GraphRelationKind::Supersedes,
                active: true,
            }],
            GraphRelationOrigin::User,
            expected_graph_version,
        )?;
        let retired = crate::knowledge_mutation::knowledge_retirement_draft(
            &plan.current,
            replacement.id,
            now_ns,
        );
        self.runtime_publish_knowledge_memory(Some(current.id), current.revision, retired, None)?;
        Ok(replacement)
    }

    pub(crate) fn runtime_publish_knowledge_memory(
        &mut self,
        id: Option<MemoryId>,
        expected_revision: u64,
        draft: MemoryDraft,
        source_ref: Option<MemorySourceRef>,
    ) -> Result<(Memory, bool), MemoryError> {
        if let Some(source_ref) = source_ref {
            self.owner
                .memories
                .publish_with_source_ref_and_temporal_inference(
                    &mut self.owner.container,
                    id,
                    expected_revision,
                    draft,
                    Some(source_ref),
                    None,
                )
        } else {
            crate::owner_lifecycle::validate_phylactery_draft_provenance(&draft)?;
            self.owner
                .memories
                .publish(&mut self.owner.container, id, expected_revision, draft)
        }
    }

    pub(crate) fn runtime_set_knowledge_relations(
        &mut self,
        changes: &[GraphRelationChange],
        origin: GraphRelationOrigin,
        expected_graph_version: u64,
    ) -> Result<Vec<crate::GraphRelation>, GraphError> {
        crate::graph_endpoint_admission::validate_memory_changes(
            &self.owner.memories,
            changes,
            origin,
        )?;
        self.owner.graph.set_relations_with_origin(
            &mut self.owner.container,
            changes,
            origin,
            expected_graph_version,
        )
    }

    pub(crate) fn runtime_set_knowledge_community_name(
        &mut self,
        community_id: CommunityId,
        name: String,
    ) -> Result<bool, CommunityError> {
        self.owner.communities.publish_semantic_name(
            &mut self.owner.container,
            &self.owner.graph,
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
