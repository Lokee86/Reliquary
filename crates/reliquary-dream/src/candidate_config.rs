pub const DEFAULT_DREAM_CANDIDATE_LIMIT: usize = 12;
pub const DEFAULT_DREAM_SEMANTIC_LIMIT: usize = 24;
pub const DEFAULT_DREAM_PRIOR_SEMANTIC_QUOTA: usize = 3;
pub const DEFAULT_DREAM_LEXICAL_LIMIT: usize = 8;
pub const DEFAULT_DREAM_TEMPORAL_LIMIT: usize = 8;
pub const MAX_DREAM_CANDIDATE_LIMIT: usize = 128;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DreamCandidateConfig {
    pub limit: usize,
    pub semantic_limit: usize,
    pub prior_semantic_quota: usize,
    pub lexical_limit: usize,
    pub temporal_limit: usize,
}

impl Default for DreamCandidateConfig {
    fn default() -> Self {
        Self {
            limit: DEFAULT_DREAM_CANDIDATE_LIMIT,
            semantic_limit: DEFAULT_DREAM_SEMANTIC_LIMIT,
            prior_semantic_quota: DEFAULT_DREAM_PRIOR_SEMANTIC_QUOTA,
            lexical_limit: DEFAULT_DREAM_LEXICAL_LIMIT,
            temporal_limit: DEFAULT_DREAM_TEMPORAL_LIMIT,
        }
    }
}
