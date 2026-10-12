use super::{ReliquaryRuntimeHost, ReliquaryRuntimeHostError, operation};
use crate::{
    ConversationSummary, EmbeddingMode, EpisodePolicy, InteractionCompletion, InteractionReceipt,
    InteractionRole, InteractionSession, ResolvedInteractionTurn,
};

impl ReliquaryRuntimeHost {
    fn with_phylactery<T>(
        &self,
        action: impl FnOnce(&mut crate::Phylactery) -> Result<T, ReliquaryRuntimeHostError>,
    ) -> Result<Option<T>, ReliquaryRuntimeHostError> {
        let mut phylactery = self
            .phylactery
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        phylactery.as_mut().map(action).transpose()
    }

    fn with_runtime<T>(
        &self,
        action: impl FnOnce(&mut crate::InteractionRuntime) -> Result<T, ReliquaryRuntimeHostError>,
    ) -> Result<T, ReliquaryRuntimeHostError> {
        let runtime = self.active_execution()?.runtime.as_ref().ok_or_else(|| {
            ReliquaryRuntimeHostError::Operation("Reliquary runtime is unavailable".into())
        })?;
        let mut runtime = runtime
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        action(&mut runtime)
    }

    #[cfg(test)]
    pub(crate) fn semantic_event_completed_for_test(
        &self,
        id: &str,
    ) -> Result<bool, ReliquaryRuntimeHostError> {
        self.with_runtime(|runtime| {
            runtime
                .cva()
                .freshness
                .contains_event(id)
                .map_err(operation)
        })
    }

    pub fn owner_id(&self) -> Result<Option<String>, ReliquaryRuntimeHostError> {
        self.with_runtime(|runtime| Ok(runtime.cva().owner_id()))
    }

    pub fn phylactery_owner_id(&self) -> Result<Option<String>, ReliquaryRuntimeHostError> {
        Ok(self
            .with_phylactery(|phylactery| Ok(phylactery.owner_id()))?
            .flatten())
    }

    pub fn conversation_summaries(
        &self,
    ) -> Result<Vec<ConversationSummary>, ReliquaryRuntimeHostError> {
        self.with_runtime(|runtime| Ok(runtime.conversation_summaries()))
    }

    pub fn conversation_transcript(
        &self,
        conversation_id: &str,
        leaf_node_id: &str,
    ) -> Result<Vec<ResolvedInteractionTurn>, ReliquaryRuntimeHostError> {
        self.with_runtime(|runtime| {
            runtime
                .conversation_transcript(conversation_id, leaf_node_id)
                .map_err(operation)
        })
    }

    pub fn conversation_transcript_page(
        &self,
        conversation_id: &str,
        end_node_id: &str,
        limit: usize,
        include_stream_records: bool,
    ) -> Result<(Vec<ResolvedInteractionTurn>, Option<String>), ReliquaryRuntimeHostError> {
        self.with_runtime(|runtime| {
            runtime
                .conversation_transcript_page(
                    conversation_id,
                    end_node_id,
                    limit,
                    include_stream_records,
                )
                .map_err(operation)
        })
    }

    pub fn conversation_branch_start_node_ids(
        &self,
        conversation_id: &str,
        leaf_node_id: &str,
    ) -> Result<Vec<String>, ReliquaryRuntimeHostError> {
        self.with_runtime(|runtime| {
            runtime
                .cva()
                .conversation_branch_start_node_ids(conversation_id, leaf_node_id)
                .map_err(operation)
        })
    }

    pub fn set_conversation_title(
        &self,
        conversation_id: &str,
        title: String,
    ) -> Result<bool, ReliquaryRuntimeHostError> {
        self.with_runtime(|runtime| {
            let changed = runtime
                .cva
                .set_conversation_title(conversation_id, title)
                .map_err(operation)?;
            if changed {
                runtime.cva.sync().map_err(operation)?;
            }
            Ok(changed)
        })
    }

