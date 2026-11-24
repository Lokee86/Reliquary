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

pub use reliquary_application::{
    ArchiveHybridHit, ArchiveHybridSearch, ArchiveTurnTemporalEvidence, TemporalArchiveHitEvidence,
    TemporalArchiveHybridSearch,
};
pub use reliquary_owner_session::{
    ArchiveLexicalIndexPublication, ArchiveVectorHit, ArchiveVectorSearch,
};

/// Receipt for one complete Archive vector generation.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
pub struct ArchiveVectorPublication {
    pub profile_id: CompatibilityProfileId,
    pub generation_id: [u8; 32],
    pub source_archive_version: u64,
    pub fragment_count: usize,
}

pub struct PreparedVectorPublication {
    instance: Uuid,
    owner: OwnerId,
    prepared: PreparedMemoryVectors,
}
impl Instance {
    /// Read the active Archive generation for an authorized REL.
    pub fn search_archive_vectors(
        &self,
        owner: OwnerId,
        profile_id: CompatibilityProfileId,
        query: &[f32],
        limit: usize,
    ) -> Result<ArchiveVectorSearch, GatewayError> {
        let view = self
            .with_reliquary(owner, |rel| rel.archive_vector_view(profile_id))??
            .ok_or(GatewayError::RetrievalUnavailable(
                "no active Archive vectors for the embedding profile",
            ))?;
        if query.len() != view.dimensions() {
            return Err(GatewayError::InvalidPreparedRetrieval);
        }
        let result = view.search(query, limit)?;
        self.authorize_rel_access(owner)?;
        Ok(result)
    }

    /// Active embedding profiles that have an accepted Archive generation.
    pub fn archive_embedding_profiles(
        &self,
        owner: OwnerId,
    ) -> Result<Vec<reliquary_owner_session::model::CompatibilityProfile>, GatewayError> {
        self.with_reliquary(owner, RelSession::archive_embedding_profiles)
    }

    pub fn search_archive_hybrid(
        &self,
        owner: OwnerId,
        profile_id: CompatibilityProfileId,
        text: &str,
        query: &[f32],
        policy: reliquary_application::RetrievalConfig,
    ) -> Result<ArchiveHybridSearch, GatewayError> {
        let view = self.with_reliquary(owner, |rel| {
            if rel
                .active_compatibility_profile(profile_id)
                .is_some_and(|profile| query.len() != profile.dimensions as usize)
            {
                return Err(GatewayError::InvalidPreparedRetrieval);
            }
            rel.archive_hybrid_view(profile_id)?
                .ok_or(GatewayError::RetrievalUnavailable(
                    "no active Archive vectors for the embedding profile",
                ))
        })??;
        let result = reliquary_application::rank_archive_hybrid(view, text, query, policy)
            .map_err(GatewayError::Retrieval)?;
        self.authorize_rel_access(owner)?;
        Ok(result)
    }

    pub fn search_archive_hybrid_temporal(
        &self,
        owner: OwnerId,
        profile_id: CompatibilityProfileId,
        text: &str,
        query: &[f32],
        policy: reliquary_application::RetrievalConfig,
        filter: reliquary_application::TemporalFilter,
    ) -> Result<reliquary_application::TemporalArchiveHybridSearch, GatewayError> {
        let view = self.with_reliquary(owner, |rel| {
            if rel
                .active_compatibility_profile(profile_id)
                .is_some_and(|profile| query.len() != profile.dimensions as usize)
            {
                return Err(GatewayError::InvalidPreparedRetrieval);
            }
            rel.archive_temporal_hybrid_view(profile_id)?
                .ok_or(GatewayError::RetrievalUnavailable(
                    "no active Archive vectors for the embedding profile",
                ))
        })??;
        let result =
            reliquary_application::rank_archive_hybrid_temporal(view, text, query, policy, filter)
                .map_err(GatewayError::Retrieval)?;
        self.authorize_rel_access(owner)?;
        Ok(result)
    }

    /// Build the current Archive lexical indexes from authorized source captures.
    pub fn publish_archive_lexical_indexes(
        &self,
        owner: OwnerId,
    ) -> Result<Vec<ArchiveLexicalIndexPublication>, GatewayError> {
        let mut profiles = self.with_rel_writer(owner, |writer| {
            writer
                .read_session()
                .archive_embedding_profiles()
                .into_iter()
                .map(|profile| profile.id)
                .collect::<Vec<_>>()
        })?;
        profiles.sort_unstable();
        profiles
            .into_iter()
            .map(|profile| {
                let input = self
                    .with_rel_writer(owner, |writer| writer.archive_lexical_index_input(profile))??
                    .ok_or(GatewayError::StaleRetrieval)?;
                let prepared = input.prepare()?;
                Ok(self.with_rel_writer(owner, |writer| {
                    writer.publish_archive_lexical_index(prepared)
                })??)
            })
            .collect()
    }

    pub fn publish_archive_vectors(
        &self,
        owner: OwnerId,
        profile: CompatibilityProfileId,
        endpoint: &impl EmbeddingEndpoint,
    ) -> Result<Option<reliquary_owner_session::VectorGeneration>, GatewayError> {
        self.publish_archive_vectors_for_scope(owner, None, profile, endpoint)
            .map(|(generation, _)| generation)
    }

    /// Publish a complete owner Archive generation through its accepted import
    /// epoch. The writer rechecks the epoch and cut after provider calls.
    pub fn publish_native_import_archive_vectors(
        &self,
        owner: OwnerId,
        import_id: &str,
        endpoint: &impl EmbeddingEndpoint,
    ) -> Result<ArchiveVectorPublication, GatewayError> {
        self.with_rel_writer(owner, |writer| {
            writer.validate_native_import_archive_publication(import_id)
        })??;
        let profile = prepare_compatibility_profile(endpoint).map_err(vector_error)?;
        let profile = self.with_rel_writer(owner, |writer| {
            writer.publish_native_import_archive_profile(import_id, profile)
        })??;
        let (generation, fragment_count) =
            self.publish_archive_vectors_for_scope(owner, Some(import_id), profile, endpoint)?;
        let generation =
            generation.ok_or_else(|| vector_error("Archive vector generation missing"))?;
        Ok(ArchiveVectorPublication {
            profile_id: profile,
            generation_id: generation.id.0,
            source_archive_version: generation.source_archive_version,
            fragment_count,
        })
    }

    fn publish_archive_vectors_for_scope(
        &self,
        owner: OwnerId,
        import_id: Option<&str>,
        profile: CompatibilityProfileId,
        endpoint: &impl EmbeddingEndpoint,
    ) -> Result<(Option<reliquary_owner_session::VectorGeneration>, usize), GatewayError> {
        let input = self.with_rel_writer(owner, |writer| match import_id {
            Some(import_id) => writer.native_import_archive_vector_input(import_id, profile),
            None => writer.archive_vector_input(profile),
        })??;
        let rows = reliquary_application::evaluate_archive_vectors(&input, endpoint)
            .map_err(vector_error)?;
        let generation = self.with_rel_writer(owner, |writer| match import_id {
            Some(import_id) => writer.publish_native_import_archive_vector_batch(
                import_id,
                input.owner_id(),
                profile,
                input.source_archive_version(),
                &rows,
            ),
            None => writer.publish_archive_vector_batch(
                input.owner_id(),
                profile,
                input.source_archive_version(),
                &rows,
            ),
        })??;
        Ok((generation, input.fragments().len()))
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
