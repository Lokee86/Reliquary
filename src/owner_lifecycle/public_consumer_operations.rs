//! Concrete owner operations previously assembled by public product consumers.
use crate::owner_lifecycle::ReliquaryOwner;

pub(crate) struct SemanticMemoryTarget {
    pub(crate) id: crate::MemoryId,
    pub(crate) body_id: crate::MemoryBodyId,
}

pub(crate) fn semantic_memory_targets(
    memories: &crate::memory_store::MemoryStore,
    vectors: &crate::memory_vector_store::MemoryVectorStore,
    profile_id: crate::CompatibilityProfileId,
) -> Result<Vec<SemanticMemoryTarget>, crate::MemoryError> {
    let mut ids = memories.current_ids();
    ids.sort_by_cached_key(|id| {
        memories
            .current_body_id(*id)
            .ok()
            .and_then(|body| vectors.location(profile_id, body))
            .map(|location| location.set_id.0)
            .unwrap_or([255; 32])
    });
    ids.into_iter()
        .map(|id| {
            let body_id = memories.current_body_id(id)?;
            Ok(SemanticMemoryTarget { id, body_id })
        })
        .collect()
}

impl ReliquaryOwner {
    pub(crate) fn memory_read_view(&self) -> crate::memory_store::MemoryReadView<'_> {
        crate::memory_store::MemoryReadView::new(&self.memories, &self.container)
    }
    pub(crate) fn community_snapshot(&self) -> Option<crate::CommunitySnapshot> {
        self.communities
            .latest_if_live_members(&self.graph)
            .cloned()
    }

    pub(crate) fn community_stats(&self) -> crate::CommunityStats {
        self.communities.stats(&self.graph)
    }

    pub(crate) fn community_semantic_names(&self) -> Vec<crate::CommunitySemanticName> {
        self.communities.current_semantic_names(&self.graph)
    }

    pub(crate) fn memory_graph_version(&self) -> u64 {
        self.graph.memory_graph_version()
    }

    pub(crate) fn insomnia_stats(&self) -> crate::InsomniaStats {
        self.insomnia.stats()
    }

    pub(crate) fn entity_version(&self) -> u64 {
        self.entities.entity_version()
    }

    pub(crate) fn entity_resolution(
        &self,
        key: crate::MemoryEntityMentionKey,
    ) -> Option<&crate::MemoryEntityResolution> {
        self.entity_resolutions.get(key)
    }

    pub(crate) fn ego_version(&self) -> u64 {
        self.ego.version()
    }

    pub(crate) fn conversation_turns(
        &mut self,
        conversation: &str,
        leaf: &str,
    ) -> Result<Vec<crate::ResolvedTurn>, crate::ArchiveError> {
        self.archive
            .conversation_turns(&mut self.container, conversation, leaf)
    }

    pub(crate) fn conversation_branch_start_node_ids(
        &self,
        conversation: &str,
        leaf: &str,
    ) -> Result<Vec<String>, crate::ArchiveError> {
        self.archive
            .conversation_branch_start_node_ids(conversation, leaf)
    }

    pub(crate) fn link_file_to_memory(
        &mut self,
        file_id: crate::FileId,
        memory_id: crate::MemoryId,
    ) -> Result<crate::FileMemoryLink, crate::ArchiveError> {
        self.memories
            .current_body_id(memory_id)
            .map_err(|_| crate::ArchiveError::MissingFileMemoryTarget)?;
        self.archive
            .link_file_to_memory(&mut self.container, file_id, memory_id)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn complete_insomnia_episode(
        &mut self,
        episode_id: crate::EpisodeId,
        token: crate::InsomniaLeaseToken,
        started_at_ns: i64,
        completed_at_ns: i64,
        extractor_model: String,
        extractor_version: String,
        memory_ids: Vec<crate::MemoryId>,
        rejected_count: u32,
    ) -> Result<crate::InsomniaWork, crate::InsomniaError> {
        if memory_ids
            .iter()
            .any(|id| !self.memories.contains_memory(*id))
        {
            return Err(crate::InsomniaError::InvalidTransition);
        }
        let claim = self
            .insomnia
            .active_claim(episode_id, token, completed_at_ns)?;
        let completion = crate::insomnia::completion::InsomniaCompletion {
            episode_id,
            attempt: claim.attempt_count,
            started_at_ns,
            completed_at_ns,
            transaction_time_ns: crate::Container::transaction_time_now_ns()?,
            extractor_model,
            extractor_version,
            rejected_count,
            memory_ids,
            external_memory_refs: Vec::new(),
            global_version_start: self.container.next_version_candidate(),
            bodies: Vec::new(),
            records: Vec::new(),
            routing_metadata: Vec::new(),
            freshness_birth_turn: None,
            freshness_policy_version: None,
            freshness_birth_memory_ids: Vec::new(),
        };
        let payload = crate::insomnia::completion::encode_completion(&completion)
            .map_err(|_| crate::InsomniaError::InvalidField("completion record"))?;
        self.container.append(&payload)?;
        self.container.sync()?;
        self.insomnia.apply_completion(
            &crate::owner_lifecycle::insomnia_replay::completion_receipt(&completion),
        )?;
        self.insomnia
            .work(episode_id)
            .cloned()
            .ok_or(crate::InsomniaError::MissingWork)
    }
}

impl crate::owner_lifecycle::PhylacteryOwner {
    pub(crate) fn entity_version(&self) -> u64 {
        self.entities.entity_version()
    }

    pub(crate) fn entity_resolution(
        &self,
        key: crate::MemoryEntityMentionKey,
    ) -> Option<&crate::MemoryEntityResolution> {
        self.entity_resolutions.get(key)
    }

    pub(crate) fn memory_read_view(&self) -> crate::memory_store::MemoryReadView<'_> {
        crate::memory_store::MemoryReadView::new(&self.memories, &self.container)
    }
    pub(crate) fn community_snapshot(&self) -> Option<crate::CommunitySnapshot> {
        self.communities
            .latest_if_live_members(&self.graph)
            .cloned()
    }

    pub(crate) fn community_semantic_names(&self) -> Vec<crate::CommunitySemanticName> {
        self.communities.current_semantic_names(&self.graph)
    }

    pub(crate) fn semantic_graph_relations(&self) -> Vec<crate::SemanticGraphRelation> {
        self.graph
            .active_semantic_relations()
            .into_iter()
            .filter(|relation| {
                relation.kind != crate::SemanticGraphRelationKind::PrincipalAssociation
            })
            .collect()
    }
}

macro_rules! lineage_read_operations {
    ($owner:ty) => {
        impl $owner {
            pub(crate) fn owner_history_generation(&self) -> crate::HistoryGenerationId {
                self.rewrite_lineage.generation()
            }
            pub(crate) fn owner_rewrite_genesis(&self) -> Option<&crate::RewriteGenesis> {
                self.rewrite_lineage.rewrite_genesis()
            }
        }
    };
}
lineage_read_operations!(crate::owner_lifecycle::ReliquaryOwner);
lineage_read_operations!(crate::owner_lifecycle::PhylacteryOwner);
