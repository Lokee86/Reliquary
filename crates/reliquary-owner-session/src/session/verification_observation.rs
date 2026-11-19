use crate::{MemoryId, OwnerScope, OwnerSessionError, PhySession, RelSession};
use reliquary_container::{Container, FileKind};
use std::path::{Path, PathBuf};

mod archive;
pub use archive::{ArchiveCutObservation, ArchiveOwnerHistoryObservation, ArchiveTurnOrigin};

mod memory;
pub use memory::{
    MemoryHistoryCutObservation, MemoryOwnerHistoryObservation, MemoryPublicationOrigin,
};

mod freshness;
mod hash;
use hash::hash_open_container;
mod session;
use session::CurrentOwnerSession;

#[cfg(test)]
mod tests;

pub const MAX_MEMORY_ID_PAGE_COUNT: usize = 128;
pub const MAX_MEMORY_ID_PAGE_PAYLOAD_BYTES: usize = MAX_MEMORY_ID_PAGE_COUNT * 32;
pub const MAX_MEMORY_ID_CURSOR_BYTES: usize = 512;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CurrentOwnerKind {
    Rel,
    Phy,
}

impl CurrentOwnerKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Rel => "REL",
            Self::Phy => "PHY",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CurrentOwnerObservation {
    pub kind: CurrentOwnerKind,
    pub owner_id: [u8; 16],
    pub scope: OwnerScope,
    pub latest_allocated_global_version: u64,
    pub transaction_time_ns_at_latest_allocated_version: Option<i64>,
    pub memory_version: u64,
    pub history_generation: [u8; 32],
    pub file_sha256_before: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryIdPage {
    pub memory_version: u64,
    pub ids: Vec<MemoryId>,
    pub payload_bytes: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CurrentOwnerFinalization {
    pub file_sha256_after: [u8; 32],
    pub path_matches_open_file_after: bool,
    pub physical_file_identity_unchanged: bool,
    pub candidate_unchanged: bool,
}

/// One offline, current-only owner observation. It retains the original shared
/// read-only Container lease until finalize consumes the observer. The session
/// replay itself is not size-bounded; each Memory-ID page is separately capped.
pub struct ReadOnlyCurrentOwnerObserver {
    path: PathBuf,
    container: Container,
    physical_file_identity: (u64, u64),
    session: CurrentOwnerSession,
    observation: CurrentOwnerObservation,
}

impl ReadOnlyCurrentOwnerObserver {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, String> {
        let path = path.as_ref().to_path_buf();
        let container =
            Container::open_read_only(&path).map_err(|_| "candidate_open_failed".to_string())?;
        let physical_file_identity = container
            .physical_file_identity()
            .map_err(|_| "candidate_file_identity_failed".to_string())?;
        if !container
            .names_same_file(&path)
            .map_err(|_| "candidate_path_identity_before_failed".to_string())?
        {
            return Err("candidate_path_did_not_name_open_file".into());
        }
        let file_sha256_before = hash_open_container(&container)?;
        let identity = container.identity();
        let owner_id = container.owner_id();
        let latest_allocated_global_version = container.latest_version();
        let transaction_time_ns_at_latest_allocated_version =
            container.transaction_time_ns(latest_allocated_global_version);

        let (kind, scope, session) = match identity.file_kind {
            FileKind::Reliquary => {
                let session = RelSession::open_current(&container).map_err(open_error)?;
                let scope = session.scope();
                (
                    CurrentOwnerKind::Rel,
                    scope,
                    CurrentOwnerSession::Rel(session),
                )
            }
            FileKind::Phylactery => {
                let session = PhySession::open_current(&container).map_err(open_error)?;
                (
                    CurrentOwnerKind::Phy,
                    OwnerScope::Unspecified,
                    CurrentOwnerSession::Phy(session),
                )
            }
        };
        if !container
            .names_same_file(&path)
            .map_err(|_| "candidate_path_identity_during_open_failed".to_string())?
        {
            return Err("candidate_path_replaced_during_owner_open".into());
        }

        let observation = CurrentOwnerObservation {
            kind,
            owner_id,
            scope,
            latest_allocated_global_version,
            transaction_time_ns_at_latest_allocated_version,
            memory_version: session.memory_version(),
            history_generation: session.history_generation(),
            file_sha256_before,
        };
        Ok(Self {
            path,
            container,
            physical_file_identity,
            session,
            observation,
        })
    }

    pub fn observation(&self) -> &CurrentOwnerObservation {
        &self.observation
    }

    pub fn memory_ids_page(
        &mut self,
        after: Option<MemoryId>,
        limit: usize,
    ) -> Result<MemoryIdPage, String> {
        if !(1..=MAX_MEMORY_ID_PAGE_COUNT).contains(&limit) {
            return Err("memory_page_limit_out_of_range".into());
        }
        let (memory_version, ids) = self
            .session
            .memory_ids_page(after, limit)
            .map_err(|_| "memory_id_page_failed".to_string())?;
        if memory_version != self.observation.memory_version || ids.len() > limit {
            return Err("memory_page_cut_or_count_mismatch".into());
        }
        if ids.windows(2).any(|pair| pair[0] >= pair[1])
            || after.is_some_and(|after| ids.first().is_some_and(|first| *first <= after))
        {
            return Err("memory_page_order_mismatch".into());
        }
        let payload_bytes = ids
            .len()
            .checked_mul(32)
            .ok_or("memory_page_payload_size_overflow")?;
        if payload_bytes > MAX_MEMORY_ID_PAGE_PAYLOAD_BYTES {
            return Err("memory_page_payload_exceeded_limit".into());
        }
        Ok(MemoryIdPage {
            memory_version,
            ids,
            payload_bytes,
        })
    }

    /// Finalize after all requested pages. This hashes through the held
    /// Container handle and verifies that the path still names that same file.
    pub fn finalize(self) -> Result<CurrentOwnerFinalization, String> {
        self.finalize_after_hash(|| {})
    }

    fn finalize_after_hash(
        self,
        after_hash: impl FnOnce(),
    ) -> Result<CurrentOwnerFinalization, String> {
        let file_sha256_after = hash_open_container(&self.container)?;
        after_hash();
        let path_matches_open_file_after = self
            .container
            .names_same_file(&self.path)
            .map_err(|_| "candidate_path_identity_after_failed".to_string())?;
        let final_identity = self
            .container
            .physical_file_identity()
            .map_err(|_| "candidate_file_identity_after_failed".to_string())?;
        let physical_file_identity_unchanged = final_identity == self.physical_file_identity;
        let candidate_unchanged = path_matches_open_file_after
            && physical_file_identity_unchanged
            && file_sha256_after == self.observation.file_sha256_before;
        Ok(CurrentOwnerFinalization {
            file_sha256_after,
            path_matches_open_file_after,
            physical_file_identity_unchanged,
            candidate_unchanged,
        })
    }
}

fn open_error(error: OwnerSessionError) -> String {
    format!("current_only_open_rejected:{:?}", error.failure_kind())
}
