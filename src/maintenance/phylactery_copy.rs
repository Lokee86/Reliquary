//! Validate a transformed PHY copy through the same ordered owner-open checkpoints.
use crate::container::Container;
use crate::owner_lifecycle::{OwnerPhylacteryError, PhylacteryOwner};

pub(crate) fn reopen(container: Container) -> Result<PhylacteryOwner, OwnerPhylacteryError> {
    validate_owner(crate::owner_lifecycle::open_phylactery_from_container(
        container,
    )?)
}

pub(crate) fn validate_owner(
    owner: PhylacteryOwner,
) -> Result<PhylacteryOwner, OwnerPhylacteryError> {
    crate::history::validate_phylactery_prune_evidence(
        &owner.owner_maintenance,
        &owner
            .history_source()
            .map_err(|error| OwnerPhylacteryError::Repack(error.to_string()))?,
        &owner.graph,
        &owner.memories,
        &owner.entities,
        &owner.relationships,
        &owner.compatibility_profiles,
        &owner.memory_vectors,
        &owner.packed_vectors,
    )?;
    owner.entity_resolutions.validate_local()?;
    crate::owner_lifecycle::validate_entity_resolution_references(
        &owner.memories,
        &owner.entities,
        &owner.entity_resolutions.records(),
    )?;
    Ok(owner)
}
