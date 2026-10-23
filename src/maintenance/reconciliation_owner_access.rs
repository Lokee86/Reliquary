//! Operation-specific access to concrete Reliquary owners during reconciliation.
use crate::maintenance::rewrite::owner_access::{create_rel, open_rel};
use crate::owner_lifecycle::{OwnerError, ReliquaryOwner};
use crate::{
    Container, FileKind, MemoryEntityResolution, MemoryEntityResolutionStatus, MemoryError,
    ReliquaryScopeKind,
};
use std::path::Path;

pub(crate) fn open_reconciliation_owner(path: &Path) -> Result<ReliquaryOwner, OwnerError> {
    open_rel(Container::open_read_only(path)?)
}

pub(crate) fn open_reconciliation_container(
    container: Container,
) -> Result<ReliquaryOwner, OwnerError> {
    open_rel(container)
}

pub(crate) fn create_reconciliation_owner(
    path: &Path,
    owner_uuid: [u8; 16],
    legacy_scope: Option<ReliquaryScopeKind>,
) -> Result<ReliquaryOwner, OwnerError> {
    if let Some(scope) = legacy_scope {
        let container = Container::create_with_identity_and_uuid(
            path,
            crate::ContainerIdentity {
                file_kind: FileKind::Reliquary,
                scope: Some(scope),
            },
            owner_uuid,
        )?;
        crate::owner_lifecycle::initialize_reliquary(container)
    } else {
        create_rel(path, owner_uuid, None)
    }
}

pub(crate) fn legacy_scope(owner: &ReliquaryOwner) -> Option<ReliquaryScopeKind> {
    owner
        .container
        .identity()
        .and_then(|identity| identity.scope)
}

pub(crate) fn has_derived_vector_state(owner: &ReliquaryOwner) -> bool {
    owner.packed_vectors.stats().objects > 0
        || owner.memory_vectors.stats().objects > 0
        || owner.archive_vectors.stats().objects > 0
        || owner.vector_generations.stats().generations > 0
}

pub(crate) fn import_entity_resolution(
    owner: &mut ReliquaryOwner,
    value: MemoryEntityResolution,
) -> Result<(), MemoryError> {
    owner.memories.validate_entity_mention_key(value.key)?;
    let valid = match &value.status {
        MemoryEntityResolutionStatus::Resolved { entity_id, .. } => {
            owner.entities.contains(*entity_id)
        }
        MemoryEntityResolutionStatus::Pending(pending) => pending
            .candidate_entity_ids
            .iter()
            .all(|id| owner.entities.contains(*id)),
        MemoryEntityResolutionStatus::Dormant(dormant) => dormant
            .candidate_entity_ids
            .iter()
            .all(|id| owner.entities.contains(*id)),
        MemoryEntityResolutionStatus::Rejected { .. } => true,
    };
    if !valid {
        return Err(MemoryError::InvalidField(
            "Entity resolution Entity reference",
        ));
    }
    owner
        .entity_resolutions
        .import_current(&mut owner.container, value)
}
