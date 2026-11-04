use crate::{
    CompactionCheckpoint, CompactionPlan, ContextTokenCounter, ContextView, normalize_summary,
};

/// Validate every cited source range against the ordered compacted prefix.
pub fn validate_summary_sources(view: &ContextView, through: &str, summary: &str) -> bool {
    if summary.trim().is_empty() {
        return false;
    }
    let mut positions = std::collections::HashMap::new();
    for (index, turn) in view.turns.iter().enumerate() {
        if turn.message_id.is_empty() || positions.insert(turn.message_id.as_str(), index).is_some()
        {
            return false;
        }
    }
    let Some(&end) = positions.get(through) else {
        return false;
    };
    let lower = summary.to_ascii_lowercase();
    let mut offset = 0;
    while let Some(found) = lower[offset..].find("source:") {
        let marker = offset + found;
        if marker > 0
            && (lower.as_bytes()[marker - 1].is_ascii_alphanumeric()
                || lower.as_bytes()[marker - 1] == b'_')
        {
            offset = marker + "source:".len();
            continue;
        }
        let start = marker + "source:".len();
        let Some(token) = summary[start..].split_whitespace().next() else {
            return false;
        };
        let token = token.trim_matches(|c| matches!(c, '`' | '[' | ']' | '(' | ')' | ',' | ';'));
        let Some((first, last)) = token.split_once("..") else {
            return false;
        };
        let (Some(&first), Some(&last)) = (positions.get(first), positions.get(last)) else {
            return false;
        };
        if first > last || last > end {
            return false;
        }
        offset = start + summary[start..].find(token).unwrap_or(0) + token.len();
    }
    true
}

/// Normalize model output and bind its citations to the planned source cut.
pub fn finalize_compaction(
    view: &ContextView,
    plan: &CompactionPlan,
    summary: String,
    counter: &impl ContextTokenCounter,
) -> Result<CompactionCheckpoint, String> {
    if !validate_summary_sources(view, &plan.through_message_id, &summary) {
        return Err("compaction summary cites an invalid source range".into());
    }
    let summary = normalize_summary(summary, plan.summary_token_limit, counter)?;
    if !validate_summary_sources(view, &plan.through_message_id, &summary) {
        return Err("compaction summary cites an invalid source range".into());
    }
    Ok(CompactionCheckpoint {
        through_message_id: plan.through_message_id.clone(),
        summary,
    })
}
