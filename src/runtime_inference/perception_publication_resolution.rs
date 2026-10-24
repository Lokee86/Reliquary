use crate::entity_principal::PrincipalResolutionPlan;
use crate::entity_resolution_commit::{
    EntityResolutionCommitOperation, EntityResolutionCommitPlan,
};
use crate::entity_resolution_owner::{ResolutionWriteContext, ResolutionWritePlan};
use crate::entity_resolution_store::EntityResolutionStore;
use crate::entity_store::EntityStore;
use crate::graph_store::GraphStore;
use crate::lexical_index::LexicalIndex;
use crate::memory_store::{MemoryReadView, MemoryStore};
use crate::{
    Container, EntityCandidateConfig, EntityResolutionOutcome, EntityResolutionPreparation,
    EntityResolverError, MemoryError, PhylacteryProfile,
};

pub(crate) enum ResolutionWriteResult {
    Changed(bool),
    Compacted(crate::EntityResolutionCompaction),
    Imported,
}

pub(crate) fn apply_resolution_write_plan(
    container: &mut Container,
    resolutions: &mut EntityResolutionStore,
    plan: ResolutionWritePlan,
) -> Result<ResolutionWriteResult, MemoryError> {
    match plan {
        ResolutionWritePlan::Resolved {
            key,
            expected_revision,
            entity_id,
            reason,
            now_ns,
        } => resolutions
            .put_resolved(container, key, expected_revision, entity_id, reason, now_ns)
            .map(ResolutionWriteResult::Changed),
        ResolutionWritePlan::Rejected {
            key,
            expected_revision,
            reason,
            now_ns,
        } => resolutions
            .put_rejected(container, key, expected_revision, reason, now_ns)
            .map(ResolutionWriteResult::Changed),
        ResolutionWritePlan::Unresolved {
            key,
            expected_revision,
            candidate_entity_ids,
            reason,
            candidate_set_fingerprint,
            context_fingerprint,
            now_ns,
        } => resolutions
            .put_unresolved(
                container,
                key,
                expected_revision,
                candidate_entity_ids,
                reason,
                candidate_set_fingerprint,
                context_fingerprint,
                now_ns,
            )
            .map(ResolutionWriteResult::Changed),
        ResolutionWritePlan::Reset { key, now_ns } => resolutions
            .tombstone(container, key, now_ns)
            .map(ResolutionWriteResult::Changed),
        ResolutionWritePlan::Compact {
            inactive_memory_ids,
            now_ns,
        } => resolutions
            .compact(container, &inactive_memory_ids, now_ns)
            .map(ResolutionWriteResult::Compacted),
        ResolutionWritePlan::Import(value) => resolutions
            .import_current(container, value)
            .map(|()| ResolutionWriteResult::Imported),
    }
}

pub(crate) fn prepare_entity_resolution(
    container: &Container,
    memories: &MemoryStore,
    lexical_index: &mut LexicalIndex,
    entities: &EntityStore,
    graph: &GraphStore,
    resolutions: &EntityResolutionStore,
    key: crate::MemoryEntityMentionKey,
    config: EntityCandidateConfig,
    owner_id: Option<String>,
) -> Result<EntityResolutionPreparation, EntityResolverError> {
    let memory_view = MemoryReadView::new(memories, container);
    let mut candidates = crate::entity_candidates::EntityCandidateContext {
        memories: crate::lexical_search::MemoryLexicalReadView::new(lexical_index, memory_view),
        entities,
        graph,
    };
    crate::entity_resolution_prepare::prepare_entity_resolution(
        &mut crate::entity_resolution_prepare::EntityResolutionContext {
            candidates,
            resolutions,
        },
        key,
        config,
        owner_id,
    )
}

pub(crate) fn plan_rel_principal_resolution(
    container: &Container,
    memories: &MemoryStore,
    entities: &EntityStore,
    resolutions: &EntityResolutionStore,
    graph: &GraphStore,
    archive: &crate::Archive,
    key: crate::MemoryEntityMentionKey,
    now_ns: i64,
) -> Result<Option<PrincipalResolutionPlan>, EntityResolverError> {
    let memories = MemoryReadView::new(memories, container);
    crate::entity_principal::PrincipalContext {
        memories,
        entities,
        entity_resolutions: resolutions,
        graph,
        archive: Some(archive),
    }
    .resolve_principal_entity_mention(key, now_ns)
}

pub(crate) fn plan_phy_principal_resolution(
    container: &Container,
    memories: &MemoryStore,
    entities: &EntityStore,
    resolutions: &EntityResolutionStore,
    graph: &GraphStore,
    key: crate::MemoryEntityMentionKey,
    now_ns: i64,
) -> Result<Option<PrincipalResolutionPlan>, EntityResolverError> {
    let memories = MemoryReadView::new(memories, container);
    crate::entity_principal::PrincipalContext {
        memories,
        entities,
        entity_resolutions: resolutions,
        graph,
        archive: None,
    }
    .reject_phy_principal_entity_mention(key, now_ns)
}

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
                crate::entity_owner::validate_entity_publication(&draft, reject_principal)?;
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
            crate::entity_owner::validate_entity_publication(&draft, reject_principal)?;
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

pub(crate) fn sync_principal_profile(
    container: &mut Container,
    memories: &MemoryStore,
    entities: &mut EntityStore,
    resolutions: &EntityResolutionStore,
    graph: &GraphStore,
    archive: &crate::Archive,
    principal_id: &str,
    profile: &PhylacteryProfile,
    now_ns: i64,
) -> Result<bool, crate::EntityError> {
    let plan = crate::entity_principal::PrincipalContext {
        memories: MemoryReadView::new(memories, container),
        entities,
        entity_resolutions: resolutions,
        graph,
        archive: Some(archive),
    }
    .sync_principal_profile(principal_id, profile, now_ns)?;
    let Some((id, revision, draft)) = plan else {
        return Ok(false);
    };
    entities
        .publish(container, Some(id), revision, draft)
        .map(|(_, changed)| changed)
}
