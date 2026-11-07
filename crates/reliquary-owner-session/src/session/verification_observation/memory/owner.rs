use super::origins::{MemoryPublicationOrigin, all_memory_origins};
use super::{CurrentOwnerSession, ReadOnlyCurrentOwnerObserver};
use reliquary_memory_core::{MemoryHistory, MemoryId};
use std::collections::BTreeMap;

pub struct MemoryOwnerHistoryObservation {
    pub memory_head: u64,
    pub histories: BTreeMap<MemoryId, MemoryHistory>,
    pub publication_origins: BTreeMap<u64, (MemoryId, MemoryPublicationOrigin)>,
}

impl ReadOnlyCurrentOwnerObserver {
    /// Offline retained Memory census of an owner that has passed current-only
    /// replay, including terminal IDs omitted by current-ID pagination.
    pub fn memory_owner_history(&self) -> Result<MemoryOwnerHistoryObservation, String> {
        let memories = match &self.session {
            CurrentOwnerSession::Rel(session) => &session.common.memories,
            CurrentOwnerSession::Phy(session) => &session.common.memories,
        };
        let histories = memories
            .memory_ids()
            .map(|id| {
                memories
                    .history(id)
                    .map(|history| (id, history))
                    .map_err(|_| "memory_owner_history_unavailable")
            })
            .collect::<Result<_, _>>()?;
        Ok(MemoryOwnerHistoryObservation {
            memory_head: self.observation.memory_version,
            histories,
            publication_origins: all_memory_origins(&self.container)?,
        })
    }
}
