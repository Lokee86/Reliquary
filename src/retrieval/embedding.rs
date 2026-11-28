use super::{ConfiguredRuntime, GatewayError, Instance, OwnerId, RetrievalSession};
use reliquary_application::prepare_compatibility_profile;
use reliquary_archive::redact_embedding_text;
use reliquary_owner_session::model::CompatibilityProfile;
use reliquary_runtime_gateway::MemoryQueryEmbedding;
use reliquary_switchboard::EmbeddingEndpoint;
use reliquary_vectors::{
    CompatibilityProfileId, SwitchboardEmbeddingEndpoint, compare_profiles, embed_batch_bounded,
};
use std::sync::Arc;

pub(super) enum EmbeddingScope {
    Memory,
    Archive,
}

impl Instance {
    pub(super) fn embed_scoped_query(
        &self,
        session: &RetrievalSession,
        phy: OwnerId,
        query: &str,
        endpoint: Arc<dyn EmbeddingEndpoint>,
    ) -> Result<Option<MemoryQueryEmbedding>, GatewayError> {
        let phy_profiles = self.inner.phy_retrieval_embedding_profiles(phy.into())?;
        let rel_profiles = session
            .rel
            .map(|rel| self.inner.scoped_rel_embedding_profiles(rel.into()))
            .transpose()?
            .unwrap_or_default();
        if phy_profiles.is_empty() && rel_profiles.is_empty() {
            return Ok(None);
        }

        self.inner.validate_memory_retrieval(&session.inner)?;
        let endpoint = SwitchboardEmbeddingEndpoint::new(endpoint);
        let observed = observed_profile(&endpoint)?;
        let ambiguous = "embedding route matches multiple active Memory profiles in one owner";
        let rel_profile = matching_profile(&rel_profiles, &observed, ambiguous)?;
        let phy_profile = matching_profile(&phy_profiles, &observed, ambiguous)?;
        if phy_profile.is_none() && rel_profile.is_none() {
            return Ok(None);
        }
        self.inner.validate_memory_retrieval(&session.inner)?;
        let vector = embed_query(&endpoint, query.to_owned())?;
        self.inner.validate_memory_retrieval(&session.inner)?;
        Ok(Some(MemoryQueryEmbedding {
            rel_profile,
            phy_profile,
            vector,
        }))
    }

    pub(super) fn embed_retrieval_query(
        &self,
        owner: OwnerId,
        query: &str,
        endpoint: Arc<dyn EmbeddingEndpoint>,
        scope: EmbeddingScope,
    ) -> Result<(CompatibilityProfileId, Vec<f32>), GatewayError> {
        let (profiles, missing, incompatible, ambiguous) = match scope {
            EmbeddingScope::Memory => (
                self.inner.retrieval_embedding_profiles(owner.into())?,
                "no active Memory embedding profile with vectors",
                "embedding route is incompatible with active Memory profiles",
                "embedding route matches multiple active Memory profiles",
            ),
            EmbeddingScope::Archive => (
                self.inner.archive_embedding_profiles(owner.into())?,
                "no active Archive embedding profile with vectors",
                "embedding route is incompatible with active Archive profiles",
                "embedding route matches multiple active Archive profiles",
            ),
        };
        if profiles.is_empty() {
            return Err(GatewayError::RetrievalUnavailable(missing));
        }
        let endpoint = SwitchboardEmbeddingEndpoint::new(endpoint);
        let observed = observed_profile(&endpoint)?;
        self.inner.ensure_credential_active()?;
        let profile = matching_profile(&profiles, &observed, ambiguous)?
            .ok_or(GatewayError::RetrievalUnavailable(incompatible))?;
        let query = match scope {
            EmbeddingScope::Memory => query.to_owned(),
            EmbeddingScope::Archive => redact_embedding_text(query).into_owned(),
        };
        let vector = embed_query(&endpoint, query)?;
        self.inner.ensure_credential_active()?;
        Ok((profile, vector))
    }
}

fn observed_profile(
    endpoint: &SwitchboardEmbeddingEndpoint,
) -> Result<CompatibilityProfile, GatewayError> {
    prepare_compatibility_profile(endpoint)
        .map_err(|_| GatewayError::Retrieval("embedding compatibility probe failed".into()))
}

fn matching_profile(
    profiles: &[CompatibilityProfile],
    observed: &CompatibilityProfile,
    ambiguous: &'static str,
) -> Result<Option<CompatibilityProfileId>, GatewayError> {
    if profiles.iter().any(|profile| profile.id == observed.id) {
        return Ok(Some(observed.id));
    }
    let mut compatible = profiles
        .iter()
        .filter(|profile| compare_profiles(profile, observed).compatible);
    let profile = compatible.next().map(|profile| profile.id);
    if compatible.next().is_some() {
        return Err(GatewayError::RetrievalUnavailable(ambiguous));
    }
    Ok(profile)
}

fn embed_query(
    endpoint: &SwitchboardEmbeddingEndpoint,
    query: String,
) -> Result<Vec<f32>, GatewayError> {
    let mut vectors = embed_batch_bounded(
        endpoint,
        reliquary_vectors::EmbeddingMode::Query,
        &[query],
        8 << 20,
    )
    .map_err(|_| GatewayError::Retrieval("query embedding failed".into()))?;
    Ok(vectors.remove(0))
}

pub(super) fn embedding_endpoint(
    runtime: &ConfiguredRuntime,
    route: Option<&str>,
) -> Option<Arc<dyn EmbeddingEndpoint>> {
    match route {
        Some(name) => runtime.embedding_endpoint_for(name),
        None => runtime.embedding_endpoint(),
    }
}
