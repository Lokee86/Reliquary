//! Trusted host completion. Never exposed as a model-visible tool.
use super::query::auth_error;
use super::*;
use crate::{
    AgenticMemoryController, AgenticMemoryError, AgenticMemoryHandoffConfirmation,
    AgenticMemoryOutcome, PreparedAgenticMemoryResult,
};

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
    pub fn complete_agentic_memory_delivery(
        &self,
        instance: &InstanceRuntime,
        controller: &mut AgenticMemoryController,
        prepared: &PreparedAgenticMemoryResult,
        confirmation: &AgenticMemoryHandoffConfirmation,
    ) -> Result<(), AgenticMemoryError> {
        let _envelope = self.authorize_agentic_completion(instance, prepared, confirmation)?;
        // Final authority validation has returned successfully. Delivery already happened.
        controller
            .selected_ids
            .extend(prepared.delivered.iter().cloned());
        Ok(())
    }
}

#[cfg(test)]
#[path = "runtime_host_gateway_agentic_memory_completion_tests.rs"]
mod tests;
