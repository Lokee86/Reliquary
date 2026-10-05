//! Archive preconditions for coordinator-owned sequential finalization.
use super::{
    ConversationSubmissionId, FinalizeCapture, ReliquaryRuntimeHostError, Submission, operation,
};

pub(super) fn validate_sequential_parent(
    runtime: &crate::InteractionRuntime,
    conversation_id: &str,
    submission: &Submission,
    current_head: &str,
) -> Result<(), ReliquaryRuntimeHostError> {
    let expected = submission.request_metadata.get("expected_parent_node_id");
    let expected = match expected {
        Some(serde_json::Value::String(parent)) => Some(parent.as_str()),
        Some(serde_json::Value::Null) => None,
        _ => return Err(operation("durable expected parent is missing or malformed")),
    };
    // A null parent was admitted only into an empty conversation. Several such
    // queued turns may now follow the first committed root on the same branch.
    if let Some(expected) = expected {
        let ancestry = runtime
            .cva()
            .archive()
            .branch_nodes(conversation_id, current_head)
            .map_err(operation)?;
        if !ancestry.iter().any(|node| node.id == expected) {
            return Err(operation(
                "expected parent is unrelated to the selected branch",
            ));
        }
    }
    Ok(())
}

pub(super) fn validate_publication_head(
    runtime: &crate::InteractionRuntime,
    key: &ConversationSubmissionId,
    capture: &FinalizeCapture,
) -> Result<(), ReliquaryRuntimeHostError> {
    if let Some(branch) = runtime
        .cva()
        .archive()
        .branches()
        .into_iter()
        .find(|branch| {
            branch.conversation_id == key.conversation_id.as_str() && branch.id == capture.branch_id
        })
    {
        if Some(branch.leaf_node_id.as_str()) != capture.parent_node_id.as_deref()
            && branch.leaf_node_id != key.submission_id.as_str()
        {
            return Err(operation(
                "Archive branch advanced beyond the captured finalization",
            ));
        }
    }
    Ok(())
}
