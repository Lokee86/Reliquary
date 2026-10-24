use crate::runtime_shared::unix_now_ns;
use crate::{
    DEFAULT_DREAM_FRONTIER_SIZE, DEFAULT_DREAM_INFERENCE_CONCURRENCY, DreamCandidateConfig,
    DreamProcessError, DreamProcessor, DreamVerificationPolicy, EmbeddingEndpoint, GeneralEndpoint,
    InsomniaDrainResult, InsomniaExtractor, InsomniaWorkerConfig, InsomniaWorkerError, MemoryError,
    MemoryId, RelExecution,
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
