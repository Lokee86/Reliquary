use super::*;

impl RelWriter {
    /// Durably record a reference-only source manifest after Gateway has
    /// verified the host grant and descriptor snapshot. The record contains
    /// no hydrated transcript body and does not attest Memory publication.
    pub fn record_external_reference_snapshot(
        &mut self,
        authorizing_principal: &str,
        manifest: ExternalReferenceSnapshotManifest,
    ) -> Result<ExternalReferenceSnapshotReceipt, OwnerSessionError> {
        self.ensure_write_ready()?;
        let before = self.container.writer_lease_proof();
        if before.is_none() || before != self.session.writer_lease_proof {
            return Err(OwnerSessionError::Record(
                "REL owner session is stale; reopen before external reference writes".into(),
            ));
        }
        match self.session.external_references.record(
            &mut self.container,
            authorizing_principal,
            manifest,
        ) {
            Ok(receipt) => {
                self.session.writer_lease_proof = self.container.writer_lease_proof();
                Ok(receipt)
            }
            Err(error) => {
                if matches!(
                    error,
                    crate::records::external_index::ExternalSourceError::Storage
                ) || before != self.container.writer_lease_proof()
                {
                    self.session.write_poisoned = true;
                }
                Err(OwnerSessionError::Record(error.to_string()))
            }
        }
    }
}
