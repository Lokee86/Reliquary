use crate::{ArchiveError, Cva, IncomingTurn, IngestedTurn, StoredFile};

impl Cva {
    pub fn ingest_turn(&mut self, turn: IncomingTurn) -> Result<IngestedTurn, ArchiveError> {
        self.runtime_ingest_turn(turn)
    }

    pub(crate) fn ingest_turn_with_receipt(
        &mut self,
        turn: IncomingTurn,
    ) -> Result<crate::archive_activity::TurnAcceptance<IngestedTurn>, ArchiveError> {
        self.accept_interaction_turn(turn)
    }

    pub fn files_for_source(&self, conversation_id: &str, node_id: &str) -> Vec<StoredFile> {
        self.runtime_files_for_source(conversation_id, node_id)
    }

    pub fn historical_files_for_source(
        &self,
        conversation_id: &str,
        node_id: &str,
    ) -> Vec<StoredFile> {
        self.runtime_historical_files_for_source(conversation_id, node_id)
    }
}
