use crate::entity_store::EntityStore;
use crate::memory_store::MemoryStore;
use crate::{RelationshipDraft, RelationshipError};

/// Normalize and validate relationships against concrete REL/PHY owner stores.
pub(crate) fn validate_live_relationship_refs(
    owner_id: &str,
    memories: &MemoryStore,
    entities: &EntityStore,
    draft: &mut RelationshipDraft,
) -> Result<(), RelationshipError> {
    crate::relationship_store::normalize_draft(draft)?;
    crate::owner_lifecycle::validate_relationship_entities(
        owner_id,
        entities,
        &draft.participants,
        false,
    )?;
    for evidence in &draft.evidence {
        if evidence.owner_id == owner_id && !memories.contains_memory(evidence.memory_id) {
            return Err(RelationshipError::MissingLocalMemory(evidence.memory_id));
        }
    }
    Ok(())
}
