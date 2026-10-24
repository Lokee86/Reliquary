use crate::{Container, ContainerError, Memory, MemoryError, MemoryId};

pub(crate) fn require_write_admission(container: &Container) -> Result<(), ContainerError> {
    container.require_write_admission()
}

pub(crate) fn owner_uuid(container: &Container) -> Option<[u8; 16]> {
    container.owner_uuid()
}

pub(crate) fn read_memory(
    memories: &crate::memory_store::MemoryStore,
    container: &Container,
    id: MemoryId,
) -> Result<Memory, MemoryError> {
    memories.memory(container, id)
}

pub(crate) fn append_and_sync(
    container: &mut Container,
    payload: &[u8],
) -> Result<(), ContainerError> {
    container.append(payload)?;
    container.sync()
}

pub(crate) fn persist_policy_if_needed(
    owner: &mut crate::owner_lifecycle::ReliquaryOwner,
    owner_uuid: [u8; 16],
) -> Result<(), String> {
    owner
        .freshness
        .persist_policy_if_needed(&mut owner.container, owner_uuid)
}
