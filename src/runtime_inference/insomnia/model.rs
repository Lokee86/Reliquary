use crate::{EpisodeBuildResult, EpisodeId, MemoryId, MemoryRef};

pub const MAX_INSOMNIA_CANDIDATES: usize = 64;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum InsomniaPriority {
    ImmediateLive,
    Live,
    Import,
}

impl InsomniaPriority {
    pub(crate) fn tag(self) -> u8 {
        match self {
            Self::ImmediateLive => 1,
            Self::Live => 2,
            Self::Import => 3,
        }
    }

    pub(crate) fn from_tag(tag: u8) -> Option<Self> {
        match tag {
            1 => Some(Self::ImmediateLive),
            2 => Some(Self::Live),
            3 => Some(Self::Import),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InsomniaWorkState {
    Pending,
    Processing,
    Complete,
    Failed,
    Terminal,
}

impl InsomniaWorkState {
    pub(crate) fn tag(self) -> u8 {
        match self {
            Self::Pending => 1,
            Self::Processing => 2,
            Self::Complete => 3,
            Self::Failed => 4,
            Self::Terminal => 5,
        }
    }

    pub(crate) fn from_tag(tag: u8) -> Option<Self> {
        match tag {
            1 => Some(Self::Pending),
            2 => Some(Self::Processing),
            3 => Some(Self::Complete),
            4 => Some(Self::Failed),
            5 => Some(Self::Terminal),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct InsomniaLeaseToken(pub [u8; 32]);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InsomniaWork {
    pub episode_id: EpisodeId,
    pub priority: InsomniaPriority,
    pub state: InsomniaWorkState,
    pub attempt_count: u32,
    pub lease_owner: Option<String>,
    pub lease_token: Option<InsomniaLeaseToken>,
    pub lease_expires_ns: Option<i64>,
    pub retry_after_ns: Option<i64>,
    pub last_error: Option<String>,
    pub updated_at_ns: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InsomniaAttempt {
    pub episode_id: EpisodeId,
    pub attempt: u32,
    pub state: InsomniaWorkState,
    pub started_at_ns: i64,
    pub completed_at_ns: i64,
    pub extractor_model: String,
    pub extractor_version: String,
    pub memory_ids: Vec<MemoryId>,
    pub external_memory_refs: Vec<MemoryRef>,
    pub rejected_count: u32,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EpisodeSchedulingResult {
    pub episodes: EpisodeBuildResult,
    pub queued: Vec<InsomniaWork>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InsomniaStats {
    pub total: usize,
    pub pending: usize,
    pub processing: usize,
    pub complete: usize,
    pub failed: usize,
    pub terminal: usize,
    pub attempts: usize,
}
