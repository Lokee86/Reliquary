use super::*;
use crate::agentic_memory_retrieval::{grade_agentic_memories, order_agentic_memories};
use crate::agentic_memory_retrieval_decision::select_memories;
use crate::{
    AgenticMemoryContent, AgenticMemoryController, AgenticMemoryError, AgenticMemoryOperation,
    AgenticMemoryOutcome, AgenticMemoryPayload, MemoryRef, PreparedAgenticMemoryResult,
};

impl GatewayRuntime {
    /// Prepare a new query. Only the trusted host's later handoff completion can select content.
    pub fn memory_query(
        &self,
        instance: &InstanceRuntime,
        controller: &mut AgenticMemoryController,
        query: &str,
        delivery_id: &str,
    ) -> Result<PreparedAgenticMemoryResult, AgenticMemoryError> {
        if delivery_id.trim().is_empty() {
            return Err(AgenticMemoryError::InvalidDeliveryIdentity);
        }
        let mut staged = controller.stage_query(query)?;
        let envelope = self.capture_agentic_read(instance).map_err(auth_error)?;
        let endpoint = self.agentic_query_embedding_route().map_err(read_error)?;
        let mut vectors = endpoint
            .embed(crate::EmbeddingMode::Query, &[staged.query().to_owned()])
            .map_err(read_error)?;
        if vectors.len() != 1 {
            return Err(AgenticMemoryError::Retrieval(
                "embedding route returned an invalid query count".into(),
            ));
        }
        let vector = vectors.remove(0);
        crate::memory_retrieval_vectors::validate_query(&vector).map_err(read_error)?;
        let mut ranked = Vec::new();
        for owner in &envelope.owners {
            let profile = owner
                .profile
                .ok_or_else(|| read_error("REL has no established Memory compatibility profile"))?;
            let mut candidates = self
                .with_rel(&owner.token, |runtime| {
                    let pool = runtime
                        .cva
                        .collect_memory_candidate_pool(staged.query(), &vector, profile)
                        .map_err(operation)?;
                    let ids = pool.matched_memory_ids().collect::<Vec<_>>();
                    grade_agentic_memories(&runtime.cva, &ids, owner.cut).map_err(operation)
                })
                .map_err(read_error)?;
            ranked.append(&mut candidates);
        }
        ranked.retain(|candidate| !controller.selected_ids().contains(&candidate.memory));
        order_agentic_memories(&mut ranked);
        ranked.truncate(10);
        let candidates = ranked
            .iter()
            .map(|candidate| self.hydrate_agentic_memory(&envelope, &candidate.memory))
            .collect::<Result<Vec<_>, _>>()?;
        let approved = if candidates.is_empty() {
            BTreeSet::new()
        } else {
            let decision = self.agentic_decision_route().map_err(read_error)?;
            select_memories(
                decision.as_ref(),
                AgenticMemoryOperation::Query,
                staged.query(),
                &candidates,
                None,
                &[],
            )?
        };
        staged.record_evaluated(candidates.iter().map(|candidate| candidate.memory.clone()))?;
        let content = candidates
            .into_iter()
            .filter(|candidate| approved.contains(&candidate.memory))
            .collect::<Vec<_>>();
        let outcome = if content.is_empty() {
            AgenticMemoryOutcome::NoMatches
        } else {
            AgenticMemoryOutcome::Memories(content)
        };
        let prepared = prepare_result(
            delivery_id,
            AgenticMemoryOperation::Query,
            outcome,
            Vec::new(),
            &envelope,
        );
        let prepared = self
            .with_agentic_read_delivery(instance, &envelope, || Ok(prepared))
            .map_err(auth_error)?;
        staged.commit(controller)?;
        Ok(prepared)
    }

    fn hydrate_agentic_memory(
        &self,
        envelope: &AgenticReadEnvelope,
        memory: &MemoryRef,
    ) -> Result<AgenticMemoryContent, AgenticMemoryError> {
        let owner = envelope
            .owners
            .iter()
            .find(|owner| owner.token.owner_id == memory.owner_id)
            .ok_or_else(|| auth_error("Memory is outside the authorized REL closure"))?;
        self.with_rel(&owner.token, |runtime| {
            let body = runtime.cva.memory(memory.memory_id).map_err(operation)?;
            if body.archived {
                return Err(operation(
                    "agentic Memory candidate is no longer searchable",
                ));
            }
            Ok(AgenticMemoryContent {
                memory: memory.clone(),
                title: body.title,
                content: body.content,
            })
        })
        .map_err(read_error)
    }
}

pub(super) fn prepare_result(
    delivery_id: &str,
    operation: AgenticMemoryOperation,
    outcome: AgenticMemoryOutcome,
    anchors: Vec<crate::AgenticMemoryAnchorReport>,
    envelope: &AgenticReadEnvelope,
) -> PreparedAgenticMemoryResult {
    let delivered = match &outcome {
        AgenticMemoryOutcome::Memories(content) => content
            .iter()
            .map(|item| item.memory.clone())
            .collect::<BTreeSet<_>>(),
        _ => BTreeSet::new(),
    };
    let owner_cuts = envelope
        .owners
        .iter()
        .filter(|owner| {
            delivered
                .iter()
                .any(|memory| memory.owner_id == owner.token.owner_id)
        })
        .map(|owner| (owner.token.owner_id.clone(), owner.cut))
        .collect();
    PreparedAgenticMemoryResult {
        delivery_id: delivery_id.into(),
        payload: AgenticMemoryPayload {
            operation,
            outcome,
            anchors,
        },
        delivered,
        owner_cuts,
    }
}

pub(super) fn read_error(error: impl std::fmt::Display) -> AgenticMemoryError {
    AgenticMemoryError::Retrieval(error.to_string())
}

pub(super) fn auth_error(error: impl std::fmt::Display) -> AgenticMemoryError {
    AgenticMemoryError::Authorization(error.to_string())
}

#[cfg(test)]
#[path = "runtime_host_gateway_agentic_memory_query_tests.rs"]
mod tests;
