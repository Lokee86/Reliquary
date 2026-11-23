use super::archive_lexical::ARCHIVE_LEXICAL_INDEX_MAGIC;
use super::grouped_completion::StoredGroupedCompletion;
use super::rel_dependencies::REL_DEPENDENCY_MAGIC;
use super::*;

pub(super) fn grouped_completion_receipt(
    completion: &GroupedInsomniaCompletion,
) -> AcceptedCompletionState {
    AcceptedCompletionState {
        episode_id: WorkEpisodeId(completion.episode_id),
        attempt: completion.attempt,
        started_at_ns: completion.started_at_ns,
        completed_at_ns: completion.completed_at_ns,
        extractor_model: completion.extractor_model.clone(),
        extractor_version: completion.extractor_version.clone(),
        memory_ids: completion
            .memory_ids
            .iter()
            .copied()
            .map(WorkMemoryId)
            .collect(),
        external_memory_refs: completion
            .external_memory_refs
            .iter()
            .map(|reference| reliquary_insomnia_store::ExternalMemoryRef {
                owner_id: reference.owner_id.clone(),
                memory_id: WorkMemoryId(reference.memory_id),
            })
            .collect(),
        rejected_count: completion.rejected_count,
    }
}

pub(super) fn canonical_container_path(
    container: &Container,
) -> Result<PathBuf, OwnerSessionError> {
    std::fs::canonicalize(container.path()).map_err(|error| {
        OwnerSessionError::Record(format!("cannot identify owner session container: {error}"))
    })
}

pub(super) fn require_kind(
    container: &Container,
    expected: FileKind,
) -> Result<(), OwnerSessionError> {
    let actual = container.identity().file_kind;
    if actual == expected {
        Ok(())
    } else {
        Err(OwnerSessionError::WrongFileKind { expected, actual })
    }
}

#[derive(Default)]
pub(super) struct RelReplayAdmission {
    pub completions: BTreeMap<(WorkEpisodeId, u32), StoredGroupedCompletion>,
    pub import_epochs: BTreeMap<(WorkEpisodeId, u32), String>,
    pub dependencies: RelDependencyDeclaration,
    pub has_archive_lexical_indexes: bool,
}

