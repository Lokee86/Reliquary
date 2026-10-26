//! Durable semantic Memory values, current-format codecs, and owner storage.
pub mod memory_codec;
mod memory_codec_scalar;
mod memory_codec_values;
mod memory_compact_apply;
pub mod memory_compact_codec;
pub mod memory_compact_contract;
mod memory_compact_values;
pub mod memory_error;
pub mod memory_model;
pub mod memory_rebuild;
pub mod memory_routing_codec;
pub mod memory_routing_model;
pub mod memory_store;
mod memory_temporal_codec;
mod memory_temporal_codec_kind;

pub use memory_error::MemoryError;
pub use memory_model::MemoryRecord;
pub use memory_model::{
    EpisodeId, Memory, MemoryBodyId, MemoryDraft, MemoryHistoricalPayload, MemoryId,
    MemoryLifecycleState, MemoryPayloadUnavailable, MemoryRef, MemoryRetainedRevision,
    MemoryRetentionDescriptor, MemoryRevisionId, MemorySourceRef, MemoryStats,
    MemoryTemporalInference, MemoryTombstoneCoordinates, MemoryTombstoneOutcome, SourceEpisodeId,
    TemporalIndicationKind, TemporalInference, TemporalInferenceResolution,
};
pub use memory_model::{memory_body_bytes, memory_body_id, memory_id};
pub use memory_rebuild::MemoryOpenState;
pub use memory_routing_model::{
    MAX_MEMORY_ENTITY_MENTIONS, MAX_MEMORY_ROUTING_TEXT_BYTES, MemoryEntityMention,
    MemoryEntityMentionKey, MemoryRoutingMetadata, MemoryTextField,
};
pub use memory_store::{
    MemoryGroupedBatch, MemoryGroupedBody, MemoryGroupedBodyEncoding, MemoryGroupedBodySpan,
    MemoryStore, PreparedMemoryBatch, PrunedMemoryBody,
};

#[cfg(test)]
#[path = "memory_compact_retry_tests.rs"]
mod memory_compact_retry_tests;
#[cfg(test)]
mod memory_identity_tests;

#[cfg(test)]
mod memory_codec_tests;
