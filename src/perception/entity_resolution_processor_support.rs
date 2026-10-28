use crate::{
    EntityCandidateSet, EntityDraft, EntityMaterialization, EntityResolutionDecision,
    EntityResolutionOutcome, Memory, MemoryEntityMentionKey, MemoryEntityResolutionStatus,
};

pub(super) fn bootstrap_entity_draft(
    memory: &Memory,
    set: &EntityCandidateSet,
    materialization: EntityMaterialization,
) -> EntityDraft {
    EntityDraft {
        canonical_name: set.mention.text.clone(),
        aliases: Vec::new(),
        kind: materialization.kind,
        summary: materialization.summary,
        mutation_id: bootstrap_mutation_id(set.key),
        created_at_ns: memory.created_at_ns,
        updated_at_ns: memory.created_at_ns,
    }
}

pub(super) fn terminal_outcome(
    key: MemoryEntityMentionKey,
    value: Option<&crate::MemoryEntityResolution>,
) -> Option<EntityResolutionOutcome> {
    let value = value?;
    match value.status {
        MemoryEntityResolutionStatus::Resolved { entity_id, reason } => {
            Some(EntityResolutionOutcome {
                key,
                decision: EntityResolutionDecision::ResolveExisting(entity_id),
                reason,
                entity_id: Some(entity_id),
                entity_created: false,
                association_changed: false,
                resolution_changed: false,
            })
        }
        MemoryEntityResolutionStatus::Rejected { reason } => Some(EntityResolutionOutcome {
            key,
            decision: EntityResolutionDecision::Reject,
            reason,
            entity_id: None,
            entity_created: false,
            association_changed: false,
            resolution_changed: false,
        }),
        _ => None,
    }
}

pub(super) fn status_reason(
    status: &MemoryEntityResolutionStatus,
) -> crate::EntityResolutionReason {
    match status {
        MemoryEntityResolutionStatus::Resolved { reason, .. }
        | MemoryEntityResolutionStatus::Rejected { reason } => *reason,
        MemoryEntityResolutionStatus::Pending(value) => value.reason,
        MemoryEntityResolutionStatus::Dormant(value) => value.reason,
    }
}

fn bootstrap_mutation_id(key: MemoryEntityMentionKey) -> String {
    format!(
        "perception-entity-v1:{}:{}:{}:{}",
        hex(&key.memory_id.0),
        key.field.tag(),
        key.start_byte,
        key.end_byte
    )
}

fn hex(bytes: &[u8; 32]) -> String {
    let mut out = String::with_capacity(64);
    for byte in bytes {
        use std::fmt::Write;
        let _ = write!(&mut out, "{byte:02x}");
    }
    out
}
