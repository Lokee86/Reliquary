//! Authenticated REL search, per-turn retrieval, and trusted Memory delivery.
use crate::{
    CapabilityRouter, ConfiguredRuntime, Instance, MemoryId, OwnerGrant, OwnerId, ProductError,
    ProductErrorKind,
};
use reliquary_application::RetrievalAction;
use reliquary_runtime_gateway::{GatewayError, MemoryRetrievalRoute, MemoryRetrievalTurn};
use reliquary_switchboard::DecisionConsumer;
mod embedding;
mod outcome;
use embedding::embedding_endpoint;
pub use outcome::{PreparedMemoryDelivery, PreparedRetrieval, RetrievalOutcome};

pub use reliquary_application::{
    RetrievalAnchorReport, RetrievalDirection, RetrievalRelationEvidence,
};
pub use reliquary_runtime_gateway::{
    RetrievalCut, RetrievalMemory, RetrievalMode, RetrievalPreview, RetrievalSource, SearchMode,
    SearchResult,
};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SearchOptions {
    pub mode: SearchMode,
    pub limit: Option<usize>,
    pub embedding_route: Option<String>,
}

/// Ephemeral controller bound to one authenticated Instance, REL, and external turn.
pub struct RetrievalSession {
    inner: MemoryRetrievalTurn,
    owner: OwnerId,
    mode: RetrievalMode,
    embedding_route: Option<String>,
}

impl RetrievalSession {
    pub fn owner(&self) -> OwnerId {
        self.owner
    }
    pub fn mode(&self) -> RetrievalMode {
        self.mode
    }
}

impl Instance {
    /// Passive search over the current REL cut. Lexical search invokes no provider.
    pub fn search(
        &self,
        owner: OwnerId,
        query: &str,
        options: SearchOptions,
        runtime: Option<&ConfiguredRuntime>,
    ) -> Result<SearchResult, ProductError> {
        self.require_rel_read(owner)?;
        let query = validate_text(query)?;
        if options
            .limit
            .is_some_and(|limit| limit == 0 || limit > crate::MAX_MANAGEMENT_PAGE_ITEMS)
        {
            return Err(ProductError::invalid(
                "search limit must be between 1 and 128",
            ));
        }
        let embedding = if options.mode == SearchMode::Lexical {
            if options.embedding_route.is_some() {
                return Err(ProductError::invalid(
                    "embedding route requires hybrid or semantic search",
                ));
            }
            None
        } else {
            let endpoint = runtime
                .and_then(|runtime| embedding_endpoint(runtime, options.embedding_route.as_deref()))
                .ok_or_else(|| unavailable("requested search embedding route is unavailable"))?;
            Some(
                self.embed_retrieval_query(owner, query, endpoint)
                    .map_err(retrieval_error)?,
            )
        };
        self.inner
            .search_rel_memories_with_policy(
                owner.into(),
                query,
                options.mode,
                options.limit,
                embedding,
                runtime.map(|runtime| runtime.retrieval).unwrap_or_default(),
            )
            .map_err(retrieval_error)
    }

    pub fn begin_retrieval(
        &self,
        owner: OwnerId,
        turn_id: &str,
        mode: RetrievalMode,
    ) -> Result<RetrievalSession, ProductError> {
        self.begin_retrieval_with_embedding_route(owner, turn_id, mode, None)
    }

    pub fn begin_retrieval_with_embedding_route(
        &self,
        owner: OwnerId,
        turn_id: &str,
        mode: RetrievalMode,
        embedding_route: Option<&str>,
    ) -> Result<RetrievalSession, ProductError> {
        self.require_rel_read(owner)?;
        let inner = self
            .inner
            .begin_memory_retrieval(owner.into(), turn_id, mode)?;
        Ok(RetrievalSession {
            inner,
            owner,
            mode,
            embedding_route: embedding_route.map(str::to_owned),
        })
    }

    pub fn memory_query(
        &self,
        session: &RetrievalSession,
        query: &str,
        runtime: &ConfiguredRuntime,
    ) -> Result<RetrievalOutcome, ProductError> {
        self.memory_action(
            session,
            RetrievalAction::Query(validate_text(query)?.into()),
            runtime,
        )
    }

