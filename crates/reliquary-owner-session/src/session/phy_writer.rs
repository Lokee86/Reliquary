use super::*;

impl PhyWriter {
    /// Create and initialize a new private PHY owner and its canonical writer.
    pub fn create_path(
        path: impl AsRef<Path>,
        owner_id: [u8; 16],
    ) -> Result<Self, OwnerSessionError> {
        let mut container = Container::create(path, ContainerIdentity::phylactery(), owner_id)?;
        super::create::initialize_owner(&mut container)?;
        let admission = GraphAdmission::for_replay(owner_id, [], [], [], [], []);
        let common = open_common(&container, &admission, [], false)?;
        let writer_lease_proof = container.writer_lease_proof();
        Ok(Self {
            owner_id,
            container,
            common,
            writer_lease_proof,
            poisoned: false,
        })
    }

    pub fn open_path(
        path: impl AsRef<Path>,
        graph_admission: &GraphAdmission,
        dream_admissions: impl IntoIterator<Item = DreamBeginAdmission>,
    ) -> Result<Self, OwnerSessionError> {
        let container = Container::open_writer_as(path, FileKind::Phylactery)?;
        require_kind(&container, FileKind::Phylactery)?;
        reject_rel_only_records(&container)?;
        let common = open_common(&container, graph_admission, dream_admissions, false)?;
        let writer_lease_proof = container.writer_lease_proof();
        Ok(Self {
            owner_id: container.owner_id(),
            container,
            common,
            writer_lease_proof,
            poisoned: false,
        })
    }

    pub fn owner_id(&self) -> [u8; 16] {
        self.owner_id
    }

    /// Record an evaluated pair after the application has accepted its result.
    pub fn record_dream_pair(
        &mut self,
        left: MemoryId,
        right: MemoryId,
    ) -> Result<bool, OwnerSessionError> {
        if self.poisoned {
            return Err(OwnerSessionError::Record(
                "PHY writer requires replay after a failed write".into(),
            ));
        }
        let before = self.container.writer_lease_proof();
        if before.is_none() || before != self.writer_lease_proof {
            return Err(OwnerSessionError::Record(
                "PHY owner session is stale; reopen before Dream writes".into(),
            ));
        }
        let result =
            self.common
                .dream_pairs
                .append(&mut self.container, &self.common.memories, left, right);
        match result {
            Ok(value) => {
                self.writer_lease_proof = self.container.writer_lease_proof();
                Ok(value)
            }
            Err(error) => {
                if matches!(error, OwnerSessionError::Container(_))
                    || before != self.container.writer_lease_proof()
                {
                    self.poisoned = true;
                }
                Err(error)
            }
        }
    }

    /// Publish a privacy-preserving user Memory after the Gateway verifies
    /// the exact source REL, principal and destination PHY grants. The public
    /// Memory record contains only its owner-local source reference.
    pub fn publish_routed_user_memory(
        &mut self,
        proof: RoutedUserMemoryProof,
    ) -> Result<MemoryRevision, OwnerSessionError> {
        if self.poisoned {
            return Err(OwnerSessionError::Record(
                "PHY writer requires replay after a failed write".into(),
            ));
        }
        let expected_proof = self.writer_lease_proof.ok_or_else(|| {
            OwnerSessionError::Record("PHY writer lacks its owner lease proof".into())
        })?;
        if self.container.writer_lease_proof() != Some(expected_proof) {
            return Err(OwnerSessionError::Record(
                "PHY owner session is stale; reopen before routed publication".into(),
            ));
        }
        if proof.destination_owner_id != self.owner_id {
            return Err(OwnerSessionError::Record(
                "routed Memory proof names another PHY owner".into(),
            ));
        }
        if proof.source_owner_id == self.owner_id {
            return Err(OwnerSessionError::Record(
                "routed Memory source and destination owners must differ".into(),
            ));
        }
        let draft = proof.draft;
        let published = match self
            .common
            .memories
            .publish(&mut self.container, None, 0, draft)
        {
            Ok(published) => published,
            Err(error) => {
                if matches!(error, MemoryError::Container(_) | MemoryError::Io(_))
                    || self.container.writer_lease_proof() != Some(expected_proof)
                {
                    self.poisoned = true;
                }
                return Err(error.into());
            }
        };
        if let Err(error) = self.container.sync() {
            self.poisoned = true;
            return Err(error.into());
        }
        self.writer_lease_proof = self.container.writer_lease_proof();
        if let Err(error) = self
            .common
            .refresh_dream_index(&[published.revision.record.id])
        {
            self.poisoned = true;
            return Err(error);
        }
        Ok(published.revision)
    }

    pub fn memory_history(&self, id: MemoryId) -> Result<Option<MemoryHistory>, OwnerSessionError> {
        match self.common.memories.history(id) {
            Ok(history) => Ok(Some(history)),
            Err(MemoryError::MissingMemory) => Ok(None),
            Err(error) => Err(error.into()),
        }
    }
}
