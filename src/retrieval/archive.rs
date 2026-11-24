use super::{EmbeddingScope, embedding_endpoint, retrieval_error, unavailable, validate_text};
use crate::{
    ArchiveHybridSearch, ArchiveLexicalIndexPublication, CompatibilityProfileId, ConfiguredRuntime,
    Instance, OwnerGrant, OwnerId, ProductError, RetrievalConfig, TemporalArchiveHybridSearch,
};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ArchiveSearchOptions {
    pub limit: Option<usize>,
    pub embedding_route: Option<String>,
}

impl Instance {
    /// Persist lexical indexes for the active Archive generations in a writable REL.
    pub fn publish_archive_lexical_indexes(
        &self,
        owner: OwnerId,
    ) -> Result<Vec<ArchiveLexicalIndexPublication>, ProductError> {
        self.require(OwnerGrant::OperateRel(owner))?;
        self.inner
            .publish_archive_lexical_indexes(owner.into())
            .map_err(retrieval_error)
    }

    /// Hybrid search over the accepted Archive generation using a configured query embedding.
    pub fn search_archive(
        &self,
        owner: OwnerId,
        text: &str,
        options: ArchiveSearchOptions,
        runtime: &ConfiguredRuntime,
    ) -> Result<ArchiveHybridSearch, ProductError> {
        self.require_rel_read(owner)?;
        let text = validate_text(text)?;
        let policy = archive_policy(runtime.retrieval, options.limit)?;
        let endpoint = embedding_endpoint(runtime, options.embedding_route.as_deref())
            .ok_or_else(|| unavailable("requested search embedding route is unavailable"))?;
        let (profile, query) = self
            .embed_retrieval_query(owner, text, endpoint, EmbeddingScope::Archive)
            .map_err(retrieval_error)?;
        self.inner
            .search_archive_hybrid(owner.into(), profile, text, &query, policy)
            .map_err(retrieval_error)
    }

    /// Search the active Archive generation with a source or mentioned-time window.
    pub fn search_archive_temporal(
        &self,
        owner: OwnerId,
        text: &str,
        options: ArchiveSearchOptions,
        runtime: &ConfiguredRuntime,
        filter: crate::TemporalFilter,
    ) -> Result<TemporalArchiveHybridSearch, ProductError> {
        self.require_rel_read(owner)?;
        let text = validate_text(text)?;
        if filter.end_exclusive_ns <= filter.start_ns {
            return Err(ProductError::invalid("invalid temporal filter"));
        }
        let policy = archive_policy(runtime.retrieval, options.limit)?;
        let endpoint = embedding_endpoint(runtime, options.embedding_route.as_deref())
            .ok_or_else(|| unavailable("requested search embedding route is unavailable"))?;
        let (profile, query) = self
            .embed_retrieval_query(owner, text, endpoint, EmbeddingScope::Archive)
            .map_err(retrieval_error)?;
        self.inner
            .search_archive_hybrid_temporal(owner.into(), profile, text, &query, policy, filter)
            .map_err(retrieval_error)
    }

    /// Temporal Archive search with an already qualified embedding.
    pub fn search_archive_hybrid_temporal(
        &self,
        owner: OwnerId,
        profile: CompatibilityProfileId,
        text: &str,
        query: &[f32],
        limit: usize,
        filter: crate::TemporalFilter,
    ) -> Result<TemporalArchiveHybridSearch, ProductError> {
        self.require_rel_read(owner)?;
        let text = validate_text(text)?;
        if filter.end_exclusive_ns <= filter.start_ns {
            return Err(ProductError::invalid("invalid temporal filter"));
        }
        validate_archive_vector(query)?;
        let policy = archive_policy(RetrievalConfig::default(), Some(limit))?;
        self.inner
            .search_archive_hybrid_temporal(owner.into(), profile, text, query, policy, filter)
            .map_err(retrieval_error)
    }

    /// Hybrid search using an embedding already qualified for this Archive profile.
    pub fn search_archive_hybrid(
        &self,
        owner: OwnerId,
        profile: CompatibilityProfileId,
        text: &str,
        query: &[f32],
        limit: usize,
    ) -> Result<ArchiveHybridSearch, ProductError> {
        self.require_rel_read(owner)?;
        let text = validate_text(text)?;
        validate_archive_vector(query)?;
        let policy = archive_policy(RetrievalConfig::default(), Some(limit))?;
        self.inner
            .search_archive_hybrid(owner.into(), profile, text, query, policy)
            .map_err(retrieval_error)
    }
}

fn archive_policy(
    mut policy: RetrievalConfig,
    limit: Option<usize>,
) -> Result<RetrievalConfig, ProductError> {
    let limit = limit.unwrap_or(policy.result_limit);
    validate_archive_limit(limit)?;
    policy.result_limit = limit;
    policy.candidate_limit = policy.candidate_limit.max(limit);
    Ok(policy)
}

pub(super) fn validate_archive_limit(limit: usize) -> Result<(), ProductError> {
    if !(1..=128).contains(&limit) {
        return Err(ProductError::invalid(
            "Archive search limit must be between 1 and 128",
        ));
    }
    Ok(())
}

pub(super) fn validate_archive_vector(query: &[f32]) -> Result<(), ProductError> {
    if query.is_empty() || query.iter().any(|v| !v.is_finite()) || query.iter().all(|v| *v == 0.0) {
        return Err(ProductError::invalid("invalid Archive query vector"));
    }
    Ok(())
}
