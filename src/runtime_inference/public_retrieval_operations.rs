//! Canonical interactive/offline retrieval invocation over one owner's facts.
use crate::{
    CompatibilityProfileId, MemoryRetrievalConfig, MemoryRetrievalError, MemoryRetrievalIndex,
    MemoryRetrievalResult,
};

macro_rules! retrieval_operations {
    ($runtime:ty) => {
        impl $runtime {
            pub(crate) fn native_build_memory_retrieval_index(
                &mut self,
                profile: CompatibilityProfileId,
                subcentroids: usize,
            ) -> Result<MemoryRetrievalIndex, MemoryRetrievalError> {
                let facts = self.owner.memory_retrieval_facts(profile);
                crate::memory_retrieval_build::build_memory_retrieval_index(
                    &facts.memories,
                    facts.graph,
                    facts.communities,
                    &facts.vectors,
                    profile,
                    subcentroids,
                )
            }
            pub(crate) fn native_retrieve_memories_with_index(
                &self,
                index: &MemoryRetrievalIndex,
                query: &[f32],
                config: MemoryRetrievalConfig,
            ) -> Result<MemoryRetrievalResult, MemoryRetrievalError> {
                let facts = self
                    .owner
                    .memory_retrieval_facts(index.compatibility_profile_id);
                crate::memory_retrieval::retrieve_memories(
                    facts.graph,
                    facts.memory_version,
                    facts.communities.map(|s| s.generation),
                    facts.vector_bindings,
                    index,
                    query,
                    config,
                )
            }
            pub(crate) fn native_retrieve_memories(
                &mut self,
                profile: CompatibilityProfileId,
                query: &[f32],
                config: MemoryRetrievalConfig,
            ) -> Result<MemoryRetrievalResult, MemoryRetrievalError> {
                let index = self.native_build_memory_retrieval_index(
                    profile,
                    config.subcentroids_per_community,
                )?;
                self.native_retrieve_memories_with_index(&index, query, config)
            }
        }
    };
}
retrieval_operations!(crate::RelExecutionOwner);
retrieval_operations!(crate::phy_execution_owner::PhyExecutionOwner);

impl crate::RelExecutionOwner {
    pub(crate) fn runtime_complete_memory_lexical_matches(
        &mut self,
        query: &str,
    ) -> Result<Vec<crate::lexical_index::MemoryLexicalHit>, crate::MemoryError> {
        let memory_read = self.owner.memory_read_view();
        let mut view =
            crate::lexical_search::MemoryLexicalReadView::new(&mut self.lexical_index, memory_read);
        crate::lexical_search::complete_memory_lexical_matches(&mut view, query)
    }

