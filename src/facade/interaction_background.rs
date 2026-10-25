//! Supported offline background product composition; mounted workers use native execution.
use crate::runtime_shared::unix_now_ns;
use crate::{
    DEFAULT_DREAM_FRONTIER_SIZE, DEFAULT_DREAM_INFERENCE_CONCURRENCY, DreamCandidateConfig,
    DreamProcessError, DreamProcessor, DreamVerificationPolicy, EmbeddingEndpoint, GeneralEndpoint,
    InsomniaDrainResult, InsomniaExtractor, InsomniaWorkerConfig, InsomniaWorkerError, MemoryError,
    MemoryId, RelExecution,
};
use std::fmt;

use crate::{
    RuntimeBackgroundConfig, RuntimeBackgroundError, RuntimeBackgroundResult,
    RuntimeDreamCompletion, RuntimeDreamFailure,
};

impl RelExecution {
    pub fn process_background_work<E, M, C, V>(
        &mut self,
        insomnia_extractor: &InsomniaExtractor<E>,
        embedding_endpoint: &M,
        dream_processor: &DreamProcessor<C, V>,
        config: RuntimeBackgroundConfig,
    ) -> Result<RuntimeBackgroundResult, RuntimeBackgroundError>
    where
        E: GeneralEndpoint,
        M: EmbeddingEndpoint,
        C: GeneralEndpoint,
        V: GeneralEndpoint,
    {
        if config.max_dream_memories == 0 {
            return Err(RuntimeBackgroundError::InvalidConfig("max_dream_memories"));
        }
        if config.dream_frontier_size == 0 {
            return Err(RuntimeBackgroundError::InvalidConfig("dream_frontier_size"));
        }
        if config.dream_inference_concurrency == 0 {
            return Err(RuntimeBackgroundError::InvalidConfig(
                "dream_inference_concurrency",
            ));
        }

        let insomnia = crate::insomnia::worker_runtime::drain_with_vectors(
            &mut self.cva,
            insomnia_extractor,
            embedding_endpoint,
            &config.insomnia,
        )?;
        let dream_now_ns = unix_now_ns();
        let pending = pending_dream_memories_at(&mut self.cva, dream_now_ns)?;
        if pending.is_empty() {
            self.cva
                .sync_owner()
                .map_err(RuntimeBackgroundError::Sync)?;
            return Ok(RuntimeBackgroundResult {
                insomnia,
                dream_attempted: 0,
                dream_completed: Vec::new(),
                dream_failures: Vec::new(),
                dream_remaining: 0,
            });
        }

        let profile_id = insomnia
            .memory_vector_profile_id
            .ok_or(RuntimeBackgroundError::MissingMemoryVectorProfile)?;
        let attempted: Vec<_> = pending
            .into_iter()
            .take(config.max_dream_memories)
            .collect();
        let attempted_ids: Vec<_> = attempted.iter().map(|(id, _)| *id).collect();
        let mut completed = Vec::new();
        let mut failures = Vec::new();
        let outcomes = dream_processor
            .process_memories_with_concurrency(
                &mut self.cva,
                profile_id,
                &attempted_ids,
                config.dream_candidates,
                config.dream_verification,
                config.dream_frontier_size,
                config.dream_inference_concurrency,
            )
            .map_err(RuntimeBackgroundError::Dream)?;

        for outcome in outcomes {
            match outcome.result {
                Ok(result) => {
                    if let Some((_, epoch)) = attempted
                        .iter()
                        .find(|(memory_id, _)| *memory_id == outcome.memory_id)
                    {
                        self.cva
                            .mark_dream_processed(outcome.memory_id, *epoch, unix_now_ns())?;
                    }
                    completed.push(RuntimeDreamCompletion {
                        memory_id: outcome.memory_id,
                        candidate_count: result.candidate_count,
                        lifecycle_state: result.source.lifecycle_state,
                        archived: result.source.archived,
                    });
                }
                Err(
                    error @ (DreamProcessError::Classification(_)
                    | DreamProcessError::Verification(_)),
                ) => failures.push(RuntimeDreamFailure {
                    memory_id: outcome.memory_id,
                    error: error.to_string(),
                }),
                Err(error) => {
                    self.cva
                        .sync_owner()
                        .map_err(RuntimeBackgroundError::Sync)?;
                    return Err(RuntimeBackgroundError::Dream(error));
                }
            }
        }

        self.cva
            .sync_owner()
            .map_err(RuntimeBackgroundError::Sync)?;
        let remaining = pending_dream_memories_at(&mut self.cva, dream_now_ns)?.len();
        Ok(RuntimeBackgroundResult {
            insomnia,
            dream_attempted: attempted_ids.len(),
            dream_completed: completed,
            dream_failures: failures,
            dream_remaining: remaining,
        })
    }
}

fn pending_dream_memories_at(
    cva: &mut crate::RelExecutionOwner,
    now_ns: i64,
) -> Result<Vec<(MemoryId, u64)>, MemoryError> {
    let ids = cva.runtime_memory_ids();
    let mut pending = Vec::new();
    for id in ids {
        if let Some(epoch) = cva.dream_eligible_epoch(id, now_ns)? {
            pending.push((id, epoch));
        }
    }
    Ok(pending)
}
