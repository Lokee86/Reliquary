//! Deterministic planning and execution for intentional fresh-history rewrites.
#[path = "rewrite/lineage.rs"]
mod lineage;

#[cfg(test)]
#[path = "rewrite/semantic_state.rs"]
mod semantic_state;
pub use crate::maintenance::rewrite::RewriteError;
pub use crate::maintenance::rewrite::model::{
    ArchiveSelection, FreshnessRewriteMode, IdentityPolicy, MemoryBase, MemoryRevisionSelection,
    MemoryRewriteDecision, MemorySelection, ProvenancePolicy, RevisionPolicy, RewritePlan,
    RewriteReport, RewriteRequest,
};
pub use crate::maintenance::rewrite::{plan_rewrite_file, rewrite_file, rewrite_from_plan};
#[path = "facade/rewrite_plan.rs"]
mod plan_adapter;
