//! Exclusive native import epoch mutation and source publication.
use super::*;

impl RelWriter {
    /// Durably open or resume the one REL import epoch. The marker captures
    /// only clocks owned by this runtime; unavailable legacy clock domains are
    /// explicit `None` values in format 2.
    pub fn start_native_import(
        &mut self,
        import_key: &str,
        source_namespace: &str,
        source_format: &str,
        started_at_ns: i64,
    ) -> Result<ImportEpochStatus, OwnerSessionError> {
        if self.session.write_poisoned {
            return Err(OwnerSessionError::Record(
                "owner session requires reopen after an uncertain write".into(),
            ));
        }
        let before = self.container.writer_lease_proof();
        if before.is_none() || before != self.session.writer_lease_proof {
            return Err(OwnerSessionError::Record(
                "REL owner session is stale; reopen before native import".into(),
            ));
        }
        if started_at_ns < 0 {
            return Err(OwnerSessionError::Record(
                "native import start time must be nonnegative".into(),
            ));
        }
        let id = import_id(self.session.owner_id, import_key);
        if let Some(status) = self.session.imports.status(&id) {
            let start = match &status {
                ImportEpochStatus::Open(start) | ImportEpochStatus::Completed { start, .. } => {
                    start
                }
            };
            if start.format_version == 1 {
                return Err(OwnerSessionError::Record(
                    "legacy import epoch is retained for fencing and cannot be resumed by the current clock contract".into(),
                ));
            }
            if start.import_key != import_key
                || start.source_namespace != source_namespace
                || start.source_format != source_format
            {
                return Err(OwnerSessionError::Record(
                    "native import retry conflicts with its durable identity".into(),
                ));
            }
            return Ok(status);
        }
        if self.session.imports.open_epoch().is_some() {
            return Err(OwnerSessionError::Record(
                "another native import epoch is open".into(),
            ));
        }
        let start = ImportStart {
            format_version: 2,
            owner_uuid: self.session.owner_id,
            import_id: id,
            import_key: import_key.to_owned(),
            source_namespace: source_namespace.to_owned(),
            source_format: source_format.to_owned(),
            started_at_ns,
            pre: ImportWatermarks {
                global_version: self.container.latest_version(),
                archive_version: Some(self.session.archive.archive_version()),
                memory_version: self.session.common.memories.memory_version(),
                entity_version: None,
                relationship_version: None,
                graph_version: self.session.common.graph.graph_version(),
                memory_graph_version: None,
                vector_version: Some(self.session.generations.vector_version()),
            },
        };
        if let Err(error) = self
            .session
            .imports
            .append_start(&mut self.container, start.clone())
        {
            self.session.write_poisoned = true;
            return Err(OwnerSessionError::Record(error));
        }
        self.session.writer_lease_proof = self.container.writer_lease_proof();
        Ok(ImportEpochStatus::Open(start))
    }

    /// Admit one prepared import turn and advance its named native branch.
    /// The epoch ID is required on every scoped mutation; ordinary writers
    /// remain fenced until a completion coordinator settles the epoch.
    pub fn accept_native_import_turn(
        &mut self,
        import_id: &str,
        input: reliquary_archive::TurnInput,
        branch: reliquary_archive::Branch,
    ) -> Result<reliquary_archive::Node, OwnerSessionError> {
        self.ensure_import_scope(import_id)?;
        self.accept_scoped_turn_and_advance_branch(input, branch)
    }

