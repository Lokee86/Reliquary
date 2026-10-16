use super::query::{auth_error, read_error};
use super::*;
use crate::agentic_memory_retrieval::StagedAgenticMemoryOperation;
use crate::agentic_memory_retrieval_decision::rewrite_query;
use crate::{AgenticMemoryController, AgenticMemoryError};

impl GatewayRuntime {
    /// Rewrite preparation never changes CurrentQuery; reconsideration must succeed first.
    fn stage_agentic_refinement(
        &self,
        instance: &InstanceRuntime,
        controller: &AgenticMemoryController,
        refinement: &str,
    ) -> Result<(AgenticReadEnvelope, StagedAgenticMemoryOperation), AgenticMemoryError> {
        let current = controller
            .current_query()
            .ok_or(AgenticMemoryError::NoActiveQuery)?;
        let envelope = self.capture_agentic_read(instance).map_err(auth_error)?;
        let decision = self.agentic_decision_route().map_err(read_error)?;
        let revised = rewrite_query(decision.as_ref(), current, refinement)?;
        let staged = controller.stage_refinement(&revised)?;
        self.with_agentic_read_delivery(instance, &envelope, || Ok(()))
            .map_err(auth_error)?;
        Ok((envelope, staged))
    }
}

impl GatewayRuntime {
    /// Reconsider exactly the previously evaluated, not-yet-delivered set.
    /// No ordinary retrieval or embedding is performed by this operation.
    pub fn refine_query(
        &self,
        instance: &InstanceRuntime,
        controller: &mut AgenticMemoryController,
        refinement: &str,
        delivery_id: &str,
    ) -> Result<crate::PreparedAgenticMemoryResult, AgenticMemoryError> {
        use crate::agentic_memory_retrieval_decision::select_memories;
        use crate::{AgenticMemoryOperation, AgenticMemoryOutcome};
        if delivery_id.trim().is_empty() {
            return Err(AgenticMemoryError::InvalidDeliveryIdentity);
        }
        let (envelope, mut staged) =
            self.stage_agentic_refinement(instance, controller, refinement)?;
        let declined = controller.declined_ids().into_iter().collect::<Vec<_>>();
        let mut content = Vec::new();
        if !declined.is_empty() {
            let decision = self.agentic_decision_route().map_err(read_error)?;
            for batch in declined.chunks(10) {
                let candidates = batch
                    .iter()
                    .map(|memory| self.hydrate_agentic_memory(&envelope, memory))
                    .collect::<Result<Vec<_>, _>>()?;
                let approved = select_memories(
                    decision.as_ref(),
                    AgenticMemoryOperation::Refinement,
                    staged.query(),
                    &candidates,
                    None,
                    &[],
                )?;
                staged.record_evaluated(batch.iter().cloned())?;
                content.extend(
                    candidates
                        .into_iter()
                        .filter(|candidate| approved.contains(&candidate.memory)),
                );
            }
        }
        let outcome = if content.is_empty() {
            AgenticMemoryOutcome::NoMatches
        } else {
            AgenticMemoryOutcome::Memories(content)
        };
        let prepared = super::query::prepare_result(
            delivery_id,
            AgenticMemoryOperation::Refinement,
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn refinement_rewrite_is_staged_and_errors_preserve_current_query() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target/test-tmp")
            .join(format!("agentic-rewrite-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let route = Arc::new(crate::SimulatedDecisionEndpoint::new(
            "rewrite",
            vec![
                json!({"version": 1, "query": "new full query"}),
                json!({"version": 1, "query": ""}),
            ],
        ));
        let gateway = GatewayRuntime::new(
            root,
            ReliquaryRuntimeRoutes::default().with_memory_retrieval_decision(Some(route)),
            InsomniaWorkerConfig {
                workers: 0,
                ..Default::default()
            },
            EpisodePolicy::default(),
        );
        let instance = gateway
            .attach_instance("rewrite-reader".into(), None, BTreeMap::new())
            .unwrap();
        let mut controller = AgenticMemoryController::new();
        assert!(matches!(
            gateway.stage_agentic_refinement(&instance, &controller, "correct"),
            Err(AgenticMemoryError::NoActiveQuery)
        ));
        controller
            .stage_query("old full query")
            .unwrap()
            .commit(&mut controller)
            .unwrap();
        let before = controller.clone();
        let (_, staged) = gateway
            .stage_agentic_refinement(&instance, &controller, "correct")
            .unwrap();
        assert_eq!(staged.query(), "new full query");
        assert_eq!(controller, before);
        assert!(
            gateway
                .stage_agentic_refinement(&instance, &controller, "correct")
                .is_err()
        );
        assert_eq!(controller, before);
    }
}

#[cfg(test)]
#[path = "runtime_host_gateway_agentic_memory_refinement_tests.rs"]
mod reconsideration_tests;
