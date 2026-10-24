use crate::entity_resolution_commit::{
    EntityResolutionCommitContext, plan_entity_resolution_commit,
};
use crate::entity_resolution_engine::EntityResolutionEngineCore;
use crate::entity_resolver::EntityResolverCore;
use crate::owner_lifecycle::perception_publication_resolution::{
    apply_entity_resolution_commit_plan, apply_principal_resolution_plan,
};
use crate::{
    EntityCandidateConfig, EntityCandidateError, EntityCandidateSet, EntityResolutionEvaluation,
    EntityResolutionOutcome, EntityResolutionPreparation, EntityResolutionPrepared,
    EntityResolverError, GeneralEndpoint, MemoryEntityMentionKey, PhylacteryProfile,
};

pub(crate) fn audit_rel(owner: &crate::RelExecutionOwner) -> crate::EntityAuditReport {
    crate::entity_audit::audit_owner(&crate::entity_audit::EntityAuditContext {
        entities: &owner.owner.entities,
        memories: &owner.owner.memories,
        graph: &owner.owner.graph,
        resolutions: &owner.owner.entity_resolutions,
    })
}

pub(crate) fn audit_phy(
    owner: &crate::phy_execution_owner::PhyExecutionOwner,
) -> crate::EntityAuditReport {
    crate::entity_audit::audit_owner(&crate::entity_audit::EntityAuditContext {
        entities: &owner.owner.entities,
        memories: &owner.owner.memories,
        graph: &owner.owner.graph,
        resolutions: &owner.owner.entity_resolutions,
    })
}

pub(crate) fn candidates_rel(
    owner: &mut crate::RelExecutionOwner,
    key: MemoryEntityMentionKey,
    config: EntityCandidateConfig,
) -> Result<EntityCandidateSet, EntityCandidateError> {
    let memories =
        crate::memory_store::MemoryReadView::new(&owner.owner.memories, &owner.owner.container);
    let mut context = crate::entity_candidates::EntityCandidateContext {
        memories: crate::lexical_search::MemoryLexicalReadView::new(
            &mut owner.lexical_index,
            memories,
        ),
        entities: &owner.owner.entities,
        graph: &owner.owner.graph,
    };
    crate::entity_candidates::entity_candidates_for_mention(&mut context, key, config)
}

pub(crate) fn candidates_phy(
    owner: &mut crate::phy_execution_owner::PhyExecutionOwner,
    key: MemoryEntityMentionKey,
    config: EntityCandidateConfig,
) -> Result<EntityCandidateSet, EntityCandidateError> {
    let memories =
        crate::memory_store::MemoryReadView::new(&owner.owner.memories, &owner.owner.container);
    let mut context = crate::entity_candidates::EntityCandidateContext {
        memories: crate::lexical_search::MemoryLexicalReadView::new(
            &mut owner.lexical_index,
            memories,
        ),
        entities: &owner.owner.entities,
        graph: &owner.owner.graph,
    };
    crate::entity_candidates::entity_candidates_for_mention(&mut context, key, config)
}

pub(crate) fn prepare_rel(
    owner: &mut crate::RelExecutionOwner,
    key: MemoryEntityMentionKey,
    config: EntityCandidateConfig,
) -> Result<EntityResolutionPreparation, EntityResolverError> {
    crate::perception_publication_resolution::prepare_entity_resolution(
        crate::memory_store::MemoryReadView::new(&owner.owner.memories, &owner.owner.container),
        &mut owner.lexical_index,
        &owner.owner.entities,
        &owner.owner.graph,
        &owner.owner.entity_resolutions,
        key,
        config,
        owner.owner.owner_id(),
    )
}

pub(crate) fn prepare_phy(
    owner: &mut crate::phy_execution_owner::PhyExecutionOwner,
    key: MemoryEntityMentionKey,
    config: EntityCandidateConfig,
) -> Result<EntityResolutionPreparation, EntityResolverError> {
    crate::perception_publication_resolution::prepare_entity_resolution(
        crate::memory_store::MemoryReadView::new(&owner.owner.memories, &owner.owner.container),
        &mut owner.lexical_index,
        &owner.owner.entities,
        &owner.owner.graph,
        &owner.owner.entity_resolutions,
        key,
        config,
        owner.owner.owner_id(),
    )
}

