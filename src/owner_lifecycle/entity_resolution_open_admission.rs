use crate::MemoryError;
use crate::entity_resolution_store::EntityResolutionStore;
use crate::entity_store::EntityStore;
use crate::memory_store::MemoryStore;

/// Validates replayed PHY EntityResolution records before admitting the owner.
/// Keep local status validation ahead of cross-store reference validation.
pub(crate) fn validate_open_entity_resolutions(
    memories: &MemoryStore,
    entities: &EntityStore,
    resolutions: &EntityResolutionStore,
) -> Result<(), MemoryError> {
    resolutions.validate_local()?;
    super::read_validation::validate_entity_resolution_references(
        memories,
        entities,
        &resolutions.records(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity_resolution_codec::DecodedEntityResolution;
    use crate::{
        EntityId, EntityResolutionReason, MemoryEntityMentionKey, MemoryEntityResolutionStatus,
        MemoryId, MemoryTextField,
    };

    #[test]
    fn open_admission_preserves_cross_store_reference_error() {
        let memories = MemoryStore::empty();
        let entities = EntityStore::empty();
        let resolutions = EntityResolutionStore::from_fixture_changes([DecodedEntityResolution {
            key: MemoryEntityMentionKey {
                memory_id: MemoryId([7; 32]),
                field: MemoryTextField::Content,
                start_byte: 0,
                end_byte: 2,
            },
            revision: 1,
            updated_at_ns: 10,
            status: Some(MemoryEntityResolutionStatus::Resolved {
                entity_id: EntityId([1; 32]),
                reason: EntityResolutionReason::NamedReferent,
            }),
        }])
        .expect("fixture status is locally valid");

        assert!(matches!(
            validate_open_entity_resolutions(&memories, &entities, &resolutions),
            Err(MemoryError::InvalidField("Entity resolution mention"))
        ));
        assert_eq!(resolutions.records().len(), 1);
    }
}
