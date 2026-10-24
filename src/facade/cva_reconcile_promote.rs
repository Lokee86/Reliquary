//! Public orchestration over the canonical offline promotion operation.
use crate::{Cva, CvaReconcileError, CvaReconcileResult};
use std::path::Path;
impl Cva {
    pub fn reconcile_and_promote(
        canonical_path: impl AsRef<Path>,
        conflicted_path: impl AsRef<Path>,
    ) -> Result<CvaReconcileResult, CvaReconcileError> {
        let canonical = canonical_path.as_ref();
        let conflicted = conflicted_path.as_ref();
        let promotion = crate::maintenance::reconciliation_promotion::PromotionSource::prepare(
            canonical, conflicted,
        )?;
        let (result, candidate) =
            Self::reconcile_held(canonical, conflicted, promotion.candidate_path())?;
        promotion.promote(result, candidate.into_owner())
    }
}
