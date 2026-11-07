//! Validation-only commitments for accepted repository correlations and file bindings.
use crate::historical_fingerprint::{HistoricalFingerprint, OwnerProjection};
use crate::{Cva, ProjectRepositoryKind, ProjectRepositoryManagement, ProjectRevisionRef};
use serde_json::{Value, json};

fn revision_value(revision: &ProjectRevisionRef) -> Value {
    let repository = &revision.repository;
    let kind = match repository.kind {
        ProjectRepositoryKind::Lore => "lore",
        ProjectRepositoryKind::Git => "git",
    };
    json!({
        "kind": kind,
        "repository_id": repository.repository_id,
        "project_path": repository.project_path,
        "revision": revision.revision,
    })
}

pub(super) fn observe(owner: &Cva) -> Result<HistoricalFingerprint, String> {
    let owner_id = owner
        .owner_uuid()
        .ok_or("project witness needs owner UUID")?;
    // Both tables replay inside the historical native owner. Neither requires
    // opening a referenced repository or resolving its contents on the host.
    let mut projection = OwnerProjection::new(owner_id, "project_operational", 0, None)?;
    for record in owner.project_revision_correlations() {
        let management = match record.repository_management {
            ProjectRepositoryManagement::WarlockManaged => "warlock_managed",
            ProjectRepositoryManagement::External => "external",
        };
        let cut = record.rel_cut;
        if cut.global_version > owner.latest_global_version()
            || cut.archive_version > owner.archive_version()
            || cut.memory_version > owner.memory_version()
        {
            return Err("project revision correlation exceeds a retained owner head".into());
        }
        projection.insert(
            "project_revision_correlation",
            record.sequence.to_be_bytes(),
            json!({
                "sequence": record.sequence,
                "global_version": cut.global_version,
                "archive_version": cut.archive_version,
                "memory_version": cut.memory_version,
                "repository_management": management,
                "project_revision": revision_value(&record.project_revision),
            }),
        )?;
    }
    for (file_id, reference) in owner.project_files.witness_bindings() {
        projection.insert(
            "project_file_binding",
            file_id.0,
            json!({
                "file_id": file_id.0,
                "project_revision": revision_value(&reference.revision),
                "path": reference.path,
                "content_hash": reference.content_hash,
            }),
        )?;
    }
    projection.fingerprint()
}
