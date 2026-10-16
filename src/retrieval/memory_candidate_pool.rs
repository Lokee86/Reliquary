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

impl crate::Cva {
    /// REL-local complete eligibility union. Scores are evidence, never presentation policy.
    pub(crate) fn collect_memory_candidate_pool(
        &mut self,
        query: &str,
        query_vector: &[f32],
        profile_id: CompatibilityProfileId,
    ) -> Result<MemoryCandidatePool, crate::MemoryRetrievalError> {
        use crate::memory_retrieval_vectors::{
            exact_memory_scores, load_searchable_memory_vectors, validate_query,
        };
        use std::collections::BTreeMap;
        validate_query(query_vector)?;
        // An absent profile has no semantic population, but lexical eligibility is independent.
        if let Ok(profile) = self.compatibility_profile(profile_id) {
            if query_vector.len() != profile.dimensions as usize {
                return Err(crate::MemoryRetrievalError::InvalidQueryVector);
            }
        }
        let lexical = self.complete_memory_lexical_matches(query)?;
        let (_, vectors) = load_searchable_memory_vectors(
            &mut self.container,
            &self.memories,
            &self.memory_vectors,
            &self.packed_vectors,
            profile_id,
        )?;
        let mut ids = vectors.keys().copied().collect::<Vec<_>>();
        ids.sort();
        let semantic = exact_memory_scores(query_vector, &vectors, &ids)?;
        let mut evidence = BTreeMap::<MemoryId, MemoryCandidateEvidence>::new();
        for hit in lexical {
            evidence.entry(hit.memory_id).or_default().lexical_score = Some(hit.score);
        }
        for hit in semantic {
            evidence.entry(hit.memory_id).or_default().semantic_score = Some(hit.score);
        }
        Ok(MemoryCandidatePool {
            compatibility_profile_id: profile_id,
            candidates: evidence
                .into_iter()
                .map(|(memory_id, evidence)| MemoryCandidate {
                    memory_id,
                    evidence,
                })
                .collect(),
        })
    }
}

#[cfg(test)]
#[path = "memory_candidate_pool_tests.rs"]
mod tests;
