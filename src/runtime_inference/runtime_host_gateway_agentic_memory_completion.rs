//! Trusted host completion. Never exposed as a model-visible tool.
use super::super::events::RelInvalidation;
use super::super::mutation_events::RelMutationFamily;
use super::query::auth_error;
use super::*;
use crate::{
    AgenticMemoryController, AgenticMemoryError, AgenticMemoryHandoffConfirmation,
    AgenticMemoryOutcome, PreparedAgenticMemoryResult,
};
use sha2::{Digest, Sha256};

impl GatewayRuntime {
    fn authorize_agentic_completion(
        &self,
        instance: &InstanceRuntime,
        prepared: &PreparedAgenticMemoryResult,
        confirmation: &AgenticMemoryHandoffConfirmation,
    ) -> Result<AgenticReadEnvelope, AgenticMemoryError> {
        match confirmation {
            AgenticMemoryHandoffConfirmation::Succeeded {
                delivery_id,
                manifest_digest,
            } if delivery_id == prepared.delivery_id()
                && *manifest_digest == prepared.manifest_digest() => {}
            _ => {
                return Err(AgenticMemoryError::Handoff(
                    "no exact successful host confirmation".into(),
                ));
            }
        }
        let content = match &prepared.payload.outcome {
            AgenticMemoryOutcome::Memories(content) => content.as_slice(),
            _ => &[],
        };
        let included = content
            .iter()
            .map(|body| body.memory.clone())
            .collect::<BTreeSet<_>>();
        let owners = included
            .iter()
            .map(|memory| memory.owner_id.clone())
            .collect::<BTreeSet<_>>();
        if prepared.delivery_id.trim().is_empty()
            || included.len() != content.len()
            || included != prepared.delivered
            || owners != prepared.owner_cuts.keys().cloned().collect()
        {
            return Err(AgenticMemoryError::Handoff(
                "inconsistent prepared delivery manifest".into(),
            ));
        }
        let envelope = self.capture_agentic_read(instance).map_err(auth_error)?;
        for owner in &owners {
            if !envelope
                .owners
                .iter()
                .any(|read| read.token.owner_id == *owner)
            {
                return Err(auth_error(
                    "delivered Memory owner is outside the authorized REL closure",
                ));
            }
        }
        self.with_agentic_read_delivery(instance, &envelope, || Ok(()))
            .map_err(auth_error)?;
        Ok(envelope)
    }

    /// Called only after actual successful host handoff of this exact manifest.
    /// Provider success, transcript admission and ConversationExecution are not evidence.
    /// SelectedIDs remains committed if receipt application fails; retry this exact manifest.
    pub fn complete_agentic_memory_delivery(
        &self,
        instance: &InstanceRuntime,
        controller: &mut AgenticMemoryController,
        prepared: &PreparedAgenticMemoryResult,
        confirmation: &AgenticMemoryHandoffConfirmation,
    ) -> Result<(), AgenticMemoryError> {
        let envelope = self.authorize_agentic_completion(instance, prepared, confirmation)?;
        // Final authority validation has returned successfully. Delivery already happened.
        controller
            .selected_ids
            .extend(prepared.delivered.iter().cloned());
        let mut grouped = BTreeMap::<String, Vec<crate::MemoryId>>::new();
        for memory in &prepared.delivered {
            grouped
                .entry(memory.owner_id.clone())
                .or_default()
                .push(memory.memory_id);
        }
        let mut first_error = None;
        for (owner, memories) in grouped {
            // authorize_agentic_completion checked both sets against this envelope.
            let read = envelope
                .owners
                .iter()
                .find(|read| read.token.owner_id == owner)
                .expect("validated delivery owner");
            let accepted_turn = prepared.owner_cuts[&owner];
            let use_id = accepted_use_id(prepared.delivery_id(), &owner);
            let result = self.with_rel_mutation(
                &read.token,
                RelMutationFamily::AcceptedMemoryUse,
                |runtime| {
                    let owner_uuid = runtime
                        .cva
                        .owner_uuid()
                        .ok_or_else(|| operation("REL lacks owner identity"))?;
                    let invalidations = memories
                        .iter()
                        .copied()
                        .map(RelInvalidation::Memory)
                        .collect();
                    let applied = runtime
                        .cva
                        .record_accepted_memory_use(
                            crate::AcceptedMemoryUseReceipt {
                                owner_uuid,
                                accepted_turn,
                                use_id,
                                memories,
                            },
                            crate::freshness::default_worker_count(),
                        )
                        .map_err(operation)?;
                    Ok(((), applied != 0, invalidations))
                },
            );
            if let Err(error) = result {
                first_error.get_or_insert_with(|| {
                    AgenticMemoryError::Handoff(format!(
                        "accepted-use receipt for {owner} failed: {error}"
                    ))
                });
            }
        }
        // A receipt failure cannot undo content that was already handed off. All owners
        // are attempted; exact retry lets the existing receipt authority skip successes.
        first_error.map_or(Ok(()), Err)
    }
}

fn accepted_use_id(delivery_id: &str, owner: &str) -> String {
    // Deliberately independent of payload/cut: conflicting identity reuse must fail
    // existing receipt validation, not create a fresh reinforcement identity.
    let bytes = serde_json::to_vec(&(delivery_id, owner)).expect("identity tuple is JSON");
    let digest = Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("agentic-memory-use-v1:{digest}")
}

#[cfg(test)]
#[path = "runtime_host_gateway_agentic_memory_completion_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "runtime_host_gateway_agentic_memory_completion_authorization_tests.rs"]
mod authorization_tests;
#[cfg(test)]
#[path = "runtime_host_gateway_agentic_memory_completion_contract_tests.rs"]
mod contract_tests;
