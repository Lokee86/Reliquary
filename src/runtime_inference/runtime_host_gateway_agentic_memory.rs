//! REL-only authorization snapshots for turn-scoped agentic retrieval.
use super::*;
use crate::{CompatibilityProfileId, DecisionEndpoint, EmbeddingEndpoint};

struct AgenticOwnerRead {
    token: OwnerAccessToken,
    profile: Option<CompatibilityProfileId>,
    cut: u64,
}

struct AgenticReadEnvelope {
    authorization: InstanceAuthorization,
    root: Option<String>,
    owners: Vec<AgenticOwnerRead>,
}

impl GatewayRuntime {
    fn capture_agentic_read(
        &self,
        instance: &InstanceRuntime,
    ) -> Result<AgenticReadEnvelope, ReliquaryRuntimeHostError> {
        let authorization = instance.authorization().map_err(operation)?;
        let root = self.active_rel_id(instance)?;
        let mut owners = Vec::new();
        if let Some(root) = &root {
            for owner_id in self.dependency_closure_for(instance, root)? {
                let token = self.authorize(instance, &owner_id, OwnerGrant::ViewRel)?;
                let profile = {
                    let rels = self
                        .rel_executions
                        .read()
                        .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
                    let execution = rels
                        .get(&owner_id)
                        .ok_or_else(|| operation("agentic REL owner is not mounted"))?;
                    let owner = Arc::clone(&execution.owner);
                    drop(rels);
                    let owner = owner
                        .lock()
                        .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
                    let profile = *owner
                        .memory_profile
                        .lock()
                        .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
                    profile
                };
                let cut = self.with_rel(&token, |runtime| Ok(runtime.cva.rel_turn_count()))?;
                owners.push(AgenticOwnerRead {
                    token,
                    profile,
                    cut,
                });
            }
        }
        let envelope = AgenticReadEnvelope {
            authorization,
            root,
            owners,
        };
        self.with_agentic_read_delivery(instance, &envelope, || Ok(()))?;
        Ok(envelope)
    }

    fn agentic_query_embedding_route(
        &self,
    ) -> Result<Arc<dyn EmbeddingEndpoint + Send + Sync>, ReliquaryRuntimeHostError> {
        self.routes
            .read()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
            .embedding()
            .ok_or_else(|| operation("agentic Memory query requires an embedding route"))
    }

    fn agentic_decision_route(
        &self,
    ) -> Result<Arc<dyn DecisionEndpoint>, ReliquaryRuntimeHostError> {
        self.routes
            .read()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
            .memory_retrieval_decision()
            .ok_or_else(|| {
                operation("agentic Memory requires the dedicated retrieval decision route")
            })
    }

    /// No external I/O belongs inside this final short read-delivery closure.
    fn with_agentic_read_delivery<T>(
        &self,
        instance: &InstanceRuntime,
        envelope: &AgenticReadEnvelope,
        deliver: impl FnOnce() -> Result<T, ReliquaryRuntimeHostError>,
    ) -> Result<T, ReliquaryRuntimeHostError> {
        self.with_instance_delivery(instance, envelope.authorization, || {
            if self.active_rel_id(instance)? != envelope.root {
                return Err(operation(
                    "selected REL changed during agentic Memory operation",
                ));
            }
            let current = match &envelope.root {
                Some(root) => self.dependency_closure_for(instance, root)?,
                None => Vec::new(),
            };
            if current
                != envelope
                    .owners
                    .iter()
                    .map(|owner| owner.token.owner_id.clone())
                    .collect::<Vec<_>>()
            {
                return Err(operation(
                    "REL dependency closure changed during agentic Memory operation",
                ));
            }
            for owner in &envelope.owners {
                self.with_rel(&owner.token, |_| Ok(()))?;
            }
            deliver()
        })
    }
}

#[cfg(test)]
#[path = "runtime_host_gateway_agentic_memory_tests.rs"]
mod tests;

#[path = "runtime_host_gateway_agentic_memory_query.rs"]
mod query;

#[path = "runtime_host_gateway_agentic_memory_refinement.rs"]
mod refinement;

#[path = "runtime_host_gateway_agentic_memory_graph.rs"]
mod graph;
