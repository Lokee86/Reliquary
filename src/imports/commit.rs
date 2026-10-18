use super::*;
use crate::{Branch, Cva, CvaError, EpisodeConfig, EpisodeOrigin, InsomniaPriority};
use sha2::{Digest, Sha256};

impl Cva {
    /// Offline/exclusive native import. Mounted management must compose this through
    /// its exact REL mutation gate; this method never opens a second runtime.
    /// Retry the same normalized input after partial failure. A receipt acknowledges
    /// all source records, branch heads and Import-priority queue entries after sync.
    pub fn import_native(
        &mut self,
        input: &NativeImport,
        now_ns: i64,
    ) -> Result<ImportReceipt, ImportError> {
        let prepared = prepare(input)?;
        let owner_uuid = self.owner_uuid().ok_or(ImportError::Epoch(
            "native import requires a durable owner UUID",
        ))?;
        let import_id = super::epoch_store::import_id(owner_uuid, &prepared.receipt.import_key);
        if let Some(existing) = self
            .archive
            .import_receipts
            .get(&prepared.receipt.import_key)
        {
            if existing != &prepared.receipt {
                return Err(ImportError::Invalid("import key collision"));
            }
            let receipt = existing.clone();
            self.container.sync()?;
            return Ok(receipt);
        }
        // Reject immutable source conflicts before accepting a durable epoch.
        self.archive.validate_import_revisions(&prepared.receipt)?;
        if let Some(open) = self.import_epochs.open() {
            if open.import_id != import_id
                || open.import_key != prepared.receipt.import_key
                || open.source_namespace != prepared.receipt.source_namespace
                || open.source_format != prepared.receipt.source_format
            {
                return Err(ImportError::Epoch(
                    "conflicting import while an epoch is open",
                ));
            }
        } else {
            let start = ImportStart {
                owner_uuid,
                import_id: import_id.clone(),
                import_key: prepared.receipt.import_key.clone(),
                source_namespace: prepared.receipt.source_namespace.clone(),
                source_format: prepared.receipt.source_format.clone(),
                started_at_ns: now_ns,
                pre: self.import_watermarks(),
            };
            self.import_epochs
                .append_start(&mut self.container, start)?;
            self.container.sync()?;
        }
        self.with_import_write_scope(|cva| cva.commit_prepared_import(prepared, now_ns))
    }

    fn commit_prepared_import(
        &mut self,
        prepared: super::prepare::PreparedImport,
        now_ns: i64,
    ) -> Result<ImportReceipt, ImportError> {
        for turn in prepared.turns {
            self.ingest_turn(turn)?;
        }
        for conversation in &prepared.receipt.conversations {
            let has_turns = conversation.messages.iter().any(|m| m.node_id.is_some());
            if has_turns && let Some(title) = &conversation.title {
                self.set_conversation_title(&conversation.conversation_id, title.clone())?;
            }
            for branch in &conversation.branches {
                if let Some(leaf) = &branch.leaf_node_id {
                    // A historical source head may already have a native continuation.
                    // Preserve that head and mutable native canonical metadata.
                    if self
                        .archive
                        .branches
                        .get(&conversation.conversation_id, &branch.branch_id)
                        .is_none()
                    {
                        self.append_branch(Branch {
                            id: branch.branch_id.clone(),
                            conversation_id: conversation.conversation_id.clone(),
                            leaf_node_id: leaf.clone(),
                            canonical: branch.canonical,
                        })?;
                    }
                    self.finalize_import_path_and_queue(
                        &conversation.conversation_id,
                        leaf,
                        EpisodeConfig::default(),
                        now_ns,
                    )?;
                }
            }
            // Repair a crash between immutable Episode publication and queue admission.
            // Exact Episode identity preserves completed/leased work; no second queue.
            for episode in self
                .episodes_for_conversation(&conversation.conversation_id)
                .into_iter()
                .filter(|e| e.origin == EpisodeOrigin::Import)
            {
                self.queue_insomnia_episode(episode.id, InsomniaPriority::Import, now_ns)?;
            }
        }
        self.archive
            .put_import_receipt(&mut self.container, prepared.receipt.clone())?;
        self.container.sync()?;
        Ok(prepared.receipt)
    }

