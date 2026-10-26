use super::*;
use reliquary_echo::{EchoEvent, EchoEventKind};

struct WordCounter;

impl ContextTokenCounter for WordCounter {
    fn count_text(&self, text: &str) -> u64 {
        text.split_whitespace().count() as u64
    }

    fn count_message(&self, message: &ContextMessage) -> u64 {
        self.count_text(&message.content).saturating_add(2)
    }
}

fn view(count: usize) -> ContextView {
    ContextView {
        turns: (0..count)
            .map(|index| ContextTurn {
                message_id: format!("m{index}"),
                role: if index % 2 == 0 {
                    ContextRole::User
                } else {
                    ContextRole::Assistant
                },
                content: "raw".into(),
            })
            .collect(),
    }
}

#[test]
fn checkpoint_replaces_only_older_context() {
    let checkpoint = CompactionCheckpoint {
        through_message_id: "m3".into(),
        summary: "summary".into(),
    };
    let messages = request_messages(&view(8), Some(&checkpoint));
    assert_eq!(messages.len(), 5);
    assert!(messages[0].content.contains("summary"));
    assert_eq!(messages[1].content, "raw");
    assert!(COMPACTED_CONTEXT_INSTRUCTIONS.contains("live_transcript_read"));
    assert!(COMPACTED_CONTEXT_INSTRUCTIONS.contains("live_transcript_search"));
}

#[test]
fn default_budget_has_hysteresis_and_token_tail() {
    let budget = compaction_budget(100_000, DEFAULT_COMPACTION_TRIGGER_PERCENT).unwrap();
    assert_eq!(budget.trigger_tokens, 50_000);
    assert_eq!(budget.target_tokens, 40_000);
    assert_eq!(budget.raw_tail_tokens, 20_000);
    assert_eq!(budget.summary_tokens, 10_000);
    assert_eq!(budget.compaction_input_tokens, 80_000);
    assert!(compaction_needed(50_000, budget));
    assert!(budget_satisfied(40_000, budget));
    assert!(!budget_satisfied(40_001, budget));
}

#[test]
fn compaction_tail_is_selected_by_tokens_not_turn_count() {
    let mut current = view(8);
    for turn in &mut current.turns[..6] {
        turn.content = "old ".repeat(20);
    }
    current.turns[6].content = "recent ".repeat(8);
    current.turns[7].content = "latest ".repeat(8);
    let budget = CompactionBudget {
        trigger_tokens: 30,
        target_tokens: 24,
        raw_tail_tokens: 20,
        summary_tokens: 4,
        compaction_input_tokens: 300,
    };
    let plan = plan_compaction(&current, None, budget, &WordCounter)
        .unwrap()
        .unwrap();
    assert_eq!(plan.through_message_id, "m5");
    assert_eq!(plan.messages.len(), 6);
    assert_eq!(plan.summary_token_limit, 4);
}

#[test]
fn giant_latest_turn_is_preserved_but_older_turns_remain_compactable() {
    let mut current = view(6);
    current.turns[5].content = "huge ".repeat(100);
    let budget = CompactionBudget {
        trigger_tokens: 30,
        target_tokens: 24,
        raw_tail_tokens: 10,
        summary_tokens: 4,
        compaction_input_tokens: 200,
    };
    let plan = plan_compaction(&current, None, budget, &WordCounter)
        .unwrap()
        .unwrap();
    assert_eq!(plan.through_message_id, "m4");
}

#[test]
fn compaction_input_budget_advances_checkpoint_in_bounded_chunks() {
    let mut current = view(12);
    for turn in &mut current.turns {
        turn.content = "token ".repeat(20);
    }
    let budget = CompactionBudget {
        trigger_tokens: 100,
        target_tokens: 80,
        raw_tail_tokens: 25,
        summary_tokens: 10,
        compaction_input_tokens: 160,
    };
    let first = plan_compaction(&current, None, budget, &WordCounter)
        .unwrap()
        .unwrap();
    assert!(first.messages.len() < 11);
    assert_ne!(first.through_message_id, "m10");

    let checkpoint = CompactionCheckpoint {
        through_message_id: first.through_message_id,
        summary: "summary".into(),
    };
    let second = plan_compaction(&current, Some(&checkpoint), budget, &WordCounter)
        .unwrap()
        .unwrap();
    assert!(second.through_message_id > checkpoint.through_message_id);
    assert!(
        second.messages[0]
            .content
            .contains("Earlier compacted conversation")
    );
}

#[test]
fn stale_checkpoint_is_ignored() {
    let checkpoint = CompactionCheckpoint {
        through_message_id: "other-branch".into(),
        summary: "wrong".into(),
    };
    assert_eq!(request_messages(&view(6), Some(&checkpoint)).len(), 6);
}

#[test]
fn assistant_evidence_renders_semantic_events_and_omits_activity() {
    let rendered = render_assistant_content_from_echo(
        "final",
        &[
            EchoEvent {
                conversation_id: "c".into(),
                message_id: "m".into(),
                sequence: 0,
                timestamp_ns: 1,
                model_round: Some(1),
                kind: EchoEventKind::ReasoningTrace,
                correlation_id: None,
                name: None,
                content: "reasoning".into(),
            },
            EchoEvent {
                conversation_id: "c".into(),
                message_id: "m".into(),
                sequence: 1,
                timestamp_ns: 2,
                model_round: None,
                kind: EchoEventKind::ActivityStarted,
                correlation_id: None,
                name: None,
                content: "Searching memory".into(),
            },
        ],
    );
    assert!(rendered.contains("[reasoning_trace round=1]"));
    assert!(rendered.contains("reasoning"));
    assert!(rendered.contains("[Final assistant response]\nfinal"));
    assert!(!rendered.contains("Searching memory"));
}

#[test]
fn summary_is_enforced_by_token_count() {
    let summary = normalize_summary("one two three four five".into(), 3, &WordCounter).unwrap();
    assert_eq!(summary, "one two three");
}

#[test]
fn session_compaction_uses_request_token_trigger_without_resume_gate() {
    let current = view(10);
    let budget = CompactionBudget {
        trigger_tokens: 20,
        target_tokens: 16,
        raw_tail_tokens: 6,
        summary_tokens: 4,
        compaction_input_tokens: 100,
    };
    assert!(
        plan_session_compaction(&current, None, 19, budget, &WordCounter)
            .unwrap()
            .is_none()
    );
    assert!(
        plan_session_compaction(&current, None, 20, budget, &WordCounter)
            .unwrap()
            .is_some()
    );
}
