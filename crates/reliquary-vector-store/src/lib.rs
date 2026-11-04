//! Stable vector identities, compatibility profiles, and packed vector payloads.

pub mod archive;
pub mod archive_codec;
mod codec;
mod error;
pub mod generation;
pub mod memory;
mod memory_codec;
mod model;
pub mod profile;
pub mod rebuild;
pub mod store;

pub use archive::{
    AcceptedArchiveMapping, ArchiveVectorOpenState, ArchiveVectorStore,
    archive_vector_object_packed_id,
};
pub use error::{
    ArchiveVectorError, GenerationStoreError, MemoryVectorError, PackedVectorError,
    PayloadUnavailable,
};
pub use generation::{
    GenerationPayload, VectorGenerationOpenState, VectorGenerationStore, decode_generation,
    encode_generation, vector_generation_id,
};
pub use memory::{
    AcceptedMemoryMapping, MemoryVectorOpenState, MemoryVectorStore, validate_replay_record,
};
pub use model::{
    ArchiveFragmentId, ArchiveVectorId, ArchiveVectorInfo, ArchiveVectorRecord, ArchiveVectorStats,
    CompatibilityProbeReference, CompatibilityProfile, CompatibilityProfileId, EmbeddingMode,
    MemoryBodyId, MemoryVectorId, MemoryVectorInfo, MemoryVectorLocation, MemoryVectorRecord,
    MemoryVectorStats, PackedVectorId, PackedVectorInfo, PackedVectorStats, VectorGeneration,
    VectorGenerationId, VectorGenerationStats, VectorNormalization,
};
pub use profile::{
    CompatibilityProfileOpenState, CompatibilityProfileStats, CompatibilityProfileStore,
    ProfileStoreError,
};
pub use rebuild::PackedVectorOpenState;
pub use store::{PackedVectorStore, archive_vector_id, info, memory_vector_id, packed_vector_id};

#[cfg(test)]
mod tests;
