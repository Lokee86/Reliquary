//! Owner-qualified, read-only Memory history access.

use crate::EntityOwnerSession;
use reliquary_owner_session::model::{MemoryHistory, MemoryId, TerminalTombstone};
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryHistoryStatus {
    Available,
    Tombstoned(TerminalTombstone),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryHistoryRead {
    pub owner_id: [u8; 16],
    pub memory_id: MemoryId,
    pub status: MemoryHistoryStatus,
    /// Historical revisions retain the owner's explicit payload availability
    /// state; absent bodies are not reconstructed or guessed.
    pub history: MemoryHistory,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MemoryHistoryLookup {
    Unavailable {
        owner_id: [u8; 16],
        memory_id: MemoryId,
    },
    Found(MemoryHistoryRead),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MemoryHistoryWorkflowError {
    OwnerMismatch,
    CutBeyondCurrent,
    OwnerRead(String),
}

impl fmt::Display for MemoryHistoryWorkflowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OwnerMismatch => f.write_str("history owner does not match caller authorization"),
            Self::CutBeyondCurrent => {
                f.write_str("history cut exceeds the accepted Memory version")
            }
            Self::OwnerRead(error) => write!(f, "owner history read failed: {error}"),
        }
    }
}

/// Read revisions visible at an exact owner-local Memory version. The cut is
/// checked against the immutable session before lookup and cannot expose a
/// revision published later. Payloads pruned after the cut remain unavailable.
pub fn read_owner_memory_history_at_cut(
    owner: &EntityOwnerSession<'_>,
    authorized_owner_id: [u8; 16],
    memory_id: MemoryId,
    memory_version: u64,
) -> Result<MemoryHistoryLookup, MemoryHistoryWorkflowError> {
    let owner_id = owner.owner_id();
    if owner_id != authorized_owner_id {
        return Err(MemoryHistoryWorkflowError::OwnerMismatch);
    }
    if memory_version > owner.memory_version() {
        return Err(MemoryHistoryWorkflowError::CutBeyondCurrent);
    }
    let Some(history) = owner
        .memory_history_at_cut(memory_id, memory_version)
        .map_err(|error| MemoryHistoryWorkflowError::OwnerRead(error.to_string()))?
    else {
        return Ok(MemoryHistoryLookup::Unavailable {
            owner_id,
            memory_id,
        });
    };
    let status = match history.tombstone {
        Some(tombstone) => MemoryHistoryStatus::Tombstoned(tombstone),
        None => MemoryHistoryStatus::Available,
    };
    Ok(MemoryHistoryLookup::Found(MemoryHistoryRead {
        owner_id,
        memory_id,
        status,
        history,
    }))
}

impl std::error::Error for MemoryHistoryWorkflowError {}

