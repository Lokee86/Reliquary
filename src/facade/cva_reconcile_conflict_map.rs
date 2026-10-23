use crate::cva_reconcile_archive::ArchiveReplayRecord;
use crate::{ArchiveError, Cva, CvaReconcileError};

pub(crate) fn archive_replay_error(
    destination: &Cva,
    record: &ArchiveReplayRecord,
    error: ArchiveError,
) -> CvaReconcileError {
    crate::owner_lifecycle::archive_replay_error::archive_replay_error(
        &destination.archive,
        record,
        error,
    )
}
