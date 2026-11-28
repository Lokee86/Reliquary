use super::{Admission, Control, PhyWork};
use crate::{GatewayError, OwnerSessionError};
use reliquary_application::perception_workflow::PerceptionRoutes;
use reliquary_switchboard::{
    DecisionEndpoint, DecisionEndpointError, EmbeddingEndpoint, EmbeddingEndpointError,
    EmbeddingMode, EmbeddingNormalization, GeneralEndpoint, GeneralEndpointError,
};
use serde_json::Value;
use std::sync::{Arc, Weak, atomic::Ordering};

const STALE: &str = "PHY Perception admission changed";

#[derive(Clone)]
struct Fence {
    admission: Arc<Admission>,
    control: Weak<Control>,
    routes: Weak<PerceptionRoutes>,
}

impl Fence {
    fn check(&self) -> Result<(), GatewayError> {
        self.admission.check_mount()?;
        let control = self
            .control
            .upgrade()
            .ok_or(OwnerSessionError::ManagementStale)?;
        let routes = control
            .routes
            .read()
            .map_err(|_| GatewayError::RegistryPoisoned)?;
        if control.stopping.load(Ordering::Acquire)
            || !control.allows_owner(self.admission.owner)
            || routes
                .as_ref()
                .is_none_or(|live| !self.routes.ptr_eq(&Arc::downgrade(live)))
        {
            return Err(OwnerSessionError::ManagementStale.into());
        }
        Ok(())
    }
}

struct Endpoint<T: ?Sized> {
    fence: Fence,
    inner: Arc<T>,
}

impl GeneralEndpoint for Endpoint<dyn GeneralEndpoint> {
    fn model(&self) -> &str {
        self.inner.model()
    }

    fn complete_json(
        &self,
        prompt: &str,
        payload: &str,
        name: &str,
        schema: &Value,
    ) -> Result<Value, GeneralEndpointError> {
        self.fence
            .check()
            .map_err(|_| GeneralEndpointError::Failure(STALE.into()))?;
        self.inner.complete_json(prompt, payload, name, schema)
    }
}

impl DecisionEndpoint for Endpoint<dyn DecisionEndpoint> {
    fn model(&self) -> &str {
        self.inner.model()
    }

    fn evaluate(&self, state: &Value, questions: &Value) -> Result<Value, DecisionEndpointError> {
        self.fence
            .check()
            .map_err(|_| DecisionEndpointError::Failure(STALE.into()))?;
        self.inner.evaluate(state, questions)
    }

    fn evaluate_bounded(
        &self,
        state: &Value,
        questions: &Value,
        limit: usize,
    ) -> Result<Value, DecisionEndpointError> {
        self.fence
            .check()
            .map_err(|_| DecisionEndpointError::Failure(STALE.into()))?;
        self.inner.evaluate_bounded(state, questions, limit)
    }
}

impl EmbeddingEndpoint for Endpoint<dyn EmbeddingEndpoint> {
    fn model(&self) -> Option<&str> {
        self.inner.model()
    }
    fn dimensions(&self) -> u32 {
        self.inner.dimensions()
    }
    fn normalization(&self) -> EmbeddingNormalization {
        self.inner.normalization()
    }
    fn supports_bounded_embedding(&self) -> bool {
        self.inner.supports_bounded_embedding()
    }

    fn embed(
        &self,
        mode: EmbeddingMode,
        inputs: &[String],
    ) -> Result<Vec<Vec<f32>>, EmbeddingEndpointError> {
        self.fence
            .check()
            .map_err(|_| EmbeddingEndpointError::Failure(STALE.into()))?;
        self.inner.embed(mode, inputs)
    }

    fn embed_bounded(
        &self,
        mode: EmbeddingMode,
        inputs: &[String],
        limit: usize,
    ) -> Result<Vec<Vec<f32>>, EmbeddingEndpointError> {
        self.fence
            .check()
            .map_err(|_| EmbeddingEndpointError::Failure(STALE.into()))?;
        self.inner.embed_bounded(mode, inputs, limit)
    }
}

impl PhyWork<'_> {
    pub(super) fn fenced_routes(&self) -> PerceptionRoutes {
        let fence = Fence {
            admission: self.admission.clone(),
            control: Arc::downgrade(self.control),
            routes: Arc::downgrade(self.routes),
        };
        PerceptionRoutes {
            general: Arc::new(Endpoint {
                fence: fence.clone(),
                inner: self.routes.general.clone(),
            }),
            contribution: Arc::new(Endpoint {
                fence: fence.clone(),
                inner: self.routes.contribution.clone(),
            }),
            verification: Arc::new(Endpoint {
                fence: fence.clone(),
                inner: self.routes.verification.clone(),
            }),
            embedding: Arc::new(Endpoint {
                fence,
                inner: self.routes.embedding.clone(),
            }),
            identity: self.routes.identity,
            embedding_identity: self.routes.embedding_identity,
        }
    }
}
