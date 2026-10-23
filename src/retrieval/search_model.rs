use crate::{Fragment, VectorGenerationId};

#[derive(Clone, Debug, PartialEq)]
pub struct SearchCandidate {
    pub fragment: Fragment,
    pub lexical_score: f64,
    pub semantic_score: f64,
    pub combined_score: f64,
    pub generation_id: Option<VectorGenerationId>,
}
