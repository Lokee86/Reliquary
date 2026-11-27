use super::{ConfiguredRuntime, GatewayError, Instance, OwnerId, RetrievalSession};
use reliquary_application::prepare_compatibility_profile;
use reliquary_archive::redact_embedding_text;
use reliquary_switchboard::EmbeddingEndpoint;
use reliquary_vectors::{SwitchboardEmbeddingEndpoint, compare_profiles, embed_batch_bounded};
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
    ) -> Result<Option<(reliquary_vectors::CompatibilityProfileId, Vec<f32>)>, GatewayError> {
        let mut profiles = self.inner.phy_retrieval_embedding_profiles(phy.into())?;
        if let Some(rel) = session.rel {
            profiles.extend(self.inner.retrieval_embedding_profiles(rel.into())?);
        }
        profiles.sort_by_key(|profile| profile.id);
        profiles.dedup_by_key(|profile| profile.id);
        if profiles.is_empty() {
            return Ok(None);
        }
        self.embed_with_profiles(query, endpoint, &profiles, EmbeddingScope::Memory, || {
            self.inner.validate_memory_retrieval(&session.inner)
        })
        .map(Some)
    }

    pub(super) fn embed_retrieval_query(
        &self,
        owner: OwnerId,
        query: &str,
        endpoint: Arc<dyn EmbeddingEndpoint>,
        scope: EmbeddingScope,
    ) -> Result<(reliquary_vectors::CompatibilityProfileId, Vec<f32>), GatewayError> {
        let profiles = match scope {
            EmbeddingScope::Memory => self.inner.retrieval_embedding_profiles(owner.into())?,
            EmbeddingScope::Archive => self.inner.archive_embedding_profiles(owner.into())?,
        };
        self.embed_with_profiles(query, endpoint, &profiles, scope, || {
            self.inner.ensure_credential_active()
        })
    }

    fn embed_with_profiles(
        &self,
        query: &str,
        endpoint: Arc<dyn EmbeddingEndpoint>,
        profiles: &[reliquary_owner_session::model::CompatibilityProfile],
        scope: EmbeddingScope,
        authorize: impl Fn() -> Result<(), GatewayError>,
    ) -> Result<(reliquary_vectors::CompatibilityProfileId, Vec<f32>), GatewayError> {
        let (missing, incompatible, ambiguous) = match scope {
            EmbeddingScope::Memory => (
                "no active Memory embedding profile with vectors",
                "embedding route is incompatible with active Memory profiles",
                "embedding route matches multiple active Memory profiles",
            ),
            EmbeddingScope::Archive => (
                "no active Archive embedding profile with vectors",
                "embedding route is incompatible with active Archive profiles",
                "embedding route matches multiple active Archive profiles",
            ),
        };
        if profiles.is_empty() {
            return Err(GatewayError::RetrievalUnavailable(missing));
        }
        authorize()?;
        let endpoint = SwitchboardEmbeddingEndpoint::new(endpoint);
        let observed = prepare_compatibility_profile(&endpoint)
            .map_err(|_| GatewayError::Retrieval("embedding compatibility probe failed".into()))?;
        authorize()?;
        let profile = if let Some(exact) = profiles.iter().find(|profile| profile.id == observed.id)
        {
            exact
        } else {
            let mut compatible = profiles
                .iter()
                .filter(|profile| compare_profiles(profile, &observed).compatible);
            let profile = compatible
                .next()
                .ok_or(GatewayError::RetrievalUnavailable(incompatible))?;
            if compatible.next().is_some() {
                return Err(GatewayError::RetrievalUnavailable(ambiguous));
            }
            profile
        };
        let query = match scope {
            EmbeddingScope::Memory => query.to_owned(),
            EmbeddingScope::Archive => redact_embedding_text(query).into_owned(),
        };
        let mut vectors = embed_batch_bounded(
            &endpoint,
            reliquary_vectors::EmbeddingMode::Query,
            &[query],
            8 << 20,
        )
        .map_err(|_| GatewayError::Retrieval("query embedding failed".into()))?;
        authorize()?;
        Ok((profile.id, vectors.remove(0)))
    }
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
