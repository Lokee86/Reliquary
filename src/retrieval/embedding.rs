use super::{ConfiguredRuntime, GatewayError, Instance, OwnerId};
use reliquary_application::prepare_compatibility_profile;
use reliquary_switchboard::EmbeddingEndpoint;
use reliquary_vectors::{SwitchboardEmbeddingEndpoint, compare_profiles, embed_batch_bounded};
use std::sync::Arc;

impl Instance {
    pub(super) fn embed_retrieval_query(
        &self,
        owner: OwnerId,
        query: &str,
        endpoint: Arc<dyn EmbeddingEndpoint>,
    ) -> Result<(reliquary_vectors::CompatibilityProfileId, Vec<f32>), GatewayError> {
        let profiles = self.inner.retrieval_embedding_profiles(owner.into())?;
        if profiles.is_empty() {
            return Err(GatewayError::RetrievalUnavailable(
                "no active Memory embedding profile with vectors",
            ));
        }
        let endpoint = SwitchboardEmbeddingEndpoint::new(endpoint);
        let observed = prepare_compatibility_profile(&endpoint)
            .map_err(|_| GatewayError::Retrieval("embedding compatibility probe failed".into()))?;
        self.inner.ensure_credential_active()?;
        let profile = if let Some(exact) = profiles.iter().find(|profile| profile.id == observed.id)
        {
            exact
        } else {
            let mut compatible = profiles
                .iter()
                .filter(|profile| compare_profiles(profile, &observed).compatible);
            let profile = compatible.next().ok_or_else(|| {
                GatewayError::RetrievalUnavailable(
                    "embedding route is incompatible with active Memory profiles",
                )
            })?;
            if compatible.next().is_some() {
                return Err(GatewayError::RetrievalUnavailable(
                    "embedding route matches multiple active Memory profiles",
                ));
            }
            profile
        };
        let mut vectors = embed_batch_bounded(
            &endpoint,
            reliquary_vectors::EmbeddingMode::Query,
            &[query.to_owned()],
            8 << 20,
        )
        .map_err(|_| GatewayError::Retrieval("query embedding failed".into()))?;
        self.inner.ensure_credential_active()?;
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
