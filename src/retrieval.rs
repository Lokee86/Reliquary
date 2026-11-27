//! Authenticated search, per-turn retrieval, and trusted Memory delivery.
use crate::{
    CapabilityRouter, CompatibilityProfileId, ConfiguredRuntime, Instance, MemoryId, OwnerGrant,
    OwnerId, ProductError, ProductErrorKind, TemporalFilter,
};
use reliquary_application::RetrievalAction;
use reliquary_runtime_gateway::{
    GatewayError, MemoryQueryEmbedding, MemoryRetrievalRoute, MemoryRetrievalTurn,
};
use reliquary_switchboard::DecisionConsumer;
mod archive;
mod archive_publication;
mod chat;
mod embedding;
mod outcome;
pub use archive::ArchiveSearchOptions;
use archive::{validate_archive_limit, validate_archive_vector};
pub use archive_publication::ArchiveVectorPublication;
pub use chat::{ChatArchiveHit, ChatArchiveResult, ChatMemoryResult, ChatRetrieval};
use embedding::{EmbeddingScope, embedding_endpoint};
pub use outcome::{PreparedMemoryDelivery, PreparedRetrieval, RetrievalOutcome};

pub use reliquary_application::{
    ResolvedArchiveTemporalQuery, RetrievalAnchorReport, RetrievalDirection,
    RetrievalRelationEvidence, TemporalQueryError, resolve_archive_temporal_query,
};
pub use reliquary_runtime_gateway::{
    ArchiveHybridHit, ArchiveHybridSearch, ArchiveLexicalIndexPublication,
    ArchiveTurnTemporalEvidence, ArchiveVectorHit, ArchiveVectorSearch, MemoryTemporalMatch,
    RetrievalCut, RetrievalDeliveryOutcome, RetrievalMemory, RetrievalMemoryReference,
    RetrievalMode, RetrievalObservation, RetrievalPreview, RetrievalRankedHit, RetrievalSource,
    SearchMode, SearchResult, TemporalArchiveHitEvidence, TemporalArchiveHybridSearch,
};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SearchOptions {
    pub mode: SearchMode,
    pub limit: Option<usize>,
    pub embedding_route: Option<String>,
}

/// Ordinary Memory hits plus the normalized window and each hit's dated evidence.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct TemporalSearchResult {
    #[serde(flatten)]
    pub search: SearchResult,
    pub temporal_filter: crate::TemporalFilter,
    pub temporal_evidence: Vec<MemoryTemporalMatch>,
}

/// The active private owner of a trusted host profile. Invalidate the binding
/// before switching private owners or replacing the profile.
#[derive(Clone)]
pub struct ActivePhyRecall(reliquary_runtime_gateway::ActivePhyRecall);

impl ActivePhyRecall {
    pub fn new(owner: OwnerId) -> Self {
        Self(reliquary_runtime_gateway::ActivePhyRecall::new(
            owner.into(),
        ))
    }

    pub fn owner(&self) -> OwnerId {
        self.0.owner().into()
    }

    pub fn invalidate(&self) {
        self.0.invalidate();
    }
}

/// Ephemeral controller bound to an authenticated Instance, owner scope, and external turn.
pub struct RetrievalSession {
    inner: MemoryRetrievalTurn,
    owner: OwnerId,
    rel: Option<OwnerId>,
    phy: Option<OwnerId>,
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

    pub fn rel_owner(&self) -> Option<OwnerId> {
        self.rel
    }

    pub fn phy_owner(&self) -> Option<OwnerId> {
        self.phy
    }
}

impl Instance {
    /// Reports whether this authorized owner has vectors under an active
    /// profile. Hosts can bind non-secret runtime metadata before first
    /// publication without opening the owner store.
    pub fn has_active_vectors(&self, owner: OwnerId) -> Result<bool, ProductError> {
        let rel_access = self.require_rel_read(owner);
        if rel_access.is_err() && self.require_phy_read(owner).is_err() {
            rel_access?;
        }
        self.inner
            .has_active_vectors(owner.into())
            .map_err(Into::into)
    }

    /// Search the accepted Archive vectors for an explicit compatibility profile.
    /// The caller supplies an embedding already qualified for that profile.
    pub fn search_archive_vectors(
        &self,
        owner: OwnerId,
        profile_id: CompatibilityProfileId,
        query: &[f32],
        limit: usize,
    ) -> Result<ArchiveVectorSearch, ProductError> {
        self.require_rel_read(owner)?;
        validate_archive_limit(limit)?;
        validate_archive_vector(query)?;
        self.inner
            .search_archive_vectors(owner.into(), profile_id, query, limit)
            .map_err(retrieval_error)
    }

