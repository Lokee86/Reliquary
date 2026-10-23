//! Generic copy validation after a physical copy has acquired its own identity.
use crate::owner_lifecycle::{HistoryReadAccess, OwnerError, ReliquaryOwner};
use crate::{ArchiveError, CvaReconcileError};
use std::{fs, path::Path};
pub(crate) fn copy_and_validate(
    source_path: &Path,
    output_path: &Path,
) -> Result<ReliquaryOwner, CvaReconcileError> {
    let destination = crate::owner_lifecycle::physical_copy::copy(source_path, output_path)?;
    let output_guard = destination.read_source()?;
    let validated = (|| {
        let source =
            super::rewrite::owner_access::open_rel(crate::Container::open_read_only(source_path)?)?;
        let reopened = super::rewrite::owner_access::open_rel(destination)?;
        let source_access = HistoryReadAccess::new(&source.container);
        let copy_access = HistoryReadAccess::new(&reopened.container);
        if !crate::history::historical_copy_preserves_import_authority(
            crate::history::HistoricalCopyView {
                archive: &source.archive,
                graph: &source.graph,
                freshness: &source.freshness,
                import_epochs: &source.import_epochs,
                read_access: &source_access,
                accepted_global_head: source.container.latest_version(),
            },
            crate::history::HistoricalCopyView {
                archive: &reopened.archive,
                graph: &reopened.graph,
                freshness: &reopened.freshness,
                import_epochs: &reopened.import_epochs,
                read_access: &copy_access,
                accepted_global_head: reopened.container.latest_version(),
            },
        )? {
            return Err(OwnerError::Repack("physical transformation changed historical checkpoint scoring or receipt/proof authority".into()).into());
        }
        if !source.archive.activity_history_matches(&reopened.archive) {
            return Err(CvaReconcileError::Archive(ArchiveError::CorruptRecord(
                "copied REL activity history changed",
            )));
        }
        Ok(reopened)
    })();
    if validated.is_err() && output_guard.validate_path_identity().is_ok() {
        let _ = fs::remove_file(output_path);
    }
    validated
}
