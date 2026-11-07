use super::*;
use reliquary_archive::{Branch, ContentId, Node, TurnInput};
use sha2::{Digest, Sha256};

impl RelWriter {
    fn require_archive_import_version(&self, version: u64) -> Result<(), OwnerSessionError> {
        self.ensure_write_ready()?;
        if self.session.writer_lease_proof.is_none()
            || self.session.writer_lease_proof != self.container.writer_lease_proof()
            || self.session.archive.archive_version().checked_add(1) != Some(version)
        {
            return Err(OwnerSessionError::Record(
                "accepted Archive revision does not follow the owner head".into(),
            ));
        }
        Ok(())
    }

    /// Append one already accepted, attachment-free Archive turn at its exact local revision.
    ///
    /// A Branch revision is a separate accepted record and must be imported in its own position.
    pub fn import_accepted_archive_turn(
        &mut self,
        version: u64,
        node: Node,
        content: String,
    ) -> Result<(), OwnerSessionError> {
        self.require_archive_import_version(version)?;
        if node.content_id != ContentId(Sha256::digest(content.as_bytes()).into())
            || self
                .session
                .archive
                .node(&node.conversation_id, &node.id)
                .is_some()
        {
            return Err(OwnerSessionError::Record(
                "accepted Archive turn has a conflicting identity".into(),
            ));
        }
        let input = TurnInput {
            id: node.id,
            conversation_id: node.conversation_id,
            parent_id: node.parent_id,
            role: node.role,
            principal_id: node.principal_id,
            timestamp_ns: node.timestamp_ns,
            content,
            attachments: Vec::new(),
            project_attachments: Vec::new(),
        };
        let result = self.session.archive.accept_turn(&mut self.container, input);
        self.finish_archive_import(result.map(|turn| turn.inserted))
    }

    /// Append one already accepted Branch revision at its exact local revision.
    pub fn import_accepted_archive_branch(
        &mut self,
        version: u64,
        branch: Branch,
    ) -> Result<(), OwnerSessionError> {
        self.require_archive_import_version(version)?;
        let result = self.session.archive.put_branch(&mut self.container, branch);
        self.finish_archive_import(result)
    }

    fn finish_archive_import(
        &mut self,
        result: Result<bool, reliquary_archive::ArchiveError>,
    ) -> Result<(), OwnerSessionError> {
        match result {
            Ok(true) => {
                self.session.writer_lease_proof = self.container.writer_lease_proof();
                Ok(())
            }
            Ok(false) => Err(OwnerSessionError::Record(
                "accepted Archive revision is a duplicate".into(),
            )),
            Err(error) => {
                if matches!(error, reliquary_archive::ArchiveError::Container(_)) {
                    self.session.write_poisoned = true;
                }
                Err(error.into())
            }
        }
    }
}
