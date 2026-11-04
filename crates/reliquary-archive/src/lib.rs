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

pub use archive::Archive;
pub use echo_model::{EchoError, EchoEvent, EchoEventKind};
pub use echo_store::EchoStore;
pub use fragment::{Fragment, FragmentConfig, FragmentId, fragment_id, path_fragment_windows};
pub use model::{
    AcceptedTurnSource, ArchiveError, AttachmentInput, Branch, ContentId, Episode, EpisodeBoundary,
    EpisodeId, EpisodeOrigin, Node, ProjectAttachmentInput, ProjectFileRef, ProjectRepositoryKind,
    ProjectRepositoryRef, ProjectRevisionRef, ResolvedTurn, SourceAttachment, TurnAcceptance,
    TurnInput,
};

pub use archive::validate_project_file_reference;

/// Identify an Episode publication in a physical frame without exposing its codec.
pub fn episode_publication_id(payload: &[u8]) -> Result<Option<EpisodeId>, ArchiveError> {
    if !payload.starts_with(b"RELEPIS1") {
        return Ok(None);
    }
    match codec::decode_record(payload)? {
        Some(codec::Record::Episode(episode)) => Ok(Some(episode.id)),
        _ => Ok(None),
    }
}

#[cfg(test)]
#[path = "echo_tests.rs"]
mod echo_tests;
