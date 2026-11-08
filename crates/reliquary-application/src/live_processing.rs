//! LIVE stage order and exact post-extraction work selection.
mod dream;
mod entity;
pub use dream::{PreparedLiveDream, commit_live_dream, prepare_live_dream};
pub use entity::{LiveEntityInput, accepted_live_mentions};

use crate::processing_plan::{ImportStage, StageState};
use reliquary_owner_session::{ManagementRead, model::MemoryId};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum LiveProcessingSelection {
    #[default]
    All,
    Insomnia,
    Entity,
    Dream,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct LiveStageProgress {
    pub stage: ImportStage,
    pub state: StageState,
    pub pending: usize,
    pub held: usize,
    pub failed: usize,
    pub completed: usize,
    pub unsupported: usize,
    pub work_ids: Vec<String>,
}

impl LiveStageProgress {
    pub fn new(stage: ImportStage) -> Self {
        Self {
            stage,
            state: StageState::Pending,
            pending: 0,
            held: 0,
            failed: 0,
            completed: 0,
            unsupported: 0,
            work_ids: Vec::new(),
        }
    }

    pub fn pending_work(&mut self, id: String) {
        self.pending += 1;
        if self.work_ids.len() < 128 {
            self.work_ids.push(id);
        }
    }

    pub fn finish_status(&mut self) {
        self.state = if self.held > 0 {
            StageState::Held
        } else if self.pending > 0 {
            StageState::Pending
        } else if self.unsupported > 0 {
            StageState::Unsupported
        } else {
            StageState::Complete
        };
    }
}

impl LiveProcessingSelection {
    pub fn drains_insomnia(self) -> bool {
        matches!(self, Self::All | Self::Insomnia)
    }

    pub fn stages(self) -> &'static [ImportStage] {
        use ImportStage::*;
        match self {
            Self::Insomnia => &[],
            Self::Entity => &[Entity],
            Self::Dream => &[Vectors, Dream],
            Self::All => &[Entity, Vectors, Dream, Relationship],
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum LiveDreamState {
    Started,
    Pending,
    Complete,
    Unavailable,
}

pub fn live_dream_state(read: &ManagementRead<'_>, source: MemoryId) -> LiveDreamState {
    match read.first_dream_pass(source) {
        Some((_, true)) => LiveDreamState::Complete,
        Some((_, false)) => LiveDreamState::Started,
        None => match read.memory_projection(source, None) {
            Ok((record, Some(_))) if !record.archived => LiveDreamState::Pending,
            _ => LiveDreamState::Unavailable,
        },
    }
}
