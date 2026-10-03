use crate::{ArchiveError, Cva, IncomingTurn, IngestedTurn, StoredFile};

impl Cva {
    pub fn ingest_turn(&mut self, turn: IncomingTurn) -> Result<IngestedTurn, ArchiveError> {
        self.ingest_turn_with_receipt(turn)
            .map(|accepted| accepted.value)
    }

    pub(crate) fn ingest_turn_with_receipt(
        &mut self,
        turn: IncomingTurn,
    ) -> Result<crate::archive_activity::TurnAcceptance<IngestedTurn>, ArchiveError> {
        self.archive
            .ingest_turn(&mut self.container, &self.project_files, turn)
    }

    pub fn files_for_source(&self, conversation_id: &str, node_id: &str) -> Vec<StoredFile> {
        self.archive.files_for_source(conversation_id, node_id)
    }
}
