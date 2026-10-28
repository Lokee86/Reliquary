use crate::entity_reconciliation_support::{choose_survivor, merge_alias_budget_fits};
use crate::{
    Cva, DEFAULT_ENTITY_RECONCILIATION_PAIR_LIMIT, DEFAULT_ENTITY_RECONCILIATION_ROUNDS,
    EntityAuditReport, EntityId, EntityReconciliationError, EntityReconciliationRelation,
    EntityReconciliationReport, GeneralEndpoint, Memory, Phylactery,
};
use reliquary_entity_resolution::classify_pair;
use reliquary_entity_resolution::reconciliation_candidates;
use std::collections::{HashMap, HashSet};

macro_rules! impl_reconciliation {
    ($owner:ty) => {
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
                    final_audit: EntityAuditReport::default(),
                };
                let mut seen_model_pairs = HashSet::new();
                let mut timestamp = now_ns;

                for round in 0..DEFAULT_ENTITY_RECONCILIATION_ROUNDS {
                    let degrees = self
                        .entities()
                        .into_iter()
                        .map(|entity| (entity.id, self.memories_for_entity(entity.id).len()))
                        .collect::<HashMap<_, _>>();
                    let candidates = reconciliation_candidates(
                        &self.entities(),
                        DEFAULT_ENTITY_RECONCILIATION_PAIR_LIMIT,
                    );
                    report.candidate_pairs += candidates.len();
                    let mut changed = false;

                    for candidate in candidates {
                        if self.retired_entity_replacement(candidate.left).is_some()
                            || self.retired_entity_replacement(candidate.right).is_some()
                        {
                            continue;
                        }
                        let left = self.entity(candidate.left).map_err(|error| {
                            EntityReconciliationError::Operation(error.to_string())
                        })?;
                        let right = self.entity(candidate.right).map_err(|error| {
                            EntityReconciliationError::Operation(error.to_string())
                        })?;
                        if left.kind != right.kind {
                            continue;
                        }

                        let should_merge = if candidate.deterministic {
                            true
                        } else if seen_model_pairs.insert((candidate.left, candidate.right)) {
                            let left_support = reconciliation_support(self, left.id)?;
                            let right_support = reconciliation_support(self, right.id)?;
                            matches!(
                                classify_pair(
                                    endpoint,
                                    &left,
                                    &right,
                                    &left_support,
                                    &right_support
                                )?,
                                EntityReconciliationRelation::SameIdentity
                            )
                        } else {
                            false
                        };

                        if !should_merge {
                            if !candidate.deterministic {
                                report.unresolved_pairs += 1;
                            }
                            continue;
                        }

                        let (survivor, retired) = choose_survivor(&left, &right, &degrees);
                        let (survivor_entity, retired_entity) = if survivor == left.id {
                            (&left, &right)
                        } else {
                            (&right, &left)
                        };
                        if !merge_alias_budget_fits(survivor_entity, retired_entity) {
                            report.unresolved_pairs += 1;
                            continue;
                        }
                        timestamp = timestamp.saturating_add(1);
                        let outcome =
                            self.merge_entities(survivor, retired, timestamp)
                                .map_err(|error| {
                                    EntityReconciliationError::Operation(error.to_string())
                                })?;
                        if outcome.changed {
                            changed = true;
                            if candidate.deterministic {
                                report.deterministic_merges += 1;
                            } else {
                                report.model_merges += 1;
                            }
                        }
                    }

                    let reconsidered =
                        reconsider_late_exact_rejections(self, endpoint, &mut timestamp)?;
                    report.rejected_mentions_reconsidered += reconsidered;
                    changed |= reconsidered > 0;
                    self.sync()
                        .map_err(|error| EntityReconciliationError::Operation(error.to_string()))?;

                    report.rounds = round + 1;
                    if !changed {
                        break;
                    }
                }

                report.final_audit = self.audit_entities();
                Ok(report)
            }
        }
    };
}

impl_reconciliation!(Cva);
impl_reconciliation!(Phylactery);

pub(crate) trait ReconciliationOwner {
    fn support_memories(
        &mut self,
        entity_id: EntityId,
    ) -> Result<Vec<Memory>, EntityReconciliationError>;
    fn reconsider_rejections<E: GeneralEndpoint>(
        &mut self,
        endpoint: &E,
        timestamp: &mut i64,
    ) -> Result<usize, EntityReconciliationError>;
}

fn reconciliation_support<O: ReconciliationOwner>(
    owner: &mut O,
    entity_id: EntityId,
) -> Result<Vec<Memory>, EntityReconciliationError> {
    owner.support_memories(entity_id)
}

fn reconsider_late_exact_rejections<O: ReconciliationOwner, E: GeneralEndpoint>(
    owner: &mut O,
    endpoint: &E,
    timestamp: &mut i64,
) -> Result<usize, EntityReconciliationError> {
    owner.reconsider_rejections(endpoint, timestamp)
}
