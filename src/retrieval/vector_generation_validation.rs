use crate::compatibility_profile_store::CompatibilityProfileStore;
use crate::{
    Archive, ArchiveVectorId, CompatibilityProfileId, ScalarType, VectorGeneration,
    VectorGenerationError, VectorGenerationId,
};
use reliquary_vector_store::ArchiveVectorStore;
use reliquary_vector_store::PackedVectorStore;

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
        && archive_vectors.has_exact_fragment_coverage(
            generation.archive_vector_id,
            &archive
                .fragments()
                .iter()
                .map(|fragment| reliquary_vector_store::ArchiveFragmentId(fragment.id.0))
                .collect::<Vec<_>>(),
        )
}

pub(crate) fn vector_generation_id(
    compatibility_profile_id: CompatibilityProfileId,
    archive_vector_id: ArchiveVectorId,
    source_archive_version: u64,
) -> VectorGenerationId {
    reliquary_vector_store::generation::vector_generation_id(
        compatibility_profile_id,
        archive_vector_id,
        source_archive_version,
    )
}
