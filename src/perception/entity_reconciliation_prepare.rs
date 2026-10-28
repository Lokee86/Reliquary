use crate::entity_reconciliation_model::{
    EntityReconciliationPrepared, EntityReconciliationPreparedPair,
};
use crate::entity_reconciliation_support::exact_entity_for_surface;
use crate::{
    Cva, DEFAULT_ENTITY_RECONCILIATION_PAIR_LIMIT, EntityReconciliationError,
    MemoryEntityResolutionStatus, Phylactery,
};
use reliquary_entity_resolution::reconciliation_candidates;

macro_rules! impl_owner {
    ($owner:ty) => {
        impl $owner {
            pub(crate) fn prepare_entity_reconciliation(
                &mut self,
            ) -> Result<EntityReconciliationPrepared, EntityReconciliationError> {
                let entities = self.entities();
                let candidates =
                    reconciliation_candidates(&entities, DEFAULT_ENTITY_RECONCILIATION_PAIR_LIMIT);
                let mut pairs = Vec::with_capacity(candidates.len());
                for candidate in candidates {
                    let left = self
                        .entity(candidate.left)
                        .map_err(|error| EntityReconciliationError::Operation(error.to_string()))?;
                    let right = self
                        .entity(candidate.right)
                        .map_err(|error| EntityReconciliationError::Operation(error.to_string()))?;
                    let left_support = self
                        .memories_for_entity(left.id)
                        .into_iter()
                        .take(3)
                        .map(|id| {
                            self.memory(id).map_err(|error| {
                                EntityReconciliationError::Operation(error.to_string())
                            })
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    let right_support = self
                        .memories_for_entity(right.id)
                        .into_iter()
                        .take(3)
                        .map(|id| {
                            self.memory(id).map_err(|error| {
                                EntityReconciliationError::Operation(error.to_string())
                            })
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    pairs.push(EntityReconciliationPreparedPair {
                        candidate,
                        left,
                        right,
                        left_support,
                        right_support,
                    });
                }

                let mut late_rejected = Vec::new();
                for state in self.entity_resolutions() {
                    if !matches!(state.status, MemoryEntityResolutionStatus::Rejected { .. }) {
                        continue;
                    }
                    let Some(metadata) = self.memory_routing_metadata(state.key.memory_id) else {
                        continue;
                    };
                    let Some(mention) = metadata.entity_mentions.iter().find(|mention| {
                        mention.field == state.key.field
                            && mention.start_byte == state.key.start_byte
                            && mention.end_byte == state.key.end_byte
                    }) else {
                        continue;
                    };
                    let Some(entity) = exact_entity_for_surface(&entities, &mention.text) else {
                        continue;
                    };
                    if entity.created_at_ns > state.updated_at_ns {
                        late_rejected.push(state.key);
                    }
                }

                Ok(EntityReconciliationPrepared {
                    entity_version: self.entity_version(),
                    graph_version: self.graph_version(),
                    memory_version: self.memory_version(),
                    pairs,
                    late_rejected,
                })
            }
        }
    };
}

impl_owner!(Cva);
impl_owner!(Phylactery);
