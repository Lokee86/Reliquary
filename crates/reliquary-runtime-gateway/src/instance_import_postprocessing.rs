//! Named authenticated import postprocessing operations.
use super::*;
use reliquary_application::{
    DreamClassifier, DreamVerificationPolicy, DreamVerifier, DreamWorkflowError, EmbeddingEndpoint,
    EntityMemoryMention, commit_native_import_memory_vectors, evaluate_memory_vectors,
    native_import_workflow::{
        NativeImportDreamCandidates, NativeImportPostprocessingOutcome,
        NativeImportPostprocessingPlan, commit_native_import_dream,
        commit_native_import_entity_mention, finish_native_import_postprocessing,
        prepare_native_import_dream, prepare_native_import_entity_mention,
        publish_native_import_relationship,
    },
    prepare_compatibility_profile, verify_compatibility_profile,
};
use reliquary_owner_session::{
    CreateRelationshipRequest,
    model::{CompatibilityProfileId, MemoryEntityResolution, MemoryId, Relationship},
};
use reliquary_switchboard::GeneralEndpoint;

impl Instance {
    pub fn native_import_memory_ids(
        &self,
        owner: OwnerId,
        import_id: &str,
    ) -> Result<Vec<MemoryId>, GatewayError> {
        Ok(self.with_reliquary(owner, |read| read.native_import_memory_ids(import_id))??)
    }

    /// Return only accepted routes to the explicitly authorized PHY.
    pub fn native_import_routed_memory_ids(
        &self,
        owner: OwnerId,
        import_id: &str,
        destination: OwnerId,
    ) -> Result<Vec<MemoryId>, GatewayError> {
        self.authorize_routed_owners(owner, destination)?;
        let ids = self.with_reliquary(owner, |read| {
            read.native_import_routed_memory_ids(import_id, destination.as_bytes())
        })??;
        self.authorize_routed_owners(owner, destination)?;
        Ok(ids)
    }

    /// True means settled. False means verification withheld the frontier.
    /// Retry reuses the pinned candidates or the accepted publication.
    pub fn process_native_import_dream<C: GeneralEndpoint, V: GeneralEndpoint>(
        &self,
        owner: OwnerId,
        import_id: &str,
        source: MemoryId,
        candidates: NativeImportDreamCandidates<'_>,
        classifier: &DreamClassifier<C>,
        verifier: Option<&DreamVerifier<V>>,
        policy: DreamVerificationPolicy,
        updated_at_ns: i64,
    ) -> Result<bool, GatewayError> {
        let prepared = self
            .with_rel_writer(owner, |writer| {
                prepare_native_import_dream(writer, import_id, source, candidates)
            })?
            .map_err(postprocessing_error)?;
        let Some(prepared) = prepared else {
            return Ok(true);
        };
        let evaluated = prepared
            .evaluate(classifier, verifier, policy)
            .map_err(postprocessing_error)?;
        let result = self.with_rel_writer(owner, |writer| {
            commit_native_import_dream(writer, &evaluated, updated_at_ns)
        })?;
        match result {
            Ok(_) => Ok(true),
            Err(DreamWorkflowError::IncompleteFrontier) => Ok(false),
            Err(error) => Err(postprocessing_error(error)),
        }
    }

    pub fn resolve_native_import_entity_mention<E: GeneralEndpoint>(
        &self,
        import_id: &str,
        input: EntityMemoryMention,
        endpoint: E,
        updated_at_ns: i64,
    ) -> Result<MemoryEntityResolution, GatewayError> {
        let owner = OwnerId::from_bytes(input.owner_id);
        let prepared = self
            .with_rel_writer(owner, |writer| {
                prepare_native_import_entity_mention(writer, import_id, input)
            })?
            .map_err(postprocessing_error)?;
        if let Some(existing) = prepared.existing_resolution() {
            return Ok(existing.clone());
        }
        let proposal = prepared
            .evaluate_with_endpoint(endpoint)
            .map_err(postprocessing_error)?;
        self.with_rel_writer(owner, |writer| {
            commit_native_import_entity_mention(writer, import_id, proposal, updated_at_ns)
        })?
        .map_err(postprocessing_error)
    }

    pub fn publish_native_import_relationship(
        &self,
        owner: OwnerId,
        import_id: &str,
        request: CreateRelationshipRequest,
    ) -> Result<Relationship, GatewayError> {
        Ok(self.with_rel_writer(owner, |writer| {
            publish_native_import_relationship(writer, import_id, request)
        })??)
    }

    pub fn publish_native_import_embedding_profile(
        &self,
        owner: OwnerId,
        import_id: &str,
        endpoint: &impl EmbeddingEndpoint,
    ) -> Result<CompatibilityProfileId, GatewayError> {
        let accepted = self.with_rel_writer(owner, |writer| {
            writer.validate_native_import_postprocessing(import_id)?;
            writer
                .read_session()
                .native_import_processing_profile(import_id)
        })??;
        if let Some(profile) = accepted {
            verify_compatibility_profile(&profile, endpoint).map_err(postprocessing_error)?;
            return Ok(profile.id);
        }
        let profile = prepare_compatibility_profile(endpoint).map_err(postprocessing_error)?;
        Ok(self.with_rel_writer(owner, |writer| {
            writer.publish_native_import_profile(import_id, profile)
        })??)
    }

    pub fn native_import_missing_memory_vectors(
        &self,
        owner: OwnerId,
        import_id: &str,
        profile: CompatibilityProfileId,
    ) -> Result<Vec<MemoryId>, GatewayError> {
        Ok(self.with_rel_writer(owner, |writer| {
            writer.native_import_missing_memory_vectors(import_id, profile)
        })??)
    }

    pub fn publish_native_import_memory_vectors(
        &self,
        owner: OwnerId,
        import_id: &str,
        profile: CompatibilityProfileId,
        ids: &[MemoryId],
        endpoint: &impl EmbeddingEndpoint,
    ) -> Result<bool, GatewayError> {
        let input = self.with_rel_writer(owner, |writer| {
            writer.native_import_memory_vector_input(import_id, profile, ids)
        })??;
        let prepared = evaluate_memory_vectors(input, endpoint).map_err(postprocessing_error)?;
        Ok(self.with_rel_writer(owner, |writer| {
            commit_native_import_memory_vectors(writer, import_id, &prepared)
        })??)
    }

    pub fn finish_native_import_postprocessing(
        &self,
        owner: OwnerId,
        import_id: &str,
        plan: &NativeImportPostprocessingPlan,
        completed_at_ns: i64,
    ) -> Result<NativeImportPostprocessingOutcome, GatewayError> {
        Ok(self.with_rel_writer(owner, |writer| {
            finish_native_import_postprocessing(writer, import_id, plan, completed_at_ns)
        })??)
    }
}

fn postprocessing_error(error: impl std::fmt::Display) -> GatewayError {
    OwnerSessionError::Record(error.to_string()).into()
}
