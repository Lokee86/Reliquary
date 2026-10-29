use super::{CompactionBudget, ContextMessage};
use std::fmt;

pub const DEFAULT_COMPACTION_TRIGGER_PERCENT: u8 = 50;
pub const COMPACTION_TARGET_OF_TRIGGER_PERCENT: u8 = 80;
pub const RAW_TAIL_OF_TARGET_PERCENT: u8 = 50;
pub const SUMMARY_OF_TARGET_PERCENT: u8 = 25;
pub const COMPACTION_INPUT_PERCENT: u8 = 80;
pub const MAX_SUMMARY_TOKENS: u64 = 10_000;

pub trait ContextTokenCounter {
    fn count_text(&self, text: &str) -> u64;
    fn count_message(&self, message: &ContextMessage) -> u64;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CompactionError {
    InvalidBudget,
    InputBudgetUnsatisfiable { message_id: String },
}

impl fmt::Display for CompactionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidBudget => write!(f, "invalid compaction budget"),
            Self::InputBudgetUnsatisfiable { message_id } => write!(
                f,
                "compaction input budget cannot fit message '{message_id}'"
            ),
        }
    }
}

impl std::error::Error for CompactionError {}

pub fn compaction_budget(
    context_limit: u64,
    trigger_percent: u8,
) -> Result<CompactionBudget, CompactionError> {
    if context_limit == 0 || !(1..=95).contains(&trigger_percent) {
        return Err(CompactionError::InvalidBudget);
    }
    let trigger_tokens = percent_of(context_limit, trigger_percent);
    let target_tokens = percent_of(trigger_tokens, COMPACTION_TARGET_OF_TRIGGER_PERCENT);
    let raw_tail_tokens = percent_of(target_tokens, RAW_TAIL_OF_TARGET_PERCENT);
    let summary_tokens = percent_of(target_tokens, SUMMARY_OF_TARGET_PERCENT)
        .max(1)
        .min(MAX_SUMMARY_TOKENS);
    let compaction_input_tokens = percent_of(context_limit, COMPACTION_INPUT_PERCENT);
    if trigger_tokens == 0
        || target_tokens == 0
        || raw_tail_tokens == 0
        || compaction_input_tokens == 0
        || raw_tail_tokens.saturating_add(summary_tokens) > target_tokens
    {
        return Err(CompactionError::InvalidBudget);
    }
    Ok(CompactionBudget {
        trigger_tokens,
        target_tokens,
        raw_tail_tokens,
        summary_tokens,
        compaction_input_tokens,
    })
}

pub fn compaction_needed(current_tokens: u64, budget: CompactionBudget) -> bool {
    current_tokens >= budget.trigger_tokens
}

pub fn budget_satisfied(current_tokens: u64, budget: CompactionBudget) -> bool {
    current_tokens <= budget.target_tokens
}

pub fn normalize_summary(
    summary: String,
    max_tokens: u64,
    counter: &impl ContextTokenCounter,
) -> Result<String, String> {
    let summary = summary.trim();
    if summary.is_empty() {
        return Err("compaction returned an empty summary".into());
    }
    if counter.count_text(summary) <= max_tokens {
        return Ok(summary.to_owned());
    }
    let chars = summary.chars().collect::<Vec<_>>();
    let mut low = 0_usize;
    let mut high = chars.len();
    while low < high {
        let mid = low + (high - low).div_ceil(2);
        let candidate = chars[..mid].iter().collect::<String>();
        if counter.count_text(&candidate) <= max_tokens {
            low = mid;
        } else {
            high = mid - 1;
        }
    }
    let truncated = chars[..low].iter().collect::<String>();
    let truncated = truncated.trim();
    if truncated.is_empty() {
        return Err("compaction summary cannot fit its token budget".into());
    }
    Ok(truncated.to_owned())
}

fn percent_of(value: u64, percent: u8) -> u64 {
    value.saturating_mul(u64::from(percent)) / 100
}
