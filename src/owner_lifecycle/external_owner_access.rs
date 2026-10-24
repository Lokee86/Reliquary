use crate::compatibility_profile_store::CompatibilityProfileStore;
use crate::external_index::store::{ExternalIndexStore, SourceState};
use crate::external_source::index::PreparedIndex;
use crate::external_source::processing::PreparedExternalMemory;
use crate::external_source::{self, ExternalIndexStatus, ExternalSourceError};
use crate::freshness_dream_journal::DreamFreshnessJournal;
use crate::freshness_storage::FreshnessStore;
use crate::memory_store::MemoryStore;
use crate::packed_vector_store::PackedVectorStore;
use crate::{CompatibilityProfile, CompatibilityProfileId, Container, Memory, MemoryError};

pub(crate) fn profile(
    profiles: &CompatibilityProfileStore,
    state: &SourceState,
) -> Result<CompatibilityProfile, ExternalSourceError> {
    profiles
        .get(CompatibilityProfileId(state.profile))
        .cloned()
        .ok_or(ExternalSourceError::RebuildRequired)
}

pub(crate) fn install(
    container: &mut Container,
    index: &mut ExternalIndexStore,
    profiles: &mut CompatibilityProfileStore,
    packed_vectors: &mut PackedVectorStore,
    prepared: PreparedIndex,
) -> Result<ExternalIndexStatus, ExternalSourceError> {
    if index
        .sources
        .get(&prepared.state.namespace)
        .map(|state| state.revision)
        != prepared.expected_revision
    {
        return Err(ExternalSourceError::Conflict);
    }
    if prepared.expected_revision == Some(prepared.state.revision) {
        return Ok(prepared.state.status());
    }
    profiles
        .put(container, prepared.profile)
        .map_err(|_| ExternalSourceError::Storage)?;
    for packed in prepared.matrices {
        packed_vectors
            .put(container, packed)
            .map_err(|_| ExternalSourceError::Storage)?;
    }
    let status = prepared.state.status();
    index.install(container, prepared.state, prepared.expected_revision)?;
    Ok(status)
}

pub(crate) fn packed_vector_read<'a>(
    container: &'a Container,
    packed_vectors: &'a PackedVectorStore,
) -> crate::packed_vector_store::PackedVectorReadView<'a> {
    crate::packed_vector_store::PackedVectorReadView::new(packed_vectors, container)
}

pub(crate) fn publish_window(
    container: &mut Container,
    memories: &mut MemoryStore,
    freshness: &mut FreshnessStore,
    dream_freshness: &mut DreamFreshnessJournal,
    owner_uuid: Option<[u8; 16]>,
    turn: u64,
    window: [u8; 32],
    prepared: &[PreparedExternalMemory],
) -> Result<Vec<Memory>, MemoryError> {
    if let Some(existing) = memories.external_window(container, window)? {
        return Ok(existing);
    }
    let mut after = freshness.clone();
    if let Some(owner) = owner_uuid {
        for memory in prepared {
            after
                .derive_birth(
                    owner,
                    crate::memory_model::memory_id(&memory.draft.mutation_id),
                    turn,
                    false,
                )
                .map_err(MemoryError::Freshness)?;
        }
    }
    let results = external_source::publication::publish(
        container,
        memories,
        window,
        prepared,
        owner_uuid.map(|_| turn),
    )?;
    *freshness = after;
    for memory in &results {
        dream_freshness.derive_publication_birth(memory.id);
    }
    Ok(results)
}
