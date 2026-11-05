//! Authenticated vector publication; inference never holds owner locks.
use super::*;
use reliquary_application::{
    EmbeddingEndpoint, PreparedMemoryVectors, commit_phy_memory_vectors, commit_rel_memory_vectors,
    evaluate_memory_vectors, prepare_compatibility_profile,
};
use reliquary_owner_session::{
    MemoryVectorInput,
    model::{CompatibilityProfileId, MemoryId},
};

pub struct PreparedVectorPublication {
    instance: Uuid,
    owner: OwnerId,
    prepared: PreparedMemoryVectors,
}
impl Instance {
    pub fn publish_archive_vectors(
        &self,
        owner: OwnerId,
        profile: CompatibilityProfileId,
        endpoint: &impl EmbeddingEndpoint,
    ) -> Result<Option<reliquary_owner_session::VectorGeneration>, GatewayError> {
        let input =
            self.with_rel_writer(owner, |writer| writer.archive_vector_input(profile))??;
        let rows = reliquary_application::evaluate_archive_vectors(&input, endpoint)
            .map_err(vector_error)?;
        Ok(self.with_rel_writer(owner, |writer| {
            writer.publish_archive_vector_batch(
                input.owner_id(),
                profile,
                input.source_archive_version(),
                &rows,
            )
        })??)
    }
    fn require_vector_writer(&self, owner: OwnerId) -> Result<(), GatewayError> {
        self.authorize_vector_owner(owner)?;
        if self.rel_grants.contains(&owner) {
            self.with_rel_writer(owner, |_| ())
        } else {
            self.with_entity_phy_writer(owner, |_| ())
        }
    }
    fn authorize_vector_owner(&self, owner: OwnerId) -> Result<(), GatewayError> {
        self.ensure_credential_active()?;
        if !self.rel_grants.contains(&owner) && !self.phy_grants.contains(&owner) {
            return Err(GatewayError::UnauthorizedRel {
                principal: self.principal().into(),
                owner,
            });
        }
        Ok(())
    }
    pub fn publish_embedding_profile(
        &self,
        owner: OwnerId,
        endpoint: &impl EmbeddingEndpoint,
    ) -> Result<CompatibilityProfileId, GatewayError> {
        self.require_vector_writer(owner)?;
        let profile = prepare_compatibility_profile(endpoint).map_err(vector_error)?;
        self.authorize_vector_owner(owner)?;
        if self.rel_grants.contains(&owner) {
            Ok(self.with_rel_writer(owner, |writer| {
                writer.publish_compatibility_profile(profile)
            })??)
        } else {
            Ok(self.with_entity_phy_writer(owner, |writer| {
                writer.publish_compatibility_profile(profile)
            })??)
        }
    }
    pub fn prepare_owner_memory_vectors(
        &self,
        owner: OwnerId,
        profile: CompatibilityProfileId,
        ids: &[MemoryId],
        endpoint: &impl EmbeddingEndpoint,
    ) -> Result<PreparedVectorPublication, GatewayError> {
        self.require_vector_writer(owner)?;
        let input = self.vector_input(owner, profile, ids)?;
        let prepared = evaluate_memory_vectors(input, endpoint).map_err(vector_error)?;
        self.authorize_vector_owner(owner)?;
        Ok(PreparedVectorPublication {
            instance: self.instance_id,
            owner,
            prepared,
        })
    }
    pub fn publish_owner_memory_vectors(
        &self,
        prepared: &PreparedVectorPublication,
    ) -> Result<bool, GatewayError> {
        self.authorize_vector_owner(prepared.owner)?;
        if self.instance_id != prepared.instance
            || prepared.prepared.owner_id() != prepared.owner.as_bytes()
        {
            return Err(GatewayError::Retrieval(
                "vector proposal belongs to another Instance".into(),
            ));
        }
        if self.rel_grants.contains(&prepared.owner) {
            Ok(self.with_rel_writer(prepared.owner, |writer| {
                commit_rel_memory_vectors(writer, &prepared.prepared)
            })??)
        } else {
            Ok(self.with_entity_phy_writer(prepared.owner, |writer| {
                commit_phy_memory_vectors(writer, &prepared.prepared)
            })??)
        }
    }
    fn vector_input(
        &self,
        owner: OwnerId,
        profile: CompatibilityProfileId,
        ids: &[MemoryId],
    ) -> Result<MemoryVectorInput, GatewayError> {
        self.authorize_vector_owner(owner)?;
        if self.rel_grants.contains(&owner) {
            match self.reliquary(owner) {
                Ok(rel) => Ok(rel.memory_vector_input(profile, ids)?),
                Err(GatewayError::WriterUnavailable) => Ok(self
                    .with_writable_reliquary(owner, |rel| {
                        rel.memory_vector_input(profile, ids)
                    })??),
                Err(error) => Err(error),
            }
        } else {
            match self.phylactery(owner) {
                Ok(phy) => Ok(phy.memory_vector_input(profile, ids)?),
                Err(GatewayError::WriterUnavailable) => Ok(self
                    .with_entity_phy_writer(owner, |writer| {
                        writer.memory_vector_input(profile, ids)
                    })??),
                Err(error) => Err(error),
            }
        }
    }
}
fn vector_error(error: impl std::fmt::Display) -> GatewayError {
    OwnerSessionError::Record(error.to_string()).into()
}

#[cfg(test)]
#[path = "instance_vectors/tests.rs"]
mod tests;