pub(crate) fn plan_commit_rel(
    owner: &crate::RelExecutionOwner,
    prepared: EntityResolutionPrepared,
    evaluation: EntityResolutionEvaluation,
    now_ns: i64,
) -> Result<Option<crate::entity_resolution_commit::EntityResolutionCommitPlan>, EntityResolverError>
{
    let context = EntityResolutionCommitContext {
        owner_id: owner.owner.owner_id(),
        memories: &owner.owner.memories,
        entities: &owner.owner.entities,
        graph: &owner.owner.graph,
        resolutions: &owner.owner.entity_resolutions,
    };
    plan_entity_resolution_commit(&context, prepared, evaluation, now_ns)
}

pub(crate) fn plan_commit_phy(
    owner: &crate::phy_execution_owner::PhyExecutionOwner,
    prepared: EntityResolutionPrepared,
    evaluation: EntityResolutionEvaluation,
    now_ns: i64,
) -> Result<Option<crate::entity_resolution_commit::EntityResolutionCommitPlan>, EntityResolverError>
{
    let context = EntityResolutionCommitContext {
        owner_id: owner.owner.owner_id(),
        memories: &owner.owner.memories,
        entities: &owner.owner.entities,
        graph: &owner.owner.graph,
        resolutions: &owner.owner.entity_resolutions,
    };
    plan_entity_resolution_commit(&context, prepared, evaluation, now_ns)
}

pub(crate) fn commit_rel(
    owner: &mut crate::RelExecutionOwner,
    prepared: EntityResolutionPrepared,
    evaluation: EntityResolutionEvaluation,
    now_ns: i64,
) -> Result<Option<EntityResolutionOutcome>, EntityResolverError> {
    let Some(plan) = plan_commit_rel(owner, prepared, evaluation, now_ns)? else {
        return Ok(None);
    };
    apply_entity_resolution_commit_plan(
        &mut owner.owner.container,
        &owner.owner.memories,
        &mut owner.owner.entities,
        &mut owner.owner.entity_resolutions,
        &mut owner.owner.graph,
        plan,
        false,
    )
    .map(Some)
}

pub(crate) fn commit_phy(
    owner: &mut crate::phy_execution_owner::PhyExecutionOwner,
    prepared: EntityResolutionPrepared,
    evaluation: EntityResolutionEvaluation,
    now_ns: i64,
) -> Result<Option<EntityResolutionOutcome>, EntityResolverError> {
    let Some(plan) = plan_commit_phy(owner, prepared, evaluation, now_ns)? else {
        return Ok(None);
    };
    apply_entity_resolution_commit_plan(
        &mut owner.owner.container,
        &owner.owner.memories,
        &mut owner.owner.entities,
        &mut owner.owner.entity_resolutions,
        &mut owner.owner.graph,
        plan,
        true,
    )
    .map(Some)
}

pub(crate) fn apply_commit_rel(
    owner: &mut crate::RelExecutionOwner,
    plan: crate::entity_resolution_commit::EntityResolutionCommitPlan,
) -> Result<EntityResolutionOutcome, EntityResolverError> {
    apply_entity_resolution_commit_plan(
        &mut owner.owner.container,
        &owner.owner.memories,
        &mut owner.owner.entities,
        &mut owner.owner.entity_resolutions,
        &mut owner.owner.graph,
        plan,
        false,
    )
}

pub(crate) fn apply_commit_phy(
    owner: &mut crate::phy_execution_owner::PhyExecutionOwner,
    plan: crate::entity_resolution_commit::EntityResolutionCommitPlan,
) -> Result<EntityResolutionOutcome, EntityResolverError> {
    apply_entity_resolution_commit_plan(
        &mut owner.owner.container,
        &owner.owner.memories,
        &mut owner.owner.entities,
        &mut owner.owner.entity_resolutions,
        &mut owner.owner.graph,
        plan,
        true,
    )
}

