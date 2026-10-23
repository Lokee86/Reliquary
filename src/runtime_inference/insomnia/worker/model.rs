use crate::{
    ArchiveError, CompatibilityProfileError, CompatibilityProfileId, InsomniaError,
    InsomniaProcessError, MemoryVectorError, MemoryVectorId,
};
use std::fmt;

pub const DEFAULT_INSOMNIA_WORKERS: usize = 48;
pub const DEFAULT_INSOMNIA_SCOPE: &str = "private";
pub const MAX_INSOMNIA_WORKERS: usize = 64;
pub const DEFAULT_INSOMNIA_LEASE_NS: i64 = 15 * 60 * 1_000_000_000;
pub const DEFAULT_INSOMNIA_RETRY_DELAY_NS: i64 = 5 * 1_000_000_000;
pub const DEFAULT_INSOMNIA_POLL_NS: i64 = 50 * 1_000_000;
pub const DEFAULT_INSOMNIA_MAX_ATTEMPTS: u32 = 5;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InsomniaWorkerConfig {
    pub workers: usize,
    pub scope: String,
    pub worker_id_prefix: String,
    pub lease_duration_ns: i64,
    pub retry_delay_ns: i64,
    pub poll_interval_ns: i64,
    pub max_attempts: u32,
}

impl Default for InsomniaWorkerConfig {
    fn default() -> Self {
        Self {
            workers: DEFAULT_INSOMNIA_WORKERS,
            scope: DEFAULT_INSOMNIA_SCOPE.into(),
            worker_id_prefix: "insomnia".into(),
            lease_duration_ns: DEFAULT_INSOMNIA_LEASE_NS,
            retry_delay_ns: DEFAULT_INSOMNIA_RETRY_DELAY_NS,
            poll_interval_ns: DEFAULT_INSOMNIA_POLL_NS,
            max_attempts: DEFAULT_INSOMNIA_MAX_ATTEMPTS,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct InsomniaDrainResult {
    pub workers: usize,
    pub peak_active_workers: usize,
    pub claimed_attempts: usize,
    pub completed_episodes: usize,
    pub failed_attempts: usize,
    pub terminal_episodes: usize,
    pub paused_for_backpressure: bool,
    pub memories_created: usize,
    pub memories_existing: usize,
    pub user_memories_created: usize,
    pub user_memories_existing: usize,
    pub rejected_candidates: usize,
    pub evidence_turns: usize,
    pub memory_vector_profile_id: Option<CompatibilityProfileId>,
    pub memory_vectors_embedded: usize,
    pub memory_vectors_already_present: usize,
    pub memory_vector_set_id: Option<MemoryVectorId>,
    pub user_memory_vector_profile_id: Option<CompatibilityProfileId>,
    pub user_memory_vectors_embedded: usize,
    pub user_memory_vectors_already_present: usize,
    pub user_memory_vector_set_id: Option<MemoryVectorId>,
}

#[derive(Debug)]
pub enum InsomniaWorkerError {
    InvalidConfig(&'static str),
    Archive(ArchiveError),
    Queue(InsomniaError),
    Process(InsomniaProcessError),
    CompatibilityProfile(CompatibilityProfileError),
    MemoryVectors(MemoryVectorError),
    LockPoisoned,
    ThreadPanicked,
}

impl fmt::Display for InsomniaWorkerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidConfig(field) => {
                write!(f, "invalid Insomnia worker configuration: {field}")
            }
            Self::Archive(error) => write!(f, "{error}"),
            Self::Queue(error) => write!(f, "{error}"),
            Self::Process(error) => write!(f, "{error}"),
            Self::CompatibilityProfile(error) => write!(f, "{error}"),
            Self::MemoryVectors(error) => write!(f, "{error}"),
            Self::LockPoisoned => write!(f, "Insomnia worker CVA lock is poisoned"),
            Self::ThreadPanicked => write!(f, "Insomnia worker thread panicked"),
        }
    }
}

impl std::error::Error for InsomniaWorkerError {}

impl From<ArchiveError> for InsomniaWorkerError {
    fn from(value: ArchiveError) -> Self {
        Self::Archive(value)
    }
}

impl From<InsomniaError> for InsomniaWorkerError {
    fn from(value: InsomniaError) -> Self {
        Self::Queue(value)
    }
}

impl From<InsomniaProcessError> for InsomniaWorkerError {
    fn from(value: InsomniaProcessError) -> Self {
        Self::Process(value)
    }
}

impl From<CompatibilityProfileError> for InsomniaWorkerError {
    fn from(value: CompatibilityProfileError) -> Self {
        Self::CompatibilityProfile(value)
    }
}

impl From<MemoryVectorError> for InsomniaWorkerError {
    fn from(value: MemoryVectorError) -> Self {
        Self::MemoryVectors(value)
    }
}

pub(crate) struct RuntimeInsomniaClaim {
    pub(crate) work: crate::InsomniaWork,
    pub(crate) episode: crate::Episode,
    pub(crate) turns: Vec<crate::ResolvedTurn>,
    pub(crate) started_at_ns: i64,
}
