//! Deterministic planning and execution for intentional fresh-history rewrites.
#[path = "rewrite/file.rs"]
mod file;
#[path = "rewrite/lineage.rs"]
mod lineage;
#[path = "rewrite/model.rs"]
mod model;
#[path = "rewrite/plan.rs"]
mod plan;
#[path = "rewrite/semantic_state.rs"]
mod semantic_state;
pub use file::{plan_rewrite_file, rewrite_file, rewrite_from_plan};
pub use model::{
    ArchiveSelection, FreshnessRewriteMode, IdentityPolicy, MemoryBase, MemoryRevisionSelection,
    MemoryRewriteDecision, MemorySelection, ProvenancePolicy, RevisionPolicy, RewritePlan,
    RewriteReport, RewriteRequest,
};
pub use plan::RewriteError;