pub(super) fn validate_rel_replay_admission(
    container: &Container,
    archive: &Archive,
    leased: bool,
) -> Result<RelReplayAdmission, OwnerSessionError> {
    let mut admission = RelReplayAdmission::default();
    let mut imports = ImportEpochStore::prefix(container.owner_id());
    let mut visible_episodes = std::collections::BTreeSet::new();
    let mut visible_turns = std::collections::BTreeSet::new();
    let mut visible_memories = std::collections::BTreeSet::new();
    let mut activity = 0u64;
    let mut grouped = container.matching_physical_cursor(|prefix| {
        let body = match reliquary_container::decode_publication(prefix) {
            Ok(Some(publication)) => publication.body,
            Ok(None) => prefix,
            Err(_) => return true,
        };
        body.starts_with(b"RELINSC")
    })?;
    let needs_archive_activity = grouped.next_frame()?.is_some();
    let select: fn(&[u8]) -> bool = if needs_archive_activity {
        crate::records::frame_selection::without_packed_vectors
    } else {
        // Archive replay already validates source identity and content. With
        // no grouped completion there is no owner activity cut to compare.
        crate::records::frame_selection::without_archive_or_packed
    };
    let mut frames = container.scan_matching_frames(select)?;
    while let Some(frame) = frames.next_frame()? {
        if crate::records::import::is_import_epoch_marker(frame.payload) {
            imports
                .ingest_prefix(frame.payload)
                .map_err(OwnerSessionError::Record)?;
            continue;
        }
        let envelope = reliquary_container::decode_publication(frame.payload)?;
        let body = envelope.as_ref().map_or(frame.payload, |value| value.body);
        if body.starts_with(ARCHIVE_LEXICAL_INDEX_MAGIC) {
            if envelope.is_some() {
                return Err(OwnerSessionError::Record(
                    "Archive lexical index must be a top-level frame".into(),
                ));
            }
            admission.has_archive_lexical_indexes = true;
            continue;
        }
        if body.starts_with(REL_DEPENDENCY_MAGIC) {
            if envelope.is_some() {
                return Err(OwnerSessionError::Record(
                    "REL dependency declaration must be a top-level frame".into(),
                ));
            }
            admission.dependencies.replay(container.owner_id(), body)?;
            continue;
        }
        if body.starts_with(b"RELRELM") {
            if envelope.is_some() {
                return Err(OwnerSessionError::Record("wrapped REL metadata".into()));
            }
            crate::records::owner_metadata::decode_retained_owner_metadata(body)
                .map_err(OwnerSessionError::Record)?;
        }
        let unsupported_group = body.starts_with(b"RELINSC")
            && (envelope.is_some()
                || !(body.starts_with(b"RELINSC7")
                    || (body.starts_with(b"RELINSC8") && matches!(body.get(8), Some(3 | 5 | 6)))));
        if (body.starts_with(b"RELXMEM") && !body.starts_with(b"RELXMEM3")) || unsupported_group {
            return Err(OwnerSessionError::Record(
                "unsupported owner publication requires authenticated replay".into(),
            ));
        }
        let payload = envelope
            .filter(|envelope| envelope.body.starts_with(b"RELMCRT1"))
            .map_or(frame.payload, |envelope| envelope.body);
        match reliquary_archive::source_publication(frame.payload)? {
            Some(reliquary_archive::SourcePublication::Episode(id)) => {
                visible_episodes.insert(id);
            }
            Some(reliquary_archive::SourcePublication::Turn {
                conversation_id,
                node_id,
                counts_activity,
            }) => {
                if visible_turns.insert((conversation_id, node_id)) && counts_activity {
                    activity = activity.checked_add(1).ok_or_else(|| {
                        OwnerSessionError::Record("Archive activity exhausted".into())
                    })?;
                }
            }
            None => {}
        }
        if payload.starts_with(b"RELMCRT1") {
            let creation = reliquary_memory_core::decode_grouped_creation(payload)?;
            visible_memories.insert(creation.id.0);
        }
        if !crate::records::insomnia_completion::is_grouped_completion(payload) {
            continue;
        }
        let retained = payload.starts_with(b"RELINSC8");
        let completion =
            GroupedInsomniaCompletion::decode(payload).map_err(OwnerSessionError::Record)?;
        let episode_id = reliquary_archive::EpisodeId(completion.episode_id);
        if archive.episode(episode_id).is_none() || !visible_episodes.contains(&episode_id) {
            return Err(OwnerSessionError::Record(
                "grouped completion Episode is not admitted by this Archive".into(),
            ));
        }
        if !completion.memory_records.is_empty()
            && completion.freshness_birth_turn != Some(activity)
        {
            return Err(OwnerSessionError::Record(
                "grouped completion Freshness turn differs from Archive prefix".into(),
            ));
        }
        completion
            .validate_replay_episode_layout(completion.episode_id, retained)
            .map_err(OwnerSessionError::Record)?;
        let births = completion
            .memory_records
            .iter()
            .map(|bytes| {
                reliquary_memory_core::decode_grouped_creation(bytes).map(|record| record.id.0)
            })
            .collect::<Result<std::collections::BTreeSet<_>, _>>()?;
        if completion
            .memory_ids
            .iter()
            .any(|id| !visible_memories.contains(id) && !births.contains(id))
        {
            return Err(OwnerSessionError::Record(
                "grouped completion references a Memory outside its physical prefix".into(),
            ));
        }
        visible_memories.extend(births);
        if !retained && !completion.routing_metadata.is_empty() {
            return Err(OwnerSessionError::Record(
                "grouped routing metadata replay is unsupported".into(),
            ));
        }
        let key = (WorkEpisodeId(completion.episode_id), completion.attempt);
        if let Some(epoch) = imports
            .open_epoch()
            .filter(|epoch| epoch.format_version == 2)
        {
            if imports.settlement(&epoch.import_id).is_some() {
                return Err(OwnerSessionError::Record(
                    "grouped extraction follows its sealed import settlement".into(),
                ));
            }
            admission.import_epochs.insert(key, epoch.import_id.clone());
        }
        let stored = StoredGroupedCompletion::from_admitted(
            completion,
            leased.then_some((frame.object, payload)),
        )?;
        if admission.completions.insert(key, stored).is_some() {
            return Err(OwnerSessionError::Record(
                "duplicate grouped completion attempt".into(),
            ));
        }
    }
    Ok(admission)
}

