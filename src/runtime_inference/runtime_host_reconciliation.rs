#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RelReconciliationReport {
    pub canonical_changed: bool,
    pub vector_rebuild_required: bool,
    pub vector_rebuild_completed: bool,
    pub candidates: Vec<RelReconciliationCandidateReport>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RelReconciliationCandidateReport {
    pub path: String,
    pub status: String,
    pub relation: Option<String>,
    pub conflict_kind: Option<String>,
    pub message: Option<String>,
}
