pub mod archive;
pub mod archive_codec;
pub mod codec;
mod error;
pub mod generation;
pub mod memory;
mod memory_codec;
mod model;
pub mod profile;
pub mod rebuild;
pub mod store;

pub use archive::{
    AcceptedArchiveMapping, ArchiveVectorError, ArchiveVectorOpenState, ArchiveVectorReplayRecord,
    ArchiveVectorStore, archive_vector_id, archive_vector_object_packed_id,
};
pub use error::{PackedVectorError, PayloadUnavailable};
pub use memory::{
    AcceptedMemoryMapping, DecodedMemoryVectorObject, MemoryVectorError, MemoryVectorOpenState,
    MemoryVectorReplayRecord, MemoryVectorStore, decode_memory_vector_object, memory_vector_id,
    validate_replay_record,
};
pub use memory_codec::encode_format as encode_memory_vector_format;
pub use model::{ArchiveFragmentId, ArchiveVectorInfo, ArchiveVectorRecord, ArchiveVectorStats};
pub use model::{
    ArchiveVectorId, CompatibilityProbeReference, CompatibilityProfile, CompatibilityProfileId,
    EmbeddingMode, VectorGeneration, VectorGenerationId, VectorGenerationStats,
    VectorNormalization,
};
pub use model::{
    MemoryBodyId, MemoryVectorId, MemoryVectorInfo, MemoryVectorLocation, MemoryVectorRecord,
    MemoryVectorStats,
};
pub use model::{PackedVectorId, PackedVectorInfo, PackedVectorStats};
pub use profile::{
    CompatibilityProfileOpenState, CompatibilityProfileStats, CompatibilityProfileStore,
    ProfileStoreError,
};
pub use rebuild::PackedVectorOpenState;
pub use store::{PackedVectorStore, info, packed_vector_id};
