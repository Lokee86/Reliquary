//! Ordered REL owner-store finishing for the validated open path.
use super::OwnerError;
use crate::dream_pair_history::DreamPairStore;
use crate::ego_store::EgoStore;
use crate::entity_rebuild::EntityOpenState;
use crate::entity_store::EntityStore;
use crate::memory_store::MemoryStore;
use crate::processing_epoch_store::ProcessingEpochStore;
use crate::relationship_rebuild::RelationshipOpenState;
use crate::relationship_store::RelationshipStore;
use crate::{Archive, Container};

pub(crate) struct ReliquaryRelations {
    pub(crate) entities: EntityStore,
    pub(crate) relationships: RelationshipStore,
}

/// Finish and validate the contiguous Entity/Relationship portion of REL open.
/// The ordering here is intentionally the historical facade ordering: later
/// graph/history, Dream, Freshness, vector and Import checkpoints remain outside.
#[allow(clippy::too_many_arguments)]
pub(crate) fn finish_reliquary_relations(
    container: &Container,
    entity_state: EntityOpenState,
    relationship_state: RelationshipOpenState,
    memories: &MemoryStore,
    ego: &EgoStore,
    archive: &Archive,
    processing_epochs: &ProcessingEpochStore,
    dream_pairs: &DreamPairStore,
) -> Result<ReliquaryRelations, OwnerError> {
    let entities = entity_state.finish(container)?;
    let relationships = relationship_state.finish(container)?;
    super::validate_current_relationship_references(
        container.owner_id().as_deref(),
        memories,
        &entities,
        &relationships.current(),
    )?;

    ego.validate_memory_version(memories.memory_version())?;
    super::memory_provenance::validate(memories, archive)?;
    processing_epochs.validate(memories)?;
    super::validate_dream_pairs(dream_pairs, memories)?;
    for link in archive.file_memory_link_records() {
        if !memories.contains_historical_memory(link.memory_id) {
            return Err(crate::ArchiveError::MissingFileMemoryTarget.into());
        }
    }

    Ok(ReliquaryRelations {
        entities,
        relationships,
    })
}
