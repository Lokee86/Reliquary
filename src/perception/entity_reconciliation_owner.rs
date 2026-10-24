use crate::entity_resolution_store::EntityResolutionStore;
use crate::entity_store::EntityStore;
use crate::memory_store::MemoryReadView;
use crate::{
    EntityResolutionPrepared, EntityResolutionReason, EntityResolverError, GeneralEndpoint,
    MemoryEntityMentionKey, MemoryEntityResolutionStatus,
};

pub(crate) struct ReconsiderableLateRejection {
    pub(crate) key: MemoryEntityMentionKey,
    pub(crate) reason: EntityResolutionReason,
}

pub(crate) struct RejectionSelectionContext<'a, 'm> {
    pub(crate) memories: &'a MemoryReadView<'m>,
    pub(crate) entities: &'a EntityStore,
    pub(crate) resolutions: &'a EntityResolutionStore,
}

pub(crate) fn select_late_exact_rejections(
    context: &RejectionSelectionContext<'_, '_>,
) -> Vec<ReconsiderableLateRejection> {
    let states = context.resolutions.records();
    let entities = context.entities.current();
    let mut selected = Vec::new();
    for state in states {
        let MemoryEntityResolutionStatus::Rejected { reason } = state.status else {
            continue;
        };
        let Some(metadata) = context.memories.routing_metadata(state.key.memory_id) else {
            continue;
        };
        let Some(mention) = metadata.entity_mentions.iter().find(|mention| {
            mention.field == state.key.field
                && mention.start_byte == state.key.start_byte
                && mention.end_byte == state.key.end_byte
        }) else {
            continue;
        };
        let Some(entity) = crate::entity_reconciliation_support::exact_entity_for_surface(
            &entities,
            &mention.text,
        ) else {
            continue;
        };
        if entity.created_at_ns > state.updated_at_ns {
            selected.push(ReconsiderableLateRejection {
                key: state.key,
                reason,
            });
        }
    }
    selected
}

pub(crate) fn evaluate_prepared_reconsideration<E: GeneralEndpoint>(
    endpoint: &E,
    prepared: &EntityResolutionPrepared,
) -> Result<crate::EntityResolutionEvaluation, EntityResolverError> {
    crate::entity_resolution_engine::EntityResolutionEngineCore::new(endpoint)
        .evaluate_prepared(prepared)
}
