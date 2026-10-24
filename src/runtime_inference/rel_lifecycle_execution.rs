//! Canonical REL lifecycle composition over physical owner and History contracts.
use crate::{CvaError, RelExecutionOwner, ReliquaryScopeKind};
use std::path::Path;

pub(crate) fn create(
    path: &Path,
    owner_uuid: Option<[u8; 16]>,
    scope: Option<ReliquaryScopeKind>,
    type_label: Option<String>,
    sync_metadata: bool,
) -> Result<RelExecutionOwner, CvaError> {
    Ok(RelExecutionOwner::from_owner(
        crate::owner_lifecycle::reliquary_creation::create(
            path,
            owner_uuid,
            scope,
            type_label,
            sync_metadata,
        )?,
    ))
}

pub(crate) fn open(path: &Path, read_only: bool) -> Result<RelExecutionOwner, CvaError> {
    reconstruct(crate::owner_lifecycle::reliquary_creation::open(
        path, read_only,
    )?)
}

pub(crate) fn reconstruct(
    mut replay: crate::owner_lifecycle::ReliquaryReplay,
) -> Result<RelExecutionOwner, CvaError> {
    let reconstruction = crate::history::reconstruct_reliquary(&mut replay)?;
    let (owner, due) = replay.into_owner(reconstruction);
    Ok(RelExecutionOwner::from_reconstructed(owner, due))
}

pub(crate) fn from_container(container: crate::Container) -> Result<RelExecutionOwner, CvaError> {
    reconstruct(crate::owner_lifecycle::ReliquaryReplay::read(container)?)
}
