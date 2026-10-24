use crate::interaction_session::SessionState;
use crate::{
    ArchiveError, ConversationSummary, CvaError, EpisodeBoundary, EpisodeOrigin, EpisodePolicy,
    EpisodeSchedulingResult, IngestedTurn, InsomniaError, InsomniaPriority, InteractionError,
    InteractionStreamStatus, InteractionTurn, InteractionTurnStatus, ResolvedInteractionTurn,
    ResolvedTurn,
};
use std::collections::HashMap;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InteractionReceipt {
    pub turn: IngestedTurn,
    pub archive_version: u64,
    /// Whether Archive accepted a novel logical Node in this operation.
    pub inserted: bool,
    /// The eligible turn's original REL-local activity position, including replay.
    pub activity_position: Option<u64>,
    /// REL activity count at canonical acceptance, acknowledged after sync.
    pub rel_turn_count: u64,
}

#[derive(Debug)]
pub struct InteractionCompletion {
    pub receipt: InteractionReceipt,
    pub scheduling: Result<EpisodeSchedulingResult, InteractionError>,
}

pub struct RelExecution {
    pub(crate) cva: crate::RelExecutionOwner,
    pub(crate) sessions: HashMap<String, SessionState>,
}

impl RelExecution {
    pub(crate) fn from_owner(cva: crate::RelExecutionOwner) -> Self {
        Self {
            cva,
            sessions: HashMap::new(),
        }
    }

    pub(crate) fn accept_turn_operation(
        &mut self,
        turn: InteractionTurn,
    ) -> Result<InteractionReceipt, CvaError> {
        let accepted = self.cva.accept_interaction_turn(turn.into())?;
        self.cva.sync_owner()?;
        Ok(InteractionReceipt {
            turn: accepted.value,
            archive_version: self.cva.archive.archive_version(),
            inserted: accepted.inserted,
            activity_position: accepted.activity_position,
            rel_turn_count: accepted.rel_turn_count,
        })
    }

    pub(crate) fn schedule_session_operation(
        &mut self,
        session_id: &str,
        policy: EpisodePolicy,
        now_ns: i64,
    ) -> Result<EpisodeSchedulingResult, InteractionError> {
        let leaf = self.session_leaf(session_id)?;
        let policy = policy
            .validate()
            .map_err(|_| InsomniaError::InvalidField("episode policy"))?;
        let episodes = self
            .cva
            .owner
            .archive
            .materialize_path_episodes(
                &mut self.cva.owner.container,
                session_id,
                &leaf,
                policy.episode,
                EpisodeOrigin::Live,
                None,
            )
            .map_err(|_| InsomniaError::InvalidField("episode materialization"))?;
        let result = self.queue_episode_result(episodes, InsomniaPriority::Live, now_ns)?;
        self.cva.sync_owner()?;
        Ok(result)
    }

    pub(crate) fn finalize_inactive_session_operation(
        &mut self,
        session_id: &str,
        policy: EpisodePolicy,
        now_ns: i64,
    ) -> Result<Option<EpisodeSchedulingResult>, InteractionError> {
        let leaf = self.session_leaf(session_id)?;
        let policy = policy
            .validate()
            .map_err(|_| InsomniaError::InvalidField("episode inactivity"))?;
        let last_source_ns = self
            .cva
            .archive
            .require_node(session_id, &leaf, ArchiveError::MissingLeaf)
            .map_err(|_| InsomniaError::InvalidField("episode inactivity"))?
            .timestamp_ns;
        let result = if !policy.inactive(last_source_ns, now_ns) {
            None
        } else {
            let episodes = self
                .cva
                .owner
                .archive
                .materialize_path_episodes(
                    &mut self.cva.owner.container,
                    session_id,
                    &leaf,
                    policy.episode,
                    EpisodeOrigin::Live,
                    Some((EpisodeBoundary::Inactivity, now_ns)),
                )
                .map_err(|_| InsomniaError::InvalidField("episode inactivity"))?;
            Some(self.queue_episode_result(episodes, InsomniaPriority::Live, now_ns)?)
        };
        if result.is_some() {
            self.cva.sync_owner()?;
        }
        Ok(result)
    }

