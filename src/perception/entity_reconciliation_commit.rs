use crate::entity_reconciliation_model::{
    EntityReconciliationCommit, EntityReconciliationEvaluation, EntityReconciliationPrepared,
};
use crate::entity_reconciliation_support::{
    choose_survivor, exact_entity_for_surface, merge_alias_budget_fits,
};
use crate::{
    Cva, EntityId, EntityReconciliationError, EntityReconciliationRelation,
    EntityReconciliationReport, MemoryEntityResolutionStatus, Phylactery,
};
use std::collections::HashMap;

macro_rules! impl_owner {
    ($owner:ty) => {
        impl $owner {
            pub(crate) fn commit_entity_reconciliation(
                &mut self,
                prepared: EntityReconciliationPrepared,
                evaluation: EntityReconciliationEvaluation,
                now_ns: i64,
            ) -> Result<Option<EntityReconciliationCommit>, EntityReconciliationError> {
                if self.entity_version() != prepared.entity_version
                    || self.graph_version() != prepared.graph_version
                    || self.memory_version() != prepared.memory_version
                {
                    return Ok(None);
                }
                let mut timestamp = now_ns;
                let mut deterministic_merges = 0usize;
                let mut model_merges = 0usize;
                let degrees = self
                    .entities()
                    .into_iter()
                    .map(|entity| (entity.id, self.memories_for_entity(entity.id).len()))
                    .collect::<HashMap<EntityId, usize>>();

                for decision in evaluation.decisions {
                    if decision.relation != EntityReconciliationRelation::SameIdentity
                        || self.retired_entity_replacement(decision.left).is_some()
                        || self.retired_entity_replacement(decision.right).is_some()
                    {
                        continue;
                    }
                    let left = self
                        .entity(decision.left)
                        .map_err(|error| EntityReconciliationError::Operation(error.to_string()))?;
                    let right = self
                        .entity(decision.right)
                        .map_err(|error| EntityReconciliationError::Operation(error.to_string()))?;
                    if left.kind != right.kind {
                        continue;
                    }
                    let (survivor, retired) = choose_survivor(&left, &right, &degrees);
                    let (survivor_entity, retired_entity) = if survivor == left.id {
                        (&left, &right)
                    } else {
                        (&right, &left)
                    };
                    if !merge_alias_budget_fits(survivor_entity, retired_entity) {
                        continue;
                    }
                    timestamp = timestamp.saturating_add(1);
                    let outcome = self
                        .merge_entities(survivor, retired, timestamp)
                        .map_err(|error| EntityReconciliationError::Operation(error.to_string()))?;
                    if outcome.changed {
                        if decision.deterministic {
                            deterministic_merges += 1;
                        } else {
                            model_merges += 1;
                        }
                    }
                }

                let mut wake_keys = Vec::new();
                for key in prepared.late_rejected.iter().copied() {
                    let Some(state) = self.entity_resolution(key) else {
                        continue;
                    };
                    if !matches!(state.status, MemoryEntityResolutionStatus::Rejected { .. }) {
                        continue;
                    }
                    let entities = self.entities();
                    let Some(metadata) = self.memory_routing_metadata(key.memory_id) else {
                        continue;
                    };
                    let Some(mention) = metadata.entity_mentions.iter().find(|mention| {
                        mention.field == key.field
                            && mention.start_byte == key.start_byte
                            && mention.end_byte == key.end_byte
                    }) else {
                        continue;
                    };
                    let Some(entity) = exact_entity_for_surface(&entities, &mention.text) else {
                        continue;
                    };
                    if entity.created_at_ns <= state.updated_at_ns {
                        continue;
                    }
                    timestamp = timestamp.saturating_add(1);
                    self.reset_entity_resolution(key, timestamp)
                        .map_err(|error| EntityReconciliationError::Operation(error.to_string()))?;
                    wake_keys.push(key);
                }

                self.sync()
                    .map_err(|error| EntityReconciliationError::Operation(error.to_string()))?;
                let unresolved_pairs = prepared
                    .pairs
                    .len()
                    .saturating_sub(deterministic_merges + model_merges);
                let report = EntityReconciliationReport {
                    rounds: 1,
                    candidate_pairs: prepared.pairs.len(),
                    deterministic_merges,
                    model_merges,
                    rejected_mentions_reconsidered: wake_keys.len(),
                    unresolved_pairs,
                    final_audit: self.audit_entities(),
                };
                Ok(Some(EntityReconciliationCommit { report, wake_keys }))
            }
        }
    };
}

impl_owner!(Cva);
impl_owner!(Phylactery);
