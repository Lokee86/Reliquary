use crate::{EpisodeId, InsomniaDrainResult, InsomniaStats};

#[derive(Clone, Debug)]
pub struct ConfiguredInsomniaOptions {
    pub workers: usize,
    pub scope: String,
    pub embedding_batch_size: usize,
    pub embedding_concurrency: usize,
    pub existing_queue_only: bool,
}

#[derive(Clone, Debug)]
pub struct InsomniaTerminalFailure {
    pub episode_id: EpisodeId,
    pub error: String,
}

#[derive(Clone, Debug)]
pub struct ConfiguredInsomniaReport {
    pub before: InsomniaStats,
    pub queued: InsomniaStats,
    pub final_stats: InsomniaStats,
    pub drain: InsomniaDrainResult,
    pub terminal_failures: Vec<InsomniaTerminalFailure>,
}
