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
        let accepted = self
            .archive
            .ingest_turn(&mut self.container, &self.project_files, turn)?;
        let accepted_turn = self.archive.rel_turn_count();
        self.freshness_on_accepted_activity(accepted_turn);
        // Semantic work is rediscovered after vector publication, not attempted
        // prematurely or silently discarded during accepted source publication.
        Ok(accepted)
    }

    pub fn files_for_source(&self, conversation_id: &str, node_id: &str) -> Vec<StoredFile> {
        self.archive.files_for_source(conversation_id, node_id)
    }
}
