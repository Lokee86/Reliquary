//! Archive source history owner. Container is its sole internal dependency.
pub mod archive;
pub mod archive_activity;
pub mod archive_codec;
mod archive_conversation;
mod archive_conversation_metadata;
mod archive_conversation_tombstone;
pub mod archive_error;
pub mod archive_history;
pub mod archive_history_codec;
pub mod archive_history_model;
mod archive_import;
mod archive_lookup;
pub mod archive_model;
mod archive_object_index;
pub mod archive_rebuild;
pub mod archive_record_index;
pub mod archive_store;
mod conversation_compaction_allocator;
pub mod conversation_compaction_codec;
pub mod conversation_compaction_model;
pub mod conversation_compaction_store;
pub mod conversation_metadata_codec;
mod conversation_metadata_index;
pub mod conversation_metadata_model;
mod episode_builder;
mod episode_codec;
mod episode_import;
mod episode_index;
pub mod episode_model;
pub mod episode_policy;
mod episode_store;
mod file_index;
mod file_memory_link_codec;
mod file_memory_link_index;
pub mod file_memory_link_model;
mod file_memory_link_store;
pub mod file_model;
pub mod file_store;
pub mod fragment_model;
pub mod fragment_store;
pub mod fragmenter;
mod import_receipt;
pub mod interaction_stream_codec;
pub mod interaction_stream_record;
pub mod interaction_stream_store;
mod project_file_binding_codec;
pub mod project_file_binding_store;
pub mod project_history_codec;
pub mod project_history_model;
pub mod project_history_store;
mod source_attachment_index;
mod source_id;
pub mod turn_ingest_codec;
pub mod turn_ingest_model;
mod turn_ingest_store;
pub use archive::Archive;
pub use archive_error::{ArchiveError, ArchivePayloadUnavailable};
pub use archive_history_model::ArchiveRecordVersion;
pub use archive_model::{
    ArchiveStats, Branch, ContentId, ConversationSummary, ConversationTombstone,
    ConversationTombstoneClosure, Node, ResolvedTurn,
};
pub use conversation_compaction_model::{
    ConversationCompaction, ConversationCompactionError, MAX_CONVERSATION_COMPACTION_SUMMARY_BYTES,
};
pub use conversation_metadata_model::{ConversationMetadata, ConversationPolicy};
pub use episode_model::{
    DEFAULT_EPISODE_MAX_INPUT_BYTES, Episode, EpisodeBoundary, EpisodeBuildResult, EpisodeConfig,
    EpisodeId, EpisodeOrigin,
};
pub use episode_policy::{DEFAULT_EPISODE_INACTIVITY_NS, EpisodePolicy};
pub use file_memory_link_model::FileMemoryLink;
pub use file_model::{FileId, FileSearchHit, StoredFile};
pub use fragment_model::{
    DEFAULT_FRAGMENT_OVERLAP, DEFAULT_FRAGMENT_TURNS, Fragment, FragmentConfig, FragmentId,
};
pub use import_receipt::*;
pub use interaction_stream_record::InteractionRole;
pub use interaction_stream_record::{
    InteractionStreamRecord, InteractionStreamStatus, InteractionTurnStatus,
    ResolvedInteractionTurn,
};
pub use project_history_model::{
    ProjectFileRef, ProjectRepositoryKind, ProjectRepositoryManagement, ProjectRepositoryRef,
    ProjectRevisionCorrelation, ProjectRevisionRef, RelSemanticCut,
};
pub use source_id::MemoryId;
pub use turn_ingest_model::{IncomingAttachment, IncomingTurn, IngestedTurn};

#[cfg(test)]
mod extraction_tests;

mod source;
