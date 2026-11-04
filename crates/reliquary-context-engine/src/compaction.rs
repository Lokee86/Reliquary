use crate::{
    CompactionBudget, CompactionCheckpoint, CompactionError, CompactionPlan, ContextMessage,
    ContextRequest, ContextRole, ContextTokenCounter, ContextTurn, ContextView, compaction_needed,
};
pub const MIN_RAW_TAIL_TURNS: usize = 1;
pub const COMPACTED_CONTEXT_INSTRUCTIONS: &str = "Earlier conversation was compacted and may omit detail. The summary contains exact source ranges for mapped material. When mapped detail matters, use live_transcript_read. When relevant material is not mapped or a range is insufficient, use live_transcript_search. Treat the original transcript as authoritative and do not guess omitted history.";
pub fn request_context(
    view: &ContextView,
    checkpoint: Option<&CompactionCheckpoint>,
) -> ContextRequest {
    let compacted = checkpoint.is_some_and(|c| checkpoint_applies(view, c));
    ContextRequest {
        instructions: if compacted {
            COMPACTED_CONTEXT_INSTRUCTIONS.to_owned()
        } else {
            String::new()
        },
        messages: request_messages(view, checkpoint),
    }
}
pub fn request_messages(
    view: &ContextView,
    checkpoint: Option<&CompactionCheckpoint>,
) -> Vec<ContextMessage> {
    let mut out = Vec::new();
    if let Some(c) = checkpoint.filter(|c| checkpoint_applies(view, c)) {
        out.push(ContextMessage {
            role: ContextRole::Assistant,
            content: format!("[Compacted earlier conversation]\n{}", c.summary),
        });
        out.extend(turn_messages(tail_after(view, &c.through_message_id)));
    } else {
        out.extend(turn_messages(view.turns.iter()));
    }
    out
}
pub fn plan_session_compaction(
    view: &ContextView,
    checkpoint: Option<&CompactionCheckpoint>,
    current_tokens: u64,
    budget: CompactionBudget,
    counter: &impl ContextTokenCounter,
) -> Result<Option<CompactionPlan>, CompactionError> {
    if !compaction_needed(current_tokens, budget) {
        return Ok(None);
    }
    plan_compaction(view, checkpoint, budget, counter)
}
pub fn plan_compaction(
    view: &ContextView,
    checkpoint: Option<&CompactionCheckpoint>,
    budget: CompactionBudget,
    counter: &impl ContextTokenCounter,
) -> Result<Option<CompactionPlan>, CompactionError> {
    let checkpoint = checkpoint.filter(|c| checkpoint_applies(view, c));
    let tail = checkpoint
        .map(|c| {
            tail_after(view, &c.through_message_id)
                .cloned()
                .collect::<Vec<_>>()
        })
        .unwrap_or_else(|| view.turns.clone());
    if tail.len() <= MIN_RAW_TAIL_TURNS {
        return Ok(None);
    }
    let source_tokens = tail
        .iter()
        .map(|t| counter.count_message(&message(t)))
        .collect::<Vec<_>>();
    let compact_tokens = tail
        .iter()
        .map(|t| counter.count_message(&compact_message(t)))
        .collect::<Vec<_>>();
    let compactable = compactable_prefix_len(&source_tokens, budget.raw_tail_tokens);
    if compactable == 0 {
        return Ok(None);
    }
    let instructions = format!(
        "Create a continuation map of at most {} tokens. The transcript is authoritative and retrievable. Output only WORKING STATE and RETRIEVAL MAP. WORKING STATE: current goal/state, active decisions/constraints, blockers, next actions; enough to continue immediately. RETRIEVAL MAP: older topics/outcomes. Each entry must include source: <start_message_id>..<end_message_id> using supplied IDs only. Omit recoverable detail. On recompaction rewrite: discard obsolete state, demote completed work, merge or broaden old entries. Never invent IDs. Do not answer.",
        budget.summary_tokens
    );
    let mut messages = Vec::new();
    let mut used = counter.count_text(&instructions);
    if let Some(c) = checkpoint {
        let prior = ContextMessage {
            role: ContextRole::Assistant,
            content: format!("[Earlier compacted conversation]\n{}", c.summary),
        };
        used = used.saturating_add(counter.count_message(&prior));
        messages.push(prior)
    }
    let mut count = 0;
    for (i, t) in tail.iter().take(compactable).enumerate() {
        let next = used.saturating_add(compact_tokens[i]);
        if next > budget.compaction_input_tokens {
            break;
        }
        messages.push(compact_message(t));
        used = next;
        count += 1
    }
    if count == 0 {
        return Err(CompactionError::InputBudgetUnsatisfiable {
            message_id: tail[0].message_id.clone(),
        });
    }
    Ok(Some(CompactionPlan {
        through_message_id: tail[count - 1].message_id.clone(),
        instructions,
        messages,
        summary_token_limit: budget.summary_tokens,
    }))
}
pub fn checkpoint_applies(view: &ContextView, c: &CompactionCheckpoint) -> bool {
    crate::validate_summary_sources(view, &c.through_message_id, &c.summary)
}
fn compactable_prefix_len(tokens: &[u64], limit: u64) -> usize {
    if tokens.len() <= 1 {
        return 0;
    }
    let mut start = tokens.len() - 1;
    let mut kept = tokens[start..]
        .iter()
        .fold(0u64, |s, n| s.saturating_add(*n));
    while start > 0 {
        let next = kept.saturating_add(tokens[start - 1]);
        if next > limit {
            break;
        }
        start -= 1;
        kept = next
    }
    start
}
fn message(t: &ContextTurn) -> ContextMessage {
    ContextMessage {
        role: t.role,
        content: t.content.clone(),
    }
}
fn compact_message(t: &ContextTurn) -> ContextMessage {
    ContextMessage {
        role: t.role,
        content: format!("[message_id:{}]\n{}", t.message_id, t.content),
    }
}
fn turn_messages<'a>(
    turns: impl Iterator<Item = &'a ContextTurn> + 'a,
) -> impl Iterator<Item = ContextMessage> + 'a {
    turns.map(message)
}
fn tail_after<'a>(view: &'a ContextView, id: &str) -> impl Iterator<Item = &'a ContextTurn> {
    view.turns
        .iter()
        .skip_while(move |t| t.message_id != id)
        .skip(1)
}
