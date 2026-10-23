use crate::{
    DreamLifecycleResult, DreamPairClassification, DreamPairVerification, DreamPublicationOutcome,
    Memory,
};

#[derive(Clone, Debug)]
pub struct DreamProcessedPair {
    pub classification: DreamPairClassification,
    pub verification: Option<DreamPairVerification>,
    pub publication: DreamPublicationOutcome,
}

#[derive(Clone, Debug)]
pub struct DreamProcessResult {
    pub source: Memory,
    pub candidate_count: usize,
    pub pairs: Vec<DreamProcessedPair>,
    pub lifecycle: DreamLifecycleResult,
}

pub const DEFAULT_DREAM_FRONTIER_SIZE: usize = 6;

#[derive(Debug)]
pub struct DreamMemoryProcessOutcome {
    pub memory_id: crate::MemoryId,
    pub result: Result<DreamProcessResult, crate::DreamProcessError>,
}
