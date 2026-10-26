use super::*;
use crate::{Branch, Cva, EpisodeConfig, EpisodeOrigin, InsomniaPriority};

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
            .import_receipt(&prepared.receipt.import_key)
            .cloned()
        {
            if existing != prepared.receipt {
                return Err(ImportError::Invalid("import key collision"));
            }
            let receipt = existing;
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
            self.refresh_import_write_admission(true)?;
            self.container.sync()?;
        }
        self.with_import_write_scope(|cva| cva.commit_prepared_import(prepared, now_ns))
            .map_err(ImportError::from)?
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
                    if !self
                        .archive
                        .has_branch(&conversation.conversation_id, &branch.branch_id)
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
    ) -> Result<T, reliquary_container::ContainerError> {
        let _scope = self
            .import_write_admission
            .as_ref()
            .ok_or(reliquary_container::ContainerError::WriteAdmissionClosed)?
            .enter(&self.container)?;
        Ok(operation(self))
    }

    pub(crate) fn refresh_import_write_admission(
        &mut self,
        open: bool,
    ) -> Result<(), reliquary_container::ContainerError> {
        if open && self.import_write_admission.is_none() {
            self.import_write_admission = Some(self.container.close_write_admission()?);
        } else if !open && let Some(admission) = self.import_write_admission.as_ref() {
            admission.reopen(&mut self.container)?;
            self.import_write_admission = None;
        }
        Ok(())
    }

    pub fn native_import_receipts(&self) -> Vec<ImportReceipt> {
        self.archive.all_import_receipts()
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
