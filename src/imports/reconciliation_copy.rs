//! Import-owned semantic copy validation over the shared physical copy primitive.
use crate::owner_lifecycle::{ReliquaryOwner, import_epoch_transport};
use crate::{Container, CvaReconcileError};
use std::path::Path;

pub(super) fn open_read_only(path: &Path) -> Result<ReliquaryOwner, CvaReconcileError> {
    let container = import_epoch_transport::open_read_only_container(path)?;
    open_from_container(container)
}

pub(super) fn open_from_container(
    container: Container,
) -> Result<ReliquaryOwner, CvaReconcileError> {
    let mut replay = crate::owner_lifecycle::ReliquaryReplay::read(container)?;
    let reconstruction = crate::history::reconstruct_reliquary(&mut replay)?;
    let (owner, _freshness_due) = replay.into_owner(reconstruction);
    Ok(owner)
}

pub(super) fn copy_import_source(
    source: &Path,
    output: &Path,
) -> Result<ReliquaryOwner, CvaReconcileError> {
    let destination = crate::owner_lifecycle::physical_copy::copy(source, output)?;
    let owned =
        crate::owner_lifecycle::import_epoch_transport::hold_created_output_identity(&destination)?;
    let result = (|| {
        let source = open_read_only(source)?;
        let reopened = open_from_container(destination)?;
        validate_historical_import_copy(&source, &reopened)?;
        if !source.archive.activity_history_matches(&reopened.archive) {
            return Err(CvaReconcileError::Archive(
                crate::ArchiveError::CorruptRecord("copied REL activity history changed"),
            ));
        }
        Ok(reopened)
    })();
    if result.is_err() {
        crate::owner_lifecycle::import_epoch_transport::cleanup_created_output_after_failure(
            owned, output,
        );
    }
    result
}

fn validate_historical_import_copy(
    source: &ReliquaryOwner,
    copy: &ReliquaryOwner,
) -> Result<(), CvaReconcileError> {
    let source_access = source.history_read_access();
    let copy_access = copy.history_read_access();
    if !crate::history::historical_copy_preserves_import_authority(
        crate::history::HistoricalCopyView {
            archive: &source.archive,
            graph: &source.graph,
            freshness: &source.freshness,
            import_epochs: &source.import_epochs,
            read_access: &source_access,
            accepted_global_head: source.latest_global_version(),
        },
        crate::history::HistoricalCopyView {
            archive: &copy.archive,
            graph: &copy.graph,
            freshness: &copy.freshness,
            import_epochs: &copy.import_epochs,
            read_access: &copy_access,
            accepted_global_head: copy.latest_global_version(),
        },
    )? {
        return Err(crate::owner_lifecycle::OwnerError::Repack(
            "physical transformation changed historical checkpoint scoring or receipt/proof authority"
                .into(),
        )
        .into());
    }
    Ok(())
}
