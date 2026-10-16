use crate::{CompatibilityProfileId, MemoryId};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct MemoryCandidateEvidence {
    pub(crate) lexical_score: Option<f64>,
    pub(crate) semantic_score: Option<f64>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct MemoryCandidate {
    pub(crate) memory_id: MemoryId,
    pub(crate) evidence: MemoryCandidateEvidence,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct MemoryCandidatePool {
    pub(crate) compatibility_profile_id: CompatibilityProfileId,
    pub(crate) candidates: Vec<MemoryCandidate>,
}

impl MemoryCandidatePool {
    pub(crate) fn matched_memory_ids(&self) -> impl Iterator<Item = MemoryId> + '_ {
        self.candidates.iter().map(|candidate| candidate.memory_id)
    }
}