    pub(crate) fn finalize_explicit_session_operation(
        &mut self,
        session_id: &str,
        policy: EpisodePolicy,
        now_ns: i64,
    ) -> Result<EpisodeSchedulingResult, InteractionError> {
        let leaf = self.session_leaf(session_id)?;
        let policy = policy
            .validate()
            .map_err(|_| InsomniaError::InvalidField("episode policy"))?;
        let episodes = self
            .cva
            .owner
            .archive
            .materialize_path_episodes(
                &mut self.cva.owner.container,
                session_id,
                &leaf,
                policy.episode,
                EpisodeOrigin::Live,
                Some((EpisodeBoundary::Explicit, now_ns)),
            )
            .map_err(|_| InsomniaError::InvalidField("explicit episode"))?;
        let result =
            self.queue_episode_result(episodes, InsomniaPriority::ImmediateLive, now_ns)?;
        self.cva.sync_owner()?;
        Ok(result)
    }

    pub(crate) fn conversation_summaries_operation(&self) -> Vec<ConversationSummary> {
        self.cva.archive.conversation_summaries()
    }

    pub fn conversation_turns(
        &mut self,
        conversation_id: &str,
        leaf_node_id: &str,
    ) -> Result<Vec<ResolvedTurn>, ArchiveError> {
        self.cva.owner.archive.conversation_turns(
            &mut self.cva.owner.container,
            conversation_id,
            leaf_node_id,
        )
    }

    pub fn search_conversation_branch(
        &mut self,
        conversation_id: &str,
        leaf_node_id: &str,
        query: &str,
        limit: usize,
    ) -> Result<Vec<crate::ConversationSearchHit>, crate::SearchError> {
        self.search_conversation_branch_operation(conversation_id, leaf_node_id, query, limit)
    }

    pub(crate) fn search_conversation_branch_operation(
        &mut self,
        conversation_id: &str,
        leaf_node_id: &str,
        query: &str,
        limit: usize,
    ) -> Result<Vec<crate::ConversationSearchHit>, crate::SearchError> {
        let archive = crate::archive::ArchiveReadView::new(
            &self.cva.owner.archive,
            &self.cva.owner.container,
        );
        let vectors = crate::vector_generation_store::VectorReadView::new(
            &self.cva.owner.vector_generations,
            &self.cva.owner.archive_vectors,
            &self.cva.owner.packed_vectors,
            &self.cva.owner.container,
        );
        crate::conversation_search::search_conversation_branch_parts(
            &archive,
            &mut self.cva.lexical_index,
            &vectors,
            None,
            None,
            conversation_id,
            leaf_node_id,
            query,
            limit,
        )
    }

    pub(crate) fn search_conversation_branch_with_vector(
        &mut self,
        compatibility_profile_id: crate::CompatibilityProfileId,
        query_vector: &[f32],
        conversation_id: &str,
        leaf_node_id: &str,
        query: &str,
        limit: usize,
    ) -> Result<Vec<crate::ConversationSearchHit>, crate::SearchError> {
        self.search_conversation_branch_with_vector_operation(
            compatibility_profile_id,
            query_vector,
            conversation_id,
            leaf_node_id,
            query,
            limit,
        )
    }

    pub(crate) fn search_conversation_branch_with_vector_operation(
        &mut self,
        compatibility_profile_id: crate::CompatibilityProfileId,
        query_vector: &[f32],
        conversation_id: &str,
        leaf_node_id: &str,
        query: &str,
        limit: usize,
    ) -> Result<Vec<crate::ConversationSearchHit>, crate::SearchError> {
        let archive = crate::archive::ArchiveReadView::new(
            &self.cva.owner.archive,
            &self.cva.owner.container,
        );
        let vectors = crate::vector_generation_store::VectorReadView::new(
            &self.cva.owner.vector_generations,
            &self.cva.owner.archive_vectors,
            &self.cva.owner.packed_vectors,
            &self.cva.owner.container,
        );
        crate::conversation_search::search_conversation_branch_parts(
            &archive,
            &mut self.cva.lexical_index,
            &vectors,
            Some(compatibility_profile_id),
            Some(query_vector),
            conversation_id,
            leaf_node_id,
            query,
            limit,
        )
    }

    pub(crate) fn conversation_transcript_operation(
        &mut self,
        conversation_id: &str,
        leaf_node_id: &str,
    ) -> Result<Vec<ResolvedInteractionTurn>, ArchiveError> {
        self.conversation_transcript_excluding_streams(
            conversation_id,
            leaf_node_id,
            &std::collections::HashSet::new(),
        )
    }

    pub(crate) fn conversation_transcript_excluding_streams(
        &mut self,
        conversation_id: &str,
        leaf_node_id: &str,
        excluded: &std::collections::HashSet<String>,
    ) -> Result<Vec<ResolvedInteractionTurn>, ArchiveError> {
        let durable = self.cva.owner.archive.conversation_turns(
            &mut self.cva.owner.container,
            conversation_id,
            leaf_node_id,
        )?;
        let mut transcript = self.resolve_interaction_turns(conversation_id, durable);
        self.append_stream_records(conversation_id, &mut transcript, excluded);
        transcript.sort_by_key(|turn| turn.timestamp_ns);
        Ok(transcript)
    }

