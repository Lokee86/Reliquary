//! Native conversation Archive persistence over the physical REL Container.
//!
//! Archive owns accepted-turn, branch, and Episode semantics. Container owns
//! framing and bytes; Archive has no dependency on application or other owners.

mod archive;
mod codec;
mod echo_codec;
mod echo_model;
mod echo_store;
mod model;

pub use archive::Archive;
pub use echo_model::{EchoError, EchoEvent, EchoEventKind};
pub use echo_store::EchoStore;
pub use model::{
    AcceptedTurnSource, ArchiveError, AttachmentInput, Branch, ContentId, Episode, EpisodeBoundary,
    EpisodeId, EpisodeOrigin, Node, ProjectAttachmentInput, ProjectFileRef, ProjectRepositoryKind,
    ProjectRepositoryRef, ProjectRevisionRef, ResolvedTurn, SourceAttachment, TurnAcceptance,
    TurnInput,
};

pub use archive::validate_project_file_reference;

#[cfg(test)]
#[path = "echo_tests.rs"]
mod echo_tests;