pub(crate) fn resolve_principal_rel(
    owner: &mut crate::RelExecutionOwner,
    key: MemoryEntityMentionKey,
    now_ns: i64,
) -> Result<Option<EntityResolutionOutcome>, EntityResolverError> {
    let memory_view =
        crate::memory_store::MemoryReadView::new(&owner.owner.memories, &owner.owner.container);
    let plan = crate::perception_publication_resolution::plan_rel_principal_resolution(
        memory_view,
        &owner.owner.entities,
        &owner.owner.entity_resolutions,
        &owner.owner.graph,
        &owner.owner.archive,
        key,
        now_ns,
    )?;
    plan.map(|plan| {
        apply_principal_resolution_plan(
            &mut owner.owner.container,
            &owner.owner.memories,
            &mut owner.owner.entities,
            &mut owner.owner.entity_resolutions,
            &mut owner.owner.graph,
            plan,
            now_ns,
        )
    })
    .transpose()
}

pub(crate) fn resolve_principal_phy(
    owner: &mut crate::phy_execution_owner::PhyExecutionOwner,
    key: MemoryEntityMentionKey,
    now_ns: i64,
) -> Result<Option<EntityResolutionOutcome>, EntityResolverError> {
    let memory_view =
        crate::memory_store::MemoryReadView::new(&owner.owner.memories, &owner.owner.container);
    let Some(plan) = crate::perception_publication_resolution::plan_phy_principal_resolution(
        memory_view,
        &owner.owner.entities,
        &owner.owner.entity_resolutions,
        &owner.owner.graph,
        key,
        now_ns,
    )?
    else {
        return Ok(None);
    };
    apply_principal_resolution_plan(
        &mut owner.owner.container,
        &owner.owner.memories,
        &mut owner.owner.entities,
        &mut owner.owner.entity_resolutions,
        &mut owner.owner.graph,
        plan,
        now_ns,
    )
    .map(Some)
}

pub(crate) fn resolve_rel<E: GeneralEndpoint>(
    owner: &mut crate::RelExecutionOwner,
    resolver: &EntityResolverCore<E>,
    key: MemoryEntityMentionKey,
    config: EntityCandidateConfig,
    now_ns: i64,
) -> Result<EntityResolutionOutcome, EntityResolverError> {
    if let Some(outcome) = resolve_principal_rel(owner, key, now_ns)? {
        return Ok(outcome);
    }
    match prepare_rel(owner, key, config)? {
        EntityResolutionPreparation::Complete(outcome) => Ok(outcome),
        EntityResolutionPreparation::Ready(prepared) => {
            let evaluation = resolver.evaluate_prepared(&prepared)?;
            commit_rel(owner, prepared, evaluation, now_ns)?.ok_or_else(stale_resolution_snapshot)
        }
    }
}

pub(crate) fn resolve_phy<E: GeneralEndpoint>(
    owner: &mut crate::phy_execution_owner::PhyExecutionOwner,
    resolver: &EntityResolverCore<E>,
    key: MemoryEntityMentionKey,
    config: EntityCandidateConfig,
    now_ns: i64,
) -> Result<EntityResolutionOutcome, EntityResolverError> {
    if let Some(outcome) = resolve_principal_phy(owner, key, now_ns)? {
        return Ok(outcome);
    }
    match prepare_phy(owner, key, config)? {
        EntityResolutionPreparation::Complete(outcome) => Ok(outcome),
        EntityResolutionPreparation::Ready(prepared) => {
            let evaluation = resolver.evaluate_prepared(&prepared)?;
            commit_phy(owner, prepared, evaluation, now_ns)?.ok_or_else(stale_resolution_snapshot)
        }
    }
}

