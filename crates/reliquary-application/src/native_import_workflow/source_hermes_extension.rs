//! Bind an authorized copied suffix to its immutable completed predecessor.
use super::*;
use reliquary_owner_session::records::{ImportEpochStatus, import_id};

pub(in crate::native_import_source) fn attach_predecessor(
    session: &RelSession,
    scope: &HermesCopyScope,
    namespace: &str,
    predecessor: &[u8; 32],
    retry: bool,
    turns: &mut [(TurnInput, Branch)],
) -> Result<(), NativeImportStageError> {
    let predecessor_key = predecessor
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let id = import_id(session.owner_id(), &predecessor_key);
    if !matches!(
        session.import_status(&id),
        Some(ImportEpochStatus::Completed { .. })
    ) {
        return Err(NativeImportStageError::Conflict);
    }
    let prior = scope
        .accepted_conversation(session, predecessor, namespace)
        .ok_or(NativeImportStageError::Conflict)?;
    let parent = prior
        .messages
        .last()
        .and_then(|message| message.node_id.as_deref())
        .ok_or(NativeImportStageError::Conflict)?;
    let (_, branch) = turns.first().ok_or(NativeImportStageError::Conflict)?;
    let same_branch = match prior.branches.as_slice() {
        [] => branch.id == copied_branch_id(None),
        [previous] => {
            branch.id == previous.branch_id
                && previous.canonical
                && previous.leaf_node_id.as_deref() == Some(parent)
        }
        _ => false,
    };
    if !same_branch {
        return Err(NativeImportStageError::Conflict);
    }
    if !retry {
        let is_current = session
            .archive_branch(&branch.conversation_id, &branch.id)
            .is_some_and(|head| head.canonical && head.leaf_node_id == parent);
        let repeats_message = turns.iter().any(|(turn, _)| {
            session
                .archive_node(&turn.conversation_id, &turn.id)
                .is_some()
        });
        if !is_current || repeats_message {
            return Err(NativeImportStageError::Conflict);
        }
    }
    // An accepted partial retry keeps the original anchor even after its own
    // branch advanced. Source ancestry remains unknown in the import receipt.
    turns[0].0.parent_id = Some(parent.to_owned());
    Ok(())
}
