//! Archive preconditions and target selection for coordinator-owned finalization.
use super::{
    ConversationSubmissionId, FinalizeCapture, ReliquaryRuntimeHostError, Submission,
    metadata_branch, operation,
};
use crate::ConversationPolicy;
use sha2::{Digest, Sha256};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct FinalizationTarget {
    pub branch_id: String,
    pub branch_label: Option<String>,
    pub parent_node_id: Option<String>,
}

pub(super) fn expected_parent(
    submission: &Submission,
) -> Result<Option<String>, ReliquaryRuntimeHostError> {
    match submission.request_metadata.get("expected_parent_node_id") {
        Some(serde_json::Value::String(parent)) => Ok(Some(parent.clone())),
        Some(serde_json::Value::Null) => Ok(None),
        _ => Err(operation("durable expected parent is missing or malformed")),
    }
}

pub(super) fn automatic_branch_identity(
    submission: &Submission,
) -> Result<(String, String), ReliquaryRuntimeHostError> {
    let original_branch = metadata_branch(submission);
    let expected = expected_parent(submission)?;
    let mut digest = Sha256::new();
    for component in [
        submission.id.conversation_id.as_str(),
        original_branch.as_str(),
        submission.id.submission_id.as_str(),
        expected.as_deref().unwrap_or("<root>"),
    ] {
        digest.update((component.len() as u64).to_le_bytes());
        digest.update(component.as_bytes());
    }
    let hex = digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    Ok((
        format!("auto-{}", &hex[..24]),
        format!("{original_branch} conflict {}", &hex[..8]),
    ))
}

pub(super) fn finalization_target(
    runtime: &crate::InteractionRuntime,
    conversation_id: &str,
    submission: &Submission,
    policy: ConversationPolicy,
) -> Result<FinalizationTarget, ReliquaryRuntimeHostError> {
    let submitted_branch = metadata_branch(submission);
    let expected = expected_parent(submission)?;
    let branches = runtime.cva().archive().branches();

    if let Some(branch) = branches
        .iter()
        .find(|branch| branch.conversation_id == conversation_id && branch.id == submitted_branch)
    {
        if let Some(expected_parent) = expected.as_deref() {
            let ancestry = runtime
                .cva()
                .archive()
                .branch_nodes(conversation_id, &branch.leaf_node_id)
                .map_err(operation)?;
            if !ancestry.iter().any(|node| node.id == expected_parent) {
                return Err(operation(
                    "expected parent is unrelated to the selected branch",
                ));
            }
        }

        if policy == ConversationPolicy::VisibleAutoBranch
            && expected.as_deref() != Some(branch.leaf_node_id.as_str())
        {
            let (branch_id, branch_label) = automatic_branch_identity(submission)?;
            if let Some(existing) = branches.iter().find(|existing| {
                existing.conversation_id == conversation_id && existing.id == branch_id
            }) {
                if existing.leaf_node_id != submission.id.submission_id.as_str() {
                    return Err(operation(
                        "deterministic automatic branch ID is already occupied",
                    ));
                }
            }
            return Ok(FinalizationTarget {
                branch_id,
                branch_label: Some(branch_label),
                parent_node_id: expected,
            });
        }

        return Ok(FinalizationTarget {
            branch_id: submitted_branch,
            branch_label: None,
            parent_node_id: Some(branch.leaf_node_id.clone()),
        });
    }

    let leaves = runtime
        .conversation_summaries()
        .into_iter()
        .find(|summary| summary.conversation_id == conversation_id)
        .map(|summary| summary.leaf_node_ids)
        .unwrap_or_default();

    let parent_node_id = match (policy, expected) {
        (ConversationPolicy::VisibleAutoBranch, Some(parent))
            if runtime.cva().archive().has_node(conversation_id, &parent) =>
        {
            Some(parent)
        }
        (ConversationPolicy::VisibleAutoBranch, Some(_)) => {
            return Err(operation("expected Archive parent is unrelated"));
        }
        (ConversationPolicy::VisibleAutoBranch, None) if leaves.is_empty() => None,
        (ConversationPolicy::VisibleAutoBranch, None) => {
            let (branch_id, branch_label) = automatic_branch_identity(submission)?;
            return Ok(FinalizationTarget {
                branch_id,
                branch_label: Some(branch_label),
                parent_node_id: None,
            });
        }
        (ConversationPolicy::SequentialChronological, Some(parent)) if leaves.contains(&parent) => {
            Some(parent)
        }
        (ConversationPolicy::SequentialChronological, Some(_)) => {
            return Err(operation("expected Archive parent is no longer a leaf"));
        }
        (ConversationPolicy::SequentialChronological, None) if leaves.is_empty() => None,
        (ConversationPolicy::SequentialChronological, None) if leaves.len() == 1 => {
            Some(leaves[0].clone())
        }
        (ConversationPolicy::SequentialChronological, None) => {
            return Err(operation(
                "conversation has multiple leaves; select one expected parent",
            ));
        }
    };

    Ok(FinalizationTarget {
        branch_id: submitted_branch,
        branch_label: None,
        parent_node_id,
    })
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
