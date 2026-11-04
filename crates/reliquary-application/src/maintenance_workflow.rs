//! Read-only preparation for terminal Memory history maintenance.
//!
//! This deliberately stops before a prune manifest or transformed copy: the
//! current application layer cannot prove cross-domain references or reopen an
//! independently transformed owner copy.

use crate::{
    EntityOwnerSession, MemoryHistoryLookup, MemoryHistoryWorkflowError,
    read_owner_memory_history,
};
use reliquary_memory_core::{MemoryBodyId, MemoryId, PayloadState};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, fmt};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MemoryMaintenancePreparation {
    Unavailable { owner_id: [u8; 16], memory_id: MemoryId },
    Blocked {
        owner_id: [u8; 16],
        memory_id: MemoryId,
        reason: MaintenanceBlocker,
    },
    Candidate(MemoryPruneCandidate),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MaintenanceBlocker {
    NotTerminallyTombstoned,
    NoRetainedPayloadCandidates,
}

/// A candidate records exact owner-local history and payload identities. It
/// is not proof that removal is safe: Archive, Graph, vector, and other
/// surviving references still require owner-domain validation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryPruneCandidate {
    pub owner_id: [u8; 16],
    pub memory_id: MemoryId,
    pub tombstone_revision: u64,
    pub tombstone_memory_version: u64,
    pub source_history_digest: [u8; 32],
    pub candidate_payloads: Vec<MemoryBodyId>,
    pub requires_cross_domain_reference_audit: bool,
    pub requires_verified_copy_and_reopen: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MaintenanceWorkflowError {
    History(MemoryHistoryWorkflowError),
    OwnerRead(String),
}

impl fmt::Display for MaintenanceWorkflowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::History(error) => write!(f, "history read failed: {error}"),
            Self::OwnerRead(error) => write!(f, "owner history recheck failed: {error}"),
        }
    }
}

impl std::error::Error for MaintenanceWorkflowError {}

/// Prepare a conservative candidate from an authorized immutable owner
/// snapshot. This performs no writes and does not authorize destructive use.
pub fn prepare_tombstoned_memory_maintenance(
    owner: &EntityOwnerSession<'_>,
    authorized_owner_id: [u8; 16],
    memory_id: MemoryId,
) -> Result<MemoryMaintenancePreparation, MaintenanceWorkflowError> {
    let lookup = read_owner_memory_history(owner, authorized_owner_id, memory_id)
        .map_err(MaintenanceWorkflowError::History)?;
    let MemoryHistoryLookup::Found(history) = lookup else {
        return Ok(MemoryMaintenancePreparation::Unavailable {
            owner_id: authorized_owner_id,
            memory_id,
        });
    };
    let Some(tombstone) = history.history.tombstone else {
        return Ok(MemoryMaintenancePreparation::Blocked {
            owner_id: history.owner_id,
            memory_id,
            reason: MaintenanceBlocker::NotTerminallyTombstoned,
        });
    };
    let latest_revision = history.history.revisions.last().map(|revision| revision.record.revision);
    // Tombstones are separate terminal records, one revision after the last body revision.
    if latest_revision.and_then(|revision| revision.checked_add(1)) != Some(tombstone.revision) {
        return Ok(MemoryMaintenancePreparation::Blocked {
            owner_id: history.owner_id,
            memory_id,
            reason: MaintenanceBlocker::NotTerminallyTombstoned,
        });
    }
    let mut payloads = BTreeSet::new();
    for revision in &history.history.revisions {
        if let PayloadState::Available(body) = &revision.payload {
            if reliquary_memory_core::memory_body_id(&body.title, &body.content)
                != revision.record.body_id
            {
                return Err(MaintenanceWorkflowError::OwnerRead(
                    "Memory body identity failed validation".into(),
                ));
            }
            payloads.insert(revision.record.body_id);
        }
    }
    if payloads.is_empty() {
        return Ok(MemoryMaintenancePreparation::Blocked {
            owner_id: history.owner_id,
            memory_id,
            reason: MaintenanceBlocker::NoRetainedPayloadCandidates,
        });
    }
    let source_history_digest = digest_history(
        history.owner_id,
        memory_id,
        &history.history,
    );
    Ok(MemoryMaintenancePreparation::Candidate(MemoryPruneCandidate {
        owner_id: history.owner_id,
        memory_id,
        tombstone_revision: tombstone.revision,
        tombstone_memory_version: tombstone.memory_version,
        source_history_digest,
        candidate_payloads: payloads.into_iter().collect(),
        requires_cross_domain_reference_audit: true,
        requires_verified_copy_and_reopen: true,
    }))
}

/// Recheck a candidate against a newly authorized immutable owner snapshot.
/// A successful result only confirms history identity; it is not prune
/// authorization and must be followed by a verified-copy workflow.
pub fn recheck_tombstoned_memory_maintenance(
    owner: &EntityOwnerSession<'_>,
    authorized_owner_id: [u8; 16],
    candidate: &MemoryPruneCandidate,
) -> Result<bool, MaintenanceWorkflowError> {
    if authorized_owner_id != candidate.owner_id || owner.owner_id() != candidate.owner_id {
        return Ok(false);
    }
    match prepare_tombstoned_memory_maintenance(owner, authorized_owner_id, candidate.memory_id)? {
        MemoryMaintenancePreparation::Candidate(current) => Ok(current == *candidate),
        _ => Ok(false),
    }
}

