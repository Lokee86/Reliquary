//! Explicit legacy terminal framing for offline migration fixtures only.
use super::*;

impl MemoryStore {
    pub(crate) fn tombstone_legacy_fixture(
        &mut self,
        container: &mut Container,
        id: MemoryId,
        expected_revision: u64,
    ) -> Result<crate::owner_lifecycle::TombstoneOutcome, MemoryError> {
        container.require_storage_format_writable()?;
        let index = *self.current.get(&id).ok_or(MemoryError::MissingMemory)?;
        if self.records[index].revision != expected_revision {
            return Err(MemoryError::RevisionConflict);
        }
        self.next_memory_version
            .checked_add(1)
            .ok_or(MemoryError::VersionExhausted)?;
        container
            .next_version_candidate()
            .checked_add(1)
            .ok_or(MemoryError::VersionExhausted)?;
        let terminal = MemoryTombstone {
            id,
            revision: expected_revision
                .checked_add(1)
                .ok_or(MemoryError::VersionExhausted)?,
        };
        let record = container.append(&crate::memory_codec::encode_tombstone(terminal))?;
        let global_version = container.allocate_version()?;
        let memory_version = self.next_memory_version;
        container.append(&encode_version(MemoryVersion {
            global_version,
            memory_version,
            record,
        }))?;
        self.insert_tombstone_rebuilt(terminal, global_version, memory_version)?;
        Ok(crate::owner_lifecycle::TombstoneOutcome::Published(
            crate::owner_lifecycle::TombstoneCoordinates {
                global_version: Some(global_version),
                domain_version: Some(memory_version),
            },
        ))
    }
}
