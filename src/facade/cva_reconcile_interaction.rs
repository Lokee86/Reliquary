use crate::{Cva, CvaError, CvaReconcileError, InteractionStreamRecord};

pub(crate) fn merge_interaction_streams(
    left: Vec<InteractionStreamRecord>,
    right: Vec<InteractionStreamRecord>,
) -> Result<(Vec<InteractionStreamRecord>, bool), CvaReconcileError> {
    crate::interaction_stream_reconciliation::merge_interaction_streams(left, right)
        .map_err(CvaError::from)
        .map_err(CvaReconcileError::from)
}

pub(crate) fn replay_interaction_streams(
    output: &mut Cva,
    records: Vec<InteractionStreamRecord>,
) -> Result<(), CvaReconcileError> {
    crate::interaction_stream_reconciliation::replay_interaction_streams(
        &mut output.interaction_streams,
        &mut output.container,
        records,
    )
    .map_err(CvaError::from)
    .map_err(CvaReconcileError::from)
}
