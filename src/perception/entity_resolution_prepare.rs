use crate::entity_candidates::EntityCandidateContext;
use crate::entity_resolution_evidence::{
    EvidenceContext, hydrate_admission_context, hydrate_evidence,
};
use crate::entity_resolution_processor_support::{
    candidate_fingerprint, context_fingerprint, status_reason, terminal_outcome,
};
use crate::entity_resolution_store::EntityResolutionStore;
use crate::{
    EntityCandidateConfig, EntityResolutionDecision, EntityResolutionOutcome,
    EntityResolutionPreparation, EntityResolutionPrepared, EntityResolverError,
    MemoryEntityMentionKey,
};

pub(crate) struct EntityResolutionContext<'a> {
    pub(crate) candidates: EntityCandidateContext<'a>,
    pub(crate) resolutions: &'a EntityResolutionStore,
}

pub(crate) fn prepare_entity_resolution(
    context: &mut EntityResolutionContext<'_>,
    key: MemoryEntityMentionKey,
    config: EntityCandidateConfig,
    owner_id: Option<String>,
) -> Result<EntityResolutionPreparation, EntityResolverError> {
    if let Some(outcome) = terminal_outcome(key, context.resolutions.get(key)) {
        return Ok(EntityResolutionPreparation::Complete(outcome));
    }

    let owner_id = owner_id.ok_or(crate::MemoryError::InvalidField(
        "Entity resolution owner ID",
    ))?;
    let candidates = crate::entity_candidates::entity_candidates_for_mention(
        &mut context.candidates,
        key,
        config,
    )?;
    let memory = context.candidates.memory(key.memory_id)?;
    let evidence_context = EvidenceContext {
        memories: context.candidates.memories,
        graph: context.candidates.graph,
        container: context.candidates.container,
    };
    let evidence = hydrate_evidence(&evidence_context, &candidates, key.memory_id)?;
    let admission_context =
        hydrate_admission_context(&evidence_context, &candidates, key.memory_id)?;
    let candidate_fingerprint = candidate_fingerprint(&candidates);
    let context_fingerprint =
        context_fingerprint(&memory, &candidates, &evidence, &admission_context);

    if !context
        .resolutions
        .retry_needed(key, candidate_fingerprint, context_fingerprint)
    {
        if let Some(value) = context.resolutions.get(key) {
            return Ok(EntityResolutionPreparation::Complete(
                EntityResolutionOutcome {
                    key,
                    decision: EntityResolutionDecision::Unresolved,
                    reason: status_reason(&value.status),
                    entity_id: None,
                    entity_created: false,
                    association_changed: false,
                    resolution_changed: false,
                },
            ));
        }
    }

    Ok(EntityResolutionPreparation::Ready(
        EntityResolutionPrepared {
            owner_id,
            key,
            memory,
            candidates,
            evidence,
            admission_context,
            candidate_fingerprint,
            context_fingerprint,
            expected_resolution_revision: context
                .resolutions
                .get(key)
                .map_or(0, |value| value.revision),
            memory_version: context.candidates.memories.memory_version(),
            entity_version: context.candidates.entities.entity_version(),
            graph_version: context.candidates.graph.graph_version(),
        },
    ))
}