    pub fn set_conversation_active(
        &self,
        conversation_id: &str,
        active: bool,
    ) -> Result<bool, ReliquaryRuntimeHostError> {
        self.with_runtime(|runtime| {
            let changed = runtime
                .cva
                .set_conversation_active(conversation_id, active)
                .map_err(operation)?;
            if changed {
                runtime.cva.sync().map_err(operation)?;
            }
            Ok(changed)
        })
    }

    pub fn clear_active_conversations(&self) -> Result<usize, ReliquaryRuntimeHostError> {
        self.with_runtime(|runtime| {
            let active_ids = runtime
                .conversation_summaries()
                .into_iter()
                .filter(|summary| summary.active)
                .map(|summary| summary.conversation_id)
                .collect::<Vec<_>>();
            let mut changed = 0;
            for conversation_id in active_ids {
                if runtime
                    .cva
                    .set_conversation_active(&conversation_id, false)
                    .map_err(operation)?
                {
                    changed += 1;
                }
            }
            if changed > 0 {
                runtime.cva.sync().map_err(operation)?;
            }
            Ok(changed)
        })
    }

    pub fn conversation_compactions(
        &self,
        conversation_id: &str,
    ) -> Result<Vec<crate::ConversationCompaction>, ReliquaryRuntimeHostError> {
        self.with_runtime(|runtime| Ok(runtime.cva().conversation_compactions(conversation_id)))
    }

    pub fn put_echo_event(
        &self,
        event: crate::EchoEvent,
    ) -> Result<bool, ReliquaryRuntimeHostError> {
        self.with_runtime(|runtime| {
            let changed = runtime.cva.put_echo_event(event).map_err(operation)?;
            if changed {
                runtime.cva.sync().map_err(operation)?;
            }
            Ok(changed)
        })
    }

    pub fn echo_events(
        &self,
        conversation_id: &str,
        message_id: &str,
    ) -> Result<Vec<crate::EchoEvent>, ReliquaryRuntimeHostError> {
        self.with_runtime(|runtime| Ok(runtime.cva().echo_events(conversation_id, message_id)))
    }

    pub fn put_conversation_compaction(
        &self,
        conversation_id: String,
        through_message_id: String,
        summary: String,
        supersedes_through_message_id: Option<&str>,
    ) -> Result<crate::ConversationCompaction, ReliquaryRuntimeHostError> {
        self.with_runtime(|runtime| {
            runtime
                .cva
                .put_conversation_compaction(
                    conversation_id,
                    through_message_id,
                    summary,
                    supersedes_through_message_id,
                )
                .map_err(operation)
        })
    }