pub(crate) fn resolve_rel_with_engine<E: GeneralEndpoint>(
    owner: &mut crate::RelExecutionOwner,
    engine: &EntityResolutionEngineCore<E>,
    key: MemoryEntityMentionKey,
    config: EntityCandidateConfig,
    now_ns: i64,
) -> Result<EntityResolutionOutcome, EntityResolverError> {
    if let Some(outcome) = resolve_principal_rel(owner, key, now_ns)? {
        return Ok(outcome);
    }
    match prepare_rel(owner, key, config)? {
        EntityResolutionPreparation::Complete(outcome) => Ok(outcome),
        EntityResolutionPreparation::Ready(prepared) => {
            let evaluation = engine.evaluate_prepared(&prepared)?;
            commit_rel(owner, prepared, evaluation, now_ns)?.ok_or_else(stale_resolution_snapshot)
        }
    }
}

pub(crate) fn resolve_phy_with_engine<E: GeneralEndpoint>(
    owner: &mut crate::phy_execution_owner::PhyExecutionOwner,
    engine: &EntityResolutionEngineCore<E>,
    key: MemoryEntityMentionKey,
    config: EntityCandidateConfig,
    now_ns: i64,
) -> Result<EntityResolutionOutcome, EntityResolverError> {
    if let Some(outcome) = resolve_principal_phy(owner, key, now_ns)? {
        return Ok(outcome);
    }
    match prepare_phy(owner, key, config)? {
        EntityResolutionPreparation::Complete(outcome) => Ok(outcome),
        EntityResolutionPreparation::Ready(prepared) => {
            let evaluation = engine.evaluate_prepared(&prepared)?;
            commit_phy(owner, prepared, evaluation, now_ns)?.ok_or_else(stale_resolution_snapshot)
        }
    }
}

pub(crate) fn resolve_rel_parallel<E: GeneralEndpoint>(
    owner: &mut crate::RelExecutionOwner,
    engine: &EntityResolutionEngineCore<E>,
    keys: &[MemoryEntityMentionKey],
    config: EntityCandidateConfig,
    now_ns: i64,
    workers: usize,
) -> Result<crate::EntityResolutionBatchOutcome, EntityResolverError> {
    resolve_batch_rel(owner, engine, keys, config, now_ns, workers)
}

pub(crate) fn resolve_phy_parallel<E: GeneralEndpoint>(
    owner: &mut crate::phy_execution_owner::PhyExecutionOwner,
    engine: &EntityResolutionEngineCore<E>,
    keys: &[MemoryEntityMentionKey],
    config: EntityCandidateConfig,
    now_ns: i64,
    workers: usize,
) -> Result<crate::EntityResolutionBatchOutcome, EntityResolverError> {
    resolve_batch_phy(owner, engine, keys, config, now_ns, workers)
}

fn resolve_batch_rel<E: GeneralEndpoint>(
    owner: &mut crate::RelExecutionOwner,
    engine: &EntityResolutionEngineCore<E>,
    keys: &[MemoryEntityMentionKey],
    config: EntityCandidateConfig,
    now_ns: i64,
    workers: usize,
) -> Result<crate::EntityResolutionBatchOutcome, EntityResolverError> {
    let wave_size = workers.max(1);
    let mut outcomes = Vec::with_capacity(keys.len());
    let mut speculative_evaluations = 0usize;
    let mut reevaluations = 0usize;
    let mut timestamp = now_ns;
    for wave in keys.chunks(wave_size) {
        let mut prepared = Vec::with_capacity(wave.len());
        for key in wave {
            if let Some(outcome) = resolve_principal_rel(owner, *key, timestamp)? {
                prepared.push(EntityResolutionPreparation::Complete(outcome));
            } else {
                prepared.push(prepare_rel(owner, *key, config)?);
            }
        }
        let mut slots = crate::entity_resolution_batch::evaluate_wave(
            engine,
            prepared,
            &mut speculative_evaluations,
        )?;
        if let Some(error) = crate::entity_resolution_batch::take_first_error(&mut slots) {
            return Err(error);
        }
        for slot in slots {
            let outcome = match slot.expect("parallel Entity resolution omitted slot") {
                crate::entity_resolution_batch::SpeculativeEvaluation::Complete(outcome) => outcome,
                crate::entity_resolution_batch::SpeculativeEvaluation::Evaluated(
                    old,
                    Ok(evaluation),
                ) => match prepare_rel(owner, old.key, config)? {
                    EntityResolutionPreparation::Complete(outcome) => outcome,
                    EntityResolutionPreparation::Ready(fresh) => {
                        let evaluation =
                            if crate::entity_resolution_batch::same_relevant_snapshot(&old, &fresh)
                            {
                                evaluation
                            } else {
                                reevaluations += 1;
                                crate::entity_resolution_batch::evaluate_prepared_with_retry(
                                    engine, &fresh,
                                )?
                            };
                        timestamp = timestamp.saturating_add(1);
                        commit_rel(owner, fresh, evaluation, timestamp)?
                            .ok_or_else(stale_resolution_snapshot)?
                    }
                },
                crate::entity_resolution_batch::SpeculativeEvaluation::Evaluated(_, Err(_)) => {
                    unreachable!()
                }
            };
            outcomes.push(outcome);
        }
    }
    Ok(crate::EntityResolutionBatchOutcome {
        outcomes,
        speculative_evaluations,
        reevaluations,
    })
}

