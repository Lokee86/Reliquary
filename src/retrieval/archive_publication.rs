use super::{embedding_endpoint, retrieval_error, unavailable};
use crate::{
    CompatibilityProfileId, ConfiguredRuntime, Instance, OwnerGrant, OwnerId, ProductError,
};
use reliquary_vectors::SwitchboardEmbeddingEndpoint;

/// Accepted Archive vector generation for an owner and embedding profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
pub struct ArchiveVectorPublication {
    pub generation_id: [u8; 32],
    pub compatibility_profile_id: CompatibilityProfileId,
    pub source_archive_version: u64,
    pub vector_version: u64,
}

impl Instance {
    /// Embed the accepted Archive with a configured route and activate the complete generation.
    /// Use a writable mounted REL after any open native import epoch has completed.
    pub fn publish_archive_vectors(
        &self,
        owner: OwnerId,
        runtime: &ConfiguredRuntime,
        embedding_route: Option<&str>,
    ) -> Result<Option<ArchiveVectorPublication>, ProductError> {
        self.require(OwnerGrant::OperateRel(owner))?;
        let endpoint = embedding_endpoint(runtime, embedding_route)
            .ok_or_else(|| unavailable("requested Archive embedding route is unavailable"))?;
        let endpoint = SwitchboardEmbeddingEndpoint::new(endpoint);
        let profile = self
            .inner
            .publish_embedding_profile(owner.into(), &endpoint)
            .map_err(retrieval_error)?;
        let generation = self
            .inner
            .publish_archive_vectors(owner.into(), profile, &endpoint)
            .map_err(retrieval_error)?;
        Ok(generation.map(|value| ArchiveVectorPublication {
            generation_id: value.id.0,
            compatibility_profile_id: value.compatibility_profile_id,
            source_archive_version: value.source_archive_version,
            vector_version: value.vector_version,
        }))
    }
}
