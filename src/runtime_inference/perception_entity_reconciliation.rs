use crate::entity_reconciliation::{
    ReconciliationCandidateAction, ReconciliationRoundOutcome, evaluate_reconciliation_candidate,
    plan_reconciliation_round,
};
use crate::entity_reconciliation_model::{
    EntityReconciliationEvaluation, EntityReconciliationPrepared, EntityReconciliationReport,
};
use crate::entity_reconciliation_owner::{
    RejectionSelectionContext, evaluate_prepared_reconsideration, select_late_exact_rejections,
};
use crate::{EntityReconciliationError, GeneralEndpoint};
use std::collections::HashSet;

macro_rules! impl_reconciliation {
    ($owner:ty, $merge_error:ty, $merge_fn:ident, $principal_fn:ident, $prepare_fn:ident, $plan_fn:ident, $apply_fn:ident, $sync_fn:ident, $audit_fn:ident, $reset_fn:ident, $reject_fn:ident) => {
        impl $owner {
            pub(crate) fn reconcile_entities_runtime<E: GeneralEndpoint>(
                &mut self,
                endpoint: &E,
                now_ns: i64,
            ) -> Result<EntityReconciliationReport, EntityReconciliationError> {
                let mut report = EntityReconciliationReport {
                    rounds: 0,
                    candidate_pairs: 0,
                    deterministic_merges: 0,
                    model_merges: 0,
                    rejected_mentions_reconsidered: 0,
                    unresolved_pairs: 0,
                    final_audit: crate::EntityAuditReport::default(),
                };
                let mut seen_model_pairs = HashSet::new();
                let mut timestamp = now_ns;
                for round in 0..crate::DEFAULT_ENTITY_RECONCILIATION_ROUNDS {
                    let plan = plan_reconciliation_round(&self.owner.entities, &self.owner.graph);
                    let mut result = ReconciliationRoundOutcome {
                        candidate_pairs: plan.candidates.len(),
                        ..ReconciliationRoundOutcome::default()
                    };
                    for candidate in &plan.candidates {
                        let action = {
                            let memories = crate::memory_store::MemoryReadView::new(
                                &self.owner.memories,
                                &self.owner.container,
                            );
                            evaluate_reconciliation_candidate(
                                &self.owner.entities,
                                &self.owner.graph,
                                &memories,
                                &plan,
                                candidate,
                                endpoint,
                                &mut seen_model_pairs,
                            )?
                        };
                        match action {
                            ReconciliationCandidateAction::Skip => {}
                            ReconciliationCandidateAction::Unresolved => {
                                result.unresolved_pairs += 1;
                            }
                            ReconciliationCandidateAction::Merge {
                                survivor,
                                retired,
                                deterministic,
                            } => {
                                timestamp = timestamp.saturating_add(1);
                                let merged = crate::perception_entity_merge_runtime::$merge_fn(
                                    self,
                                    survivor,
                                    retired,
                                    timestamp,
                                )
                                .map_err(|error| {
                                    EntityReconciliationError::Operation(error.to_string())
                                })?;
                                if merged.changed {
                                    result.changed = true;
                                    if deterministic {
                                        result.deterministic_merges += 1;
                                    } else {
                                        result.model_merges += 1;
                                    }
                                }
                            }
                        }
                    }
                    report.candidate_pairs += result.candidate_pairs;
                    report.deterministic_merges += result.deterministic_merges;
                    report.model_merges += result.model_merges;
                    report.unresolved_pairs += result.unresolved_pairs;

                    let reconsidered = self.reconsider_late_exact_rejections_runtime(
                        endpoint,
                        &mut timestamp,
                    )?;
                    report.rejected_mentions_reconsidered += reconsidered;
                    self.$sync_fn()
                        .map_err(|error| EntityReconciliationError::Operation(error.to_string()))?;
                    report.rounds = round + 1;
                    if !result.changed && reconsidered == 0 {
                        break;
                    }
                }
                report.final_audit = crate::perception_entity_resolution_runtime::$audit_fn(self);
                Ok(report)
            }

            pub(crate) fn prepare_entity_reconciliation_runtime(
                &self,
            ) -> Result<EntityReconciliationPrepared, EntityReconciliationError> {
                crate::perception_publication_reconciliation::prepare_entity_reconciliation(
                    crate::memory_store::MemoryReadView::new(
                        &self.owner.memories,
                        &self.owner.container,
                    ),
                    &self.owner.entities,
                    &self.owner.graph,
                    &self.owner.entity_resolutions,
                )
            }

            pub(crate) fn commit_entity_reconciliation_runtime(
                &mut self,
                prepared: EntityReconciliationPrepared,
                evaluation: EntityReconciliationEvaluation,
                now_ns: i64,
            ) -> Result<Option<crate::entity_reconciliation_model::EntityReconciliationCommit>, EntityReconciliationError> {
                crate::perception_publication_reconciliation::commit_entity_reconciliation::<$merge_error>(
                    &mut crate::owner_lifecycle::perception_publication_merge::ReconciliationPublication::new(
                        &mut self.owner.container,
                        &self.owner.memories,
                        &mut self.owner.entities,
                        &mut self.owner.entity_resolutions,
                        &mut self.owner.graph,
                        &mut self.owner.relationships,
                    ),
                    prepared,
                    evaluation,
                    now_ns,
                )
            }

            fn reconsider_late_exact_rejections_runtime<E: GeneralEndpoint>(
                &mut self,
                endpoint: &E,
                timestamp: &mut i64,
            ) -> Result<usize, EntityReconciliationError> {
                let selected = {
                    let memories = crate::memory_store::MemoryReadView::new(
                        &self.owner.memories,
                        &self.owner.container,
                    );
                    let context = RejectionSelectionContext {
                        memories: &memories,
                        entities: &self.owner.entities,
                        resolutions: &self.owner.entity_resolutions,
                    };
                    select_late_exact_rejections(&context)
                };
                let mut changed = 0;
                for candidate in selected {
                    *timestamp = timestamp.saturating_add(1);
                    crate::perception_entity_resolution_runtime::$reset_fn(
                        self,
                        candidate.key,
                        *timestamp,
                    )
                    .map_err(|error| EntityReconciliationError::Operation(error.to_string()))?;
                    *timestamp = timestamp.saturating_add(1);
                    let result = (|| -> Result<(), crate::EntityResolverError> {
                        if crate::perception_entity_resolution_runtime::$principal_fn(
                            self,
                            candidate.key,
                            *timestamp,
                        )?
                        .is_some()
                        {
                            return Ok(());
                        }
                        match crate::perception_entity_resolution_runtime::$prepare_fn(
                            self,
                            candidate.key,
                            crate::EntityCandidateConfig::default(),
                        )? {
                            crate::EntityResolutionPreparation::Complete(_) => Ok(()),
                            crate::EntityResolutionPreparation::Ready(prepared) => {
                                let evaluation = evaluate_prepared_reconsideration(endpoint, &prepared)?;
                                let commit = crate::perception_entity_resolution_runtime::$plan_fn(
                                    self,
                                    prepared,
                                    evaluation,
                                    *timestamp,
                                )?
                                .ok_or_else(crate::perception_entity_resolution_runtime::stale_resolution_snapshot)?;
                                crate::perception_entity_resolution_runtime::$apply_fn(self, commit)?;
                                Ok(())
                            }
                        }
                    })();
                    if let Err(error) = result {
                        *timestamp = timestamp.saturating_add(1);
                        crate::perception_entity_resolution_runtime::$reject_fn(
                            self,
                            candidate.key,
                            0,
                            candidate.reason,
                            *timestamp,
                        )
                        .map_err(|restore| {
                            EntityReconciliationError::Operation(format!(
                                "resolver failed ({error}); failed to restore rejection: {restore}"
                            ))
                        })?;
                        return Err(error.into());
                    }
                    changed += 1;
                }
                Ok(changed)
            }
        }
    };
}

