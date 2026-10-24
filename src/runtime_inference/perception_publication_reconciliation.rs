use crate::entity_reconciliation_model::{
    EntityReconciliationEvaluation, EntityReconciliationPrepared,
};
use crate::entity_resolution_store::EntityResolutionStore;
use crate::entity_store::EntityStore;
use crate::graph_store::GraphStore;
use crate::memory_store::{MemoryReadView, MemoryStore};
use crate::{
    EntityError, EntityMergeOutcome, EntityReconciliationError, GraphError, MemoryError,
    RelationshipError,
};

use crate::owner_lifecycle::perception_publication_merge::ReconciliationPublication;

pub(crate) fn prepare_entity_reconciliation(
    memory_view: MemoryReadView<'_>,
    entities: &EntityStore,
    graph: &GraphStore,
    resolutions: &EntityResolutionStore,
) -> Result<EntityReconciliationPrepared, EntityReconciliationError> {
    let context = crate::entity_reconciliation_prepare::ReconciliationPrepareContext {
        entities,
        graph,
        memories: &memory_view,
        resolutions,
    };
    crate::entity_reconciliation_prepare::prepare_entity_reconciliation(&context)
}

pub(crate) fn commit_entity_reconciliation<E>(
    owner: &mut ReconciliationPublication<'_>,
    prepared: EntityReconciliationPrepared,
    evaluation: EntityReconciliationEvaluation,
    now_ns: i64,
) -> Result<
    Option<crate::entity_reconciliation_model::EntityReconciliationCommit>,
    EntityReconciliationError,
>
where
    E: From<EntityError>
        + From<MemoryError>
        + From<GraphError>
        + From<RelationshipError>
        + std::fmt::Display,
{
    let memory_view = owner.memory_read();
    let context = crate::entity_reconciliation_commit::ReconciliationCommitContext {
        memories: &memory_view,
        entities: owner.entities,
        graph: owner.graph,
        resolutions: owner.resolutions,
    };
    let Some(plan) = crate::entity_reconciliation_commit::plan_entity_reconciliation_commit(
        &context, prepared, evaluation,
    ) else {
        return Ok(None);
    };

    let mut timestamp = now_ns;
    let mut deterministic_merges = 0usize;
    let mut model_merges = 0usize;
    for decision in &plan.decisions {
        let planned = {
            let memory_view = owner.memory_read();
            let context = crate::entity_reconciliation_commit::ReconciliationCommitContext {
                memories: &memory_view,
                entities: owner.entities,
                graph: owner.graph,
                resolutions: owner.resolutions,
            };
            crate::entity_reconciliation_commit::plan_reconciliation_merge(
                &context, &plan, decision, timestamp,
            )?
        };
        let Some((merge_plan, deterministic)) = planned else {
            continue;
        };
        timestamp = timestamp.saturating_add(1);
        let publication = crate::entity_merge::prepare_merge_publication(
            merge_plan,
            owner.owner_id(),
            owner.memories,
            owner.entities,
            owner.graph,
            owner.relationships,
        );
        let outcome = owner
            .apply_merge::<E>(publication)
            .map_err(|error| EntityReconciliationError::Operation(error.to_string()))?;
        if outcome.changed {
            if deterministic {
                deterministic_merges += 1;
            } else {
                model_merges += 1;
            }
        }
    }

    let mut wake_keys = Vec::new();
    for key in plan.late_rejected.iter().copied() {
        let wake = {
            let memory_view = owner.memory_read();
            let context = crate::entity_reconciliation_commit::ReconciliationCommitContext {
                memories: &memory_view,
                entities: owner.entities,
                graph: owner.graph,
                resolutions: owner.resolutions,
            };
            crate::entity_reconciliation_commit::plan_late_rejection_wake(&context, key)
        };
        let Some(wake) = wake else {
            continue;
        };
        timestamp = timestamp.saturating_add(1);
        owner
            .reset_resolution(wake.key, timestamp)
            .map_err(|error| EntityReconciliationError::Operation(error.to_string()))?;
        wake_keys.push(wake.key);
    }

    owner
        .sync()
        .map_err(|error| EntityReconciliationError::Operation(error.to_string()))?;
    let report = crate::EntityReconciliationReport {
        rounds: 1,
        candidate_pairs: plan.candidate_pairs,
        deterministic_merges,
        model_merges,
        rejected_mentions_reconsidered: wake_keys.len(),
        unresolved_pairs: plan
            .candidate_pairs
            .saturating_sub(deterministic_merges + model_merges),
        final_audit: crate::entity_audit::audit_owner(&crate::entity_audit::EntityAuditContext {
            entities: owner.entities,
            memories: owner.memories,
            graph: owner.graph,
            resolutions: owner.resolutions,
        }),
    };
    Ok(Some(
        crate::entity_reconciliation_model::EntityReconciliationCommit { report, wake_keys },
    ))
}
