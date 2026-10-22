//! Read-only proof of one accepted semantic Import scope.

use crate::freshness_storage::{HistoricalImportFreshnessSettlement, ImportSemanticScope};
use crate::imports::epoch_model::{ImportEpochStatus, ImportStart};
use crate::imports::epoch_store::ImportEpochStore;
use crate::owner_lifecycle::{HistorySource, OwnerError};
use crate::{Archive, Episode, EpisodeOrigin, MemoryId};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

/// A concrete outcome from the pre-scan proof. Initial history does not require
/// physical access; framed scopes carry the accepted start and source facts to
/// the bounded physical scan.
pub(crate) enum PreparedSemanticImportScope {
    InitialHistory,
    Framed(FramedSemanticImportScope),
}

pub(crate) struct FramedSemanticImportScope {
    start: ImportStart,
    source_nodes: BTreeSet<(String, String)>,
    born_memories: BTreeSet<MemoryId>,
    settlement_global_version: u64,
}

pub(crate) fn prepare_semantic_import_scope(
    owner_uuid: Option<[u8; 16]>,
    archive: &Archive,
    epochs: &ImportEpochStore,
    settlement: &HistoricalImportFreshnessSettlement,
    scope: &ImportSemanticScope,
) -> Result<PreparedSemanticImportScope, OwnerError> {
    let fail = |message: &str| OwnerError::Freshness(message.into());
    if owner_uuid != Some(scope.owner) {
        return Err(fail("semantic import owner mismatch"));
    }
    let id: String = scope.import_id.iter().map(|b| format!("{b:02x}")).collect();
    let start = match epochs.status(&id) {
        Some(ImportEpochStatus::Open(start)) | Some(ImportEpochStatus::Completed { start, .. }) => {
            start
        }
        None => {
            // Explicit initial-history settlement, including a normalized
            // legacy predecessor. It cannot impersonate a later epoch.
            if scope.base_cut != 0 || scope.born_memories != settlement.cohort {
                return Err(fail(
                    "unframed semantic settlement is not an initial history",
                ));
            }
            return Ok(PreparedSemanticImportScope::InitialHistory);
        }
    };
    let start_digest: [u8; 32] =
        Sha256::digest(crate::imports::epoch_codec::encode_start(&start)?).into();
    let base_cut = archive
        .activity_cut_at_archive_version(start.pre.archive_version)
        .map_err(|error| fail(&error.to_string()))?;
    let receipt = archive
        .import_receipts
        .get(&start.import_key)
        .ok_or_else(|| fail("semantic settlement import receipt missing"))?;
    let source_nodes: BTreeSet<_> = receipt
        .conversations
        .iter()
        .flat_map(|conversation| {
            conversation.messages.iter().filter_map(|message| {
                message
                    .node_id
                    .as_ref()
                    .map(|node| (conversation.conversation_id.clone(), node.clone()))
            })
        })
        .collect();
    if scope.start_evidence_digest != start_digest
        || scope.base_cut != base_cut
        || scope.source_nodes != source_nodes
    {
        return Err(fail("semantic import start/source evidence mismatch"));
    }
    Ok(PreparedSemanticImportScope::Framed(
        FramedSemanticImportScope {
            start,
            source_nodes,
            born_memories: scope.born_memories.clone(),
            settlement_global_version: settlement.global_version,
        },
    ))
}

pub(crate) fn validate_semantic_import_births(
    source: &HistorySource<'_>,
    archive: &Archive,
    memories: &crate::memory_store::MemoryStore,
    facts: FramedSemanticImportScope,
) -> Result<(), OwnerError> {
    let mut births = BTreeSet::new();
    let mut cursor = source
        .cursor()
        .map_err(|error| OwnerError::Container(error.0))?;
    while let Some(frame) = cursor
        .next_frame()
        .map_err(|error| OwnerError::Container(error.0))?
    {
        let payload = frame.payload.as_slice();
        if let Some(completion) = crate::insomnia::completion::decode_completion(payload)
            .map_err(|error| OwnerError::Freshness(error.into()))?
        {
            let Some(episode) = archive.episodes.get(completion.episode_id) else {
                continue;
            };
            if episode.origin != EpisodeOrigin::Import
                || !episode_matches_source(archive, episode, &facts.source_nodes)?
            {
                continue;
            }
            for id in completion.freshness_birth_memory_ids {
                let record = completion
                    .records
                    .iter()
                    .find(|record| record.id == id)
                    .ok_or_else(|| {
                        OwnerError::Freshness(
                            "semantic import birth lacks publication record".into(),
                        )
                    })?;
                if record.global_version > facts.start.pre.global_version
                    && record.global_version <= facts.settlement_global_version
                {
                    if record.revision != 1
                        || record.source_episode_id != Some(episode.id)
                        || memories.first_mutation_id(id) != Some(record.mutation_id.as_str())
                    {
                        return Err(OwnerError::Freshness(
                            "semantic import birth publication conflicts".into(),
                        ));
                    }
                    births.insert(id);
                }
            }
        }
    }
    if births != facts.born_memories {
        return Err(OwnerError::Freshness(
            "semantic import birth cohort mismatch".into(),
        ));
    }
    Ok(())
}

pub(crate) fn episode_matches_source(
    archive: &Archive,
    episode: &Episode,
    source_nodes: &BTreeSet<(String, String)>,
) -> Result<bool, OwnerError> {
    if !source_nodes.contains(&(
        episode.conversation_id.clone(),
        episode.start_node_id.clone(),
    )) || !source_nodes.contains(&(episode.conversation_id.clone(), episode.end_node_id.clone()))
    {
        return Ok(false);
    }
    // Reopen authenticates retained evidence even after the conversation becomes
    // terminal. Current-only Episode hydration would discard that authority.
    let mut cursor = Some(episode.end_node_id.clone());
    let mut seen = BTreeSet::new();
    while let Some(id) = cursor {
        if !seen.insert(id.clone()) {
            return Err(OwnerError::Freshness("cyclic import Episode path".into()));
        }
        if !source_nodes.contains(&(episode.conversation_id.clone(), id.clone())) {
            return Ok(false);
        }
        let node = archive
            .nodes
            .get(&episode.conversation_id, &id)
            .ok_or_else(|| OwnerError::Freshness("missing retained import Episode Node".into()))?;
        if id == episode.start_node_id {
            return Ok(true);
        }
        cursor = node.parent_id.clone();
    }
    Ok(false)
}
