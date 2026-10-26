use sha2::{Digest, Sha256};

pub const DEFAULT_EPISODE_MAX_INPUT_BYTES: usize = 32 << 10;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct EpisodeId(pub [u8; 32]);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EpisodeOrigin {
    Live,
    Import,
}

impl EpisodeOrigin {
    pub fn tag(self) -> u8 {
        match self {
            Self::Live => 1,
            Self::Import => 2,
        }
    }

    pub(crate) fn from_tag(tag: u8) -> Option<Self> {
        match tag {
            1 => Some(Self::Live),
            2 => Some(Self::Import),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EpisodeBoundary {
    Size,
    Inactivity,
    CreateMemory,
    ImportEnd,
    Explicit,
}

impl EpisodeBoundary {
    pub fn tag(self) -> u8 {
        match self {
            Self::Size => 1,
            Self::Inactivity => 2,
            Self::CreateMemory => 3,
            Self::ImportEnd => 4,
            Self::Explicit => 5,
        }
    }

    pub(crate) fn from_tag(tag: u8) -> Option<Self> {
        match tag {
            1 => Some(Self::Size),
            2 => Some(Self::Inactivity),
            3 => Some(Self::CreateMemory),
            4 => Some(Self::ImportEnd),
            5 => Some(Self::Explicit),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Episode {
    pub id: EpisodeId,
    pub conversation_id: String,
    pub start_node_id: String,
    pub end_node_id: String,
    pub origin: EpisodeOrigin,
    pub boundary: EpisodeBoundary,
    pub source_through_ns: i64,
    pub finalized_at_ns: i64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EpisodeConfig {
    pub max_input_bytes: usize,
}

impl Default for EpisodeConfig {
    fn default() -> Self {
        Self {
            max_input_bytes: DEFAULT_EPISODE_MAX_INPUT_BYTES,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EpisodeBuildResult {
    pub created: Vec<Episode>,
    pub has_open_tail: bool,
    pub open_from_node_id: Option<String>,
}

pub(crate) fn episode_id(
    conversation_id: &str,
    start_node_id: &str,
    end_node_id: &str,
) -> EpisodeId {
    let mut hash = Sha256::new();
    for value in [conversation_id, start_node_id, end_node_id] {
        hash.update((value.len() as u64).to_le_bytes());
        hash.update(value.as_bytes());
    }
    EpisodeId(hash.finalize().into())
}
