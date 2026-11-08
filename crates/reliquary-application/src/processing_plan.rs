//! Work selection and progress projection shared by owner processing callers.
use reliquary_owner_session::{
    InsomniaWorkState, OwnerSessionError, RelSession, model::EpisodeId, records::ImportEpochStatus,
};
use serde::Serialize;
use sha2::{Digest, Sha256};

pub const MAX_PROCESS_EPISODES: usize = 1024;
const MAX_STATUS_IDS: usize = 128;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportStage {
    Entity,
    Relationship,
    Vectors,
    Dream,
    HistoricalFreshness,
}

// Imported entity resolution needs the original accepted Memory body. Vector
// coverage supplies the ranked Dream frontier. Settlement seals that frontier.
pub const IMPORT_POSTPROCESSING: [ImportStage; 5] = [
    ImportStage::Entity,
    ImportStage::Relationship,
    ImportStage::Vectors,
    ImportStage::Dream,
    ImportStage::HistoricalFreshness,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StageState {
    Pending,
    Executed,
    NotConfigured,
    Unsupported,
    Held,
    Failed,
    Complete,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct StageProgress {
    pub stage: ImportStage,
    pub state: StageState,
    pub required: bool,
    pub completed: usize,
}

pub fn import_stages() -> Vec<StageProgress> {
    IMPORT_POSTPROCESSING
        .into_iter()
        .map(|stage| StageProgress {
            stage,
            state: StageState::Pending,
            required: stage != ImportStage::Relationship,
            completed: 0,
        })
        .collect()
}

pub fn import_plan_identity(routes: [u8; 32]) -> [u8; 32] {
    let mut identity = Sha256::new();
    identity.update(b"reliquary-import-processing-policy-v1\0");
    identity.update(routes);
    identity
        .update(serde_json::to_vec(&import_stages()).expect("static stage policy is serializable"));
    identity.finalize().into()
}

pub enum ImportStageOutcome {
    Complete(usize),
    NotConfigured,
    Held,
}

pub enum ImportStageRun {
    Ready,
    Incomplete,
    Held,
    Stopped,
}

/// Advance only after the current stage's durable operation has completed.
pub fn coordinate_import_stages<E>(
    stopped: impl Fn() -> bool,
    mut run: impl FnMut(ImportStage) -> Result<ImportStageOutcome, E>,
    mut report: impl FnMut(ImportStage, StageState, usize),
) -> Result<ImportStageRun, E> {
    for stage in import_stages() {
        if stopped() {
            return Ok(ImportStageRun::Stopped);
        }
        report(stage.stage, StageState::Executed, 0);
        match run(stage.stage) {
            Ok(ImportStageOutcome::Complete(count)) => {
                if stopped() && stage.stage != ImportStage::HistoricalFreshness {
                    return Ok(ImportStageRun::Stopped);
                }
                report(stage.stage, StageState::Complete, count);
            }
            Ok(ImportStageOutcome::NotConfigured) => {
                report(stage.stage, StageState::NotConfigured, 0);
                if stage.required {
                    return Ok(ImportStageRun::Incomplete);
                }
            }
            Ok(ImportStageOutcome::Held) => {
                report(stage.stage, StageState::Held, 0);
                return Ok(ImportStageRun::Held);
            }
            Err(error) => {
                report(stage.stage, StageState::Failed, 0);
                return Err(error);
            }
        }
    }
    Ok(ImportStageRun::Ready)
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct InsomniaProgress {
    pub pending: usize,
    pub processing: usize,
    pub held: usize,
    pub failed: usize,
    pub complete: usize,
    pub rel_memories: usize,
    pub phy_memories: usize,
    pub phy_counts_visible: bool,
    pub rejected: u64,
    pub incomplete_ids: Vec<String>,
    pub import_complete: bool,
}

impl InsomniaProgress {
    pub fn unfinished(&self) -> usize {
        self.pending + self.processing + self.held + self.failed
    }
}

pub fn inspect_insomnia_work(
    owner: &RelSession,
    import_id: Option<&str>,
    now_ns: i64,
    max_due: usize,
) -> Result<(InsomniaProgress, Vec<EpisodeId>), OwnerSessionError> {
    if max_due > MAX_PROCESS_EPISODES {
        return Err(OwnerSessionError::Record(
            "processing bound exceeded".into(),
        ));
    }
    let mut progress = InsomniaProgress {
        import_complete: import_id.is_some_and(|id| {
            matches!(
                owner.import_status(id),
                Some(ImportEpochStatus::Completed { .. })
            )
        }),
        ..InsomniaProgress::default()
    };
    let mut due = Vec::new();
    owner.scoped_insomnia_status(import_id, now_ns, |episode| {
        match episode.state {
            InsomniaWorkState::Pending => progress.pending += 1,
            InsomniaWorkState::Processing => progress.processing += 1,
            InsomniaWorkState::Held => progress.held += 1,
            InsomniaWorkState::Failed | InsomniaWorkState::Terminal => progress.failed += 1,
            InsomniaWorkState::Complete => progress.complete += 1,
        }
        progress.rel_memories += episode.rel_memories;
        progress.phy_memories += episode.phy_memories;
        progress.rejected += u64::from(episode.rejected);
        if episode.state != InsomniaWorkState::Complete
            && progress.incomplete_ids.len() < MAX_STATUS_IDS
        {
            progress
                .incomplete_ids
                .push(episode.id.0.iter().map(|b| format!("{b:02x}")).collect());
        }
        if episode.due && due.len() < max_due {
            due.push(episode.id);
        }
    })?;
    Ok((progress, due))
}
