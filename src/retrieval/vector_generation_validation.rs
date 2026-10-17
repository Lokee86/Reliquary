use crate::archive_vector_store::ArchiveVectorStore;
use crate::compatibility_profile_store::CompatibilityProfileStore;
use crate::packed_vector_store::PackedVectorStore;
use crate::{
    Archive, ArchiveVectorId, CompatibilityProfileId, ScalarType, VectorGeneration,
    VectorGenerationError, VectorGenerationId,
};
use sha2::{Digest, Sha256};

pub(crate) fn validate_generation_reference(
    archive: &Archive,
    packed_vectors: &PackedVectorStore,
    archive_vectors: &ArchiveVectorStore,
    profiles: &CompatibilityProfileStore,
    generation: &VectorGeneration,
) -> Result<(), VectorGenerationError> {
    if generation.source_archive_version == 0
        || generation.source_archive_version > archive.archive_version()
    {
        return Err(VectorGenerationError::SourceArchiveVersion);
    }
    let profile = profiles
        .get(generation.compatibility_profile_id)
        .ok_or(VectorGenerationError::MissingProfile)?;
    let archive_vectors = archive_vectors
        .info(generation.archive_vector_id)
        .ok_or(VectorGenerationError::MissingArchiveVectors)?;
    if generation.source_archive_version < archive_vectors.max_fragment_archive_version {
        return Err(VectorGenerationError::SourceArchiveVersion);
    }
    let packed = packed_vectors
        .info(archive_vectors.packed_vector_id)
        .ok_or(VectorGenerationError::MissingArchiveVectors)?;
    if profile.dimensions != packed.schema.dimensions {
        return Err(VectorGenerationError::DimensionMismatch);
    }
    if packed.schema.scalar != ScalarType::F32 {
        return Err(VectorGenerationError::UnsupportedScalar(
            packed.schema.scalar,
        ));
    }
    Ok(())
}

pub(crate) fn has_complete_current_coverage(
    archive: &Archive,
    archive_vectors: &ArchiveVectorStore,
    generation: &VectorGeneration,
) -> bool {
    generation.source_archive_version == archive.archive_version()
        && archive_vectors
            .info(generation.archive_vector_id)
            .is_some_and(|vectors| vectors.rows == archive.fragments().len() as u64)
}

pub(crate) fn vector_generation_id(
    compatibility_profile_id: CompatibilityProfileId,
    archive_vector_id: ArchiveVectorId,
    source_archive_version: u64,
) -> VectorGenerationId {
    let mut hash = Sha256::new();
    hash.update(b"CVA-VECTOR-GENERATION-V2\0");
    hash.update(compatibility_profile_id.0);
    hash.update(archive_vector_id.0);
    hash.update(source_archive_version.to_le_bytes());
    VectorGenerationId(hash.finalize().into())
}