    pub(crate) fn with_import_write_scope<T>(
        &mut self,
        operation: impl FnOnce(&mut Self) -> T,
    ) -> T {
        self.container.set_owner_write_scope(true);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| operation(self)));
        self.container.set_owner_write_scope(false);
        match result {
            Ok(value) => value,
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }

    pub fn native_import_receipts(&self) -> Vec<ImportReceipt> {
        self.archive.import_receipts.values().cloned().collect()
    }

    pub fn open_import_epoch(&self) -> Option<ImportStart> {
        self.import_epochs.open().cloned()
    }

    pub fn import_epoch_status(&self, import_id: &str) -> Option<ImportEpochStatus> {
        self.import_epochs.status(import_id)
    }

    pub fn import_epoch_statuses(&self) -> Vec<ImportEpochStatus> {
        self.import_epochs.statuses()
    }

    /// Append an explicit compatibility frame for one accepted legacy CVAFRS03
    /// settlement. The frame binds the complete pre-frame physical prefix; it
    /// does not manufacture native ImportStart/ImportComplete authority.
    pub fn migrate_legacy_import_framing(&mut self) -> Result<bool, CvaError> {
        if !self.import_epochs.legacy_epochs().is_empty() {
            return Ok(false);
        }
        if self
            .freshness
            .import_settlements()
            .iter()
            .all(|settlement| settlement.epoch.is_some())
        {
            return Ok(false);
        }
        self.container.sync()?;
        if self.import_epochs.open().is_some() || !self.import_epochs.statuses().is_empty() {
            return Err(
                ImportError::Epoch("legacy framing requires no native import epochs").into(),
            );
        }
        let owner_uuid = self.owner_uuid().ok_or(ImportError::Epoch(
            "legacy framing requires a durable owner UUID",
        ))?;
        let settlements = self.freshness.import_settlements();
        if settlements.len() != 1
            || settlements[0].epoch.is_some()
            || settlements[0].owner != owner_uuid
        {
            return Err(ImportError::Epoch(
                "legacy framing requires exactly one unscoped owner settlement",
            )
            .into());
        }
        let mut legacy_payloads = Vec::new();
        let objects = self.container.chunks()?;
        for object in objects {
            let payload = self.container.read(object)?;
            if payload.starts_with(b"CVAFRS03") {
                legacy_payloads.push(payload);
            }
        }
        if legacy_payloads.len() != 1 {
            return Err(ImportError::Epoch(
                "legacy framing requires exactly one physical CVAFRS03 record",
            )
            .into());
        }
        let settlement_payload = &legacy_payloads[0];
        let settlement_digest = format!("{:x}", Sha256::digest(settlement_payload));
        let source_prefix_len = std::fs::metadata(self.container.path())
            .map_err(crate::ContainerError::Io)?
            .len();
        let source_prefix_digest =
            super::epoch_store::hash_file_prefix(self.container.path(), source_prefix_len)?;
        let frame = super::LegacyImportEpoch {
            origin: super::LegacyImportEpochOrigin::LegacyCvafrs03,
            status: super::LegacyImportEpochStatus::CompletedInitial,
            owner_uuid,
            import_id: settlements[0].import_id,
            source_settlement_digest: settlement_digest,
            source_prefix_len,
            source_prefix_digest,
        };
        let bytes = super::legacy_epoch::encode(&frame)?;
        let position = self.container.append(&bytes)?;
        self.container.sync()?;
        self.import_epochs.ingest_at(Some(position), &bytes)?;
        Ok(true)
    }

    pub fn legacy_import_epochs(&self) -> Vec<LegacyImportEpoch> {
        self.import_epochs.legacy_epochs()
    }

    pub(super) fn import_watermarks(&self) -> ImportWatermarks {
        ImportWatermarks {
            global_version: self.latest_global_version(),
            archive_version: self.archive_version(),
            memory_version: self.memory_version(),
            entity_version: self.entities.entity_version(),
            relationship_version: self.relationships.relationship_version(),
            graph_version: self.graph.graph_version(),
            memory_graph_version: self.graph.memory_graph_version(),
            vector_version: self.vector_generations.vector_version(),
        }
    }
}
