//! Public InteractionRuntime binding over the canonical lower REL execution.

use crate::{
    ConversationSummary, Cva, CvaError, EpisodePolicy, EpisodeSchedulingResult, InteractionError,
    InteractionReceipt, InteractionTurn, ResolvedInteractionTurn,
};

pub use crate::interaction_runtime::RelExecution as InteractionRuntime;

impl InteractionRuntime {
    pub fn new(cva: Cva) -> Self {
        Self::from_owner(cva)
    }

    pub fn cva(&self) -> &Cva {
        &self.cva
    }

    pub fn into_cva(self) -> Cva {
        self.cva
    }

    pub fn accept_turn(&mut self, turn: InteractionTurn) -> Result<InteractionReceipt, CvaError> {
        self.accept_turn_operation(turn)
    }

    pub fn schedule_session(
        &mut self,
        session_id: &str,
        policy: EpisodePolicy,
        now_ns: i64,
    ) -> Result<EpisodeSchedulingResult, InteractionError> {
        self.schedule_session_operation(session_id, policy, now_ns)
    }

    pub fn finalize_inactive_session(
        &mut self,
        session_id: &str,
        policy: EpisodePolicy,
        now_ns: i64,
    ) -> Result<Option<EpisodeSchedulingResult>, InteractionError> {
        self.finalize_inactive_session_operation(session_id, policy, now_ns)
    }

    pub fn finalize_explicit_session(
        &mut self,
        session_id: &str,
        policy: EpisodePolicy,
        now_ns: i64,
    ) -> Result<EpisodeSchedulingResult, InteractionError> {
        self.finalize_explicit_session_operation(session_id, policy, now_ns)
    }

    pub fn conversation_summaries(&self) -> Vec<ConversationSummary> {
        self.conversation_summaries_operation()
    }

    pub fn conversation_transcript(
        &mut self,
        conversation_id: &str,
        leaf_node_id: &str,
    ) -> Result<Vec<ResolvedInteractionTurn>, crate::ArchiveError> {
        self.conversation_transcript_operation(conversation_id, leaf_node_id)
    }

    pub fn conversation_transcript_page(
        &mut self,
        conversation_id: &str,
        end_node_id: &str,
        limit: usize,
        include_stream_records: bool,
    ) -> Result<(Vec<ResolvedInteractionTurn>, Option<String>), crate::ArchiveError> {
        self.conversation_transcript_page_operation(
            conversation_id,
            end_node_id,
            limit,
            include_stream_records,
        )
    }

    pub fn session_leaf_node_id(&self, session_id: &str) -> Result<String, InteractionError> {
        self.session_leaf_node_id_operation(session_id)
    }
}
