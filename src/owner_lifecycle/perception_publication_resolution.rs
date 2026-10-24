use crate::entity_principal::PrincipalResolutionPlan;
use crate::entity_resolution_commit::{EntityResolutionCommitOperation, EntityResolutionCommitPlan};
use crate::entity_resolution_store::EntityResolutionStore;
use crate::entity_store::EntityStore;
use crate::graph_store::GraphStore;
use crate::memory_store::MemoryStore;
use crate::owner_lifecycle::entity_resolution_admission::{
    ResolutionWriteContext, ResolutionWritePlan, ResolutionWriteResult, apply_resolution_write_plan,
};
use crate::{
    Container, EntityDraft, EntityError, EntityId, EntityResolutionDecision, EntityResolutionOutcome,
    EntityResolverError, MemoryError, SemanticGraphRelationChange, GraphRelationOrigin,
};

pub(crate) fn apply_principal_resolution_plan(
    container: &mut Container,
    memories: &MemoryStore,
    entities: &mut EntityStore,
    resolutions: &mut EntityResolutionStore,
    graph: &mut GraphStore,
    mut plan: PrincipalResolutionPlan,
    now_ns: i64,
) -> Result<EntityResolutionOutcome, EntityResolverError> {
    if !plan.should_apply {
        return Ok(plan.outcome);
    }
    if let Some((id, draft)) = plan.entity_draft.take() {
        entities.publish(container, Some(id), 0, draft)?;
    }
    if plan.associate {
        let entity_id = plan
            .outcome
            .entity_id
            .expect("association plan has an Entity");
        let change = crate::SemanticGraphRelationChange::principal_association(
            plan.outcome.key.memory_id,
            entity_id,
            true,
        );
        crate::graph_endpoint_admission::validate_semantic_changes(
            memories,
            entities,
            &[change],
            crate::GraphRelationOrigin::Perception,
        )?;
        plan.outcome.association_changed = graph
            .set_principal_association(
                container,
                plan.outcome.key.memory_id,
                entity_id,
                true,
                graph.graph_version(),
            )?
            .is_some();
        let write = ResolutionWriteContext {
            memories,
            entities,
            entity_resolutions: resolutions,
        }
        .put_entity_resolved(
            plan.outcome.key,
            plan.expected_revision,
            entity_id,
            plan.outcome.reason,
            now_ns,
        )?;
        plan.outcome.resolution_changed =
            match apply_resolution_write_plan(container, resolutions, write)? {
                ResolutionWriteResult::Changed(value) => value,
                _ => unreachable!(),
            };
    } else if plan.outcome.decision == crate::EntityResolutionDecision::Reject {
        let write = ResolutionWriteContext {
            memories,
            entities,
            entity_resolutions: resolutions,
        }
        .put_entity_rejected(
            plan.outcome.key,
            plan.expected_revision,
            plan.outcome.reason,
            now_ns,
        )?;
        plan.outcome.resolution_changed =
            match apply_resolution_write_plan(container, resolutions, write)? {
                ResolutionWriteResult::Changed(value) => value,
                _ => unreachable!(),
            };
    }
    Ok(plan.outcome)
}

