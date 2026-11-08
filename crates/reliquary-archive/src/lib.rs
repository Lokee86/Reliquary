//! Native conversation Archive persistence over the physical REL Container.
//!
//! Archive owns accepted-turn, branch, and Episode semantics. Container owns
//! framing and bytes; Archive has no dependency on application or other owners.

mod archive;
mod codec;
mod echo_codec;
mod echo_model;
mod echo_store;
mod fragment;
mod model;

pub use archive::{
    Archive, ArchiveSearchResult, MAX_EVIDENCE_RESULT_BYTES, MAX_EVIDENCE_SEARCH_BYTES,
    MAX_EVIDENCE_SEARCH_RESULTS, MAX_EVIDENCE_SEARCH_SCAN, MAX_EVIDENCE_TURNS,
};
pub use echo_model::{EchoError, EchoEvent, EchoEventKind};
pub use echo_store::EchoStore;
pub use fragment::{
    Fragment, FragmentConfig, FragmentDefinition, FragmentId, fragment_id, path_fragment_windows,
};
pub use model::{
    AcceptedTurnSource, ArchiveError, ArchiveHistorySnapshot, AttachmentInput, Branch, ContentId,
    ConversationTitle, ConversationTombstone, Episode, EpisodeBoundary, EpisodeId, EpisodeOrigin,
    HERMES_COPIED_SOURCE_FORMAT, HistoricalArchiveBranch, HistoricalArchiveTurn, ImportReceipt,
    ImportedArtifact, ImportedBranch, ImportedConversation, ImportedMessage, ImportedSourceAuthor,
    Node, ProjectAttachmentInput, ProjectFileRef, ProjectRepositoryKind, ProjectRepositoryRef,
    ProjectRevisionRef, ResolvedTurn, SourceAttachment, TurnAcceptance, TurnInput,
};

pub use codec::{archive_turn_prune_marker, prune_turn_frame};

pub use archive::file_id_from_bytes;
pub use archive::validate_project_file_reference;

/// Identify an import receipt at its physical position during owner replay.
pub fn import_receipt_in_frame(payload: &[u8]) -> Result<Option<ImportReceipt>, ArchiveError> {
    if !payload.starts_with(b"CVAIMPT1") {
        return Ok(None);
    }
    match codec::decode_record(payload)? {
        Some(codec::Record::Import(receipt)) => Ok(Some(receipt)),
        _ => Err(ArchiveError::CorruptRecord("invalid import receipt frame")),
    }
}

/// Owner-session source facts needed to validate a physical Archive prefix.
pub enum SourcePublication {
    Turn {
        conversation_id: String,
        node_id: String,
        counts_activity: bool,
    },
    Episode(EpisodeId),
}

/// Identify accepted source publications without exposing the Archive codec.
pub fn source_publication(payload: &[u8]) -> Result<Option<SourcePublication>, ArchiveError> {
    if !payload.starts_with(b"RELEPIS1") && !payload.starts_with(b"RELTURN2") {
        return Ok(None);
    }
    match codec::decode_record(payload)? {
        Some(codec::Record::Episode(episode)) => Ok(Some(SourcePublication::Episode(episode.id))),
        Some(codec::Record::Turn(node, _, _)) => Ok(Some(SourcePublication::Turn {
            conversation_id: node.conversation_id,
            node_id: node.id,
            counts_activity: matches!(node.role.as_str(), "user" | "assistant"),
        })),
        _ => Ok(None),
    }
}

#[cfg(test)]
#[path = "echo_tests.rs"]
mod echo_tests;
