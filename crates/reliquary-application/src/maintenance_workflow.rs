//! Prepare terminal Memory maintenance and publish verified snapshots.
//!
//! Candidate preparation is read-only. The owner proves payload eligibility,
//! validates the staged snapshot, and publishes it at the requested path.

use crate::{
    EntityOwnerSession, MemoryHistoryLookup, MemoryHistoryWorkflowError, read_owner_memory_history,
};
use reliquary_owner_session::model::{
    MemoryBodyId, MemoryHistory, MemoryId, PayloadState, memory_body_id,
};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, fmt};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MemoryMaintenancePreparation {
    Unavailable {
        owner_id: [u8; 16],
        memory_id: MemoryId,
    },
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
    let latest_revision = history
        .history
        .revisions
        .last()
        .map(|revision| revision.record.revision);
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
            if memory_body_id(&body.title, &body.content) != revision.record.body_id {
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
    let source_history_digest = digest_history(history.owner_id, memory_id, &history.history);
    Ok(MemoryMaintenancePreparation::Candidate(
        MemoryPruneCandidate {
            owner_id: history.owner_id,
            memory_id,
            tombstone_revision: tombstone.revision,
            tombstone_memory_version: tombstone.memory_version,
            source_history_digest,
            candidate_payloads: payloads.into_iter().collect(),
            requires_cross_domain_reference_audit: true,
            requires_verified_copy_and_reopen: true,
        },
    ))
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

fn digest_history(owner_id: [u8; 16], memory_id: MemoryId, history: &MemoryHistory) -> [u8; 32] {
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

use reliquary_owner_session::{
    GraphAdmission, PrunedOwnerCopyReport, VerifiedOwnerCopyPlan, promote_terminal_memory_copy,
    prune_terminal_memory_copy,
};
use std::path::Path;

/// Transform selected terminal Memory bodies into a smaller REL/PHY copy.
/// Candidate read preparation remains optional: the owner rechecks every
/// tombstone and surviving reference against its exact immutable source cut.
pub fn prune_verified_memory_copy(
    plan: &VerifiedOwnerCopyPlan,
    source_path: &Path,
    destination_path: &Path,
    authorized_owner_id: [u8; 16],
    selected: &[MemoryId],
    graph_admission: &GraphAdmission,
) -> Result<PrunedOwnerCopyReport, MaintenanceWorkflowError> {
    if plan.owner_id() != authorized_owner_id {
        return Err(MaintenanceWorkflowError::OwnerRead(
            "maintenance source is outside the authorized owner".into(),
        ));
    }
    prune_terminal_memory_copy(
        plan,
        source_path,
        destination_path,
        selected,
        graph_admission,
    )
    .map_err(MaintenanceWorkflowError::OwnerRead)
}

/// Replace an authorized offline owner after independent validation and retain
/// its original bytes at a new backup path.
pub fn promote_verified_memory_copy(
    plan: &VerifiedOwnerCopyPlan,
    source_path: &Path,
    backup_path: &Path,
    authorized_owner_id: [u8; 16],
    selected: &[MemoryId],
    graph_admission: &GraphAdmission,
) -> Result<PrunedOwnerCopyReport, MaintenanceWorkflowError> {
    if plan.owner_id() != authorized_owner_id {
        return Err(MaintenanceWorkflowError::OwnerRead(
            "maintenance source is outside the authorized owner".into(),
        ));
    }
    promote_terminal_memory_copy(plan, source_path, backup_path, selected, graph_admission)
        .map_err(MaintenanceWorkflowError::OwnerRead)
}

#[cfg(test)]
#[path = "maintenance_workflow/tests.rs"]
mod tests;
