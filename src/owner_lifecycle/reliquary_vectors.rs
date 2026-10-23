//! Ordered finishing of REL's packed, bound and generated vector owner stores.
use super::OwnerError;
use crate::Archive;
use crate::archive_vector_rebuild::ArchiveVectorOpenState;
use crate::archive_vector_store::ArchiveVectorStore;
use crate::compatibility_profile_rebuild::CompatibilityProfileOpenState;
use crate::compatibility_profile_store::CompatibilityProfileStore;
use crate::external_index::store::ExternalIndexStore;
use crate::memory_store::MemoryStore;
use crate::memory_vector_rebuild::MemoryVectorOpenState;
use crate::memory_vector_store::MemoryVectorStore;
use crate::packed_vector_rebuild::PackedVectorOpenState;
use crate::packed_vector_store::PackedVectorStore;
use crate::vector_generation_rebuild::VectorGenerationOpenState;
use crate::vector_generation_store::VectorGenerationStore;

pub(crate) struct ReliquaryVectors {
    pub(crate) packed: PackedVectorStore,
    pub(crate) memory: MemoryVectorStore,
    pub(crate) archive: ArchiveVectorStore,
    pub(crate) profiles: CompatibilityProfileStore,
    pub(crate) generations: VectorGenerationStore,
}

/// Finish concrete vector owner stores in the same order used by REL replay.
/// External-index backing is a derived-state check between profile and binding
/// reconstruction; it does not confer semantic authority on that index.
#[allow(clippy::too_many_arguments)]
pub(crate) fn finish_reliquary_vectors(
    packed_state: PackedVectorOpenState,
    profile_state: CompatibilityProfileOpenState,
    memory_vector_state: MemoryVectorOpenState,
    archive_vector_state: ArchiveVectorOpenState,
    generation_state: VectorGenerationOpenState,
    external_index: &mut ExternalIndexStore,
    memories: &MemoryStore,
    archive: &Archive,
) -> Result<ReliquaryVectors, OwnerError> {
    let packed = packed_state.finish()?;
    let profiles = profile_state.finish()?;
    super::validate_external_index_backing(external_index, &packed, &profiles);

    let accepted_bodies = memory_vector_state
        .referenced_body_ids()
        .filter(|body| memories.contains_body(*body))
        .collect();
    let memory = memory_vector_state.finish(&accepted_bodies, &profiles, &packed)?;
    let fragment_versions = archive_vector_state
        .referenced_fragment_ids()
        .filter_map(|id| {
            archive
                .fragment_archive_version(id)
                .map(|version| (id, version))
        })
        .collect();
    let archive_vectors = archive_vector_state.finish(&packed, &fragment_versions)?;
    let generations = generation_state.finish(
        archive.archive_version(),
        &packed,
        &archive_vectors,
        &profiles,
    )?;

    Ok(ReliquaryVectors {
        packed,
        memory,
        archive: archive_vectors,
        profiles,
        generations,
    })
}
