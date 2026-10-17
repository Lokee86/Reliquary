use crate::cva_reconcile_archive::ArchiveReplayRecord;
use crate::{ArchiveError, Cva, CvaReconcileConflict, CvaReconcileError, Memory, MemoryError};

pub(crate) fn archive_replay_error(
    destination: &Cva,
    record: &ArchiveReplayRecord,
    error: ArchiveError,
) -> CvaReconcileError {
    let conflict = match (record, &error) {
        (ArchiveReplayRecord::Node(turn), ArchiveError::ConflictingNode)
        | (ArchiveReplayRecord::IngestedTurn(turn), ArchiveError::ConflictingNode)
        | (ArchiveReplayRecord::IngestedTurn(turn), ArchiveError::ConflictingTurnIngest)
        | (ArchiveReplayRecord::IngestedTurn(turn), ArchiveError::ConflictingFile) => {
            Some(CvaReconcileConflict::SourceTurn {
                conversation_id: turn.conversation_id.clone(),
                node_id: turn.id.clone(),
            })
        }
        (ArchiveReplayRecord::Branch(branch), ArchiveError::InvalidBranchRevision) => {
            let existing_leaf_node_id = destination
                .branches()
                .into_iter()
                .find(|existing| {
                    existing.id == branch.id && existing.conversation_id == branch.conversation_id
                })
                .map(|existing| existing.leaf_node_id)
                .unwrap_or_default();
            Some(CvaReconcileConflict::Branch {
                conversation_id: branch.conversation_id.clone(),
                branch_id: branch.id.clone(),
                existing_leaf_node_id,
                incoming_leaf_node_id: branch.leaf_node_id.clone(),
            })
        }
        (ArchiveReplayRecord::Episode(episode), ArchiveError::ConflictingEpisode)
        | (ArchiveReplayRecord::Episode(episode), ArchiveError::EpisodeBranchConflict) => {
            Some(CvaReconcileConflict::Episode {
                episode_id: episode.id,
            })
        }
        (ArchiveReplayRecord::Fragment(fragment), ArchiveError::ConflictingFragment) => {
            Some(CvaReconcileConflict::Fragment {
                fragment_id: fragment.id,
            })
        }
        (ArchiveReplayRecord::File(file, _, _), ArchiveError::ConflictingFile) => {
            Some(CvaReconcileConflict::File { file_id: file.id })
        }
        (ArchiveReplayRecord::ConversationTombstone(tombstone), _) => {
            Some(CvaReconcileConflict::ConversationTombstone {
                conversation_id: tombstone.conversation_id.clone(),
            })
        }
        (ArchiveReplayRecord::Node(turn), ArchiveError::TombstonedConversation)
        | (ArchiveReplayRecord::IngestedTurn(turn), ArchiveError::TombstonedConversation) => {
            Some(CvaReconcileConflict::ConversationTombstone {
                conversation_id: turn.conversation_id.clone(),
            })
        }
        (
            ArchiveReplayRecord::ConversationMetadata(metadata),
            ArchiveError::TombstonedConversation,
        ) => Some(CvaReconcileConflict::ConversationTombstone {
            conversation_id: metadata.conversation_id.clone(),
        }),
        (ArchiveReplayRecord::Branch(branch), ArchiveError::TombstonedConversation) => {
            Some(CvaReconcileConflict::ConversationTombstone {
                conversation_id: branch.conversation_id.clone(),
            })
        }
        _ => None,
    };
    conflict
        .map(CvaReconcileError::Conflict)
        .unwrap_or(CvaReconcileError::Archive(error))
}

pub(crate) fn memory_replay_error(
    destination: &mut Cva,
    memory: &Memory,
    error: MemoryError,
) -> CvaReconcileError {
    let conflict = match error {
        MemoryError::RevisionConflict => {
            let current_revision = destination
                .memory(memory.id)
                .map(|current| current.revision)
                .unwrap_or(0);
            Some(CvaReconcileConflict::MemoryRevision {
                memory_id: memory.id,
                expected_revision: memory.revision.saturating_sub(1),
                current_revision,
                incoming_revision: memory.revision,
            })
        }
        MemoryError::MutationConflict => Some(CvaReconcileConflict::MemoryMutation {
            memory_id: memory.id,
            mutation_id: memory.mutation_id.clone(),
            incoming_revision: memory.revision,
        }),
        MemoryError::SemanticMutation => Some(CvaReconcileConflict::MemorySemanticMutation {
            memory_id: memory.id,
            incoming_revision: memory.revision,
        }),
        other => return CvaReconcileError::Memories(other),
    };
    CvaReconcileError::Conflict(conflict.expect("mapped Memory conflict"))
}
