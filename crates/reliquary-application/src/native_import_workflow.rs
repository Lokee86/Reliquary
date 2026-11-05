//! Resumable normalized import composition. Provider decoding is intentionally
//! outside this workflow; source-to-native mappings remain Archive-owned.

use reliquary_freshness::historical_planner::{HistoricalSource, HistoricalTimeline, SourceKey};
use reliquary_owner_session::{
    OwnerSessionError, RelWriter,
    model::{Branch, EpisodeId, ImportReceipt, TurnInput},
    records::{ImportEpochStatus, ImportFreshnessSettlement, import_id},
};
use sha2::{Digest, Sha256};

#[derive(Clone, Debug)]
pub struct NativeImportEpisode {
    pub conversation_id: String,
    pub start_node_id: String,
    pub end_node_id: String,
    pub finalized_at_ns: i64,
}

#[derive(Clone, Debug)]
pub struct NativeImportBatch {
    pub source: ImportReceipt,
    pub turns: Vec<(TurnInput, Branch)>,
    pub episodes: Vec<NativeImportEpisode>,
    pub started_at_ns: i64,
}

#[derive(Clone, Debug)]
pub struct StagedNativeImport {
    pub import_id: String,
    pub receipt: ImportReceipt,
    pub episodes: Vec<EpisodeId>,
    pub already_completed: bool,
}

/// Stage canonical source turns, episodes and their checked mapping receipt.
/// Every mutation is epoch-scoped and idempotent; an interruption may be
/// recovered by reopening the owner and supplying the identical source.
pub fn stage_native_import(
    writer: &mut RelWriter,
    batch: NativeImportBatch,
) -> Result<StagedNativeImport, OwnerSessionError> {
    let owner = writer.owner_id();
    let key = batch
        .source
        .import_key
        .iter()
        .map(|value| format!("{value:02x}"))
        .collect::<String>();
    if batch.source.owner_uuid != owner || batch.source.source_digest != batch.source.import_key {
        return Err(OwnerSessionError::Record(
            "native import source identity differs from its REL owner".into(),
        ));
    }
    let id = import_id(owner, &key);
    let status = writer.start_native_import(
        &key,
        &batch.source.source_namespace,
        &batch.source.source_format,
        batch.started_at_ns,
    )?;
    if matches!(status, ImportEpochStatus::Completed { .. }) {
        return confirm_staged_retry(writer, &batch, id, true);
    }
    if writer
        .read_session()
        .native_import_settlement(&id)
        .is_some()
    {
        // A crash between the durable settlement and completion must not
        // re-admit source turns or re-queue already completed work.
        return confirm_staged_retry(writer, &batch, id, false);
    }
    for (turn, branch) in batch.turns {
        writer.accept_native_import_turn(&id, turn, branch)?;
    }
    let mut episodes = Vec::with_capacity(batch.episodes.len());
    for episode in batch.episodes {
        episodes.push(writer.finalize_native_import_episode(
            &id,
            &episode.conversation_id,
            &episode.start_node_id,
            &episode.end_node_id,
            episode.finalized_at_ns,
        )?);
    }
    let receipt = writer.commit_native_import_receipt(&id, batch.source)?;
    Ok(StagedNativeImport {
        import_id: id,
        receipt,
        episodes,
        already_completed: false,
    })
}

fn confirm_staged_retry(
    writer: &RelWriter,
    batch: &NativeImportBatch,
    import_id: String,
    already_completed: bool,
) -> Result<StagedNativeImport, OwnerSessionError> {
    let session = writer.read_session();
    let receipt = session
        .native_import_receipt(&batch.source.import_key)
        .ok_or_else(|| {
            OwnerSessionError::Record("import retry lacks its durable source receipt".into())
        })?
        .clone();
    if receipt.conversations != batch.source.conversations
        || receipt.source_namespace != batch.source.source_namespace
        || receipt.source_format != batch.source.source_format
    {
        return Err(OwnerSessionError::Record(
            "import retry source mapping changed".into(),
        ));
    }
    for (input, branch) in &batch.turns {
        let stored = session
            .resolved_archive_turn(&input.conversation_id, &input.id)?
            .ok_or_else(|| {
                OwnerSessionError::Record("import retry source turn is missing".into())
            })?;
        let node = session
            .archive_node(&input.conversation_id, &input.id)
            .ok_or_else(|| {
                OwnerSessionError::Record("import retry Archive node is missing".into())
            })?;
        if stored.content != input.content
            || stored.role != input.role
            || stored.principal_id != input.principal_id
            || stored.timestamp_ns != input.timestamp_ns
            || node.parent_id != input.parent_id
            || branch.conversation_id != input.conversation_id
            || branch.leaf_node_id != input.id
        {
            return Err(OwnerSessionError::Record(
                "import retry turn evidence differs from the accepted Archive".into(),
            ));
        }
    }
    let mut episodes = Vec::with_capacity(batch.episodes.len());
    for spec in &batch.episodes {
        let mut hash = Sha256::new();
        for part in [
            &spec.conversation_id,
            &spec.start_node_id,
            &spec.end_node_id,
        ] {
            hash.update((part.len() as u64).to_le_bytes());
            hash.update(part.as_bytes());
        }
        let id = EpisodeId(hash.finalize().into());
        let episode = session
            .episode(id)
            .ok_or_else(|| OwnerSessionError::Record("import retry Episode is missing".into()))?;
        if episode.conversation_id != spec.conversation_id
            || episode.start_node_id != spec.start_node_id
            || episode.end_node_id != spec.end_node_id
            || episode.finalized_at_ns != spec.finalized_at_ns
            || episode.origin != reliquary_owner_session::model::EpisodeOrigin::Import
        {
            return Err(OwnerSessionError::Record(
                "import retry Episode differs from accepted source".into(),
            ));
        }
        episodes.push(id);
    }
    Ok(StagedNativeImport {
        import_id,
        receipt,
        episodes,
        already_completed,
    })
}

/// Validate the accepted Archive clock before freezing the import settlement.
/// Use accepted positions so an import cannot move previously credited turns.
pub fn settle_native_import(
    writer: &mut RelWriter,
    import_id: &str,
) -> Result<ImportFreshnessSettlement, OwnerSessionError> {
    if writer
        .read_session()
        .native_import_settlement(import_id)
        .is_none()
    {
        import_timeline(writer)?;
    }
    writer.settle_native_import(import_id)
}

fn import_timeline(writer: &RelWriter) -> Result<HistoricalTimeline, OwnerSessionError> {
    let owner = writer.read_session();
    let accepted = owner.accepted_turn_sources();
    let mut sources = Vec::with_capacity(accepted.len());
    let mut positions = std::collections::BTreeMap::new();
    for source in accepted {
        let node = owner
            .archive_node(&source.conversation_id, &source.node_id)
            .ok_or_else(|| OwnerSessionError::Record("accepted Archive node is missing".into()))?;
        let key = SourceKey {
            conversation_id: source.conversation_id,
            node_id: source.node_id,
        };
        if let Some(position) = source.activity_position {
            positions.insert(key.clone(), position);
        }
        sources.push(HistoricalSource {
            key,
            parent_id: node.parent_id.clone(),
            role: node.role.clone(),
            timestamp_ns: node.timestamp_ns,
        });
    }
    HistoricalTimeline::from_accepted_archive(sources, &positions, owner.rel_turn_count())
        .map_err(|error| OwnerSessionError::Record(error.to_string()))
}

#[cfg(test)]
#[path = "native_import_workflow/tests.rs"]
mod tests;
