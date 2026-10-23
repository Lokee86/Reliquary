//! Immutable source comparison and operation result values shared by callers.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CvaRelation {
    Identical,
    LeftExtendsRight,
    RightExtendsLeft,
    Diverged,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CvaComparison {
    pub owner_id: String,
    pub common_chunk_count: usize,
    pub left_chunk_count: usize,
    pub right_chunk_count: usize,
    pub relation: CvaRelation,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CvaReconcileResult {
    pub comparison: CvaComparison,
    pub replayed_archive_records: usize,
    pub replayed_memory_revisions: usize,
    pub duplicate_memory_revisions: usize,
    pub replayed_entity_revisions: usize,
    pub duplicate_entity_revisions: usize,
    pub replayed_relationship_revisions: usize,
    pub duplicate_relationship_revisions: usize,
    pub replayed_insomnia_completions: usize,
    pub replayed_graph_transactions: usize,
    pub replayed_graph_mutations: usize,
    pub duplicate_graph_mutations: usize,
    pub replayed_file_memory_links: usize,
    pub canonical_change_required: bool,
    pub vector_rebuild_required: bool,
}
