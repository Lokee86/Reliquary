use crate::runtime_shared::unix_now_ns;
use crate::{
    DEFAULT_DREAM_FRONTIER_SIZE, DEFAULT_DREAM_INFERENCE_CONCURRENCY, DreamCandidateConfig,
    DreamProcessError, DreamProcessor, DreamVerificationPolicy, EmbeddingEndpoint, GeneralEndpoint,
    InsomniaDrainResult, InsomniaExtractor, InsomniaWorkerConfig, InsomniaWorkerError,
    InteractionRuntime, MemoryError, MemoryId,
};
use std::fmt;

pub const DEFAULT_RUNTIME_DREAM_BATCH: usize = 64;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeBackgroundConfig {
    pub insomnia: InsomniaWorkerConfig,
    pub dream_candidates: DreamCandidateConfig,
    pub dream_verification: DreamVerificationPolicy,
    pub dream_frontier_size: usize,
    pub dream_inference_concurrency: usize,
    pub max_dream_memories: usize,
}

impl Default for RuntimeBackgroundConfig {
    fn default() -> Self {
        Self {
            insomnia: InsomniaWorkerConfig::default(),
            dream_candidates: DreamCandidateConfig::default(),
            dream_verification: DreamVerificationPolicy::default(),
            dream_frontier_size: DEFAULT_DREAM_FRONTIER_SIZE,
            dream_inference_concurrency: DEFAULT_DREAM_INFERENCE_CONCURRENCY,
            max_dream_memories: DEFAULT_RUNTIME_DREAM_BATCH,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeDreamCompletion {
    pub memory_id: MemoryId,
    pub candidate_count: usize,
    pub lifecycle_state: String,
    pub archived: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeDreamFailure {
    pub memory_id: MemoryId,
    pub error: String,
}

#[derive(Debug)]
pub struct RuntimeBackgroundResult {
    pub insomnia: InsomniaDrainResult,
    pub dream_attempted: usize,
    pub dream_completed: Vec<RuntimeDreamCompletion>,
    pub dream_failures: Vec<RuntimeDreamFailure>,
    pub dream_remaining: usize,
}

#[derive(Debug)]
pub enum RuntimeBackgroundError {
    InvalidConfig(&'static str),
    Insomnia(InsomniaWorkerError),
    Memory(MemoryError),
    Dream(DreamProcessError),
    MissingMemoryVectorProfile,
    Sync(crate::CvaError),
}

impl fmt::Display for RuntimeBackgroundError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidConfig(field) => {
                write!(f, "invalid runtime background configuration: {field}")
            }
            Self::Insomnia(error) => write!(f, "{error}"),
            Self::Memory(error) => write!(f, "{error}"),
            Self::Dream(error) => write!(f, "{error}"),
            Self::MissingMemoryVectorProfile => write!(
                f,
                "Dream backlog exists without an established Memory vector profile"
            ),
            Self::Sync(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for RuntimeBackgroundError {}

impl From<InsomniaWorkerError> for RuntimeBackgroundError {
    fn from(value: InsomniaWorkerError) -> Self {
        Self::Insomnia(value)
    }
}

impl From<MemoryError> for RuntimeBackgroundError {
    fn from(value: MemoryError) -> Self {
        Self::Memory(value)
    }
}

impl InteractionRuntime {
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

        let insomnia = self.cva.drain_insomnia_backlog(
            insomnia_extractor,
            embedding_endpoint,
            config.insomnia,
        )?;
        let dream_now_ns = unix_now_ns();
        let pending = pending_dream_memories_at(&mut self.cva, dream_now_ns)?;
        if pending.is_empty() {
            self.cva.sync().map_err(RuntimeBackgroundError::Sync)?;
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
                    self.cva.sync().map_err(RuntimeBackgroundError::Sync)?;
                    return Err(RuntimeBackgroundError::Dream(error));
                }
            }
        }

        self.cva.sync().map_err(RuntimeBackgroundError::Sync)?;
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
    cva: &mut crate::Cva,
    now_ns: i64,
) -> Result<Vec<(MemoryId, u64)>, MemoryError> {
    let ids = cva.memories.current_ids();
    let mut pending = Vec::new();
    for id in ids {
        if let Some(epoch) = cva.dream_eligible_epoch(id, now_ns)? {
            pending.push((id, epoch));
        }
    }
    Ok(pending)
}
