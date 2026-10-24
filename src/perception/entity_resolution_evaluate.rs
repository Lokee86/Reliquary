use crate::entity_admission::is_obvious_transient_occurrence;
use crate::entity_resolver::EntityResolverCore;
use crate::{
    EntityAdmissionDecision, EntityResolutionDecision, EntityResolutionEvaluation,
    EntityResolutionPrepared, EntityResolverError, EntityResolverOutput, GeneralEndpoint,
};

impl<E: GeneralEndpoint> EntityResolverCore<E> {
    pub(crate) fn evaluate_prepared(
        &self,
        prepared: &EntityResolutionPrepared,
    ) -> Result<EntityResolutionEvaluation, EntityResolverError> {
        if is_obvious_transient_occurrence(&prepared.memory, &prepared.candidates) {
            return Ok(EntityResolutionEvaluation {
                output: EntityResolverOutput {
                    decision: EntityResolutionDecision::Reject,
                    reason: crate::EntityResolutionReason::TransientValue,
                },
                materialization: None,
            });
        }

        if prepared.candidates.candidates.is_empty() {
            let admission = self.admit(
                &prepared.memory,
                &prepared.candidates,
                &prepared.admission_context,
            )?;
            let output = EntityResolverOutput {
                decision: match admission.decision {
                    EntityAdmissionDecision::CreateNew => EntityResolutionDecision::CreateNew,
                    EntityAdmissionDecision::Unresolved => EntityResolutionDecision::Unresolved,
                    EntityAdmissionDecision::Reject => EntityResolutionDecision::Reject,
                },
                reason: admission.reason,
            };
            return Ok(EntityResolutionEvaluation {
                output,
                materialization: admission.materialization,
            });
        }

        let mut output =
            self.resolve(&prepared.memory, &prepared.candidates, &prepared.evidence)?;
        let materialization = if output.decision == EntityResolutionDecision::CreateNew {
            let mut admission_set = prepared.candidates.clone();
            admission_set.candidates.clear();
            let admission = self.admit(
                &prepared.memory,
                &admission_set,
                &prepared.admission_context,
            )?;
            match admission.decision {
                EntityAdmissionDecision::CreateNew => {
                    if admission
                        .materialization
                        .as_ref()
                        .is_some_and(|materialization| {
                            prepared.candidates.candidates.iter().any(|candidate| {
                                candidate.exact_surface
                                    && candidate.entity.kind.as_str()
                                        == materialization.kind.as_str()
                            })
                        })
                    {
                        output.decision = EntityResolutionDecision::Unresolved;
                        output.reason = crate::EntityResolutionReason::Ambiguous;
                        None
                    } else {
                        admission.materialization
                    }
                }
                EntityAdmissionDecision::Unresolved => {
                    output.decision = EntityResolutionDecision::Unresolved;
                    output.reason = admission.reason;
                    None
                }
                EntityAdmissionDecision::Reject => {
                    output.decision = EntityResolutionDecision::Reject;
                    output.reason = admission.reason;
                    None
                }
            }
        } else {
            None
        };
        Ok(EntityResolutionEvaluation {
            output,
            materialization,
        })
    }
}
