//! Public facade adaptation for concrete-owner reconciliation.
use crate::reconciliation_values::{CvaComparison, CvaReconcileResult};
use crate::{CvaReconcileError, RelExecutionOwner};
use std::path::Path;

pub(crate) fn reconcile_diverged(
    left_path: &Path,
    right_path: &Path,
    output_path: &Path,
    comparison: CvaComparison,
) -> Result<(CvaReconcileResult, RelExecutionOwner), CvaReconcileError> {
    let (result, owner) = crate::maintenance::reconciliation_repack::reconcile_diverged(
        left_path,
        right_path,
        output_path,
        comparison,
    )?;
    let output = crate::rel_lifecycle_execution::from_container(owner.container)?;
    Ok((result, output))
}