fn resolve_batch_phy<E: GeneralEndpoint>(
    owner: &mut crate::phy_execution_owner::PhyExecutionOwner,
    engine: &EntityResolutionEngineCore<E>,
    keys: &[MemoryEntityMentionKey],
    config: EntityCandidateConfig,
    now_ns: i64,
    workers: usize,
) -> Result<crate::EntityResolutionBatchOutcome, EntityResolverError> {
    let wave_size = workers.max(1);
    let mut outcomes = Vec::with_capacity(keys.len());
    let mut speculative_evaluations = 0usize;
    let mut reevaluations = 0usize;
    let mut timestamp = now_ns;
    for wave in keys.chunks(wave_size) {
        let mut prepared = Vec::with_capacity(wave.len());
        for key in wave {
            if let Some(outcome) = resolve_principal_phy(owner, *key, timestamp)? {
                prepared.push(EntityResolutionPreparation::Complete(outcome));
            } else {
                prepared.push(prepare_phy(owner, *key, config)?);
            }
        }
        let mut slots = crate::entity_resolution_batch::evaluate_wave(
            engine,
            prepared,
            &mut speculative_evaluations,
        )?;
        if let Some(error) = crate::entity_resolution_batch::take_first_error(&mut slots) {
            return Err(error);
        }
        for slot in slots {
            let outcome = match slot.expect("parallel Entity resolution omitted slot") {
                crate::entity_resolution_batch::SpeculativeEvaluation::Complete(outcome) => outcome,
                crate::entity_resolution_batch::SpeculativeEvaluation::Evaluated(
                    old,
                    Ok(evaluation),
                ) => match prepare_phy(owner, old.key, config)? {
                    EntityResolutionPreparation::Complete(outcome) => outcome,
                    EntityResolutionPreparation::Ready(fresh) => {
                        let evaluation =
                            if crate::entity_resolution_batch::same_relevant_snapshot(&old, &fresh)
                            {
                                evaluation
                            } else {
                                reevaluations += 1;
                                crate::entity_resolution_batch::evaluate_prepared_with_retry(
                                    engine, &fresh,
                                )?
                            };
                        timestamp = timestamp.saturating_add(1);
                        commit_phy(owner, fresh, evaluation, timestamp)?
                            .ok_or_else(stale_resolution_snapshot)?
                    }
                },
                crate::entity_resolution_batch::SpeculativeEvaluation::Evaluated(_, Err(_)) => {
                    unreachable!()
                }
            };
            outcomes.push(outcome);
        }
    }
    Ok(crate::EntityResolutionBatchOutcome {
        outcomes,
        speculative_evaluations,
        reevaluations,
    })
}

pub(crate) fn stale_resolution_snapshot() -> EntityResolverError {
    EntityResolverError::InvalidOutput("Entity resolution snapshot became stale".into())
}

