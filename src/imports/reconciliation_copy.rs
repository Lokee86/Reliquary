//! Import-owned semantic copy validation over the shared physical copy primitive.
use crate::{Cva, CvaReconcileError};
use std::path::Path;
pub(super) fn copy_import_source(source: &Path, output: &Path) -> Result<Cva, CvaReconcileError> {
    let destination = crate::owner_lifecycle::physical_copy::copy(source, output)?;
    let owned =
        crate::owner_lifecycle::import_epoch_transport::hold_created_output_identity(&destination)?;
    let result = (|| {
        let source = Cva::open_read_only(source)?;
        let reopened = Cva::open_from_container(destination)?;
        source.validate_historical_import_copy(&reopened)?;
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
