//! Persisted retrieval configuration values; no runtime/provider state.
pub const DEFAULT_LEXICAL_WEIGHT: f64 = 0.45;
pub const DEFAULT_SEMANTIC_WEIGHT: f64 = 0.55;
pub const DEFAULT_SEARCH_CANDIDATE_LIMIT: usize = 30;
pub const DEFAULT_SEARCH_RESULT_LIMIT: usize = 10;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RetrievalConfig {
    pub candidate_limit: usize,
    pub result_limit: usize,
    pub lexical_weight: f64,
    pub semantic_weight: f64,
}

impl Default for RetrievalConfig {
    fn default() -> Self {
        Self {
            candidate_limit: DEFAULT_SEARCH_CANDIDATE_LIMIT,
            result_limit: DEFAULT_SEARCH_RESULT_LIMIT,
            lexical_weight: DEFAULT_LEXICAL_WEIGHT,
            semantic_weight: DEFAULT_SEMANTIC_WEIGHT,
        }
    }
}

impl RetrievalConfig {
    pub(crate) fn normalized_weights(self) -> Option<(f64, f64)> {
        if self.candidate_limit == 0
            || self.result_limit == 0
            || self.result_limit > self.candidate_limit
            || self.lexical_weight <= 0.0
            || self.semantic_weight <= 0.0
            || !self.lexical_weight.is_finite()
            || !self.semantic_weight.is_finite()
        {
            return None;
        }
        let total = self.lexical_weight + self.semantic_weight;
        total
            .is_finite()
            .then_some((self.lexical_weight / total, self.semantic_weight / total))
    }
}