    pub fn search_conversation_branch(
        &self,
        conversation_id: &str,
        leaf_node_id: &str,
        query: &str,
        limit: usize,
    ) -> Result<Vec<crate::ConversationSearchHit>, ReliquaryRuntimeHostError> {
        let semantic = self.live_search_vector(query)?;
        self.with_runtime(|runtime| match semantic.as_ref() {
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

    pub fn search_open_session(
        &self,
        session_id: &str,
        query: &str,
        limit: usize,
    ) -> Result<Vec<crate::ConversationSearchHit>, ReliquaryRuntimeHostError> {
        let leaf = self
            .with_runtime(|runtime| runtime.session_leaf_node_id(session_id).map_err(operation))?;
        self.search_conversation_branch(session_id, &leaf, query, limit)
    }

    fn live_search_vector(
        &self,
        query: &str,
    ) -> Result<Option<(crate::CompatibilityProfileId, Vec<f32>)>, ReliquaryRuntimeHostError> {
        if query.trim().is_empty() {
            return Ok(None);
        }
        let endpoint = self
            .active_execution()?
            .routes
            .read()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
            .embedding();
        let Some(endpoint) = endpoint else {
            return Ok(None);
        };
        let profile = self.ensure_reliquary_embedding_profile()?;
        let has_generation = self.with_runtime(|runtime| {
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

    pub fn open_session(
        &self,
        session_id: String,
        resume_from: Option<String>,
    ) -> Result<InteractionSession, ReliquaryRuntimeHostError> {
        self.with_runtime(|runtime| {
            runtime
                .open_session(session_id, resume_from)
                .map_err(operation)
        })
    }

    pub fn finalize_explicit_session(
        &self,
        session_id: &str,
        now_ns: i64,
    ) -> Result<crate::EpisodeSchedulingResult, ReliquaryRuntimeHostError> {
        let policy = self.active_execution()?.episode_policy;
        let result = self.with_runtime(|runtime| {
            runtime
                .finalize_explicit_session(session_id, policy, now_ns)
                .map_err(operation)
        })?;
        self.wake()?;
        Ok(result)
    }

    pub fn close_session(
        &self,
        session_id: &str,
    ) -> Result<InteractionSession, ReliquaryRuntimeHostError> {
        let policy = self.active_execution()?.episode_policy;
        let now_ns = crate::insomnia::runtime_step::now_ns();
        let session = self.with_runtime(|runtime| {
            let has_durable_turn = runtime
                .session(session_id)
                .ok_or_else(|| operation("interaction session is not open"))?
                .leaf_message_id
                .is_some();
            if has_durable_turn {
                runtime
                    .finalize_explicit_session(session_id, policy, now_ns)
                    .map_err(operation)?;
            }
            runtime.close_session(session_id).map_err(operation)
        })?;
        self.wake()?;
        Ok(session)
    }

    pub fn begin_message(
        &self,
        session_id: &str,
        message_id: String,
        role: InteractionRole,
        timestamp_ns: i64,
    ) -> Result<(), ReliquaryRuntimeHostError> {
        let principal_id = self.phylactery_owner_id()?;
        self.with_runtime(|runtime| {
            runtime
                .begin_message_with_principal(
                    session_id,
                    message_id,
                    role,
                    principal_id,
                    timestamp_ns,
                )
                .map_err(operation)
        })
    }

    pub fn correlate_project_revision(
        &self,
        revision: crate::ProjectRevisionRef,
    ) -> Result<(crate::ProjectRevisionCorrelation, bool), ReliquaryRuntimeHostError> {
        self.with_runtime(|runtime| {
            runtime
                .cva
                .correlate_project_revision(revision)
                .map_err(operation)
        })
    }

    pub fn correlate_project_revision_with_management(
        &self,
        revision: crate::ProjectRevisionRef,
        management: crate::ProjectRepositoryManagement,
    ) -> Result<(crate::ProjectRevisionCorrelation, bool), ReliquaryRuntimeHostError> {
        self.with_runtime(|runtime| {
            runtime
                .cva
                .correlate_project_revision_with_management(revision, management)
                .map_err(operation)
        })
    }

    pub fn latest_project_revision_correlation(
        &self,
    ) -> Result<Option<crate::ProjectRevisionCorrelation>, ReliquaryRuntimeHostError> {
        self.with_runtime(|runtime| Ok(runtime.cva.latest_project_revision_correlation()))
    }

    pub fn register_project_file(
        &self,
        filename: String,
        mime_type: Option<String>,
        byte_length: u64,
        reference: crate::ProjectFileRef,
    ) -> Result<crate::StoredFile, ReliquaryRuntimeHostError> {
        self.with_runtime(|runtime| {
            runtime
                .cva
                .register_project_file(filename, mime_type, byte_length, reference)
                .map_err(operation)
        })
    }

    pub fn project_file_ref(
        &self,
        file_id: crate::FileId,
    ) -> Result<Option<crate::ProjectFileRef>, ReliquaryRuntimeHostError> {
        self.with_runtime(|runtime| Ok(runtime.cva.project_file_ref(file_id)))
    }

    pub fn files_for_source(
        &self,
        conversation_id: &str,
        node_id: &str,
    ) -> Result<Vec<crate::StoredFile>, ReliquaryRuntimeHostError> {
        self.with_runtime(|runtime| Ok(runtime.cva.files_for_source(conversation_id, node_id)))
    }

    pub fn attach_project_file(
        &self,
        session_id: &str,
        message_id: &str,
        file: crate::StoredFile,
    ) -> Result<(), ReliquaryRuntimeHostError> {
        self.with_runtime(|runtime| {
            runtime
                .attach_project_file(session_id, message_id, file)
                .map_err(operation)
        })
    }

    pub fn append_text(
        &self,
        session_id: &str,
        message_id: &str,
        delta: &str,
    ) -> Result<(), ReliquaryRuntimeHostError> {
        self.with_runtime(|runtime| {
            runtime
                .append_text(session_id, message_id, delta)
                .map_err(operation)
        })
    }

    pub fn append_checkpointed_text(
        &self,
        session_id: &str,
        message_id: &str,
        delta: &str,
    ) -> Result<(), ReliquaryRuntimeHostError> {
        self.with_runtime(|runtime| {
            runtime
                .append_checkpointed_text(session_id, message_id, delta)
                .map_err(operation)
        })
    }

    pub fn interrupt_message(
        &self,
        session_id: &str,
        message_id: &str,
    ) -> Result<(), ReliquaryRuntimeHostError> {
        self.with_runtime(|runtime| {
            runtime
                .interrupt_message(session_id, message_id)
                .map_err(operation)
        })
    }

    pub fn cancel_message(
        &self,
        session_id: &str,
        message_id: &str,
    ) -> Result<(), ReliquaryRuntimeHostError> {
        self.with_runtime(|runtime| {
            runtime
                .cancel_message(session_id, message_id)
                .map_err(operation)
        })
    }

    pub fn complete_message(
        &self,
        session_id: &str,
        message_id: &str,
    ) -> Result<InteractionReceipt, ReliquaryRuntimeHostError> {
        self.with_runtime(|runtime| {
            runtime
                .complete_message(session_id, message_id)
                .map_err(operation)
        })
    }

    pub fn complete_live_message(
        &self,
        session_id: &str,
        message_id: &str,
        policy: EpisodePolicy,
        now_ns: i64,
    ) -> Result<InteractionCompletion, ReliquaryRuntimeHostError> {
        let completion = self.with_runtime(|runtime| {
            runtime
                .complete_live_message(session_id, message_id, policy, now_ns)
                .map_err(operation)
        })?;
        self.wake()?;
        Ok(completion)
    }

    pub fn sync(&self) -> Result<(), ReliquaryRuntimeHostError> {
        self.with_runtime(|runtime| runtime.cva().sync().map_err(operation))
    }

    pub fn insomnia_stats(&self) -> Result<crate::InsomniaStats, ReliquaryRuntimeHostError> {
        self.with_runtime(|runtime| Ok(runtime.cva().insomnia_stats()))
    }

    pub fn memory_stats(&self) -> Result<crate::MemoryStats, ReliquaryRuntimeHostError> {
        self.with_runtime(|runtime| Ok(runtime.cva().memory_stats()))
    }

    pub fn memory_vector_stats(
        &self,
    ) -> Result<crate::MemoryVectorStats, ReliquaryRuntimeHostError> {
        self.with_runtime(|runtime| Ok(runtime.cva().memory_vector_stats()))
    }

    pub fn phylactery_memory_stats(
        &self,
    ) -> Result<Option<crate::MemoryStats>, ReliquaryRuntimeHostError> {
        self.with_phylactery(|phylactery| Ok(phylactery.memory_stats()))
    }

    pub fn phylactery_memory_vector_stats(
        &self,
    ) -> Result<Option<crate::MemoryVectorStats>, ReliquaryRuntimeHostError> {
        self.with_phylactery(|phylactery| Ok(phylactery.memory_vector_stats()))
    }
}
