use crate::EpisodeId;
use std::sync::mpsc::SyncSender;

pub(crate) type InsomniaStageAcknowledgement = Result<(), Box<dyn std::any::Any + Send>>;

pub const DEFAULT_INSOMNIA_PROGRESS_INTERVAL_SECS: u64 = 10;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InsomniaSemanticStage {
    Ledger,
    LedgerTurnRepair,
    LedgerFieldRepair,
    Evidence,
    Metadata,
    Ownership,
    Wording,
    Enrichment,
    Persistence,
}

/// Bounded value emitted by Insomnia extraction before a semantic stage runs.
/// The runtime gateway consumes these events and attaches worker context.
#[derive(Debug)]
pub(crate) struct InsomniaSemanticProgress {
    pub(crate) stage: InsomniaSemanticStage,
    pub(crate) items: usize,
    pub(crate) acknowledged: SyncSender<InsomniaStageAcknowledgement>,
}

impl InsomniaSemanticStage {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ledger => "ledger",
            Self::LedgerTurnRepair => "ledger_turn_repair",
            Self::LedgerFieldRepair => "ledger_field_repair",
            Self::Evidence => "evidence",
            Self::Metadata => "metadata",
            Self::Ownership => "ownership",
            Self::Wording => "wording",
            Self::Enrichment => "enrichment",
            Self::Persistence => "persistence",
        }
    }
}

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