pub(crate) fn apply_entity_resolution_commit_plan(
    container: &mut Container,
    memories: &MemoryStore,
    entities: &mut EntityStore,
    resolutions: &mut EntityResolutionStore,
    graph: &mut GraphStore,
    plan: EntityResolutionCommitPlan,
    reject_principal: bool,
) -> Result<EntityResolutionOutcome, EntityResolverError> {
    let EntityResolutionCommitPlan {
        key,
        decision,
        reason,
        operation,
    } = plan;
    let (entity_id, entity_created, association_changed, resolution_changed) = match operation {
        EntityResolutionCommitOperation::Existing {
            entity_id,
            alias_update,
            expected_revision,
            now_ns,
        } => {
            if let Some((revision, draft)) = alias_update {
                crate::owner_lifecycle::entity_publication_admission::validate_entity_publication(
                    &draft,
                    reject_principal,
                )?;
                entities.publish(container, Some(entity_id), revision, draft)?;
            }
            let change = crate::SemanticGraphRelationChange::entity_association(
                key.memory_id,
                entity_id,
                true,
            );
            crate::graph_endpoint_admission::validate_semantic_changes(
                memories,
                entities,
                &[change],
                crate::GraphRelationOrigin::Perception,
            )?;
            let association_changed = graph
                .set_entity_association(
                    container,
                    key.memory_id,
                    entity_id,
                    true,
                    graph.graph_version(),
                )?
                .is_some();
            let write = ResolutionWriteContext {
                memories,
                entities,
                entity_resolutions: resolutions,
            }
            .put_entity_resolved(key, expected_revision, entity_id, reason, now_ns)?;
            let resolution_changed =
                match apply_resolution_write_plan(container, resolutions, write)? {
                    ResolutionWriteResult::Changed(value) => value,
                    _ => unreachable!(),
                };
            (
                Some(entity_id),
                false,
                association_changed,
                resolution_changed,
            )
        }
        EntityResolutionCommitOperation::Create {
            draft,
            expected_revision,
            now_ns,
        } => {
            crate::owner_lifecycle::entity_publication_admission::validate_entity_publication(
                &draft,
                reject_principal,
            )?;
            let (entity, entity_created) = entities.publish(container, None, 0, draft)?;
            let change = crate::SemanticGraphRelationChange::entity_association(
                key.memory_id,
                entity.id,
                true,
            );
            crate::graph_endpoint_admission::validate_semantic_changes(
                memories,
                entities,
                &[change],
                crate::GraphRelationOrigin::Perception,
            )?;
            let association_changed = graph
                .set_entity_association(
                    container,
                    key.memory_id,
                    entity.id,
                    true,
                    graph.graph_version(),
                )?
                .is_some();
            let write = ResolutionWriteContext {
                memories,
                entities,
                entity_resolutions: resolutions,
            }
            .put_entity_resolved(key, expected_revision, entity.id, reason, now_ns)?;
            let resolution_changed =
                match apply_resolution_write_plan(container, resolutions, write)? {
                    ResolutionWriteResult::Changed(value) => value,
                    _ => unreachable!(),
                };
            (
                Some(entity.id),
                entity_created,
                association_changed,
                resolution_changed,
            )
        }
        EntityResolutionCommitOperation::Unresolved {
            candidate_entity_ids,
            candidate_fingerprint,
            context_fingerprint,
            expected_revision,
            now_ns,
        } => {
            let write = ResolutionWriteContext {
                memories,
                entities,
                entity_resolutions: resolutions,
            }
            .put_entity_unresolved(
                key,
                expected_revision,
                candidate_entity_ids,
                reason,
                candidate_fingerprint,
                context_fingerprint,
                now_ns,
            )?;
            let changed = match apply_resolution_write_plan(container, resolutions, write)? {
                ResolutionWriteResult::Changed(value) => value,
                _ => unreachable!(),
            };
            (None, false, false, changed)
        }
        EntityResolutionCommitOperation::Rejected {
            expected_revision,
            now_ns,
        } => {
            let write = ResolutionWriteContext {
                memories,
                entities,
                entity_resolutions: resolutions,
            }
            .put_entity_rejected(key, expected_revision, reason, now_ns)?;
            let changed = match apply_resolution_write_plan(container, resolutions, write)? {
                ResolutionWriteResult::Changed(value) => value,
                _ => unreachable!(),
            };
            (None, false, false, changed)
        }
    };
    Ok(EntityResolutionOutcome {
        key,
        decision,
        reason,
        entity_id,
        entity_created,
        association_changed,
        resolution_changed,
    })
}

pub(crate) fn apply_principal_profile_sync_plan(
    container: &mut Container,
    entities: &mut EntityStore,
    plan: Option<(EntityId, u64, EntityDraft)>,
) -> Result<bool, EntityError> {
    let Some((id, revision, draft)) = plan else {
        return Ok(false);
    };
    entities
        .publish(container, Some(id), revision, draft)
        .map(|(_, changed)| changed)
}