impl_reconciliation!(
    crate::RelExecutionOwner,
    crate::CvaError,
    merge_rel,
    resolve_principal_rel,
    prepare_rel,
    plan_commit_rel,
    apply_commit_rel,
    sync_owner,
    audit_rel,
    reset_rel,
    reject_rel
);
impl_reconciliation!(
    crate::phy_execution_owner::PhyExecutionOwner,
    crate::PhylacteryError,
    merge_phy,
    resolve_principal_phy,
    prepare_phy,
    plan_commit_phy,
    apply_commit_phy,
    sync,
    audit_phy,
    reset_phy,
    reject_phy
);

pub(crate) fn reconcile_rel<E: GeneralEndpoint>(
    owner: &mut crate::RelExecutionOwner,
    endpoint: &E,
    now_ns: i64,
) -> Result<EntityReconciliationReport, EntityReconciliationError> {
    owner.reconcile_entities_runtime(endpoint, now_ns)
}

pub(crate) fn reconcile_phy<E: GeneralEndpoint>(
    owner: &mut crate::phy_execution_owner::PhyExecutionOwner,
    endpoint: &E,
    now_ns: i64,
) -> Result<EntityReconciliationReport, EntityReconciliationError> {
    owner.reconcile_entities_runtime(endpoint, now_ns)
}