    /// Passive search over the current REL cut. Lexical search invokes no provider.
    pub fn search(
        &self,
        owner: OwnerId,
        query: &str,
        options: SearchOptions,
        runtime: Option<&ConfiguredRuntime>,
    ) -> Result<SearchResult, ProductError> {
        self.search_with_filter(owner, query, options, runtime, None)
            .map(|(search, _)| search)
    }

    /// Passive Memory search with one explicit date basis and half-open window.
    pub fn search_temporal(
        &self,
        owner: OwnerId,
        query: &str,
        options: SearchOptions,
        runtime: Option<&ConfiguredRuntime>,
        filter: crate::TemporalFilter,
    ) -> Result<TemporalSearchResult, ProductError> {
        if filter.start_ns >= filter.end_exclusive_ns {
            return Err(ProductError::invalid(
                "temporal window must have start before end",
            ));
        }
        let (search, temporal_evidence) =
            self.search_with_filter(owner, query, options, runtime, Some(filter))?;
        Ok(TemporalSearchResult {
            search,
            temporal_filter: filter,
            temporal_evidence,
        })
    }

    fn search_with_filter(
        &self,
        owner: OwnerId,
        query: &str,
        options: SearchOptions,
        runtime: Option<&ConfiguredRuntime>,
        filter: Option<crate::TemporalFilter>,
    ) -> Result<(SearchResult, Vec<MemoryTemporalMatch>), ProductError> {
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
                self.embed_retrieval_query(owner, query, endpoint, EmbeddingScope::Memory)
                    .map_err(retrieval_error)?,
            )
        };
        self.inner
            .search_rel_memories_with_filter(
                owner.into(),
                query,
                options.mode,
                options.limit,
                embedding,
                runtime.map(|runtime| runtime.retrieval).unwrap_or_default(),
                filter,
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
            rel: Some(owner),
            phy: None,
            mode,
            embedding_route: embedding_route.map(str::to_owned),
        })
    }

    /// Bind ordinary recall to the authorized active PHY and optional REL.
    pub fn begin_scoped_retrieval(
        &self,
        rel: Option<OwnerId>,
        phy: ActivePhyRecall,
        turn_id: &str,
        mode: RetrievalMode,
    ) -> Result<RetrievalSession, ProductError> {
        if let Some(rel) = rel {
            self.require_rel_read(rel)?;
        }
        let private = phy.owner();
        self.require_phy_read(private)?;
        let inner =
            self.inner
                .begin_scoped_memory_retrieval(rel.map(Into::into), phy.0, turn_id, mode)?;
        Ok(RetrievalSession {
            inner,
            owner: rel.unwrap_or(private),
            rel,
            phy: Some(private),
            mode,
            embedding_route: None,
        })
    }

    /// Recheck the authenticated owner scope and active mount before use.
    pub fn validate_retrieval(&self, session: &RetrievalSession) -> Result<(), ProductError> {
        if let Some(owner) = session.rel {
            self.require_rel_read(owner)?;
        }
        if let Some(owner) = session.phy {
            self.require_phy_read(owner)?;
        }
        self.inner
            .validate_memory_retrieval(&session.inner)
            .map_err(retrieval_error)
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
            None,
        )
    }

    /// Start a scoped ordinary query with a shared limit on returned hits.
    pub fn memory_query_limited(
        &self,
        session: &RetrievalSession,
        query: &str,
        runtime: &ConfiguredRuntime,
        limit: usize,
    ) -> Result<RetrievalOutcome, ProductError> {
        if session.phy.is_none() {
            return Err(ProductError::invalid(
                "a scoped query requires an active PHY",
            ));
        }
        if !(1..=crate::MAX_MANAGEMENT_PAGE_ITEMS).contains(&limit) {
            return Err(ProductError::invalid(
                "retrieval limit must be between 1 and 128",
            ));
        }
        let mut policy = runtime.retrieval;
        policy.result_limit = policy.result_limit.min(limit);
        self.memory_action_with_policy(
            session,
            RetrievalAction::Query(validate_text(query)?.into()),
            runtime,
            None,
            policy,
        )
    }

    /// Start a new topical query with an explicit normalized time window.
    pub fn memory_query_temporal(
        &self,
        session: &RetrievalSession,
        query: &str,
        runtime: &ConfiguredRuntime,
        filter: TemporalFilter,
    ) -> Result<RetrievalOutcome, ProductError> {
        if filter.start_ns >= filter.end_exclusive_ns {
            return Err(ProductError::invalid("retrieval window must be nonempty"));
        }
        self.memory_action(
            session,
            RetrievalAction::Query(validate_text(query)?.into()),
            runtime,
            Some(filter),
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
            None,
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
            None,
        )
    }

    /// Traverse from exact, readable Memories of a scoped ordinary result.
    pub fn traverse_scoped_graph(
        &self,
        session: &RetrievalSession,
        anchors: &[RetrievalMemoryReference],
        runtime: &ConfiguredRuntime,
    ) -> Result<RetrievalOutcome, ProductError> {
        if anchors.is_empty() || anchors.len() > 10 {
            return Err(ProductError::invalid(
                "traversal requires 1 through 10 anchors",
            ));
        }
        self.validate_retrieval(session)?;
        let ids = self
            .inner
            .validate_inspected_scoped_selection(&session.inner, anchors, &[])
            .map_err(retrieval_error)?;
        self.memory_action(session, RetrievalAction::Traverse(ids), runtime, None)
    }

    pub fn cancel_retrieval(&self, session: &RetrievalSession) -> Result<(), ProductError> {
        self.validate_retrieval(session)?;
        self.inner
            .cancel_memory_retrieval(&session.inner)
            .map_err(retrieval_error)
    }

    pub fn preview_retrieval(
        &self,
        session: &RetrievalSession,
        prepared: &PreparedMemoryDelivery,
    ) -> Result<RetrievalPreview, ProductError> {
        self.validate_retrieval(session)?;
        self.inner
            .preview_agentic_memory_delivery(&session.inner, prepared.inner())
            .map_err(retrieval_error)
    }

    /// Returns whether this invocation added a durable Memory-use receipt.
    pub fn deliver_retrieval(
        &self,
        session: &RetrievalSession,
        prepared: &PreparedMemoryDelivery,
        router: &CapabilityRouter,
    ) -> Result<bool, ProductError> {
        self.deliver_retrieval_confirmed(session, prepared, router)
            .map(RetrievalDeliveryOutcome::recorded_memory_use)
    }

    /// Confirm the exact prepared selection through the trusted host. Distinguishes
    /// a newly completed selection from a replay, including Observation-only delivery.
    pub fn deliver_retrieval_confirmed(
        &self,
        session: &RetrievalSession,
        prepared: &PreparedMemoryDelivery,
        router: &CapabilityRouter,
    ) -> Result<RetrievalDeliveryOutcome, ProductError> {
        self.validate_retrieval(session)?;
        if session.phy.is_none() || prepared.inner().has_rel_memories() {
            self.require(OwnerGrant::OperateRel(session.owner))?;
        }
        self.inner
            .deliver_agentic_memory_confirmed(&session.inner, prepared.inner(), router)
            .map_err(retrieval_error)
    }

    /// Reconciles the retained manifest through the registered trusted host.
    pub fn recover_retrieval(
        &self,
        session: &RetrievalSession,
        stable_id: &str,
        router: &CapabilityRouter,
    ) -> Result<bool, ProductError> {
        self.validate_retrieval(session)?;
        let can_record_rel = session
            .rel
            .is_some_and(|rel| self.require(OwnerGrant::OperateRel(rel)).is_ok());
        if session.phy.is_none() && !can_record_rel {
            self.require(OwnerGrant::OperateRel(session.owner))?;
        }
        self.inner
            .recover_agentic_memory_delivery_with_authority(
                &session.inner,
                stable_id,
                router,
                can_record_rel,
            )
            .map_err(retrieval_error)
    }

    fn memory_action(
        &self,
        session: &RetrievalSession,
        action: RetrievalAction,
        runtime: &ConfiguredRuntime,
        filter: Option<TemporalFilter>,
    ) -> Result<RetrievalOutcome, ProductError> {
        self.memory_action_with_policy(session, action, runtime, filter, runtime.retrieval)
    }

    fn memory_action_with_policy(
        &self,
        session: &RetrievalSession,
        action: RetrievalAction,
        runtime: &ConfiguredRuntime,
        filter: Option<TemporalFilter>,
        policy: reliquary_application::RetrievalConfig,
    ) -> Result<RetrievalOutcome, ProductError> {
        self.validate_retrieval(session)?;
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
            .evaluate_agentic_memory_with_scope_policy(
                &session.inner,
                &route,
                action,
                filter,
                policy,
                || {
                    query
                        .zip(endpoint)
                        .map(|(query, endpoint)| {
                            if let Some(phy) = session.phy {
                                self.embed_scoped_query(session, phy, &query, endpoint)
                            } else {
                                self.embed_retrieval_query(
                                    session.owner,
                                    &query,
                                    endpoint,
                                    EmbeddingScope::Memory,
                                )
                                .map(|(profile, vector)| {
                                    Some(MemoryQueryEmbedding {
                                        rel_profile: Some(profile),
                                        phy_profile: None,
                                        vector,
                                    })
                                })
                            }
                        })
                        .transpose()
                        .map(Option::flatten)
                },
            )
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