/// Read revisions only from the selected immutable owner session. The owner ID
/// must come from an earlier authorization check; it is rechecked here but is
/// not itself an authentication credential. No pruning or rewrite is allowed.
pub fn read_owner_memory_history(
    owner: &EntityOwnerSession<'_>,
    authorized_owner_id: [u8; 16],
    memory_id: MemoryId,
) -> Result<MemoryHistoryLookup, MemoryHistoryWorkflowError> {
    let owner_id = owner.owner_id();
    if owner_id != authorized_owner_id {
        return Err(MemoryHistoryWorkflowError::OwnerMismatch);
    }
    let Some(history) = owner
        .memory_history(memory_id)
        .map_err(|error| MemoryHistoryWorkflowError::OwnerRead(error.to_string()))?
    else {
        return Ok(MemoryHistoryLookup::Unavailable {
            owner_id,
            memory_id,
        });
    };
    let status = match history.tombstone {
        Some(tombstone) => MemoryHistoryStatus::Tombstoned(tombstone),
        None => MemoryHistoryStatus::Available,
    };
    Ok(MemoryHistoryLookup::Found(MemoryHistoryRead {
        owner_id,
        memory_id,
        status,
        history,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::EntityOwnerSession;
    use reliquary_container::{Container, ContainerIdentity};
    use reliquary_memory_core::{MemoryDraft, MemoryStore, PayloadState};
    use reliquary_owner_session::{GraphAdmission, PhySession};
    use reliquary_vector_store::{CompatibilityProfileStore, MemoryVectorStore, PackedVectorStore};
    use std::{
        fs,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };

    static NEXT: AtomicU64 = AtomicU64::new(1);
    struct Temp(PathBuf);
    impl Temp {
        fn new() -> Self {
            let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".test-tmp");
            fs::create_dir_all(&root).unwrap();
            Self(root.join(format!(
                "history-workflow-{}-{}.phy",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            )))
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.0);
        }
    }

    fn draft(mutation: &str) -> MemoryDraft {
        MemoryDraft {
            title: "Historical title".into(),
            content: "Historical body remains separately marked.".into(),
            category: "fact".into(),
            memory_type: "project".into(),
            authority_kind: "direct".into(),
            temporal_status: "current".into(),
            scope: "project".into(),
            lifecycle_state: "knowledge".into(),
            archived: false,
            parent_id: None,
            superseded_by: None,
            source_node_id: None,
            content_source_conversation_id: None,
            content_source_node_id: None,
            grounding_source_conversation_id: None,
            grounding_source_node_id: None,
            source_episode_id: None,
            source_ref: None,
            source_time_ns: None,
            temporal_inference: None,
            mutation_id: mutation.into(),
            created_at_ns: 1,
            updated_at_ns: 1,
        }
    }

    fn owner() -> (Temp, PhySession, [u8; 16], MemoryId, MemoryId) {
        let path = Temp::new();
        let owner_id = [109; 16];
        let mut container =
            Container::create(&path.0, ContainerIdentity::phylactery(), owner_id).unwrap();
        PackedVectorStore::default()
            .initialize(&mut container)
            .unwrap();
        CompatibilityProfileStore::default()
            .initialize(&mut container)
            .unwrap();
        MemoryVectorStore::default()
            .initialize(&mut container)
            .unwrap();
        let mut memories = MemoryStore::create(&mut container).unwrap();
        let available = memories
            .publish(&mut container, None, 0, draft("history-available"))
            .unwrap()
            .revision
            .record
            .id;
        let tombstoned = memories
            .publish(&mut container, None, 0, draft("history-tombstoned"))
            .unwrap()
            .revision
            .record
            .id;
        let _tombstone = memories.tombstone(&mut container, tombstoned, 1).unwrap();
        container.sync().unwrap();
        drop(memories);
        drop(container);
        let admission = GraphAdmission::for_replay(owner_id, [], [], [], [], []);
        let session = PhySession::open_path(&path.0, &admission, []).unwrap();
        (path, session, owner_id, available, tombstoned)
    }

    #[test]
    fn distinguishes_unavailable_live_and_tombstoned_history_owner_locally() {
        let (_path, owner, owner_id, available, tombstoned) = owner();
        let live = read_owner_memory_history(&EntityOwnerSession::Phy(&owner), owner_id, available)
            .unwrap();
        let MemoryHistoryLookup::Found(live) = live else {
            panic!("expected history");
        };
        assert_eq!(live.status, MemoryHistoryStatus::Available);
        assert_eq!(live.history.revisions.len(), 1);
        assert!(matches!(
            live.history.revisions[0].payload,
            PayloadState::Available(_)
        ));
        assert!(matches!(
            read_owner_memory_history_at_cut(
                &EntityOwnerSession::Phy(&owner),
                owner_id,
                available,
                0,
            )
            .unwrap(),
            MemoryHistoryLookup::Unavailable { .. }
        ));
        assert!(matches!(
            read_owner_memory_history_at_cut(
                &EntityOwnerSession::Phy(&owner),
                owner_id,
                available,
                u64::MAX,
            ),
            Err(MemoryHistoryWorkflowError::CutBeyondCurrent)
        ));

        let deleted =
            read_owner_memory_history(&EntityOwnerSession::Phy(&owner), owner_id, tombstoned)
                .unwrap();
        let MemoryHistoryLookup::Found(deleted) = deleted else {
            panic!("expected retained tombstone history");
        };
        assert!(matches!(deleted.status, MemoryHistoryStatus::Tombstoned(_)));
        assert_eq!(deleted.history.revisions.len(), 1);
        let historical = read_owner_memory_history_at_cut(
            &EntityOwnerSession::Phy(&owner),
            owner_id,
            tombstoned,
            2,
        )
        .unwrap();
        let MemoryHistoryLookup::Found(historical) = historical else {
            panic!("expected pre-tombstone history");
        };
        assert_eq!(historical.status, MemoryHistoryStatus::Available);
        assert!(historical.history.tombstone.is_none());

        assert_eq!(
            read_owner_memory_history(
                &EntityOwnerSession::Phy(&owner),
                owner_id,
                MemoryId([255; 32]),
            )
            .unwrap(),
            MemoryHistoryLookup::Unavailable {
                owner_id,
                memory_id: MemoryId([255; 32]),
            }
        );
        assert_eq!(
            read_owner_memory_history(&EntityOwnerSession::Phy(&owner), [110; 16], available,),
            Err(MemoryHistoryWorkflowError::OwnerMismatch)
        );
    }
}
