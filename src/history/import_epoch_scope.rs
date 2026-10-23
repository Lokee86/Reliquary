//! Read-only reconstruction and validation of one ImportEpoch source scope.

use crate::freshness_storage::ImportEventScope;
use crate::imports::epoch_codec::{self, ImportEpochRecord};
use crate::imports::epoch_model::ImportStart;
use crate::memory_store::MemoryStore;
use crate::owner_lifecycle::{HistorySource, OwnerError};
use crate::{Archive, EpisodeOrigin, MemoryId, MemoryRef, memory_model::MemoryRecord};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ImportedInsomniaOutput {
    pub episode_id: [u8; 32],
    pub memory_ids: BTreeSet<MemoryId>,
    pub external_memory_refs: BTreeSet<MemoryRef>,
    pub records: BTreeMap<MemoryId, MemoryRecord>,
}

pub(crate) struct ImportEpochScope {
    pub start: ImportStart,
    pub start_offset: u64,
    pub marker_end: u64,
    pub start_digest: [u8; 32],
    pub base_cut: u64,
    pub source_nodes: BTreeSet<(String, String)>,
    pub episodes: BTreeSet<[u8; 32]>,
    pub cohort: BTreeSet<MemoryId>,
    pub outputs: BTreeMap<[u8; 32], ImportedInsomniaOutput>,
}

/// Derive receipt source nodes before any physical replay, preserving the
/// historical missing-receipt refusal order at callers that also scan bytes.
pub(crate) fn import_source_nodes(
    archive: &Archive,
    start: &ImportStart,
) -> Result<BTreeSet<(String, String)>, OwnerError> {
    let receipt = archive
        .import_receipts
        .get(&start.import_key)
        .ok_or_else(|| {
            OwnerError::Freshness("import Archive construction/receipt is incomplete".into())
        })?;
    Ok(receipt
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
        .collect())
}

/// Reconstruct the accepted owner facts through one immutable physical prefix.
/// Callers derive receipt nodes first so missing-receipt errors keep their old
/// precedence over physical source errors.
pub(crate) fn replay_import_epoch_scope(
    source: &HistorySource<'_>,
    archive: &Archive,
    memories: &MemoryStore,
    start: ImportStart,
    through: u64,
    source_nodes: BTreeSet<(String, String)>,
) -> Result<ImportEpochScope, OwnerError> {
    let mut start_marker = None;
    let mut cursor = source
        .prefix_cursor(through)
        .map_err(|error| OwnerError::Container(error.0))?;
    while let Some(frame) = cursor
        .next_frame()
        .map_err(|error| OwnerError::Container(error.0))?
    {
        if let Some(ImportEpochRecord::Start(candidate)) = epoch_codec::decode(&frame.payload)
            .map_err(|error| OwnerError::Freshness(error.to_string()))?
            && candidate == start
        {
            let location = frame.object.legacy_bytes();
            let offset = u64::from_le_bytes(location[..8].try_into().unwrap());
            let len = u64::from_le_bytes(location[8..].try_into().unwrap());
            let end = offset + 8 + len;
            if start_marker
                .replace((offset, end, Sha256::digest(&frame.payload).into()))
                .is_some()
            {
                return Err(OwnerError::Freshness(
                    "duplicate import start marker".into(),
                ));
            }
        }
    }
    let (start_offset, marker_end, start_digest) = start_marker
        .ok_or_else(|| OwnerError::Freshness("open import start marker is unavailable".into()))?;

    let mut episodes = BTreeSet::new();
    let mut cursor = source
        .prefix_cursor(through)
        .map_err(|error| OwnerError::Container(error.0))?;
    while let Some(frame) = cursor
        .next_frame()
        .map_err(|error| OwnerError::Container(error.0))?
    {
        let location = frame.object.legacy_bytes();
        let offset = u64::from_le_bytes(location[..8].try_into().unwrap());
        if offset <= start_offset {
            continue;
        }
        if let Some(version) =
            crate::archive_codec::publication::version_at(frame.object, &frame.payload)?
        {
            let (record, applying) = source.archive_publication_with_version(archive, version)?;
            if applying.archive_version != version.archive_version {
                continue;
            }
            if let crate::archive_codec::ArchiveRecord::Episode(episode) = record
                && episode.origin == EpisodeOrigin::Import
                && super::episode_matches_source(archive, &episode, &source_nodes)?
            {
                episodes.insert(episode.id.0);
            }
        }
    }

    let mut cohort = BTreeSet::new();
    let mut outputs = BTreeMap::new();
    let mut cursor = source
        .prefix_cursor(through)
        .map_err(|error| OwnerError::Container(error.0))?;
    while let Some(frame) = cursor
        .next_frame()
        .map_err(|error| OwnerError::Container(error.0))?
    {
        let location = frame.object.legacy_bytes();
        let offset = u64::from_le_bytes(location[..8].try_into().unwrap());
        if offset <= start_offset {
            continue;
        }
        if let Some(completion) = crate::insomnia::completion::decode_completion(&frame.payload)
            .map_err(|error| OwnerError::Freshness(error.into()))?
            && episodes.contains(&completion.episode_id.0)
        {
            let episode_key = completion.episode_id.0;
            let mut memory_ids = BTreeSet::new();
            for id in &completion.memory_ids {
                if !memory_ids.insert(*id) {
                    return Err(OwnerError::Freshness(
                        "duplicate Memory in imported Insomnia completion".into(),
                    ));
                }
            }
            let external_memory_refs = completion
                .external_memory_refs
                .iter()
                .cloned()
                .collect::<BTreeSet<_>>();
            let mut records = BTreeMap::new();
            for record in completion.records.iter().cloned() {
                if !memory_ids.contains(&record.id) || records.insert(record.id, record).is_some() {
                    return Err(OwnerError::Freshness(
                        "import completion Memory output evidence conflicts".into(),
                    ));
                }
            }
            for id in &completion.freshness_birth_memory_ids {
                let record = records.get(id).ok_or_else(|| {
                    OwnerError::Freshness("import birth has no first-publication record".into())
                })?;
                if record.revision != 1
                    || record.source_episode_id != Some(completion.episode_id)
                    || memories.first_mutation_id(*id) != Some(record.mutation_id.as_str())
                    || record.global_version <= start.pre.global_version
                {
                    return Err(OwnerError::Freshness(
                        "import birth attribution conflicts".into(),
                    ));
                }
                cohort.insert(*id);
            }
            let evidence = ImportedInsomniaOutput {
                episode_id: episode_key,
                memory_ids,
                external_memory_refs,
                records,
            };
            if let Some(previous) = outputs.get(&episode_key) {
                if previous != &evidence {
                    return Err(OwnerError::Freshness(
                        "conflicting imported Insomnia completion receipts".into(),
                    ));
                }
            } else {
                outputs.insert(episode_key, evidence);
            }
        }
    }
    let base_cut = archive
        .activity_cut_at_archive_version(start.pre.archive_version)
        .map_err(|error| OwnerError::Freshness(error.to_string()))?;
    Ok(ImportEpochScope {
        start,
        start_offset,
        marker_end,
        start_digest,
        base_cut,
        source_nodes,
        episodes,
        cohort,
        outputs,
    })
}

