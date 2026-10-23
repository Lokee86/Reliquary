//! Explicit rewrite selection and emission through local Archive replay contracts.
use crate::archive_ordered_replay::{
    ArchiveReplayRecord, ArchiveReplaySource, ArchiveReplayWriteError, ArchiveReplayWriter,
    ArchiveTail,
};
use crate::owner_lifecycle::ReliquaryOwner;
use crate::{ArchiveError, CvaReconcileError};

/// Copy only the Archive history selected by a rewrite and the conversations
/// needed to support retained Memory provenance. This lives beside the replay
/// representation so filtering can happen before any source history is emitted.
pub(crate) fn replay_rewrite_archive(
    source: &mut ReliquaryOwner,
    destination: &mut ReliquaryOwner,
    memories: &[crate::Memory],
    selection: &super::model::ArchiveSelection,
) -> Result<usize, CvaReconcileError> {
    use std::collections::{BTreeSet, HashSet};
    let retained_ids: BTreeSet<_> = memories.iter().map(|memory| memory.id).collect();
    let conversations =
        crate::maintenance::rewrite::archive_selection::rewrite_archive_conversations(
            &source.archive,
            &source.container,
            memories,
            selection,
        )?;
    let mut tail = crate::archive_ordered_replay::read_archive_range(
        ArchiveReplaySource {
            container: &source.container,
            archive: &source.archive,
            project_files: &source.project_files,
        },
        0,
        usize::MAX,
    )?;
    let mut selected_file_ids = HashSet::new();
    for entry in &tail.records {
        if let ArchiveReplayRecord::IngestedTurn(turn) = &entry.record
            && conversations.contains(&turn.conversation_id)
        {
            selected_file_ids.extend(turn.project_attachments.iter().map(|file| file.id));
        }
    }
    tail.file_memory_links.retain(|entry| {
        let keep = retained_ids.contains(&entry.link.memory_id);
        if keep {
            selected_file_ids.insert(entry.link.file_id);
        }
        keep
    });
    let mut file_ids: Vec<_> = selected_file_ids.iter().copied().collect();
    file_ids.sort_by_key(|id| id.0);
    for id in file_ids {
        let file = source
            .archive
            .file(id)
            .cloned()
            .ok_or(ArchiveError::MissingFile)?;
        if let Some(reference) = source.project_files.get(id).cloned() {
            replay_project_file(destination, &file, reference)?;
        } else {
            let bytes = source.archive.file_bytes(&source.container, id)?;
            destination.archive.store_file(
                &mut destination.container,
                file.filename,
                file.mime_type,
                &bytes,
            )?;
        }
    }
    tail.records.retain(|entry| match &entry.record {
        ArchiveReplayRecord::Node(turn) | ArchiveReplayRecord::IngestedTurn(turn) => {
            conversations.contains(&turn.conversation_id)
        }
        ArchiveReplayRecord::Branch(branch) => conversations.contains(&branch.conversation_id),
        ArchiveReplayRecord::ConversationMetadata(metadata) => {
            conversations.contains(&metadata.conversation_id)
        }
        ArchiveReplayRecord::ConversationTombstone(tombstone) => {
            conversations.contains(&tombstone.conversation_id)
        }
        ArchiveReplayRecord::Episode(episode) => conversations.contains(&episode.conversation_id),
        ArchiveReplayRecord::Fragment(fragment) => {
            conversations.contains(&fragment.conversation_id)
        }
        ArchiveReplayRecord::File(file, ..) => {
            selected_file_ids.contains(&file.id)
                || matches!(selection, super::model::ArchiveSelection::AllRetained)
        }
        ArchiveReplayRecord::ImportReceipt(_) => false,
    });
    let count = replay_archive_tail(destination, &tail)?;
    Ok(count)
}

pub(crate) fn replay_rewrite_file_links(
    source: &ReliquaryOwner,
    destination: &mut ReliquaryOwner,
    retained: &std::collections::BTreeSet<crate::MemoryId>,
) -> Result<(), CvaReconcileError> {
    for file in source.archive.files() {
        for link in source.archive.file_memory_links(file.id) {
            if retained.contains(&link.memory_id) {
                destination.archive.link_file_to_memory(
                    &mut destination.container,
                    link.file_id,
                    link.memory_id,
                )?;
            }
        }
    }
    Ok(())
}

fn replay_project_file(
    destination: &mut ReliquaryOwner,
    file: &crate::StoredFile,
    reference: crate::ProjectFileRef,
) -> Result<(), CvaReconcileError> {
    ArchiveReplayWriter {
        container: &mut destination.container,
        archive: &mut destination.archive,
        project_files: &mut destination.project_files,
    }
    .project_file(file, reference)
    .map_err(|error| match error {
        ArchiveReplayWriteError::Archive(error) => CvaReconcileError::Archive(error),
        ArchiveReplayWriteError::Project(error) => {
            crate::owner_lifecycle::OwnerError::from(error).into()
        }
    })
}
fn replay_archive_tail(
    destination: &mut ReliquaryOwner,
    tail: &ArchiveTail,
) -> Result<usize, CvaReconcileError> {
    let before = destination.archive.archive_version();
    for entry in &tail.records {
        let result = ArchiveReplayWriter {
            container: &mut destination.container,
            archive: &mut destination.archive,
            project_files: &mut destination.project_files,
        }
        .replay(entry);
        match result {
            Ok(()) => {}
            Err(ArchiveReplayWriteError::Archive(error)) => {
                return Err(
                    crate::owner_lifecycle::archive_replay_error::archive_replay_error(
                        &destination.archive,
                        &entry.record,
                        error,
                    ),
                );
            }
            Err(ArchiveReplayWriteError::Project(error)) => {
                return Err(crate::owner_lifecycle::OwnerError::from(error).into());
            }
        }
    }
    Ok(destination.archive.archive_version().saturating_sub(before) as usize)
}
