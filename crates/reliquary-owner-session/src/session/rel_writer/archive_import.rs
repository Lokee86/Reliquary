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
        self.import_archive_turn(version, None, node, content)
    }

    fn import_archive_turn(
        &mut self,
        version: u64,
        coordinate: Option<reliquary_archive::ArchivePublicationCoordinate>,
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
        let result = self
            .session
            .archive
            .accept_turn_at(&mut self.container, input, coordinate);
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

    /// Reserve the next native global version for an already accepted Archive publication.
    pub fn reserve_archive_publication_version(
        &mut self,
        global_version: u64,
        transaction_time_ns: Option<i64>,
    ) -> Result<(), OwnerSessionError> {
        self.require_archive_import_version(
            self.session
                .archive_version()
                .checked_add(1)
                .ok_or_else(|| OwnerSessionError::Record("Archive version exhausted".into()))?,
        )?;
        if self.next_global_version()? != global_version {
            return Err(OwnerSessionError::Record(
                "Archive reservation does not follow the global clock".into(),
            ));
        }
        let result = self
            .container
            .reserve_version(transaction_time_ns)
            .and_then(|_| self.container.sync())
            .map(|_| true)
            .map_err(reliquary_archive::ArchiveError::from);
        self.finish_archive_import(result)
    }

    /// Append an exact current-format Archive publication for an available turn.
    pub fn import_versioned_archive_turn(
        &mut self,
        coordinate: reliquary_archive::ArchivePublicationCoordinate,
        node: Node,
        content: String,
    ) -> Result<(), OwnerSessionError> {
        self.import_archive_turn(coordinate.archive_version, Some(coordinate), node, content)
    }

    /// Append a Branch revision with its native Archive/global publication coordinate.
    pub fn import_versioned_archive_branch(
        &mut self,
        coordinate: reliquary_archive::ArchivePublicationCoordinate,
        branch: Branch,
    ) -> Result<(), OwnerSessionError> {
        self.require_archive_import_version(coordinate.archive_version)?;
        let result =
            self.session
                .archive
                .put_branch_at(&mut self.container, branch, Some(coordinate));
        self.finish_archive_import(result)
    }

    /// Retain a repeated publication without rerunning its origin's mutation.
    pub fn import_archive_repeat(
        &mut self,
        coordinate: reliquary_archive::ArchivePublicationCoordinate,
        first_applying_archive_version: u64,
    ) -> Result<(), OwnerSessionError> {
        self.require_archive_import_version(coordinate.archive_version)?;
        let result = self.session.archive.append_publication_repeat(
            &mut self.container,
            coordinate,
            first_applying_archive_version,
        );
        self.finish_archive_import(result.map(|_| true))
    }

    /// Import the accepted Episode and register its Archive-derived work order.
    pub fn import_versioned_archive_episode(
        &mut self,
        coordinate: reliquary_archive::ArchivePublicationCoordinate,
        episode: reliquary_archive::Episode,
    ) -> Result<(), OwnerSessionError> {
        self.require_archive_import_version(coordinate.archive_version)?;
        let id = episode.id;
        let result = self.session.archive.append_accepted_episode_at(
            &mut self.container,
            coordinate,
            episode,
        );
        self.finish_archive_import(result.map(|_| true))?;
        self.admit_finalized_archive_episode(id)
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
