use crate::EpisodeId;
pub use reliquary_insomnia::InsomniaSemanticStage;

pub const DEFAULT_INSOMNIA_PROGRESS_INTERVAL_SECS: u64 = 10;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InsomniaProgressEvent {
    DrainStarted {
        total: usize,
        complete: usize,
        terminal: usize,
        workers: usize,
    },
    EpisodeStarted {
        episode_id: EpisodeId,
        worker_id: String,
        attempt: u32,
        turns: usize,
        total: usize,
        complete: usize,
        terminal: usize,
    },
    EpisodeStage {
        episode_id: EpisodeId,
        worker_id: String,
        attempt: u32,
        stage: InsomniaSemanticStage,
        items: usize,
    },
    Heartbeat {
        elapsed_ms: u64,
        total: usize,
        complete: usize,
        terminal: usize,
        active_workers: usize,
        claimed_attempts: usize,
        failed_attempts: usize,
    },
    EpisodeCompleted {
        episode_id: EpisodeId,
        attempt: u32,
        elapsed_ms: u64,
        total: usize,
        complete: usize,
        terminal: usize,
        project_created: usize,
        project_existing: usize,
        user_created: usize,
        user_existing: usize,
        rejected: usize,
        evidence_turns: usize,
    },
    EpisodeRetry {
        episode_id: EpisodeId,
        attempt: u32,
        elapsed_ms: u64,
        retry_after_ms: u64,
        backpressure: bool,
        error: String,
        total: usize,
        complete: usize,
        terminal: usize,
    },
    EpisodeTerminal {
        episode_id: EpisodeId,
        attempt: u32,
        elapsed_ms: u64,
        error: String,
        total: usize,
        complete: usize,
        terminal: usize,
    },
    VectorizationStarted {
        owner: String,
        memories: usize,
    },
    VectorizationCompleted {
        owner: String,
        embedded: usize,
        already_present: usize,
    },
}

pub trait InsomniaProgressReporter: Send + Sync {
    fn report(&self, event: &InsomniaProgressEvent);
}

impl<F> InsomniaProgressReporter for F
where
    F: Fn(&InsomniaProgressEvent) + Send + Sync,
{
    fn report(&self, event: &InsomniaProgressEvent) {
        self(event);
    }
}
