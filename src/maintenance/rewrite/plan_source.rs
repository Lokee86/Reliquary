//! Immutable owner borrows for rewrite planning. There is no publication path.
use crate::{Memory, MemoryError, MemoryId};

pub(crate) struct ReliquaryRewritePlanSource<'a> {
    pub(crate) container: &'a crate::Container,
    pub(crate) memories: &'a crate::memory_store::MemoryStore,
    pub(crate) compatibility_profiles:
        &'a crate::compatibility_profile_store::CompatibilityProfileStore,
    pub(crate) memory_vectors: &'a crate::memory_vector_store::MemoryVectorStore,
    pub(crate) rewrite_lineage: &'a crate::rewrite_lineage::RewriteLineageStore,
    pub(crate) entities: &'a crate::entity_store::EntityStore,
    pub(crate) relationships: &'a crate::relationship_store::RelationshipStore,
    pub(crate) graph: &'a crate::graph_store::GraphStore,
    pub(crate) archive: &'a crate::Archive,
    pub(crate) freshness: &'a crate::freshness_storage::FreshnessStore,
}

pub(crate) struct PhylacteryRewritePlanSource<'a> {
    pub(crate) container: &'a crate::Container,
    pub(crate) memories: &'a crate::memory_store::MemoryStore,
    pub(crate) compatibility_profiles:
        &'a crate::compatibility_profile_store::CompatibilityProfileStore,
    pub(crate) memory_vectors: &'a crate::memory_vector_store::MemoryVectorStore,
    pub(crate) rewrite_lineage: &'a crate::rewrite_lineage::RewriteLineageStore,
    pub(crate) entities: &'a crate::entity_store::EntityStore,
    pub(crate) relationships: &'a crate::relationship_store::RelationshipStore,
    pub(crate) graph: &'a crate::graph_store::GraphStore,
}

impl ReliquaryRewritePlanSource<'_> {
    pub(crate) fn owner_uuid(&self) -> Option<[u8; 16]> {
        self.container.owner_uuid()
    }
    pub(crate) fn latest_global_version(&self) -> u64 {
        self.container.latest_version()
    }
    pub(crate) fn memory_version(&self) -> u64 {
        self.memories.memory_version()
    }
    pub(crate) fn history_generation(&self) -> crate::HistoryGenerationId {
        self.rewrite_lineage.generation()
    }
    pub(crate) fn memory(&self, id: crate::MemoryId) -> Result<crate::Memory, crate::MemoryError> {
        self.memories.memory(self.container, id)
    }
    pub(crate) fn memory_revision(
        &self,
        id: crate::MemoryId,
        revision: u64,
    ) -> Result<crate::Memory, crate::MemoryError> {
        self.memories.memory_revision(self.container, id, revision)
    }
    pub(crate) fn memory_retention_descriptor(
        &self,
        id: crate::MemoryId,
    ) -> Result<crate::MemoryRetentionDescriptor, crate::MemoryError> {
        self.memories.retention_descriptor(id)
    }
    pub(crate) fn historical_memory_revision(
        &self,
        id: MemoryId,
        revision: u64,
    ) -> Result<
        crate::owner_lifecycle::HistoricalPayload<Memory, crate::MemoryRetainedRevision>,
        MemoryError,
    > {
        let value = match self.memory_revision(id, revision) {
            Ok(value) => value,
            Err(MemoryError::PayloadUnavailable(reason)) => {
                let structure = self
                    .memory_retention_descriptor(id)?
                    .revisions
                    .into_iter()
                    .find(|r| r.revision == revision)
                    .ok_or(MemoryError::MissingMemory)?;
                return Ok(crate::owner_lifecycle::HistoricalPayload::Pruned {
                    structural_history: structure,
                    reason,
                });
            }
            Err(error) => return Err(error),
        };
        Ok(match self.memories.lifecycle_state(id)? {
            crate::owner_lifecycle::LifecycleState::Active => {
                crate::owner_lifecycle::HistoricalPayload::Available(value)
            }
            crate::owner_lifecycle::LifecycleState::Tombstoned(_) => {
                crate::owner_lifecycle::HistoricalPayload::Tombstoned(value)
            }
        })
    }

    pub(crate) fn rel_turn_count(&self) -> u64 {
        self.archive.rel_turn_count()
    }
    pub(crate) fn conversation_summaries(&self) -> Vec<crate::ConversationSummary> {
        self.archive.conversation_summaries()
    }
    pub(crate) fn fragments(&self) -> Vec<crate::Fragment> {
        self.archive.fragments()
    }
    pub(crate) fn freshness_policy(&self) -> crate::freshness::FreshnessPolicy {
        self.freshness.policy()
    }
    pub(crate) fn freshness_record_at(
        &self,
        id: crate::MemoryId,
        turn: u64,
    ) -> Result<Option<crate::FreshnessRecord>, crate::owner_lifecycle::OwnerError> {
        if turn > self.rel_turn_count() {
            return Err(crate::owner_lifecycle::OwnerError::Freshness(
                "Freshness cut is beyond the REL activity head".into(),
            ));
        }
        self.freshness
            .record_at(id, turn)
            .map_err(crate::owner_lifecycle::OwnerError::Freshness)
    }
    pub(crate) fn semantic_closure(
        &self,
        retained: &std::collections::BTreeSet<crate::MemoryId>,
    ) -> Result<super::closure::SemanticClosure, super::RewriteError> {
        super::closure::semantic_closure(
            self.container.owner_id().as_deref(),
            retained,
            self.memories.current_ids().into_iter().collect(),
            self.entities.current(),
            self.relationships.current(),
            self.graph.active_semantic_relations(),
        )
    }
}

impl PhylacteryRewritePlanSource<'_> {
    pub(crate) fn owner_uuid(&self) -> Option<[u8; 16]> {
        self.container.owner_uuid()
    }
    pub(crate) fn latest_global_version(&self) -> u64 {
        self.container.latest_version()
    }
    pub(crate) fn memory_version(&self) -> u64 {
        self.memories.memory_version()
    }
    pub(crate) fn history_generation(&self) -> crate::HistoryGenerationId {
        self.rewrite_lineage.generation()
    }
    pub(crate) fn memory(&self, id: crate::MemoryId) -> Result<crate::Memory, crate::MemoryError> {
        self.memories.memory(self.container, id)
    }
    pub(crate) fn memory_revision(
        &self,
        id: crate::MemoryId,
        revision: u64,
    ) -> Result<crate::Memory, crate::MemoryError> {
        self.memories.memory_revision(self.container, id, revision)
    }
    pub(crate) fn memory_retention_descriptor(
        &self,
        id: crate::MemoryId,
    ) -> Result<crate::MemoryRetentionDescriptor, crate::MemoryError> {
        self.memories.retention_descriptor(id)
    }
    pub(crate) fn semantic_closure(
        &self,
        retained: &std::collections::BTreeSet<crate::MemoryId>,
    ) -> Result<super::closure::SemanticClosure, super::RewriteError> {
        super::closure::semantic_closure(
            self.container.owner_id().as_deref(),
            retained,
            self.memories.current_ids().into_iter().collect(),
            self.entities.current(),
            self.relationships.current(),
            self.graph
                .active_semantic_relations()
                .into_iter()
                .filter(|relation| {
                    relation.kind != crate::SemanticGraphRelationKind::PrincipalAssociation
                })
                .collect(),
        )
    }
}
