//! Supported public reconciliation operations over canonical runtime composition.
pub use crate::reconciliation_values::{CvaComparison, CvaReconcileResult, CvaRelation};
use crate::{Cva, CvaReconcileError};
use std::path::Path;
impl Cva {
    pub fn compare(
        left: impl AsRef<Path>,
        right: impl AsRef<Path>,
    ) -> Result<CvaComparison, CvaReconcileError> {
        Self::native_compare(left, right)
    }
    pub fn reconcile(
        left: impl AsRef<Path>,
        right: impl AsRef<Path>,
        output: impl AsRef<Path>,
    ) -> Result<CvaReconcileResult, CvaReconcileError> {
        Self::native_reconcile(left, right, output)
    }
    pub(crate) fn reconcile_held(
        left: impl AsRef<Path>,
        right: impl AsRef<Path>,
        output: impl AsRef<Path>,
    ) -> Result<(CvaReconcileResult, Cva), CvaReconcileError> {
        Self::native_reconcile_held(left, right, output)
    }
}
