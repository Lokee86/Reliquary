use crate::interaction_session::SessionState;
use crate::{
    ArchiveError, ConversationSummary, Cva, CvaError, EpisodePolicy, EpisodeSchedulingResult,
    IngestedTurn, InteractionError, InteractionStreamStatus, InteractionTurn,
    InteractionTurnStatus, ResolvedInteractionTurn, ResolvedTurn,
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

pub struct InteractionRuntime {
    pub(crate) cva: Cva,
    pub(crate) sessions: HashMap<String, SessionState>,
}

impl InteractionRuntime {
    pub fn new(cva: Cva) -> Self {
        Self {
            cva,
            sessions: HashMap::new(),
        }
    }

    pub fn accept_turn(&mut self, turn: InteractionTurn) -> Result<InteractionReceipt, CvaError> {
        let accepted = self.cva.ingest_turn_with_receipt(turn.into())?;
        self.cva.sync()?;
        Ok(InteractionReceipt {
            turn: accepted.value,
            archive_version: self.cva.archive_version(),
            inserted: accepted.inserted,
            activity_position: accepted.activity_position,
            rel_turn_count: accepted.rel_turn_count,
        })
    }

    pub fn schedule_session(
        &mut self,
        session_id: &str,
        policy: EpisodePolicy,
        now_ns: i64,
    ) -> Result<EpisodeSchedulingResult, InteractionError> {
        let leaf = self.session_leaf(session_id)?;
        let result = self
            .cva
            .materialize_live_path_and_queue(session_id, &leaf, policy, now_ns)?;
        self.cva.sync()?;
        Ok(result)
    }

    pub fn finalize_inactive_session(
        &mut self,
        session_id: &str,
        policy: EpisodePolicy,
        now_ns: i64,
    ) -> Result<Option<EpisodeSchedulingResult>, InteractionError> {
        let leaf = self.session_leaf(session_id)?;
        let result = self
            .cva
            .finalize_inactive_path_and_queue(session_id, &leaf, policy, now_ns)?;
        if result.is_some() {
            self.cva.sync()?;
        }
        Ok(result)
    }

    pub fn finalize_explicit_session(
        &mut self,
        session_id: &str,
        policy: EpisodePolicy,
        now_ns: i64,
    ) -> Result<EpisodeSchedulingResult, InteractionError> {
        let leaf = self.session_leaf(session_id)?;
        let result = self
            .cva
            .finalize_explicit_path_and_queue(session_id, &leaf, policy, now_ns)?;
        self.cva.sync()?;
        Ok(result)
    }

    pub fn conversation_summaries(&self) -> Vec<ConversationSummary> {
        self.cva.conversation_summaries()
    }

    pub fn conversation_turns(
        &mut self,
        conversation_id: &str,
        leaf_node_id: &str,
    ) -> Result<Vec<ResolvedTurn>, ArchiveError> {
        self.cva.conversation_turns(conversation_id, leaf_node_id)
    }

    pub fn search_conversation_branch(
        &mut self,
        conversation_id: &str,
        leaf_node_id: &str,
        query: &str,
        limit: usize,
    ) -> Result<Vec<crate::ConversationSearchHit>, crate::SearchError> {
        self.cva
            .search_conversation_branch(conversation_id, leaf_node_id, query, limit)
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
        self.cva.search_conversation_branch_with_vector(
            compatibility_profile_id,
            query_vector,
            conversation_id,
            leaf_node_id,
            query,
            limit,
        )
    }

    pub fn conversation_transcript(
        &mut self,
        conversation_id: &str,
        leaf_node_id: &str,
    ) -> Result<Vec<ResolvedInteractionTurn>, ArchiveError> {
        let durable = self.cva.conversation_turns(conversation_id, leaf_node_id)?;
        let mut transcript = self.resolve_interaction_turns(conversation_id, durable);
        self.append_stream_records(conversation_id, &mut transcript);
        transcript.sort_by_key(|turn| turn.timestamp_ns);
        Ok(transcript)
    }

    pub fn conversation_transcript_page(
        &mut self,
        conversation_id: &str,
        end_node_id: &str,
        limit: usize,
        include_stream_records: bool,
    ) -> Result<(Vec<ResolvedInteractionTurn>, Option<String>), ArchiveError> {
        let (durable, older_cursor) =
            self.cva
                .conversation_turn_page(conversation_id, end_node_id, limit)?;
        let mut transcript = self.resolve_interaction_turns(conversation_id, durable);
        if include_stream_records {
            self.append_stream_records(conversation_id, &mut transcript);
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
                let attachments = self.cva.files_for_source(conversation_id, &turn.node_id);
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
    ) {
        let path_ids = transcript
            .iter()
            .map(|turn| turn.message_id.clone())
            .collect::<std::collections::HashSet<_>>();
        for record in self.cva.interaction_streams_for_session(conversation_id) {
            if self
                .cva
                .archive()
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
                content: record.content,
                attachments: Vec::new(),
                status,
            });
        }
    }

    pub fn session_leaf_node_id(&self, session_id: &str) -> Result<String, InteractionError> {
        self.session_leaf(session_id)
    }

    pub fn cva(&self) -> &Cva {
        &self.cva
    }

    pub fn into_cva(self) -> Cva {
        self.cva
    }

    fn session_leaf(&self, session_id: &str) -> Result<String, InteractionError> {
        self.sessions
            .get(session_id)
            .ok_or(InteractionError::UnknownSession)?
            .leaf_message_id
            .clone()
            .ok_or(InteractionError::SessionHasNoDurableTurn)
    }
}
