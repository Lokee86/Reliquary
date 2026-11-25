//! Bind an authorized copied suffix to its immutable completed predecessor.
use super::*;

pub(in crate::native_import_source) fn attach_predecessor(
    session: &RelSession,
    scope: &HermesCopyScope,
    namespace: &str,
    predecessor: &[u8; 32],
    retry: bool,
    turns: &mut [(TurnInput, Branch)],
) -> Result<(), NativeImportStageError> {
    let (_, branch) = turns.first().ok_or(NativeImportStageError::Conflict)?;
    let head = session
        .copied_source_head(
            namespace,
            &scope.session_id,
            &branch.conversation_id,
            predecessor,
        )
        .ok_or(NativeImportStageError::Conflict)?;
    if branch.id != head.branch_id {
        return Err(NativeImportStageError::Conflict);
    }
    if !retry {
        let current = session.copied_source_head_is_current(&branch.conversation_id, &head);
        let repeats_message = turns.iter().any(|(turn, _)| {
            session
                .archive_node(&turn.conversation_id, &turn.id)
                .is_some()
        });
        if !current || repeats_message {
            return Err(NativeImportStageError::Conflict);
        }
    }
    // A partial retry keeps the original anchor after its own branch advanced.
    // Source ancestry remains unknown in the import receipt.
    turns[0].0.parent_id = head.node_id.map(str::to_owned);
    Ok(())
}
