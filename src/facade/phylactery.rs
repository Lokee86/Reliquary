use crate::community_store::CommunityStore;
use crate::compatibility_profile_store::CompatibilityProfileStore;
use crate::dream_duplicate_index::DuplicateIndex;
use crate::dream_pair_history::DreamPairStore;
use crate::ego_store::EgoStore;
use crate::entity_resolution_store::EntityResolutionStore;
use crate::entity_store::EntityStore;
use crate::graph_store::GraphStore;
use crate::lexical_index::LexicalIndex;
use crate::memory_store::MemoryStore;
use crate::memory_vector_store::MemoryVectorStore;
use crate::packed_vector_store::PackedVectorStore;
use crate::phylactery_profile_store::PhylacteryProfileStore;
use crate::processing_epoch_store::ProcessingEpochStore;
use crate::relationship_store::RelationshipStore;
use crate::{
    Container, Memory, MemoryBodyId, MemoryDraft, MemoryError, MemoryId, MemoryStats,
    PhylacteryError,
};

pub struct Phylactery {
    pub(crate) container: Container,
    pub(crate) memories: MemoryStore,
    pub(crate) lexical_index: LexicalIndex,
    pub(crate) graph: GraphStore,
    pub(crate) communities: CommunityStore,
    pub(crate) duplicate_index: DuplicateIndex,
    pub(crate) processing_epochs: ProcessingEpochStore,
    pub(crate) dream_pairs: DreamPairStore,
    pub(crate) packed_vectors: PackedVectorStore,
    pub(crate) memory_vectors: MemoryVectorStore,
    pub(crate) compatibility_profiles: CompatibilityProfileStore,
    pub(crate) profile: PhylacteryProfileStore,
    pub(crate) ego: EgoStore,
    pub(crate) entities: EntityStore,
    pub(crate) entity_resolutions: EntityResolutionStore,
    pub(crate) relationships: RelationshipStore,
}

impl Phylactery {
    pub fn owner_id(&self) -> Option<String> {
        self.container.owner_id()
    }

    pub fn owner_uuid(&self) -> Option<[u8; 16]> {
        self.container.owner_uuid()
    }

    pub fn profile(&self) -> crate::PhylacteryProfile {
        self.profile.current()
    }

    pub fn set_profile(&mut self, display_name: Option<String>) -> Result<bool, PhylacteryError> {
        let normalize = |value: Option<String>| {
            value.and_then(|value| {
                let trimmed = value.trim().to_owned();
                (!trimmed.is_empty()).then_some(trimmed)
            })
        };
        self.profile
            .put(
                &mut self.container,
                crate::PhylacteryProfile {
                    display_name: normalize(display_name),
                },
            )
            .map_err(PhylacteryError::Profile)
    }

    pub fn latest_global_version(&self) -> u64 {
        self.container.latest_version()
    }

    pub fn transaction_time_ns(&self, version: u64) -> Option<i64> {
        self.container.transaction_time_ns(version)
    }

    pub fn version_at_or_before(&self, transaction_time_ns: i64) -> Option<u64> {
        self.container.version_at_or_before(transaction_time_ns)
    }

    pub fn publish_memory(
        &mut self,
        id: Option<MemoryId>,
        expected_revision: u64,
        draft: MemoryDraft,
    ) -> Result<(Memory, bool), MemoryError> {
        validate_phylactery_provenance(&draft)?;
        self.memories
            .publish(&mut self.container, id, expected_revision, draft)
    }

    pub fn memory(&mut self, id: MemoryId) -> Result<Memory, MemoryError> {
        self.memories.memory(&mut self.container, id)
    }

    pub fn set_memory_temporal_status(
        &mut self,
        id: MemoryId,
        expected_revision: u64,
        temporal_status: &str,
        mutation_id: String,
    ) -> Result<(Memory, bool), MemoryError> {
        if !matches!(temporal_status, "current" | "future" | "historical") {
            return Err(MemoryError::InvalidField("temporal status"));
        }
        let current = self.memory(id)?;
        if current.revision != expected_revision {
            return Err(MemoryError::RevisionConflict);
        }
        if current.temporal_status == temporal_status {
            return Ok((current, false));
        }
        let draft = MemoryDraft {
            category: current.category.clone(),
            memory_type: current.memory_type.clone(),
            authority_kind: current.authority_kind.clone(),
            temporal_status: temporal_status.to_owned(),
            title: current.title.clone(),
            content: current.content.clone(),
            scope: current.scope.clone(),
            lifecycle_state: current.lifecycle_state.clone(),
            archived: current.archived,
            superseded_by: current.superseded_by,
            parent_id: current.parent_id,
            source_node_id: current.source_node_id.clone(),
            content_source_conversation_id: current.content_source_conversation_id.clone(),
            content_source_node_id: current.content_source_node_id.clone(),
            grounding_source_conversation_id: current.grounding_source_conversation_id.clone(),
            grounding_source_node_id: current.grounding_source_node_id.clone(),
            source_episode_id: current.source_episode_id,
            source_time_ns: current.source_time_ns,
            mutation_id,
            created_at_ns: current.created_at_ns,
            updated_at_ns: current.updated_at_ns,
        };
        self.memories.publish_with_source_ref(
            &mut self.container,
            Some(id),
            expected_revision,
            draft,
            current.source_ref.clone(),
        )
    }

    pub fn memory_revision(&mut self, id: MemoryId, revision: u64) -> Result<Memory, MemoryError> {
        self.memories
            .memory_revision(&mut self.container, id, revision)
    }

    pub fn memory_stats(&self) -> MemoryStats {
        self.memories.stats()
    }

    pub fn memory_ids(&self) -> Vec<MemoryId> {
        self.memories.current_ids()
    }

    pub fn memory_version(&self) -> u64 {
        self.memories.memory_version()
    }

    pub fn memory_body_id(&self, id: MemoryId) -> Result<MemoryBodyId, MemoryError> {
        self.memories.current_body_id(id)
    }

    pub fn memory_routing_metadata(&self, id: MemoryId) -> Option<&crate::MemoryRoutingMetadata> {
        self.memories.routing_metadata(id)
    }

    pub fn put_memory_routing_metadata(
        &mut self,
        metadata: crate::MemoryRoutingMetadata,
    ) -> Result<bool, MemoryError> {
        self.memories
            .put_routing_metadata(&mut self.container, metadata)
    }

    pub fn sync(&self) -> Result<(), PhylacteryError> {
        self.container.sync()?;
        Ok(())
    }
}

fn validate_phylactery_provenance(draft: &MemoryDraft) -> Result<(), MemoryError> {
    if draft.source_episode_id.is_some()
        || draft.source_node_id.is_some()
        || draft.content_source_conversation_id.is_some()
        || draft.content_source_node_id.is_some()
        || draft.grounding_source_conversation_id.is_some()
        || draft.grounding_source_node_id.is_some()
    {
        return Err(MemoryError::InvalidProvenance);
    }
    Ok(())
}
