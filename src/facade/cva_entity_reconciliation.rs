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
use crate::memory_store::MemoryReadView;
use crate::{
    Cva, CvaError, EntityReconciliationError, GeneralEndpoint, Phylactery, PhylacteryError,
};
use std::collections::HashSet;

macro_rules! impl_reconciliation {
    ($owner:ty, $merge_error:ty, $principal_method:ident) => {
        impl $owner {
            pub fn reconcile_entities<E: GeneralEndpoint>(
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
                    let plan = plan_reconciliation_round(&self.entities, &self.graph);
                    let mut result = ReconciliationRoundOutcome {
                        candidate_pairs: plan.candidates.len(),
                        ..ReconciliationRoundOutcome::default()
                    };
                    for candidate in &plan.candidates {
                        let action = {
                            let memories = MemoryReadView::new(&self.memories, &self.container);
                            evaluate_reconciliation_candidate(
                                &self.entities,
                                &self.graph,
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
                                let merged = self
                                    .merge_entities(survivor, retired, timestamp)
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

                    let reconsidered =
                        self.reconsider_late_exact_rejections(endpoint, &mut timestamp)?;
                    report.rejected_mentions_reconsidered += reconsidered;
                    self.container
                        .sync()
                        .map_err(|error| EntityReconciliationError::Operation(error.to_string()))?;
                    report.rounds = round + 1;
                    if !result.changed && reconsidered == 0 {
                        break;
                    }
                }
                report.final_audit = self.audit_entities();
                Ok(report)
            }

            pub(crate) fn prepare_entity_reconciliation(
                &self,
            ) -> Result<EntityReconciliationPrepared, EntityReconciliationError> {
                crate::perception_publication_reconciliation::prepare_entity_reconciliation(
                    crate::memory_store::MemoryReadView::new(&self.memories, &self.container),
                    &self.entities,
                    &self.graph,
                    &self.entity_resolutions,
                )
            }

            pub(crate) fn commit_entity_reconciliation(
                &mut self,
                prepared: EntityReconciliationPrepared,
                evaluation: EntityReconciliationEvaluation,
                now_ns: i64,
            ) -> Result<
                Option<crate::entity_reconciliation_model::EntityReconciliationCommit>,
                EntityReconciliationError,
            > {
                crate::perception_publication_reconciliation::commit_entity_reconciliation::<
                    $merge_error,
                >(
                    &mut crate::owner_lifecycle::perception_publication_merge::ReconciliationPublication::new(&mut self.container, &self.memories, &mut self.entities, &mut self.entity_resolutions, &mut self.graph, &mut self.relationships),
                    prepared,
                    evaluation,
                    now_ns,
                )
            }
        }
    };
}

macro_rules! impl_reconsider {
    ($owner:ty, $principal_method:ident) => {
        impl $owner {
            fn reconsider_late_exact_rejections<E: GeneralEndpoint>(
                &mut self,
                endpoint: &E,
                timestamp: &mut i64,
            ) -> Result<usize, EntityReconciliationError> {
                let selected = {
                    let memories = MemoryReadView::new(&self.memories, &self.container);
                    let context = RejectionSelectionContext {
                        memories: &memories,
                        entities: &self.entities,
                        resolutions: &self.entity_resolutions,
                    };
                    select_late_exact_rejections(&context)
                };
                let mut changed = 0usize;
                for candidate in selected {
                    *timestamp = timestamp.saturating_add(1);
                    self.reset_entity_resolution(candidate.key, *timestamp)
                        .map_err(|error| EntityReconciliationError::Operation(error.to_string()))?;
                    *timestamp = timestamp.saturating_add(1);
                    let result = (|| -> Result<(), crate::EntityResolverError> {
                        if self.$principal_method(candidate.key, *timestamp)?.is_some() {
                            return Ok(());
                        }
                        match self.prepare_entity_resolution(
                            candidate.key,
                            crate::EntityCandidateConfig::default(),
                        )? {
                            crate::EntityResolutionPreparation::Complete(_) => Ok(()),
                            crate::EntityResolutionPreparation::Ready(prepared) => {
                                let evaluation = evaluate_prepared_reconsideration(
                                    endpoint,
                                    &prepared,
                                )?;
                                let commit = self
                                    .plan_entity_resolution_commit(
                                        prepared,
                                        evaluation,
                                        *timestamp,
                                    )?
                                    .ok_or_else(crate::entity_resolution_processor::stale_snapshot)?;
                                self.apply_entity_resolution_commit_plan(commit)?;
                                Ok(())
                            }
                        }
                    })();
                    if let Err(error) = result {
                        *timestamp = timestamp.saturating_add(1);
                        self.put_entity_rejected(candidate.key, 0, candidate.reason, *timestamp)
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

impl_reconciliation!(Cva, CvaError, resolve_principal_entity_mention);
impl_reconciliation!(
    Phylactery,
    PhylacteryError,
    resolve_principal_entity_mention
);
impl_reconsider!(Cva, resolve_principal_entity_mention);
impl_reconsider!(Phylactery, resolve_principal_entity_mention);
