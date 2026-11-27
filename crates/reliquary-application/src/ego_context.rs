//! Read-only query relevance for the derived records offered to Ego.

use std::collections::BTreeSet;

pub use reliquary_ego::package::*;
pub use reliquary_retrieval::DEFAULT_SEARCH_CANDIDATE_LIMIT as MAX_EGO_POINT_CANDIDATES;
use reliquary_retrieval::lexical_terms;

#[derive(Clone)]
pub struct EgoQueryVector {
    pub profile_id: [u8; 32],
    pub values: Vec<f32>,
}

pub struct EgoQuery<'a> {
    terms: BTreeSet<String>,
    vector: Option<&'a EgoQueryVector>,
}

impl<'a> EgoQuery<'a> {
    pub fn new(query: &str, vector: Option<&'a EgoQueryVector>) -> Self {
        Self {
            terms: lexical_terms(query).into_iter().collect(),
            vector,
        }
    }

    pub fn score(&self, text: &str, embedding: Option<([u8; 32], &[f32])>) -> f64 {
        let lexical = if self.terms.is_empty() {
            0.0
        } else {
            let terms: BTreeSet<_> = lexical_terms(text).into_iter().collect();
            self.terms.intersection(&terms).count() as f64 / self.terms.len() as f64
        };
        let Some((vector, (profile_id, values))) = self.vector.zip(embedding) else {
            return lexical;
        };
        if vector.profile_id != profile_id {
            return lexical;
        }
        let semantic = reliquary_vectors::cosine(&vector.values, values)
            .filter(|score| score.is_finite() && *score > 0.0)
            .unwrap_or(0.0);
        lexical * reliquary_retrieval::DEFAULT_LEXICAL_WEIGHT
            + semantic * reliquary_retrieval::DEFAULT_SEMANTIC_WEIGHT
    }
}