    pub fn refine_query(
        &self,
        session: &RetrievalSession,
        feedback: &str,
        runtime: &ConfiguredRuntime,
    ) -> Result<RetrievalOutcome, ProductError> {
        self.memory_action(
            session,
            RetrievalAction::Refine(validate_text(feedback)?.into()),
            runtime,
        )
    }

    pub fn traverse_graph(
        &self,
        session: &RetrievalSession,
        anchors: &[MemoryId],
        runtime: &ConfiguredRuntime,
    ) -> Result<RetrievalOutcome, ProductError> {
        if anchors.is_empty() || anchors.len() > 10 {
            return Err(ProductError::invalid(
                "traversal requires 1 through 10 anchors",
            ));
        }
        self.memory_action(
            session,
            RetrievalAction::Traverse(anchors.to_vec()),
            runtime,
        )
    }

    pub fn cancel_retrieval(&self, session: &RetrievalSession) -> Result<(), ProductError> {
        self.require_rel_read(session.owner)?;
        self.inner
            .cancel_memory_retrieval(&session.inner)
            .map_err(retrieval_error)
    }

    pub fn preview_retrieval(
        &self,
        session: &RetrievalSession,
        prepared: &PreparedMemoryDelivery,
    ) -> Result<RetrievalPreview, ProductError> {
        self.require_rel_read(session.owner)?;
        self.inner
            .preview_agentic_memory_delivery(&session.inner, prepared.inner())
            .map_err(retrieval_error)
    }

    /// Returns whether this invocation recorded the exact confirmed-use receipt.
    pub fn deliver_retrieval(
        &self,
        session: &RetrievalSession,
        prepared: &PreparedMemoryDelivery,
        router: &CapabilityRouter,
    ) -> Result<bool, ProductError> {
        self.require(OwnerGrant::OperateRel(session.owner))?;
        self.inner
            .deliver_agentic_memory(&session.inner, prepared.inner(), router)
            .map_err(retrieval_error)
    }

    /// Reconciles the retained manifest through the registered trusted host.
    pub fn recover_retrieval(
        &self,
        session: &RetrievalSession,
        stable_id: &str,
        router: &CapabilityRouter,
    ) -> Result<bool, ProductError> {
        self.require(OwnerGrant::OperateRel(session.owner))?;
        self.inner
            .recover_agentic_memory_delivery(&session.inner, stable_id, router)
            .map_err(retrieval_error)
    }

    fn memory_action(
        &self,
        session: &RetrievalSession,
        action: RetrievalAction,
        runtime: &ConfiguredRuntime,
    ) -> Result<RetrievalOutcome, ProductError> {
        self.require_rel_read(session.owner)?;
        let route = runtime
            .decision_endpoint_for(DecisionConsumer::MemoryRetrieval)
            .map(MemoryRetrievalRoute::from_endpoint)
            .ok_or_else(|| unavailable("Memory retrieval decision route is unavailable"))?;
        let query = match &action {
            RetrievalAction::Query(query) => Some(query.clone()),
            _ => None,
        };
        let endpoint = query
            .as_ref()
            .and_then(|_| embedding_endpoint(runtime, session.embedding_route.as_deref()));
        if query.is_some() && session.embedding_route.is_some() && endpoint.is_none() {
            return Err(unavailable(
                "requested retrieval embedding route is unavailable",
            ));
        }
        self.inner
            .evaluate_agentic_memory(&session.inner, &route, action, || {
                query
                    .zip(endpoint)
                    .map(|(query, endpoint)| {
                        self.embed_retrieval_query(session.owner, &query, endpoint)
                    })
                    .transpose()
            })
            .map(Into::into)
            .map_err(retrieval_error)
    }
}

fn validate_text(text: &str) -> Result<&str, ProductError> {
    reliquary_retrieval::agentic::validate_query(text).map_err(ProductError::invalid)
}

fn unavailable(message: &str) -> ProductError {
    ProductError::new(ProductErrorKind::Unavailable, message)
}

fn retrieval_error(error: GatewayError) -> ProductError {
    match error {
        GatewayError::HostStatusUnknown => {
            ProductError::new(ProductErrorKind::Conflict, error.to_string())
        }
        GatewayError::RetrievalUnavailable(reason) => unavailable(reason),
        _ => error.into(),
    }
}

#[cfg(test)]
#[path = "retrieval/tests.rs"]
mod tests;
