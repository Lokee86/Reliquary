use crate::entity_resolution_batch::SpeculativeEvaluation;
use crate::{
    Cva, EntityCandidateConfig, EntityResolutionBatchOutcome, EntityResolutionEngine,
    EntityResolutionEvaluation, EntityResolutionOutcome, EntityResolutionPreparation,
    EntityResolutionPrepared, EntityResolverError, GeneralEndpoint, GeneralEndpointError,
    MemoryEntityMentionKey, Phylactery,
};

macro_rules! impl_owner {
    ($owner:ty) => {
        impl $owner {
            pub fn resolve_entity_mentions_with_engine_parallel<E: GeneralEndpoint>(
                &mut self,
                engine: &EntityResolutionEngine<E>,
                keys: &[MemoryEntityMentionKey],
                config: EntityCandidateConfig,
                now_ns: i64,
                workers: usize,
            ) -> Result<EntityResolutionBatchOutcome, EntityResolverError> {
                let wave_size = workers.max(1);
                let mut outcomes = Vec::with_capacity(keys.len());
                let mut speculative_evaluations = 0usize;
                let mut reevaluations = 0usize;
                let mut timestamp = now_ns;

                for wave in keys.chunks(wave_size) {
                    let mut prepared = Vec::with_capacity(wave.len());
                    for key in wave {
                        if let Some(outcome) =
                            self.resolve_principal_entity_mention(*key, timestamp)?
                        {
                            prepared.push(EntityResolutionPreparation::Complete(outcome));
                        } else {
                            prepared.push(self.prepare_entity_resolution(*key, config)?);
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
                            SpeculativeEvaluation::Complete(outcome) => outcome,
                            SpeculativeEvaluation::Evaluated(old, Ok(evaluation)) => {
                                match self.prepare_entity_resolution(old.key, config)? {
                                    EntityResolutionPreparation::Complete(outcome) => outcome,
                                    EntityResolutionPreparation::Ready(fresh) => {
                                        let evaluation = if crate::entity_resolution_batch::same_relevant_snapshot(&old, &fresh) {
                                            evaluation
                                        } else {
                                            reevaluations += 1;
                                            crate::entity_resolution_batch::evaluate_prepared_with_retry(engine, &fresh)?
                                        };
                                        timestamp = timestamp.saturating_add(1);
                                        self.commit_entity_resolution(fresh, evaluation, timestamp)?
                                            .ok_or_else(crate::entity_resolution_batch::stale_snapshot)?
                                    }
                                }
                            }
                            SpeculativeEvaluation::Evaluated(_, Err(_)) => unreachable!(),
                        };
                        outcomes.push(outcome);
                    }
                }

                Ok(EntityResolutionBatchOutcome {
                    outcomes,
                    speculative_evaluations,
                    reevaluations,
                })
            }
        }
    };
}

impl_owner!(Cva);
impl_owner!(Phylactery);
