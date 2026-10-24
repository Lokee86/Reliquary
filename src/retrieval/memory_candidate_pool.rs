use crate::{CompatibilityProfileId, MemoryId};
use std::collections::BTreeMap;

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

pub(crate) fn merge_memory_candidate_evidence(
    compatibility_profile_id: CompatibilityProfileId,
    lexical: impl IntoIterator<Item = (MemoryId, f64)>,
    semantic: impl IntoIterator<Item = (MemoryId, f64)>,
) -> MemoryCandidatePool {
    let mut evidence = BTreeMap::<MemoryId, MemoryCandidateEvidence>::new();
    for (memory_id, score) in lexical {
        evidence.entry(memory_id).or_default().lexical_score = Some(score);
    }
    for (memory_id, score) in semantic {
        evidence.entry(memory_id).or_default().semantic_score = Some(score);
    }
    MemoryCandidatePool {
        compatibility_profile_id,
        candidates: evidence
            .into_iter()
            .map(|(memory_id, evidence)| MemoryCandidate {
                memory_id,
                evidence,
            })
            .collect(),
    }
}

#[cfg(test)]
#[path = "memory_candidate_pool_tests.rs"]
mod tests;