    pub(crate) fn runtime_lexical_candidates(
        &mut self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<crate::lexical_contract::LexicalHit>, crate::SearchError> {
        let archive = self.owner.archive_read_view();
        crate::lexical_search::lexical_candidates(&mut self.lexical_index, &archive, query, limit)
    }

    pub(crate) fn runtime_raw_archive_candidates(
        &mut self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<crate::lexical_contract::LexicalHit>, crate::SearchError> {
        let archive = self.owner.archive_read_view();
        crate::lexical_search::raw_archive_candidates(
            &mut self.lexical_index,
            &archive,
            query,
            limit,
        )
    }

    pub(crate) fn runtime_lexical_candidates_from_fragments(
        &self,
        query: &str,
        fragments: &[crate::Fragment],
        limit: usize,
    ) -> Result<Vec<crate::lexical_contract::LexicalHit>, crate::SearchError> {
        let archive = self.owner.archive_read_view();
        crate::lexical_search::lexical_candidates_from_fragments(&archive, query, fragments, limit)
    }

    pub(crate) fn runtime_search_files(
        &mut self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<crate::FileSearchHit>, crate::SearchError> {
        let archive = self.owner.archive_read_view();
        crate::lexical_search::search_files(&mut self.lexical_index, &archive, query, limit)
    }

    pub(crate) fn runtime_search_conversation_branch(
        &mut self,
        conversation_id: &str,
        leaf_node_id: &str,
        query: &str,
        limit: usize,
        compatibility_profile_id: Option<CompatibilityProfileId>,
        query_vector: Option<&[f32]>,
    ) -> Result<Vec<crate::ConversationSearchHit>, crate::SearchError> {
        let archive = self.owner.archive_read_view();
        let vectors = self.owner.vector_read_view();
        crate::conversation_search::search_conversation_branch_parts(
            &archive,
            &mut self.lexical_index,
            &vectors,
            compatibility_profile_id,
            query_vector,
            conversation_id,
            leaf_node_id,
            query,
            limit,
        )
    }

    pub(crate) fn runtime_semantic_search(
        &mut self,
        compatibility_profile_id: CompatibilityProfileId,
        endpoint: &impl crate::EmbeddingEndpoint,
        query: &str,
        limit: usize,
    ) -> Result<Vec<crate::SemanticSearchHit>, crate::SemanticSearchError> {
        if query.trim().is_empty() {
            return Err(crate::SemanticSearchError::EmptyQuery);
        }
        if !(1..=crate::MAX_SEMANTIC_SEARCH_LIMIT).contains(&limit) {
            return Err(crate::SemanticSearchError::InvalidLimit);
        }
        let profile = self
            .owner
            .vector_profile_get_active(compatibility_profile_id)
            .cloned()
            .ok_or(crate::SemanticSearchError::MissingProfile)?;
        if !crate::compatibility_profile_probe::verify_endpoint(&profile, endpoint)?.compatible {
            return Err(crate::SemanticSearchError::IncompatibleEndpoint);
        }
        let vectors = endpoint.embed(crate::EmbeddingMode::Query, &[query.to_string()])?;
        crate::compatibility_profile_probe::validate_embedding_batch(&profile, &vectors, 1)?;
        let query_vector = vectors
            .first()
            .ok_or(crate::SemanticSearchError::InvalidQueryVector)?;
        self.runtime_semantic_search_vector(compatibility_profile_id, query_vector, limit, None)
    }

    pub(crate) fn runtime_semantic_search_vector(
        &self,
        compatibility_profile_id: CompatibilityProfileId,
        query: &[f32],
        limit: usize,
        allowed_fragments: Option<&std::collections::HashSet<crate::FragmentId>>,
    ) -> Result<Vec<crate::SemanticSearchHit>, crate::SemanticSearchError> {
        let vectors = self.owner.vector_read_view();
        let archive = self.owner.archive_read_view();
        crate::semantic_search::semantic_search_vector(
            &vectors,
            &archive,
            compatibility_profile_id,
            query,
            limit,
            allowed_fragments,
        )
    }

    pub(crate) fn runtime_search_with_config(
        &mut self,
        compatibility_profile_id: CompatibilityProfileId,
        endpoint: &impl crate::EmbeddingEndpoint,
        query: &str,
        config: crate::RetrievalConfig,
    ) -> Result<Vec<crate::SearchCandidate>, crate::SearchError> {
        let weights = crate::search::validate_search(query, config)?;
        let lexical = self.runtime_lexical_candidates(query, config.candidate_limit)?;
        let semantic = self.runtime_semantic_search(
            compatibility_profile_id,
            endpoint,
            query,
            config.candidate_limit,
        )?;
        let archive = self.owner.archive_read_view();
        crate::search_rank::rank_search_candidates(&archive, lexical, semantic, config, weights)
    }

    pub(crate) fn runtime_search_fragments_with_vector(
        &mut self,
        compatibility_profile_id: Option<CompatibilityProfileId>,
        query_vector: Option<&[f32]>,
        query: &str,
        fragments: &[crate::Fragment],
        config: crate::RetrievalConfig,
    ) -> Result<Vec<crate::SearchCandidate>, crate::SearchError> {
        let weights = crate::search::validate_search(query, config)?;
        let lexical = self.runtime_lexical_candidates_from_fragments(
            query,
            fragments,
            config.candidate_limit,
        )?;
        let semantic = match (compatibility_profile_id, query_vector) {
            (Some(profile), Some(vector))
                if self
                    .owner
                    .vector_generation_current_generation(profile)
                    .is_some() =>
            {
                let allowed: std::collections::HashSet<crate::FragmentId> =
                    fragments.iter().map(|fragment| fragment.id).collect();
                self.runtime_semantic_search_vector(
                    profile,
                    vector,
                    config.candidate_limit,
                    Some(&allowed),
                )?
            }
            _ => Vec::new(),
        };
        let archive = self.owner.archive_read_view();
        crate::search_rank::rank_search_candidates(&archive, lexical, semantic, config, weights)
    }
}
