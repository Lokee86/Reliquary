use super::support::source_memory;
use super::*;

pub(super) fn withdraw(
    common: &mut CommonState,
    container: &mut Container,
    request: WithdrawEntityMentionRequest,
) -> Result<MemoryEntityResolution, OwnerSessionError> {
    if request.owner_id != container.owner_id() || request.key.memory_id != request.memory_id.0 {
        return Err(OwnerSessionError::Record(
            "Entity mention withdrawal owner or Memory mismatch".into(),
        ));
    }
    source_memory(
        common,
        request.memory_id,
        request.memory_revision,
        request.body_id,
        request.key.field,
        request.key.start_byte as usize,
        request.key.end_byte as usize,
        &request.source_surface,
    )?;
    let current = common
        .perception
        .mention(request.key)
        .cloned()
        .ok_or_else(|| {
            OwnerSessionError::Record("Entity mention is not currently resolved".into())
        })?;
    if current.revision != request.expected_mention_revision
        || request.withdrawn_at_ns < current.updated_at_ns
        || current.revision == u32::MAX
    {
        return Err(OwnerSessionError::Record(
            "Entity mention changed before withdrawal".into(),
        ));
    }
    let actual_targets: Vec<_> = match current.status {
        MemoryEntityResolutionStatus::Resolved { entity_id, .. } => vec![entity_id],
        MemoryEntityResolutionStatus::Ambiguous { candidates } => candidates,
        MemoryEntityResolutionStatus::Unresolved { .. } => {
            return Err(OwnerSessionError::Record(
                "Entity mention is already inactive".into(),
            ));
        }
    };
    if actual_targets.len() != request.target_revisions.len()
        || actual_targets.iter().zip(&request.target_revisions).any(
            |(id, (expected_id, revision))| {
                id != expected_id
                    || common
                        .perception
                        .entity(*id)
                        .is_none_or(|entity| entity.revision != *revision)
            },
        )
    {
        return Err(OwnerSessionError::Record(
            "Entity mention targets changed before withdrawal".into(),
        ));
    }
    Ok(common.perception.put_mention_resolution(
        container,
        request.key,
        request.expected_mention_revision,
        request.withdrawn_at_ns,
        MemoryEntityResolutionStatus::Unresolved {
            reason: EntityResolutionReason::UserWithdrawal,
        },
    )?)
}
