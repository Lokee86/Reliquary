//! Verified-copy maintenance and bounded history operations.

use crate::{MemoryId, Phylactery, ProductError, ProductErrorKind, Rel};
use reliquary_application as app;
use reliquary_owner_session::{GraphAdmission, VerifiedOwnerCopyPlan};
use serde::Serialize;
use std::path::Path;
use uuid::Uuid;

pub use reliquary_application::{
    ArchiveRewriteMode, DerivedStateRewriteMode, FreshnessRewriteMode, MaintenanceBlocker,
    MemoryHistoryLookup, MemoryHistoryRead, MemoryHistoryStatus, MemoryMaintenancePreparation,
    MemoryPruneCandidate, MemoryRewriteFilter, OwnerRewriteKind, RewriteIdentityMode, RewritePlan,
    RewritePolicy, RewriteSelection,
};
pub use reliquary_owner_session::{
    ArchivePruneReport, EntityPruneReport, FreshHistoryRewriteReport, ProfileRepairPlan,
    ProfileRepairReport, PrunedOwnerCopyReport, RelationshipPruneReport, RetiredProfilePruneReport,
    StorageReclamationReport,
};
pub use reliquary_perception_store::{EntityId, RelationshipId};
pub use reliquary_vector_store::CompatibilityProfileId;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReconciliationRelation {
    Identical,
    LeftExtendsRight,
    RightExtendsLeft,
    Diverged,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReconciliationMode {
    ExactCopy,
    SemanticReplay,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ReconciliationReport {
    pub owner_id: crate::OwnerId,
    pub relation: ReconciliationRelation,
    pub mode: ReconciliationMode,
    pub common_records: usize,
    pub left_records: usize,
    pub right_records: usize,
    pub replayed_archive_records: usize,
    pub replayed_memory_revisions: usize,
    pub destination_dream_admissions: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct HistoricalProofSummary {
    pub owner_id: crate::OwnerId,
    pub history_generation: String,
    pub digest: String,
    pub retained_records: usize,
    pub unavailable_payloads: usize,
}

/// Frozen byte/identity proof used by copy-only maintenance operations.
pub struct VerifiedCopyPlan {
    inner: VerifiedOwnerCopyPlan,
}
impl VerifiedCopyPlan {
    pub fn prepare(source: impl AsRef<Path>) -> Result<Self, ProductError> {
        VerifiedOwnerCopyPlan::prepare(source.as_ref())
            .map(|inner| Self { inner })
            .map_err(maintenance_error)
    }
    pub fn owner_id(&self) -> crate::OwnerId {
        crate::OwnerId::from_bytes(self.inner.owner_id())
    }
}

#[path = "maintenance_operations.rs"]
mod operations;
#[path = "maintenance_owner.rs"]
mod owner;
pub use operations::{
    execute_profile_repair, execute_rewrite, plan_profile_repair, prune_archive_copy,
    prune_entity_descriptions_copy, prune_memory_copy, prune_relationship_descriptions_copy,
    prune_retired_profiles_copy, reclaim_storage_copy, reconcile_rel,
};

fn read_owner_id(path: &Path) -> Result<[u8; 16], ProductError> {
    if let Ok(rel) = Rel::open(path) {
        return Ok(rel.owner_id().as_bytes());
    }
    Phylactery::open(path).map(|owner| owner.owner_id().as_bytes())
}
fn rewrite_error(error: app::RewriteError) -> ProductError {
    let kind = match error {
        app::RewriteError::InvalidPolicy(_) | app::RewriteError::UnknownMemory(_) => {
            ProductErrorKind::InvalidInput
        }
        app::RewriteError::Unsupported(_) => ProductErrorKind::Unsupported,
        app::RewriteError::SourceChanged => ProductErrorKind::Stale,
        app::RewriteError::OwnerRead(_) | app::RewriteError::SourceRead(_) => {
            ProductErrorKind::Verification
        }
    };
    ProductError::new(kind, error.to_string())
}
fn maintenance_error(error: impl ToString) -> ProductError {
    let message = error.to_string();
    let kind = if message.contains("already exists") {
        ProductErrorKind::PublicationCollision
    } else if message.contains("changed") || message.contains("stale") {
        ProductErrorKind::Stale
    } else if message.contains("unsupported") {
        ProductErrorKind::Unsupported
    } else {
        ProductErrorKind::Verification
    };
    ProductError::new(kind, message)
}
fn proof_summary(proof: reliquary_owner_session::HistoricalOwnerProof) -> HistoricalProofSummary {
    HistoricalProofSummary {
        owner_id: crate::OwnerId::from_bytes(proof.owner_id),
        history_generation: hex(&proof.history_generation),
        digest: hex(&proof.digest),
        retained_records: proof.retained_records,
        unavailable_payloads: proof.unavailable_payloads,
    }
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
