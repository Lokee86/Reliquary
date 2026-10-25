//! Physical operations used while replaying accepted Import epochs.
use crate::imports::{ImportEpochStore, ImportStart};
use crate::{Container, ContainerError};
use std::path::Path;

pub(crate) fn open_import_epoch(epochs: &ImportEpochStore) -> bool {
    epochs.open().is_some()
}

pub(crate) fn open_import_start(epochs: &ImportEpochStore) -> Option<ImportStart> {
    epochs.open().cloned()
}

pub(crate) fn append_replay_payload(
    container: &mut Container,
    payload: &[u8],
) -> Result<(), ContainerError> {
    container.append(payload).map(|_| ())
}

pub(crate) fn append_import_start(
    container: &mut Container,
    epochs: &mut ImportEpochStore,
    start: ImportStart,
) -> Result<(), crate::imports::ImportError> {
    epochs.append_start(container, start)
}

pub(crate) fn sync(container: &Container) -> Result<(), crate::owner_lifecycle::OwnerError> {
    container
        .sync()
        .map_err(crate::owner_lifecycle::OwnerError::Container)
}

pub(crate) fn enter_epoch_write_scope(container: &mut Container) {
    container.set_owner_write_scope(true);
}

pub(crate) fn leave_epoch_write_scope(container: &mut Container) {
    container.set_owner_write_scope(false);
}

/// Open a physical REL container for read-only owner reconstruction.
pub(crate) fn open_read_only_container(path: &Path) -> Result<Container, ContainerError> {
    Container::open_read_only(path)
}

/// Identity retained for one output path created by physical copy.
pub(crate) struct CreatedOutputIdentity(crate::container::PhysicalReadSource);

pub(crate) fn hold_created_output_identity(
    container: &Container,
) -> Result<CreatedOutputIdentity, ContainerError> {
    container.read_source().map(CreatedOutputIdentity)
}

pub(crate) fn cleanup_created_output_after_failure(identity: CreatedOutputIdentity, path: &Path) {
    if identity.0.path() == path && identity.0.validate_path_identity().is_ok() {
        let _ = std::fs::remove_file(path);
    }
}

pub(crate) fn replay_entity_tail(
    container: &mut Container,
    owner: &mut crate::entity_store::EntityStore,
    tail: crate::entity_ordered_replay::EntityTail,
) -> Result<crate::entity_ordered_replay::EntityReplayResult, crate::EntityError> {
    crate::entity_ordered_replay::replay_entity_tail(container, owner, tail)
}
pub(crate) fn replay_relationship_tail(
    container: &mut Container,
    owner: &mut crate::relationship_store::RelationshipStore,
    tail: crate::relationship_ordered_replay::RelationshipTail,
) -> Result<crate::relationship_ordered_replay::RelationshipReplayResult, crate::RelationshipError>
{
    crate::relationship_ordered_replay::replay_relationship_tail(container, owner, tail)
}
