use crate::compatibility_profile_store::CompatibilityProfileStore;
use crate::memory_vector_store::{MemoryVectorPublicationPlan, MemoryVectorStore};
use crate::packed_vector_store::PackedVectorStore;
use crate::{CompatibilityProfile, CompatibilityProfileError, Container, MemoryVectorError};

pub(crate) fn accept_profile(
    container: &mut Container,
    profiles: &mut CompatibilityProfileStore,
    candidate: CompatibilityProfile,
) -> Result<CompatibilityProfile, CompatibilityProfileError> {
    if let Some(existing) = profiles.compatible_with(&candidate) {
        return Ok(existing.clone());
    }
    profiles.put(container, candidate.clone())?;
    Ok(candidate)
}

pub(crate) fn publish_memory_vectors(
    container: &mut Container,
    profiles: &CompatibilityProfileStore,
    packed_vectors: &mut PackedVectorStore,
    memory_vectors: &mut MemoryVectorStore,
    plan: MemoryVectorPublicationPlan,
) -> Result<usize, MemoryVectorError> {
    let count = plan.body_ids.len();
    let packed_id = packed_vectors.put(container, plan.packed_vectors)?;
    memory_vectors.put(
        container,
        profiles,
        packed_vectors,
        plan.profile_id,
        packed_id,
        plan.body_ids,
    )?;
    Ok(count)
}

pub(crate) fn publish_semantic_archive(
    container: &mut Container,
    archive: &crate::Archive,
    profiles: &CompatibilityProfileStore,
    packed_vectors: &mut PackedVectorStore,
    archive_vectors: &mut crate::archive_vector_store::ArchiveVectorStore,
    generations: &mut crate::vector_generation_store::VectorGenerationStore,
    plan: crate::memory_vector_store::SemanticArchivePublicationPlan,
) -> Result<crate::VectorGeneration, super::OwnerError> {
    let packed_id = packed_vectors.put(container, plan.packed)?;
    crate::archive_vector_store::validate_mapping_shape(
        packed_vectors,
        packed_id,
        &plan.fragment_ids,
    )?;
    let mut seen = std::collections::HashSet::with_capacity(plan.fragment_ids.len());
    let mut max_version = 0;
    for id in &plan.fragment_ids {
        let version = archive
            .fragment_archive_version(*id)
            .ok_or(crate::ArchiveVectorError::MissingFragment)?;
        if !seen.insert(*id) {
            return Err(crate::ArchiveVectorError::DuplicateFragment.into());
        }
        max_version = max_version.max(version);
    }
    let archive_id = archive_vectors.put(
        container,
        packed_vectors,
        max_version,
        packed_id,
        plan.fragment_ids,
    )?;
    profiles
        .get_active(plan.profile_id)
        .ok_or(crate::VectorGenerationError::MissingProfile)?;
    let candidate = crate::VectorGeneration {
        id: crate::vector_generation_validation::vector_generation_id(
            plan.profile_id,
            archive_id,
            plan.archive_version,
        ),
        compatibility_profile_id: plan.profile_id,
        archive_vector_id: archive_id,
        source_archive_version: plan.archive_version,
        global_version: 0,
        vector_version: 0,
    };
    if candidate.source_archive_version == 0
        || candidate.source_archive_version > archive.archive_version()
    {
        return Err(crate::VectorGenerationError::SourceArchiveVersion.into());
    }
    crate::vector_generation_validation::validate_generation_reference(
        archive.archive_version(),
        packed_vectors,
        archive_vectors,
        profiles,
        &candidate,
    )?;
    if candidate.source_archive_version != archive.archive_version()
        || !archive_vectors.has_exact_fragment_coverage(
            candidate.archive_vector_id,
            &archive.fragments().iter().map(|f| f.id).collect::<Vec<_>>(),
        )
    {
        return Err(crate::VectorGenerationError::IncompleteCoverage.into());
    }
    Ok(generations.publish(container, plan.profile_id, archive_id, plan.archive_version)?)
}