pub(crate) fn sync_principal_profile_rel(
    owner: &mut crate::RelExecutionOwner,
    principal_id: &str,
    profile: &PhylacteryProfile,
    now_ns: i64,
) -> Result<bool, crate::EntityError> {
    let view =
        crate::memory_store::MemoryReadView::new(&owner.owner.memories, &owner.owner.container);
    let plan = crate::perception_publication_resolution::plan_principal_profile_sync(
        view,
        &owner.owner.entities,
        &owner.owner.entity_resolutions,
        &owner.owner.graph,
        &owner.owner.archive,
        principal_id,
        profile,
        now_ns,
    )?;
    let Some((id, revision, draft)) = plan else {
        return Ok(false);
    };
    crate::owner_lifecycle::perception_publication_resolution::apply_principal_profile_sync_plan(
        &mut owner.owner.container,
        &mut owner.owner.entities,
        Some((id, revision, draft)),
    )
}

pub(crate) fn reset_rel(
    owner: &mut crate::RelExecutionOwner,
    key: MemoryEntityMentionKey,
    now_ns: i64,
) -> Result<bool, crate::MemoryError> {
    let plan = crate::owner_lifecycle::entity_resolution_admission::ResolutionWriteContext {
        memories: &owner.owner.memories,
        entities: &owner.owner.entities,
        entity_resolutions: &owner.owner.entity_resolutions,
    }
    .reset_entity_resolution(key, now_ns)?;
    match crate::owner_lifecycle::entity_resolution_admission::apply_resolution_write_plan(
        &mut owner.owner.container,
        &mut owner.owner.entity_resolutions,
        plan,
    )? {
        crate::owner_lifecycle::entity_resolution_admission::ResolutionWriteResult::Changed(
            value,
        ) => Ok(value),
        _ => unreachable!(),
    }
}

pub(crate) fn reset_phy(
    owner: &mut crate::phy_execution_owner::PhyExecutionOwner,
    key: MemoryEntityMentionKey,
    now_ns: i64,
) -> Result<bool, crate::MemoryError> {
    let plan = crate::owner_lifecycle::entity_resolution_admission::ResolutionWriteContext {
        memories: &owner.owner.memories,
        entities: &owner.owner.entities,
        entity_resolutions: &owner.owner.entity_resolutions,
    }
    .reset_entity_resolution(key, now_ns)?;
    match crate::owner_lifecycle::entity_resolution_admission::apply_resolution_write_plan(
        &mut owner.owner.container,
        &mut owner.owner.entity_resolutions,
        plan,
    )? {
        crate::owner_lifecycle::entity_resolution_admission::ResolutionWriteResult::Changed(
            value,
        ) => Ok(value),
        _ => unreachable!(),
    }
}

pub(crate) fn reject_rel(
    owner: &mut crate::RelExecutionOwner,
    key: MemoryEntityMentionKey,
    expected_revision: u32,
    reason: crate::EntityResolutionReason,
    now_ns: i64,
) -> Result<bool, crate::MemoryError> {
    let plan = crate::owner_lifecycle::entity_resolution_admission::ResolutionWriteContext {
        memories: &owner.owner.memories,
        entities: &owner.owner.entities,
        entity_resolutions: &owner.owner.entity_resolutions,
    }
    .put_entity_rejected(key, expected_revision, reason, now_ns)?;
    match crate::owner_lifecycle::entity_resolution_admission::apply_resolution_write_plan(
        &mut owner.owner.container,
        &mut owner.owner.entity_resolutions,
        plan,
    )? {
        crate::owner_lifecycle::entity_resolution_admission::ResolutionWriteResult::Changed(
            value,
        ) => Ok(value),
        _ => unreachable!(),
    }
}

pub(crate) fn reject_phy(
    owner: &mut crate::phy_execution_owner::PhyExecutionOwner,
    key: MemoryEntityMentionKey,
    expected_revision: u32,
    reason: crate::EntityResolutionReason,
    now_ns: i64,
) -> Result<bool, crate::MemoryError> {
    let plan = crate::owner_lifecycle::entity_resolution_admission::ResolutionWriteContext {
        memories: &owner.owner.memories,
        entities: &owner.owner.entities,
        entity_resolutions: &owner.owner.entity_resolutions,
    }
    .put_entity_rejected(key, expected_revision, reason, now_ns)?;
    match crate::owner_lifecycle::entity_resolution_admission::apply_resolution_write_plan(
        &mut owner.owner.container,
        &mut owner.owner.entity_resolutions,
        plan,
    )? {
        crate::owner_lifecycle::entity_resolution_admission::ResolutionWriteResult::Changed(
            value,
        ) => Ok(value),
        _ => unreachable!(),
    }
}

