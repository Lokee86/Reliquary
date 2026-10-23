//! CVA adapters for owner-local Freshness reconciliation.

use crate::{Cva, CvaReconcileError};

fn source_view(cva: &Cva) -> crate::history::freshness_reconciliation::FreshnessReplaySource<'_> {
    crate::history::freshness_reconciliation::FreshnessReplaySource {
        owner_uuid: cva.owner_uuid(),
        archive: &cva.archive,
        freshness: &cva.freshness,
        dream_freshness: &cva.dream_freshness,
        graph: &cva.graph,
        communities: &cva.communities,
    }
}

pub(crate) fn replay_freshness(
    left: &Cva,
    right: &Cva,
    output: &mut Cva,
) -> Result<(), CvaReconcileError> {
    let owner_uuid = output.owner_uuid();
    crate::maintenance::freshness_reconciliation::replay_freshness(
        &source_view(left),
        &source_view(right),
        crate::maintenance::freshness_reconciliation::FreshnessReplayDestination {
            container: &mut output.container,
            archive: &output.archive,
            memories: &output.memories,
            owner_uuid,
            freshness: &mut output.freshness,
            dream_freshness: &mut output.dream_freshness,
        },
    )
}

pub(crate) fn validate_freshness(left: &Cva, right: &Cva) -> Result<(), CvaReconcileError> {
    crate::maintenance::freshness_reconciliation::validate_freshness(
        &source_view(left),
        &source_view(right),
    )
}

pub(crate) fn right_changes_freshness(left: &Cva, right: &Cva) -> Result<bool, CvaReconcileError> {
    crate::maintenance::freshness_reconciliation::right_changes_freshness(
        &source_view(left),
        &source_view(right),
    )
}