    pub(crate) fn conversation_transcript_page_operation(
        &mut self,
        conversation_id: &str,
        end_node_id: &str,
        limit: usize,
        include_stream_records: bool,
    ) -> Result<(Vec<ResolvedInteractionTurn>, Option<String>), ArchiveError> {
        let (durable, older_cursor) = self.cva.owner.archive.conversation_turn_page(
            &mut self.cva.owner.container,
            conversation_id,
            end_node_id,
            limit,
        )?;
        let mut transcript = self.resolve_interaction_turns(conversation_id, durable);
        if include_stream_records {
            self.append_stream_records(
                conversation_id,
                &mut transcript,
                &std::collections::HashSet::new(),
            );
            transcript.sort_by_key(|turn| turn.timestamp_ns);
        }
        Ok((transcript, older_cursor))
    }

    fn resolve_interaction_turns(
        &self,
        conversation_id: &str,
        durable: Vec<ResolvedTurn>,
    ) -> Vec<ResolvedInteractionTurn> {
        durable
            .into_iter()
            .map(|turn| {
                let attachments = self
                    .cva
                    .archive
                    .files_for_source(conversation_id, &turn.node_id);
                ResolvedInteractionTurn {
                    message_id: turn.node_id,
                    role: turn.role,
                    principal_id: turn.principal_id,
                    timestamp_ns: turn.timestamp_ns,
                    content: turn.content,
                    attachments,
                    status: InteractionTurnStatus::Complete,
                }
            })
            .collect()
    }

    fn append_stream_records(
        &self,
        conversation_id: &str,
        transcript: &mut Vec<ResolvedInteractionTurn>,
        excluded: &std::collections::HashSet<String>,
    ) {
        let path_ids = transcript
            .iter()
            .map(|turn| turn.message_id.clone())
            .collect::<std::collections::HashSet<_>>();
        for record in self
            .cva
            .interaction_streams
            .borrowed_records_for_session(conversation_id)
        {
            if excluded.contains(&record.message_id) {
                continue;
            }
            if self
                .cva
                .archive
                .has_node(conversation_id, &record.message_id)
            {
                continue;
            }
            if let Some(parent) = record.parent_message_id.as_deref() {
                if !path_ids.contains(parent) {
                    continue;
                }
            }
            let actively_streaming = self
                .sessions
                .get(conversation_id)
                .and_then(|state| state.in_flight.as_ref())
                .is_some_and(|message| message.message_id == record.message_id);
            let status = match (record.status, actively_streaming) {
                (InteractionStreamStatus::Streaming, true) => InteractionTurnStatus::Streaming,
                _ => InteractionTurnStatus::Interrupted,
            };
            transcript.push(ResolvedInteractionTurn {
                message_id: record.message_id.clone(),
                role: record.role.archive_role().into(),
                principal_id: self
                    .sessions
                    .get(conversation_id)
                    .and_then(|state| state.in_flight.as_ref())
                    .filter(|message| message.message_id == record.message_id)
                    .and_then(|message| message.principal_id.clone()),
                timestamp_ns: record.timestamp_ns,
                content: record.content.clone(),
                attachments: Vec::new(),
                status,
            });
        }
    }

    pub(crate) fn session_leaf_node_id_operation(
        &self,
        session_id: &str,
    ) -> Result<String, InteractionError> {
        self.session_leaf(session_id)
    }

    fn session_leaf(&self, session_id: &str) -> Result<String, InteractionError> {
        self.sessions
            .get(session_id)
            .ok_or(InteractionError::UnknownSession)?
            .leaf_message_id
            .clone()
            .ok_or(InteractionError::SessionHasNoDurableTurn)
    }

    fn queue_episode_result(
        &mut self,
        episodes: crate::EpisodeBuildResult,
        priority: InsomniaPriority,
        now_ns: i64,
    ) -> Result<EpisodeSchedulingResult, InsomniaError> {
        let mut queued = Vec::new();
        for episode in &episodes.created {
            let source = crate::insomnia_admission::episode_source(episode, priority);
            let (work, _) =
                self.cva
                    .owner
                    .insomnia
                    .queue(&mut self.cva.owner.container, &source, now_ns)?;
            queued.push(work);
        }
        Ok(EpisodeSchedulingResult { episodes, queued })
    }
}