macro_rules! impl_resolution_owner_operations {
    ($owner:ty) => {
        impl $owner {
            pub(crate) fn runtime_entity_mention_keys_for_memory(&self,id:crate::MemoryId)->Vec<MemoryEntityMentionKey>{crate::entity_resolution_owner::ResolutionReadContext{memories:&self.owner.memories,entity_resolutions:&self.owner.entity_resolutions}.entity_mention_keys_for_memory(id)}
            pub(crate) fn runtime_pending_entity_mentions_for_candidate(&self,id:crate::EntityId)->Vec<MemoryEntityMentionKey>{crate::entity_resolution_owner::ResolutionReadContext{memories:&self.owner.memories,entity_resolutions:&self.owner.entity_resolutions}.pending_entity_mentions_for_candidate(id)}
            pub(crate) fn runtime_pending_entity_mentions_for_surface(&self,surface:&str)->Vec<MemoryEntityMentionKey>{crate::entity_resolution_owner::ResolutionReadContext{memories:&self.owner.memories,entity_resolutions:&self.owner.entity_resolutions}.pending_entity_mentions_for_surface(surface)}
            pub(crate) fn runtime_schedulable_entity_mentions_for_memory(&self,id:crate::MemoryId)->Vec<MemoryEntityMentionKey>{crate::entity_resolution_owner::ResolutionReadContext{memories:&self.owner.memories,entity_resolutions:&self.owner.entity_resolutions}.schedulable_entity_mentions_for_memory(id)}
            pub(crate) fn runtime_entity_resolution(&self,key:MemoryEntityMentionKey)->Option<&crate::MemoryEntityResolution>{self.owner.entity_resolutions.get(key)}
            pub(crate) fn runtime_entity_resolutions_for_memory(&self,id:crate::MemoryId)->Vec<crate::MemoryEntityResolution>{crate::entity_resolution_owner::ResolutionReadContext{memories:&self.owner.memories,entity_resolutions:&self.owner.entity_resolutions}.entity_resolutions_for_memory(id)}
            pub(crate) fn runtime_entity_resolution_retry_needed(&self,key:MemoryEntityMentionKey,candidate:[u8;32],context:[u8;32])->bool{crate::entity_resolution_owner::ResolutionReadContext{memories:&self.owner.memories,entity_resolutions:&self.owner.entity_resolutions}.entity_resolution_retry_needed(key,candidate,context)}
            pub(crate) fn runtime_put_entity_resolved(&mut self,key:MemoryEntityMentionKey,revision:u32,id:crate::EntityId,reason:crate::EntityResolutionReason,now_ns:i64)->Result<bool,crate::MemoryError>{let plan=crate::owner_lifecycle::entity_resolution_admission::ResolutionWriteContext{memories:&self.owner.memories,entities:&self.owner.entities,entity_resolutions:&self.owner.entity_resolutions}.put_entity_resolved(key,revision,id,reason,now_ns)?;match crate::owner_lifecycle::entity_resolution_admission::apply_resolution_write_plan(&mut self.owner.container,&mut self.owner.entity_resolutions,plan)?{crate::owner_lifecycle::entity_resolution_admission::ResolutionWriteResult::Changed(value)=>Ok(value),_=>unreachable!()}}
            pub(crate) fn runtime_put_entity_rejected(&mut self,key:MemoryEntityMentionKey,revision:u32,reason:crate::EntityResolutionReason,now_ns:i64)->Result<bool,crate::MemoryError>{let plan=crate::owner_lifecycle::entity_resolution_admission::ResolutionWriteContext{memories:&self.owner.memories,entities:&self.owner.entities,entity_resolutions:&self.owner.entity_resolutions}.put_entity_rejected(key,revision,reason,now_ns)?;match crate::owner_lifecycle::entity_resolution_admission::apply_resolution_write_plan(&mut self.owner.container,&mut self.owner.entity_resolutions,plan)?{crate::owner_lifecycle::entity_resolution_admission::ResolutionWriteResult::Changed(value)=>Ok(value),_=>unreachable!()}}
            pub(crate) fn runtime_put_entity_unresolved(&mut self,key:MemoryEntityMentionKey,revision:u32,ids:Vec<crate::EntityId>,reason:crate::EntityResolutionReason,candidate:[u8;32],context:[u8;32],now_ns:i64)->Result<bool,crate::MemoryError>{let plan=crate::owner_lifecycle::entity_resolution_admission::ResolutionWriteContext{memories:&self.owner.memories,entities:&self.owner.entities,entity_resolutions:&self.owner.entity_resolutions}.put_entity_unresolved(key,revision,ids,reason,candidate,context,now_ns)?;match crate::owner_lifecycle::entity_resolution_admission::apply_resolution_write_plan(&mut self.owner.container,&mut self.owner.entity_resolutions,plan)?{crate::owner_lifecycle::entity_resolution_admission::ResolutionWriteResult::Changed(value)=>Ok(value),_=>unreachable!()}}
            pub(crate) fn runtime_reset_entity_resolution(&mut self,key:MemoryEntityMentionKey,now_ns:i64)->Result<bool,crate::MemoryError>{let plan=crate::owner_lifecycle::entity_resolution_admission::ResolutionWriteContext{memories:&self.owner.memories,entities:&self.owner.entities,entity_resolutions:&self.owner.entity_resolutions}.reset_entity_resolution(key,now_ns)?;match crate::owner_lifecycle::entity_resolution_admission::apply_resolution_write_plan(&mut self.owner.container,&mut self.owner.entity_resolutions,plan)?{crate::owner_lifecycle::entity_resolution_admission::ResolutionWriteResult::Changed(value)=>Ok(value),_=>unreachable!()}}
            pub(crate) fn runtime_compact_entity_resolutions(&mut self,now_ns:i64)->Result<crate::EntityResolutionCompaction,crate::MemoryError>{let plan=crate::owner_lifecycle::entity_resolution_admission::ResolutionWriteContext{memories:&self.owner.memories,entities:&self.owner.entities,entity_resolutions:&self.owner.entity_resolutions}.compact_entity_resolutions(now_ns)?;match crate::owner_lifecycle::entity_resolution_admission::apply_resolution_write_plan(&mut self.owner.container,&mut self.owner.entity_resolutions,plan)?{crate::owner_lifecycle::entity_resolution_admission::ResolutionWriteResult::Compacted(value)=>Ok(value),_=>unreachable!()}}
            pub(crate) fn runtime_entity_resolutions(&self)->Vec<crate::MemoryEntityResolution>{crate::entity_resolution_owner::ResolutionReadContext{memories:&self.owner.memories,entity_resolutions:&self.owner.entity_resolutions}.entity_resolutions()}
            pub(crate) fn runtime_entity_resolution_records(&self)->Vec<crate::MemoryEntityResolution>{crate::entity_resolution_owner::ResolutionReadContext{memories:&self.owner.memories,entity_resolutions:&self.owner.entity_resolutions}.entity_resolution_records()}
            pub(crate) fn runtime_import_entity_resolution(&mut self,value:crate::MemoryEntityResolution)->Result<(),crate::MemoryError>{let plan=crate::owner_lifecycle::entity_resolution_admission::ResolutionWriteContext{memories:&self.owner.memories,entities:&self.owner.entities,entity_resolutions:&self.owner.entity_resolutions}.import_entity_resolution(value)?;match crate::owner_lifecycle::entity_resolution_admission::apply_resolution_write_plan(&mut self.owner.container,&mut self.owner.entity_resolutions,plan)?{crate::owner_lifecycle::entity_resolution_admission::ResolutionWriteResult::Imported=>Ok(()),_=>unreachable!()}}
        }
    };
}

impl_resolution_owner_operations!(crate::RelExecutionOwner);
impl_resolution_owner_operations!(crate::phy_execution_owner::PhyExecutionOwner);
