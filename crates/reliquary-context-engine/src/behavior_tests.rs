use super::*;
use reliquary_archive::{EchoEvent, EchoEventKind};
struct Words;
impl ContextTokenCounter for Words {
    fn count_text(&self, text: &str) -> u64 {
        text.split_whitespace().count() as u64
    }
    fn count_message(&self, m: &ContextMessage) -> u64 {
        self.count_text(&m.content) + 2
    }
}
fn view(n: usize) -> ContextView {
    ContextView {
        turns: (0..n)
            .map(|i| ContextTurn {
                message_id: format!("m{i}"),
                role: if i % 2 == 0 {
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
fn budget_hysteresis_and_tokens() {
    let b = compaction_budget(100_000, 50).unwrap();
    assert_eq!(
        (
            b.trigger_tokens,
            b.target_tokens,
            b.raw_tail_tokens,
            b.summary_tokens,
            b.compaction_input_tokens
        ),
        (50_000, 40_000, 20_000, 10_000, 80_000)
    );
    assert!(compaction_needed(50_000, b));
    assert!(budget_satisfied(40_000, b));
    assert!(!budget_satisfied(40_001, b));
}
#[test]
fn checkpoint_replaces_only_older_messages() {
    let c = CompactionCheckpoint {
        through_message_id: "m3".into(),
        summary: "summary".into(),
    };
    let req = request_context(&view(8), Some(&c));
    assert_eq!(req.messages.len(), 5);
    assert!(req.messages[0].content.contains("summary"));
    assert!(req.instructions.contains("live_transcript_read"));
}
#[test]
fn compaction_selects_tail_by_tokens_and_advances_bounded_chunks() {
    let mut v = view(12);
    for t in &mut v.turns {
        t.content = "token ".repeat(20)
    }
    let b = CompactionBudget {
        trigger_tokens: 100,
        target_tokens: 80,
        raw_tail_tokens: 25,
        summary_tokens: 10,
        compaction_input_tokens: 160,
    };
    let first = plan_compaction(&v, None, b, &Words).unwrap().unwrap();
    assert_ne!(first.through_message_id, "m10");
    let c = CompactionCheckpoint {
        through_message_id: first.through_message_id.clone(),
        summary: "summary".into(),
    };
    let next = plan_compaction(&v, Some(&c), b, &Words).unwrap().unwrap();
    assert!(next.through_message_id > c.through_message_id);
}
#[test]
fn stale_checkpoint_and_activity_evidence_are_ignored() {
    let c = CompactionCheckpoint {
        through_message_id: "wrong".into(),
        summary: "stale".into(),
    };
    assert_eq!(request_messages(&view(3), Some(&c)).len(), 3);
    let event = EchoEvent {
        conversation_id: "c".into(),
        message_id: "m".into(),
        sequence: 0,
        timestamp_ns: 1,
        model_round: None,
        kind: EchoEventKind::ActivityStarted,
        correlation_id: None,
        name: None,
        content: "private activity".into(),
    };
    let rendered = render_assistant_content_from_echo("final", &[event]);
    assert_eq!(rendered, "final");
}
#[test]
fn summary_is_limited_by_token_counter() {
    assert_eq!(
        normalize_summary("one two three four".into(), 2, &Words).unwrap(),
        "one two"
    );
}

#[test]
fn compaction_citations_must_name_ordered_sources_before_the_cut() {
    let v = view(8);
    let plan = CompactionPlan {
        through_message_id: "m3".into(),
        instructions: String::new(),
        messages: vec![],
        summary_token_limit: 8,
    };
    let checkpoint = finalize_compaction(&v, &plan, "Goal source: m1..m3".into(), &Words).unwrap();
    assert!(checkpoint_applies(&v, &checkpoint));
    assert!(finalize_compaction(&v, &plan, "Resource: shared workspace".into(), &Words).is_ok());
    for summary in [
        "",
        "source: m1..missing",
        "source: m1..m5",
        "source: m3..m1",
        "source: m1",
    ] {
        assert!(finalize_compaction(&v, &plan, summary.into(), &Words).is_err());
        let invalid = CompactionCheckpoint {
            through_message_id: "m3".into(),
            summary: summary.into(),
        };
        assert_eq!(request_messages(&v, Some(&invalid)).len(), v.turns.len());
    }
    let mut duplicate = v.clone();
    duplicate.turns[7].message_id = "m1".into();
    assert!(finalize_compaction(&duplicate, &plan, "source: m1..m3".into(), &Words).is_err());
    let short = CompactionPlan {
        summary_token_limit: 2,
        ..plan
    };
    assert!(finalize_compaction(&v, &short, "safe words source: m1..m7".into(), &Words).is_err());
}