    /// Finalize an imported source path and queue its one Archive-owned
    /// Episode at Import priority. Exact retries reuse Archive and Insomnia
    /// identities rather than creating a second queue.
    pub fn finalize_native_import_episode(
        &mut self,
        import_id: &str,
        conversation_id: &str,
        start_node_id: &str,
        end_node_id: &str,
        finalized_at_ns: i64,
    ) -> Result<reliquary_archive::EpisodeId, OwnerSessionError> {
        self.ensure_import_scope(import_id)?;
        let episode_id = match self.session.archive.put_episode(
            &mut self.container,
            conversation_id,
            start_node_id,
            end_node_id,
            reliquary_archive::EpisodeOrigin::Import,
            reliquary_archive::EpisodeBoundary::ImportEnd,
            finalized_at_ns,
        ) {
            Ok(id) => id,
            Err(error) => {
                if matches!(error, reliquary_archive::ArchiveError::Container(_)) {
                    self.session.write_poisoned = true;
                }
                return Err(error.into());
            }
        };
        self.session.writer_lease_proof = self.container.writer_lease_proof();
        let work_id = WorkEpisodeId(episode_id.0);
        let episode =
            self.session
                .archive
                .episode(episode_id)
                .ok_or(OwnerSessionError::Insomnia(
                    InsomniaError::UnacceptedEpisode,
                ))?;
        let order = AcceptedEpisodeOrder {
            episode_id: work_id,
            source_through_ns: episode.source_through_ns,
            conversation_id: episode.conversation_id.clone(),
            start_node_id: episode.start_node_id.clone(),
        };
        self.session.insomnia.admit_order(order.clone())?;
        if let Err(error) = self.session.insomnia.queue(
            &mut self.container,
            order,
            InsomniaPriority::Import,
            finalized_at_ns,
        ) {
            self.session.write_poisoned = true;
            return Err(error.into());
        }
        self.session.writer_lease_proof = self.container.writer_lease_proof();
        Ok(episode_id)
    }

    /// Persist the Archive-owned source mapping after all referenced native
    /// records have been accepted. Retrying the same mapping is a no-op.
    pub fn commit_native_import_receipt(
        &mut self,
        import_id: &str,
        receipt: crate::model::ImportReceipt,
    ) -> Result<crate::model::ImportReceipt, OwnerSessionError> {
        self.ensure_import_scope(import_id)?;
        let start = self
            .session
            .imports
            .open_epoch()
            .expect("scope check requires an open import epoch");
        let expected_key = decode_import_key(&start.import_key).ok_or_else(|| {
            OwnerSessionError::Record("native import epoch has an invalid import key".into())
        })?;
        if receipt.owner_uuid != self.session.owner_id
            || receipt.import_key != expected_key
            || receipt.source_digest != expected_key
            || receipt.source_namespace != start.source_namespace
            || receipt.source_format != start.source_format
        {
            return Err(OwnerSessionError::Record(
                "native import receipt does not match its open durable epoch".into(),
            ));
        }
        match self
            .session
            .archive
            .put_import_receipt(&mut self.container, receipt)
        {
            Ok(receipt) => {
                self.session.writer_lease_proof = self.container.writer_lease_proof();
                Ok(receipt)
            }
            Err(error) => {
                if matches!(error, reliquary_archive::ArchiveError::Container(_)) {
                    self.session.write_poisoned = true;
                }
                Err(error.into())
            }
        }
    }

    pub(crate) fn ensure_import_scope(&self, import_id: &str) -> Result<(), OwnerSessionError> {
        if self.session.write_poisoned {
            return Err(OwnerSessionError::Record(
                "owner session requires reopen after an uncertain write".into(),
            ));
        }
        let before = self.container.writer_lease_proof();
        if before.is_none() || before != self.session.writer_lease_proof {
            return Err(OwnerSessionError::Record(
                "REL owner session is stale; reopen before native import mutation".into(),
            ));
        }
        if self.session.imports.settlement(import_id).is_some()
            || self
                .session
                .has_native_import_historical_freshness(import_id)
        {
            return Err(OwnerSessionError::Record(
                "import is sealed by a durable Freshness settlement".into(),
            ));
        }
        if !self
            .session
            .imports
            .open_epoch()
            .is_some_and(|start| start.import_id == import_id && start.format_version == 2)
        {
            return Err(OwnerSessionError::Record(
                "native import mutation is outside its open durable epoch".into(),
            ));
        }
        Ok(())
    }
}
