//! Public facade adaptation for concrete-owner reconciliation.
use crate::cva_reconcile::{CvaComparison, CvaReconcileResult};
use crate::{Cva, CvaReconcileError};
use std::path::Path;

pub(crate) fn reconcile_diverged(
    left_path: &Path,
    right_path: &Path,
    output_path: &Path,
    comparison: CvaComparison,
) -> Result<(CvaReconcileResult, Cva), CvaReconcileError> {
    let (result, owner) = crate::maintenance::reconciliation_repack::reconcile_diverged(
        left_path,
        right_path,
        output_path,
        comparison,
    )?;
    let output = Cva::open_from_container(owner.container)?;
    Ok((result, output))
}
