use super::*;

impl PhyWriter {
    /// Append one already accepted, ungrouped Memory revision with an available body.
    pub fn import_accepted_memory_revision(
        &mut self,
        global_version: u64,
        transaction_time_ns: i64,
        accepted: &MemoryRevision,
    ) -> Result<(), OwnerSessionError> {
        let before = self.container.writer_lease_proof();
        if self.poisoned || before.is_none() || self.writer_lease_proof != before {
            return Err(OwnerSessionError::Record(
                "PHY writer requires a current exclusive lease".into(),
            ));
        }
        let PayloadState::Available(body) = &accepted.payload else {
            return Err(OwnerSessionError::Record(
                "accepted Memory revision has no retained body".into(),
            ));
        };
        let draft = super::fresh_history_rewrite::draft_from(accepted, body);
        if let Err(error) = self.common.memories.publish_accepted_versioned_revision(
            &mut self.container,
            global_version,
            transaction_time_ns,
            &accepted.record,
            draft,
        ) {
            if matches!(error, MemoryError::Container(_) | MemoryError::Io(_))
                || before != self.container.writer_lease_proof()
            {
                self.poisoned = true;
            }
            return Err(error.into());
        }
        if let Err(error) = self.container.sync() {
            self.poisoned = true;
            return Err(error.into());
        }
        self.writer_lease_proof = self.container.writer_lease_proof();
        if let Err(error) = self.common.refresh_dream_index(&[accepted.record.id]) {
            self.poisoned = true;
            return Err(error);
        }
        Ok(())
    }
}
