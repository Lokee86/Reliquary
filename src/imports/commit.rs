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
        // Archive owns revision consistency for import, reopen and reconciliation.
        self.archive.validate_import_revisions(&prepared.receipt)?;
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

    pub fn native_import_receipts(&self) -> Vec<ImportReceipt> {
        self.archive.import_receipts.values().cloned().collect()
    }
}
