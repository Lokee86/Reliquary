use crate::{MemoryId, OwnerSessionError, PhySession, RelSession};

pub(super) enum CurrentOwnerSession {
    Rel(RelSession),
    Phy(PhySession),
}

impl CurrentOwnerSession {
    pub(super) fn memory_version(&self) -> u64 {
        match self {
            Self::Rel(session) => session.memory_version(),
            Self::Phy(session) => session.memory_version(),
        }
    }

    pub(super) fn history_generation(&self) -> [u8; 32] {
        match self {
            Self::Rel(session) => session.verification_history_generation(),
            Self::Phy(session) => session.verification_history_generation(),
        }
    }

    pub(super) fn memory_ids_page(
        &self,
        after: Option<MemoryId>,
        limit: usize,
    ) -> Result<(u64, Vec<MemoryId>), OwnerSessionError> {
        match self {
            Self::Rel(session) => session.memory_ids_page(after, limit),
            Self::Phy(session) => session.memory_ids_page(after, limit),
        }
    }
}
