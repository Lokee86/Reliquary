use super::{
    CompactionBudget, CompactionCheckpoint, CompactionError, CompactionPlan, ContextMessage,
    ContextRequest, ContextRole, ContextTokenCounter, ContextTurn, ContextView, compaction_needed,
};

pub const MIN_RAW_TAIL_TURNS: usize = 1;
pub const COMPACTED_CONTEXT_INSTRUCTIONS: &str = "Earlier conversation has been intentionally compacted and may omit historical detail. The compacted context contains exact source message ranges for mapped older material. When exact detail from a mapped range matters, use live_transcript_read before reconstructing it. When relevant earlier material is not mapped or its range is insufficient, use live_transcript_search. Treat the original transcript as authoritative and do not guess omitted history.";

pub fn request_context(
    view: &ContextView,
    checkpoint: Option<&CompactionCheckpoint>,
) -> ContextRequest {
    let compacted = checkpoint.is_some_and(|item| checkpoint_applies(view, item));
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
    let mut messages = Vec::new();
    if let Some(checkpoint) = checkpoint.filter(|item| checkpoint_applies(view, item)) {
        messages.push(ContextMessage {
            role: ContextRole::Assistant,
            content: format!("[Compacted earlier conversation]\n{}", checkpoint.summary),
        });
        messages.extend(messages_from_turns(tail_after(
            view,
            &checkpoint.through_message_id,
        )));
    } else {
        messages.extend(messages_from_turns(view.turns.iter()));
    }
    messages
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
    let checkpoint = checkpoint.filter(|item| checkpoint_applies(view, item));
    let tail = checkpoint
        .map(|item| {
            tail_after(view, &item.through_message_id)
                .cloned()
                .collect::<Vec<_>>()
        })
        .unwrap_or_else(|| view.turns.clone());
    if tail.len() <= MIN_RAW_TAIL_TURNS {
        return Ok(None);
    }

    let turn_tokens = tail
        .iter()
        .map(|turn| counter.count_message(&message_from_turn(turn)))
        .collect::<Vec<_>>();
    let compaction_turn_tokens = tail
        .iter()
        .map(|turn| counter.count_message(&compaction_message_from_turn(turn)))
        .collect::<Vec<_>>();
    let compactable = compactable_prefix_len(&turn_tokens, budget.raw_tail_tokens);
    if compactable == 0 {
        return Ok(None);
    }

    let instructions = format!(
        "Create a continuation map of at most {} tokens. The transcript is authoritative and retrievable. Output only WORKING STATE and RETRIEVAL MAP. WORKING STATE: current goal/state, active decisions/constraints, blockers, next actions; enough to continue immediately. RETRIEVAL MAP: older topics/outcomes. Each entry must include source: <start_message_id>..<end_message_id> using supplied IDs only. Omit recoverable detail. On recompaction rewrite: discard obsolete state, demote completed work, merge or broaden old entries. Never invent IDs. Do not answer.",
        budget.summary_tokens
    );
    let mut messages = Vec::new();
    let mut input_tokens = counter.count_text(&instructions);
    if let Some(checkpoint) = checkpoint {
        let previous = ContextMessage {
            role: ContextRole::Assistant,
            content: format!("[Earlier compacted conversation]\n{}", checkpoint.summary),
        };
        input_tokens = input_tokens.saturating_add(counter.count_message(&previous));
        messages.push(previous);
    }

    let mut compact_count = 0;
    for (index, turn) in tail.iter().take(compactable).enumerate() {
        let next = input_tokens.saturating_add(compaction_turn_tokens[index]);
        if next > budget.compaction_input_tokens {
            break;
        }
        messages.push(compaction_message_from_turn(turn));
        input_tokens = next;
        compact_count += 1;
    }
    if compact_count == 0 {
        return Err(CompactionError::InputBudgetUnsatisfiable {
            message_id: tail[0].message_id.clone(),
        });
    }

    Ok(Some(CompactionPlan {
        through_message_id: tail[compact_count - 1].message_id.clone(),
        instructions,
        messages,
        summary_token_limit: budget.summary_tokens,
    }))
}

pub fn checkpoint_applies(view: &ContextView, checkpoint: &CompactionCheckpoint) -> bool {
    view.turns
        .iter()
        .any(|turn| turn.message_id == checkpoint.through_message_id)
}

fn compactable_prefix_len(turn_tokens: &[u64], raw_tail_tokens: u64) -> usize {
    if turn_tokens.len() <= MIN_RAW_TAIL_TURNS {
        return 0;
    }
    let mut keep_start = turn_tokens.len() - MIN_RAW_TAIL_TURNS;
    let mut kept = turn_tokens[keep_start..]
        .iter()
        .fold(0_u64, |sum, tokens| sum.saturating_add(*tokens));
    while keep_start > 0 {
        let candidate = kept.saturating_add(turn_tokens[keep_start - 1]);
        if candidate > raw_tail_tokens {
            break;
        }
        keep_start -= 1;
        kept = candidate;
    }
    keep_start
}

fn message_from_turn(turn: &ContextTurn) -> ContextMessage {
    ContextMessage {
        role: turn.role,
        content: turn.content.clone(),
    }
}

fn compaction_message_from_turn(turn: &ContextTurn) -> ContextMessage {
    ContextMessage {
        role: turn.role,
        content: format!("[message_id:{}]\n{}", turn.message_id, turn.content),
    }
}

fn messages_from_turns<'a>(
    turns: impl Iterator<Item = &'a ContextTurn> + 'a,
) -> impl Iterator<Item = ContextMessage> + 'a {
    turns.map(message_from_turn)
}

fn tail_after<'a>(
    view: &'a ContextView,
    message_id: &str,
) -> impl Iterator<Item = &'a ContextTurn> {
    view.turns
        .iter()
        .skip_while(move |turn| turn.message_id != message_id)
        .skip(1)
}
