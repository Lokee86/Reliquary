use crate::entity_resolution_evidence::{hydrate_admission_context, hydrate_evidence};
use crate::entity_resolution_processor_support::{
    candidate_fingerprint, context_fingerprint, status_reason, terminal_outcome,
};
use crate::{
    Cva, EntityCandidateConfig, EntityResolutionDecision, EntityResolutionOutcome,
    EntityResolutionPreparation, EntityResolutionPrepared, EntityResolverError,
    MemoryEntityMentionKey, Phylactery,
};

macro_rules! impl_owner {
    ($owner:ty) => {
        impl $owner {
            pub(crate) fn prepare_entity_resolution(
                &mut self,
                key: MemoryEntityMentionKey,
                config: EntityCandidateConfig,
            ) -> Result<EntityResolutionPreparation, EntityResolverError> {
                if let Some(outcome) = terminal_outcome(key, self.entity_resolution(key).as_ref()) {
                    return Ok(EntityResolutionPreparation::Complete(outcome));
                }

                let owner_id = self.owner_id().ok_or(crate::MemoryError::InvalidField(
                    "Entity resolution owner ID",
                ))?;
                let candidates = self.entity_candidates_for_mention(key, config)?;
                let memory = self.memory(key.memory_id)?;
                let evidence = hydrate_evidence(self, &candidates, key.memory_id)?;
                let admission_context =
                    hydrate_admission_context(self, &candidates, key.memory_id)?;
                let candidate_fingerprint = candidate_fingerprint(&candidates);
                let context_fingerprint =
                    context_fingerprint(&memory, &candidates, &evidence, &admission_context);

                if !self.entity_resolution_retry_needed(
                    key,
                    candidate_fingerprint,
                    context_fingerprint,
                ) {
                    if let Some(value) = self.entity_resolution(key) {
                        return Ok(EntityResolutionPreparation::Complete(
                            EntityResolutionOutcome {
                                key,
                                decision: EntityResolutionDecision::Unresolved,
                                reason: status_reason(&value.status),
                                entity_id: None,
                                entity_created: false,
                                association_changed: false,
                                resolution_changed: false,
                            },
                        ));
                    }
                }

                Ok(EntityResolutionPreparation::Ready(
                    EntityResolutionPrepared {
                        owner_id,
                        key,
                        memory,
                        candidates,
                        evidence,
                        admission_context,
                        candidate_fingerprint,
                        context_fingerprint,
                        expected_resolution_revision: self
                            .entity_resolution(key)
                            .map_or(0, |value| value.revision),
                        memory_version: self.memory_version(),
                        entity_version: self.entity_version(),
                        graph_version: self.graph_version(),
                    },
                ))
            }
        }
    };
}

impl_owner!(Cva);
impl_owner!(Phylactery);