/// Verify a frozen Freshness event scope against its physical Import epoch and
/// owner-derived Archive/Insomnia facts.
pub(crate) fn validate_import_settlement_scope(
    source: &HistorySource<'_>,
    archive: &Archive,
    memories: &MemoryStore,
    replay: &ImportEventScope,
) -> Result<(), OwnerError> {
    let mut start = None;
    let mut cursor = source
        .prefix_cursor(replay.freeze_prefix)
        .map_err(|error| OwnerError::Container(error.0))?;
    while let Some(frame) = cursor
        .next_frame()
        .map_err(|error| OwnerError::Container(error.0))?
    {
        let location = frame.object.legacy_bytes();
        let offset = u64::from_le_bytes(location[..8].try_into().unwrap());
        if offset == replay.start_prefix {
            if Sha256::digest(&frame.payload).as_slice() != replay.start_evidence_digest {
                return Err(OwnerError::Freshness(
                    "import settlement start digest mismatch".into(),
                ));
            }
            if let Some(ImportEpochRecord::Start(value)) = epoch_codec::decode(&frame.payload)
                .map_err(|error| OwnerError::Freshness(error.to_string()))?
            {
                start = Some(value);
            }
        }
    }
    let start =
        start.ok_or_else(|| OwnerError::Freshness("import settlement start unavailable".into()))?;
    let source_nodes = import_source_nodes(archive, &start)?;
    let scope = replay_import_epoch_scope(
        source,
        archive,
        memories,
        start,
        replay.freeze_prefix,
        source_nodes,
    )?;
    let import_id: String = replay
        .import_id
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    if scope.start.owner_uuid != replay.owner
        || scope.start.import_id != import_id
        || scope.start_offset != replay.start_prefix
        || scope.start_digest != replay.start_evidence_digest
        || scope.base_cut != replay.base_cut
        || scope.source_nodes != replay.source_nodes
        || scope.cohort != replay.born_memories
    {
        return Err(OwnerError::Freshness(
            "import settlement source/cohort evidence mismatch".into(),
        ));
    }
    Ok(())
}
