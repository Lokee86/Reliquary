//! Pure bounded context assembly and compaction planning. This crate does not
//! write summaries, mutate Archive state, or synthesize user identity.

mod compaction;
mod compaction_budget;
mod evidence;
mod model;
mod summary_sources;

pub use compaction::{
    COMPACTED_CONTEXT_INSTRUCTIONS, MIN_RAW_TAIL_TURNS, checkpoint_applies, plan_compaction,
    plan_session_compaction, request_context, request_messages,
};
pub use compaction_budget::{
    COMPACTION_INPUT_PERCENT, COMPACTION_TARGET_OF_TRIGGER_PERCENT, CompactionError,
    ContextTokenCounter, DEFAULT_COMPACTION_TRIGGER_PERCENT, MAX_SUMMARY_TOKENS,
    RAW_TAIL_OF_TARGET_PERCENT, SUMMARY_OF_TARGET_PERCENT, budget_satisfied, compaction_budget,
    compaction_needed, normalize_summary,
};
pub use evidence::{render_assistant_content, render_assistant_content_from_echo};
pub use model::{
    CompactionBudget, CompactionCheckpoint, CompactionPlan, ContextEvidence, ContextMessage,
    ContextRequest, ContextRole, ContextTurn, ContextView, EvidenceKind,
};
pub use summary_sources::{finalize_compaction, validate_summary_sources};

#[cfg(test)]
#[path = "behavior_tests.rs"]
mod behavior_tests;
