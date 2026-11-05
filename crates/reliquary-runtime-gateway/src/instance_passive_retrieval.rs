//! Authorized passive retrieval over the same owner cut used by agentic selection.
use super::*;
use reliquary_application::{
    MemoryRetrievalConfig, MemoryRetrievalResult, OwnerMemorySelection, RetrievalConfig,
};
use reliquary_owner_session::model::CompatibilityProfileId;

impl Instance {
    pub(crate) fn retrieval_snapshot(
        &self,
        owner: OwnerId,
        profile: Option<CompatibilityProfileId>,
    ) -> Result<reliquary_owner_session::RelRetrievalSnapshot, GatewayError> {
        match self.reliquary(owner) {
            Ok(rel) => Ok(rel.retrieval_snapshot(profile)?),
            Err(GatewayError::WriterUnavailable) => self
                .with_writable_reliquary(owner, |rel| rel.retrieval_snapshot(profile))?
                .map_err(GatewayError::from),
            Err(error) => Err(error),
        }
    }

    pub fn rank_rel_memories(
        &self,
        owner: OwnerId,
        query: &str,
        profile: Option<CompatibilityProfileId>,
        vector: Option<&[f32]>,
        config: RetrievalConfig,
    ) -> Result<OwnerMemorySelection, GatewayError> {
        let snapshot = self.retrieval_snapshot(owner, profile)?;
        let result = reliquary_application::rank_owner_snapshot(&snapshot, query, vector, config)
            .map_err(GatewayError::Retrieval)?;
        self.ensure_credential_active()?;
        Ok(result)
    }

    pub fn route_rel_memories(
        &self,
        owner: OwnerId,
        profile: CompatibilityProfileId,
        vector: &[f32],
        config: MemoryRetrievalConfig,
    ) -> Result<MemoryRetrievalResult, GatewayError> {
        let snapshot = self.retrieval_snapshot(owner, Some(profile))?;
        let result = reliquary_application::route_owner_snapshot(&snapshot, vector, config)
            .map_err(GatewayError::Retrieval)?;
        self.ensure_credential_active()?;
        Ok(result)
    }
}
