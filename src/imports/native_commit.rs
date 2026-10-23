//! Native Import admission and ordered commit over accepted owner parts.
use super::*;
pub(crate) use crate::owner_lifecycle::native_import::NativeImportParts;
use crate::{Branch, EpisodeConfig, EpisodeOrigin};
pub(crate) fn import_native(
    mut parts: NativeImportParts<'_>,
    input: &NativeImport,
    now_ns: i64,
    watermarks: ImportWatermarks,
    activity: &mut Vec<u64>,
) -> Result<ImportReceipt, ImportError> {
    let prepared = prepare(input)?;
    let owner_uuid = parts.owner_uuid().ok_or(ImportError::Epoch(
        "native import requires a durable owner UUID",
    ))?;
    let import_id = super::epoch_store::import_id(owner_uuid, &prepared.receipt.import_key);
    if let Some(existing) = parts
        .archive
        .import_receipts
        .get(&prepared.receipt.import_key)
    {
        if existing != &prepared.receipt {
            return Err(ImportError::Invalid("import key collision"));
        }
        let receipt = existing.clone();
        parts.sync()?;
        return Ok(receipt);
    }
    // Reject immutable source conflicts before accepting a durable epoch.
    parts.archive.validate_import_revisions(&prepared.receipt)?;
    if let Some(open) = parts.epochs.open() {
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
            pre: watermarks,
        };
        parts.append_start(start)?;
        parts.sync()?;
    }
    let mut session = NativeImportCommit::new(parts, activity);
    session.commit_prepared_import(prepared, now_ns)
}

struct NativeImportCommit<'a, 'b> {
    parts: NativeImportParts<'a>,
    activity: &'b mut Vec<u64>,
}
impl<'a, 'b> NativeImportCommit<'a, 'b> {
    fn new(mut parts: NativeImportParts<'a>, activity: &'b mut Vec<u64>) -> Self {
        parts.enter_commit();
        Self { parts, activity }
    }
    fn commit_prepared_import(
        &mut self,
        prepared: super::prepare::PreparedImport,
        now_ns: i64,
    ) -> Result<ImportReceipt, ImportError> {
        for turn in prepared.turns {
            self.activity.push(self.parts.ingest_turn(turn)?);
        }
        for conversation in &prepared.receipt.conversations {
            let has_turns = conversation.messages.iter().any(|m| m.node_id.is_some());
            if has_turns && let Some(title) = &conversation.title {
                let mut metadata = self
                    .parts
                    .archive
                    .conversation_metadata(&conversation.conversation_id)
                    .cloned()
                    .unwrap_or_else(|| {
                        crate::ConversationMetadata::new(&conversation.conversation_id)
                    });
                metadata.title = Some(title.clone());
                self.parts.put_metadata(metadata)?;
            }
            for branch in &conversation.branches {
                if let Some(leaf) = &branch.leaf_node_id {
                    // A historical source head may already have a native continuation.
                    // Preserve that head and mutable native canonical metadata.
                    if self
                        .parts
                        .archive
                        .branches
                        .get(&conversation.conversation_id, &branch.branch_id)
                        .is_none()
                    {
                        self.parts.append_branch(Branch {
                            id: branch.branch_id.clone(),
                            conversation_id: conversation.conversation_id.clone(),
                            leaf_node_id: leaf.clone(),
                            canonical: branch.canonical,
                        })?;
                    }
                    self.parts.finalize_import_path_and_queue(
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
                .parts
                .archive
                .episodes_for_conversation(&conversation.conversation_id)
                .into_iter()
                .filter(|e| e.origin == EpisodeOrigin::Import)
            {
                self.parts.queue_import_episode(episode.id, now_ns)?;
            }
        }
        self.parts.put_receipt(prepared.receipt.clone())?;
        self.parts.sync()?;
        Ok(prepared.receipt)
    }
}

impl Drop for NativeImportCommit<'_, '_> {
    fn drop(&mut self) {
        self.parts.leave_commit();
    }
}