fn digest_history(
    owner_id: [u8; 16],
    memory_id: MemoryId,
    history: &reliquary_memory_core::MemoryHistory,
) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"reliquary-memory-maintenance-history-v1\0");
    hash.update(owner_id);
    hash.update(memory_id.0);
    hash.update((history.revisions.len() as u64).to_le_bytes());
    for revision in &history.revisions {
        hash.update(revision.record.revision.to_le_bytes());
        hash.update(revision.record.body_id.0);
        hash.update(revision.record.memory_version.to_le_bytes());
        match &revision.payload {
            PayloadState::Absent => hash.update([0]),
            PayloadState::Available(body) => {
                hash.update([1]);
                hash.update((body.title.len() as u64).to_le_bytes());
                hash.update(body.title.as_bytes());
                hash.update((body.content.len() as u64).to_le_bytes());
                hash.update(body.content.as_bytes());
            }
        }
    }
    match history.tombstone {
        Some(tombstone) => {
            hash.update([1]);
            hash.update(tombstone.revision.to_le_bytes());
            hash.update(tombstone.memory_version.to_le_bytes());
        }
        None => hash.update([0]),
    }
    hash.finalize().into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use reliquary_container::{Container, ContainerIdentity};
    use reliquary_memory_core::{MemoryDraft, MemoryStore};
    use reliquary_owner_session::{GraphAdmission, PhySession};
    use reliquary_vector_store::{CompatibilityProfileStore, MemoryVectorStore, PackedVectorStore};
    use std::{fs, path::PathBuf, sync::atomic::{AtomicU64, Ordering}};

    static NEXT: AtomicU64 = AtomicU64::new(1);
    struct Temp(PathBuf);
    impl Temp {
        fn new() -> Self {
            let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".test-tmp");
            fs::create_dir_all(&root).unwrap();
            Self(root.join(format!("maintenance-workflow-{}-{}.phy", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed))))
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) { let _ = fs::remove_file(&self.0); }
    }

    fn memory(mutation: &str) -> MemoryDraft {
        MemoryDraft {
            title: "old title".into(), content: "old content".into(), category: "fact".into(),
            memory_type: "project".into(), authority_kind: "direct".into(), temporal_status: "past".into(),
            scope: "project".into(), lifecycle_state: "knowledge".into(), archived: false,
            parent_id: None, superseded_by: None, source_node_id: None,
            content_source_conversation_id: None, content_source_node_id: None,
            grounding_source_conversation_id: None, grounding_source_node_id: None,
            source_episode_id: None, source_ref: None, source_time_ns: None, temporal_inference: None,
            mutation_id: mutation.into(), created_at_ns: 1, updated_at_ns: 1,
        }
    }

    #[test]
    fn terminal_prune_candidate_is_read_only_and_requires_reference_audit_and_copy_reopen() {
        let path = Temp::new();
        let owner_id = [74; 16];
        let mut container = Container::create(&path.0, ContainerIdentity::phylactery(), owner_id).unwrap();
        PackedVectorStore::default().initialize(&mut container).unwrap();
        CompatibilityProfileStore::default().initialize(&mut container).unwrap();
        MemoryVectorStore::default().initialize(&mut container).unwrap();
        let mut memories = MemoryStore::create(&mut container).unwrap();
        let live = memories.publish(&mut container, None, 0, memory("maint-live")).unwrap().revision.record.id;
        let deleted = memories.publish(&mut container, None, 0, memory("maint-deleted")).unwrap().revision.record.id;
        memories.tombstone(&mut container, deleted, 1).unwrap();
        container.sync().unwrap();
        drop(memories);
        drop(container);

        let admission = GraphAdmission::for_replay(owner_id, [], [], [], [], []);
        let session = PhySession::open_path(&path.0, &admission, []).unwrap();
        let owner = EntityOwnerSession::Phy(&session);
        assert!(matches!(
            prepare_tombstoned_memory_maintenance(&owner, owner_id, live).unwrap(),
            MemoryMaintenancePreparation::Blocked { reason: MaintenanceBlocker::NotTerminallyTombstoned, .. }
        ));
        let MemoryMaintenancePreparation::Candidate(candidate) =
            prepare_tombstoned_memory_maintenance(&owner, owner_id, deleted).unwrap()
        else { panic!("expected candidate") };
        assert_eq!(candidate.candidate_payloads.len(), 1);
        assert!(candidate.requires_cross_domain_reference_audit);
        assert!(candidate.requires_verified_copy_and_reopen);
        assert!(recheck_tombstoned_memory_maintenance(&owner, owner_id, &candidate).unwrap());
        assert!(!recheck_tombstoned_memory_maintenance(&owner, [75; 16], &candidate).unwrap());
        assert_eq!(session.memory_history(deleted).unwrap().unwrap().revisions.len(), 1);
        assert!(matches!(
            session.memory_history(deleted).unwrap().unwrap().revisions[0].payload,
            PayloadState::Available(_)
        ));
    }
}
