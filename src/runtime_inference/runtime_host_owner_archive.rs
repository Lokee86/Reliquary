use super::{ReliquaryRuntimeHost, ReliquaryRuntimeHostError, operation};
use crate::{ConversationSearchHit, ConversationSummary, EmbeddingMode};

impl ReliquaryRuntimeHost {
    pub fn conversation_summaries_for(
        &self,
        owner_id: &str,
    ) -> Result<Vec<ConversationSummary>, ReliquaryRuntimeHostError> {
        self.with_runtime_for(owner_id, |runtime| Ok(runtime.conversation_summaries()))
    }

    pub fn conversation_transcript_for(
        &self,
        owner_id: &str,
        conversation_id: &str,
        leaf_node_id: &str,
    ) -> Result<Vec<crate::ResolvedInteractionTurn>, ReliquaryRuntimeHostError> {
        self.with_runtime_for(owner_id, |runtime| {
            runtime
                .conversation_transcript(conversation_id, leaf_node_id)
                .map_err(operation)
        })
    }

    pub fn echo_events_for(
        &self,
        owner_id: &str,
        conversation_id: &str,
        message_id: &str,
    ) -> Result<Vec<crate::EchoEvent>, ReliquaryRuntimeHostError> {
        self.with_runtime_for(owner_id, |runtime| {
            Ok(runtime.cva().echo_events(conversation_id, message_id))
        })
    }

    pub fn search_conversation_branch_for(
        &self,
        owner_id: &str,
        conversation_id: &str,
        leaf_node_id: &str,
        query: &str,
        limit: usize,
    ) -> Result<Vec<ConversationSearchHit>, ReliquaryRuntimeHostError> {
        let semantic = self.live_search_vector_for(owner_id, query)?;
        self.with_runtime_for(owner_id, |runtime| match semantic.as_ref() {
            Some((profile, vector)) => runtime
                .search_conversation_branch_with_vector(
                    *profile,
                    vector,
                    conversation_id,
                    leaf_node_id,
                    query,
                    limit,
                )
                .map_err(operation),
            None => runtime
                .search_conversation_branch(conversation_id, leaf_node_id, query, limit)
                .map_err(operation),
        })
    }

    pub fn files_for_source_for(
        &self,
        owner_id: &str,
        conversation_id: &str,
        node_id: &str,
    ) -> Result<Vec<crate::StoredFile>, ReliquaryRuntimeHostError> {
        self.with_runtime_for(owner_id, |runtime| {
            Ok(runtime.cva.files_for_source(conversation_id, node_id))
        })
    }

    pub fn project_file_ref_for(
        &self,
        owner_id: &str,
        file_id: crate::FileId,
    ) -> Result<Option<crate::ProjectFileRef>, ReliquaryRuntimeHostError> {
        self.with_runtime_for(owner_id, |runtime| {
            Ok(runtime.cva.project_file_ref(file_id))
        })
    }

    fn live_search_vector_for(
        &self,
        owner_id: &str,
        query: &str,
    ) -> Result<Option<(crate::CompatibilityProfileId, Vec<f32>)>, ReliquaryRuntimeHostError> {
        if query.trim().is_empty() {
            return Ok(None);
        }
        let Some(endpoint) = self.embedding_route()? else {
            return Ok(None);
        };
        let profile = self.ensure_reliquary_embedding_profile_for(owner_id)?;
        let has_generation = self.with_runtime_for(owner_id, |runtime| {
            Ok(runtime.cva().current_vector_generation(profile).is_some())
        })?;
        if !has_generation {
            return Ok(None);
        }
        let vectors = endpoint
            .embed(EmbeddingMode::Query, &[query.to_owned()])
            .map_err(operation)?;
        let vector = vectors
            .into_iter()
            .next()
            .ok_or_else(|| operation("embedding route returned no query vector"))?;
        Ok(Some((profile, vector)))
    }
}