pub(super) fn reject_rel_only_records(container: &Container) -> Result<(), OwnerSessionError> {
    const REL_ONLY: &[(&[u8; 8], &str)] = &[
        (b"RELRELM1", "retained REL metadata"),
        (
            reliquary_freshness_store::RETAINED_FRESHNESS_AUTHORITY_MARKER,
            "retained Freshness settlement",
        ),
        (
            reliquary_archive::PROJECT_CORRELATION_MARKER,
            "Archive project correlation",
        ),
        (
            reliquary_archive::PROJECT_BACKING_MARKER,
            "Archive project binding",
        ),
        (
            reliquary_archive::ARCHIVE_PUBLICATION_MARKER,
            "versioned Archive publication",
        ),
        (b"RELTURN2", "Archive turn"),
        (b"RELFILE1", "Archive file"),
        (b"RELBRCH1", "Archive branch"),
        (b"RELEPIS1", "Archive Episode"),
        (b"CVAIMPT1", "Archive import receipt"),
        (b"CVAFRG01", "Archive fragment definition"),
        (b"RELECHO1", "Archive Echo"),
        (b"RELFRS02", "Freshness journal"),
        (b"RELFRP01", "Freshness policy"),
        (b"RELIEPS1", "import epoch start"),
        (b"RELIEPC1", "import epoch completion"),
        (b"RELIEPS2", "import epoch start v2"),
        (b"RELIEPC2", "import epoch completion v2"),
        (b"RELIFRS1", "import Freshness settlement"),
        (
            crate::records::IMPORT_PROCESSING_MAGIC,
            "import processing plan",
        ),
        (b"RELINSW1", "Insomnia work"),
        (b"RELINSR1", "Insomnia receipt"),
        (b"RELINSC7", "grouped Insomnia completion"),
        (b"RELINSC8", "grouped Insomnia completion v8"),
        (b"RELAVFM1", "Archive vector format"),
        (ARCHIVE_LEXICAL_INDEX_MAGIC, "Archive lexical index"),
        (b"RELAVEC1", "Archive vector"),
        (b"RELVGFM1", "vector generation format"),
        (b"RELVGEN1", "vector generation"),
        (b"RELVGRC1", "vector generation revision"),
        (b"RELXIDX2", "external source index"),
        (b"RELXMEM2", "external Memory reference"),
        (b"RELXMEM3", "external Memory intent"),
        (b"RELXMRP3", "external Memory receipt"),
        (b"RELXHST1", "external host evidence receipt"),
        (REL_DEPENDENCY_MAGIC, "REL dependency declaration"),
    ];
    use std::io::Read;
    let mut cursor = container.physical_cursor()?;
    while let Some(mut frame) = cursor.next_frame()? {
        let mut prefix = [0u8; 8];
        let mut length = 0;
        while length < prefix.len() {
            let count = frame
                .read(&mut prefix[length..])
                .map_err(ContainerError::Io)?;
            if count == 0 {
                break;
            }
            length += count;
        }
        if length == prefix.len() && prefix.starts_with(b"RELPUB") {
            // An envelope needs at most 33 bytes to validate its header and
            // inspect the complete eight-byte owner record marker.
            let mut header = [0u8; 33];
            header[..8].copy_from_slice(&prefix);
            let mut used = 8;
            while used < header.len() {
                let count = frame
                    .read(&mut header[used..])
                    .map_err(ContainerError::Io)?;
                if count == 0 {
                    break;
                }
                used += count;
            }
            let envelope = reliquary_container::decode_publication(&header[..used])?
                .ok_or_else(|| OwnerSessionError::Record("missing publication envelope".into()))?;
            prefix.copy_from_slice(&envelope.body[..8]);
        }
        if length == prefix.len() {
            for &(marker, domain) in REL_ONLY {
                if &prefix == marker {
                    return Err(OwnerSessionError::RelOnlyRecordInPhylactery(domain));
                }
            }
        }
        if length >= 6 && &prefix[..6] == b"RELAPB" {
            return Err(OwnerSessionError::RelOnlyRecordInPhylactery(
                "versioned Archive publication",
            ));
        }
        if length >= 6 && &prefix[..6] == b"RELFRS" {
            return Err(OwnerSessionError::RelOnlyRecordInPhylactery(
                "Freshness record",
            ));
        }
    }
    Ok(())
}
