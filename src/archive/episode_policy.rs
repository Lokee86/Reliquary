use crate::{ArchiveError, EpisodeConfig};

pub const DEFAULT_EPISODE_INACTIVITY_NS: i64 = 15 * 60 * 1_000_000_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EpisodePolicy {
    pub episode: EpisodeConfig,
    pub inactivity_ns: i64,
}

impl Default for EpisodePolicy {
    fn default() -> Self {
        Self {
            episode: EpisodeConfig::default(),
            inactivity_ns: DEFAULT_EPISODE_INACTIVITY_NS,
        }
    }
}

impl EpisodePolicy {
    pub fn validate(self) -> Result<Self, ArchiveError> {
        if self.episode.max_input_bytes == 0 || self.inactivity_ns <= 0 {
            return Err(ArchiveError::InvalidEpisodeConfig);
        }
        Ok(self)
    }

    pub fn inactive(self, last_source_ns: i64, now_ns: i64) -> bool {
        now_ns.saturating_sub(last_source_ns) >= self.inactivity_ns
    }
}
