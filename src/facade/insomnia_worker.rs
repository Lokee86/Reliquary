use crate::{
    Cva, EmbeddingEndpoint, GeneralEndpoint, InsomniaExtractor, InsomniaProgressEvent,
    InsomniaProgressReporter, Phylactery,
};

#[path = "../runtime_inference/insomnia/worker/model.rs"]
pub(crate) mod model;
use crate::insomnia::worker_runtime as runtime;
pub use model::{
    DEFAULT_INSOMNIA_LEASE_NS, DEFAULT_INSOMNIA_MAX_ATTEMPTS, DEFAULT_INSOMNIA_POLL_NS,
    DEFAULT_INSOMNIA_RETRY_DELAY_NS, DEFAULT_INSOMNIA_SCOPE, DEFAULT_INSOMNIA_WORKERS,
    InsomniaDrainResult, InsomniaWorkerConfig, InsomniaWorkerError, MAX_INSOMNIA_WORKERS,
};

impl Cva {
    pub fn drain_insomnia_backlog<E: GeneralEndpoint, V: EmbeddingEndpoint>(
        &mut self,
        extractor: &InsomniaExtractor<E>,
        embedding_endpoint: &V,
        config: InsomniaWorkerConfig,
    ) -> Result<InsomniaDrainResult, InsomniaWorkerError> {
        runtime::drain_with_vectors(self, extractor, embedding_endpoint, &config)
    }

    pub fn drain_insomnia_backlog_with_progress<E: GeneralEndpoint, V: EmbeddingEndpoint>(
        &mut self,
        extractor: &InsomniaExtractor<E>,
        embedding_endpoint: &V,
        config: InsomniaWorkerConfig,
        progress: &dyn InsomniaProgressReporter,
    ) -> Result<InsomniaDrainResult, InsomniaWorkerError> {
        runtime::validate_config(&config)?;
        let mut result = runtime::drain_with_progress(self, extractor, &config, progress)?;
        fill_project_vectors(self, embedding_endpoint, &mut result, Some(progress))?;
        Ok(result)
    }

    pub fn drain_insomnia_backlog_routed<E: GeneralEndpoint, V: EmbeddingEndpoint>(
        &mut self,
        phylactery: &mut Phylactery,
        extractor: &InsomniaExtractor<E>,
        embedding_endpoint: &V,
        config: InsomniaWorkerConfig,
    ) -> Result<InsomniaDrainResult, InsomniaWorkerError> {
        runtime::validate_config(&config)?;
        let mut result = runtime::drain_routed(self, phylactery, extractor, &config)?;
        fill_routed_vectors(self, phylactery, embedding_endpoint, &mut result, None)?;
        Ok(result)
    }

    pub fn drain_insomnia_backlog_routed_with_progress<E: GeneralEndpoint, V: EmbeddingEndpoint>(
        &mut self,
        phylactery: &mut Phylactery,
        extractor: &InsomniaExtractor<E>,
        embedding_endpoint: &V,
        config: InsomniaWorkerConfig,
        progress: &dyn InsomniaProgressReporter,
    ) -> Result<InsomniaDrainResult, InsomniaWorkerError> {
        runtime::validate_config(&config)?;
        let mut result =
            runtime::drain_routed_with_progress(self, phylactery, extractor, &config, progress)?;
        fill_routed_vectors(
            self,
            phylactery,
            embedding_endpoint,
            &mut result,
            Some(progress),
        )?;
        Ok(result)
    }
}

fn fill_routed_vectors<V: EmbeddingEndpoint>(
    cva: &mut Cva,
    phylactery: &mut Phylactery,
    embedding_endpoint: &V,
    result: &mut InsomniaDrainResult,
    progress: Option<&dyn InsomniaProgressReporter>,
) -> Result<(), InsomniaWorkerError> {
    fill_project_vectors(cva, embedding_endpoint, result, progress)?;
    let user_memories = phylactery.memory_stats().memories;
    if user_memories > 0 {
        report_vector_start(progress, "user", user_memories);
        let profile = phylactery.establish_compatibility_profile(embedding_endpoint)?;
        let vectors = phylactery.build_missing_memory_vectors(profile.id, embedding_endpoint)?;
        result.user_memory_vector_profile_id = Some(profile.id);
        result.user_memory_vectors_embedded = vectors.embedded;
        result.user_memory_vectors_already_present = vectors.already_present;
        result.user_memory_vector_set_id = vectors.created_set;
        report_vector_complete(progress, "user", vectors.embedded, vectors.already_present);
    }
    Ok(())
}

fn fill_project_vectors<V: EmbeddingEndpoint>(
    cva: &mut Cva,
    embedding_endpoint: &V,
    result: &mut InsomniaDrainResult,
    progress: Option<&dyn InsomniaProgressReporter>,
) -> Result<(), InsomniaWorkerError> {
    let memories = cva.memory_stats().memories;
    if memories == 0 {
        return Ok(());
    }
    report_vector_start(progress, "project", memories);
    let profile = cva.establish_compatibility_profile(embedding_endpoint)?;
    let vectors = cva.build_missing_memory_vectors(profile.id, embedding_endpoint)?;
    result.memory_vector_profile_id = Some(profile.id);
    result.memory_vectors_embedded = vectors.embedded;
    result.memory_vectors_already_present = vectors.already_present;
    result.memory_vector_set_id = vectors.created_set;
    report_vector_complete(
        progress,
        "project",
        vectors.embedded,
        vectors.already_present,
    );
    Ok(())
}

fn report_vector_start(
    progress: Option<&dyn InsomniaProgressReporter>,
    owner: &str,
    memories: usize,
) {
    if let Some(progress) = progress {
        progress.report(&InsomniaProgressEvent::VectorizationStarted {
            owner: owner.into(),
            memories,
        });
    }
}

fn report_vector_complete(
    progress: Option<&dyn InsomniaProgressReporter>,
    owner: &str,
    embedded: usize,
    already_present: usize,
) {
    if let Some(progress) = progress {
        progress.report(&InsomniaProgressEvent::VectorizationCompleted {
            owner: owner.into(),
            embedded,
            already_present,
        });
    }
}
