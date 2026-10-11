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
        let source_memory_cohort = self
            .freshness
            .records()
            .iter()
            .filter_map(|(id, record)| {
                (record.admitted_at_turn == Some(accepted_turn)).then_some(*id)
            })
            .collect();
        let _ =
            self.apply_accepted_semantic_reinforcement(&accepted.value.node, &source_memory_cohort);
        Ok(accepted)
    }

    pub fn files_for_source(&self, conversation_id: &str, node_id: &str) -> Vec<StoredFile> {
        self.archive.files_for_source(conversation_id, node_id)
    }
}
