use super::{FixtureDir, VIEWER, admission};
use reliquary_memory::{
    AuthorizedContextCheckpoint, AuthorizedContextWorkflow, CompactionBudget, CompactionCheckpoint,
    CompactionPlan, ContextMemoryOwnerKind, ContextMemoryRef, ContextMessage, ContextRequest,
    ContextRole, ContextTokenCounter, ContextTurn, ContextView, Gateway, MemoryId, OwnerId,
    ProductErrorKind, compaction_budget,
};

struct Words;

impl ContextTokenCounter for Words {
    fn count_text(&self, text: &str) -> u64 {
        text.split_whitespace().count() as u64
    }

    fn count_message(&self, message: &ContextMessage) -> u64 {
        self.count_text(&message.content) + 1
    }
}

struct UncalledCounter;

impl ContextTokenCounter for UncalledCounter {
    fn count_text(&self, _: &str) -> u64 {
        panic!("foreign checkpoint reached the caller's token counter");
    }

    fn count_message(&self, _: &ContextMessage) -> u64 {
        panic!("foreign checkpoint reached the caller's token counter");
    }
}

#[test]
fn context_viewer_can_inspect_and_compact_but_cannot_read_ungranted_phy() {
    let dir = FixtureDir::new();
    let rel = dir.create_rel();
    let gateway = Gateway::new();
    let owner = gateway.mount_rel(&rel, false).unwrap();
    let (operator, viewer, control) = admission(&gateway, owner);
    let transcript = ContextView {
        turns: vec![
            ContextTurn {
                message_id: "old".into(),
                role: ContextRole::User,
                content: "first message".into(),
            },
            ContextTurn {
                message_id: "current".into(),
                role: ContextRole::Assistant,
                content: "latest response".into(),
            },
        ],
    };
    let budget = CompactionBudget {
        raw_tail_tokens: 3,
        ..compaction_budget(200, 50).unwrap()
    };
    let ungranted = ContextMemoryRef {
        owner: OwnerId::from_bytes([42; 16]),
        kind: ContextMemoryOwnerKind::Phy,
        memory_id: MemoryId([17; 32]),
    };
    let denied = viewer.prepare_context_workflow(
        owner,
        "chat",
        transcript.clone(),
        &[ungranted],
        &[],
        &[],
        None,
        100,
        budget,
        &Words,
    );
    assert_eq!(denied.err().unwrap().kind(), ProductErrorKind::Unauthorized);

    let prepared: AuthorizedContextWorkflow = viewer
        .prepare_context_workflow(
            owner,
            "chat",
            transcript.clone(),
            &[],
            &[],
            &[],
            None,
            100,
            budget,
            &Words,
        )
        .unwrap();
    let request: ContextRequest = viewer.context_request(&prepared).unwrap();
    assert_eq!(request.messages[0].content, "first message");
    assert!(operator.context_request(&prepared).is_err());
    let plan: CompactionPlan = viewer.context_compaction_plan(&prepared).unwrap().unwrap();
    assert_eq!(plan.through_message_id, "old");
    let summary = "WORKING STATE\nsource: old..old";
    let checkpoint: AuthorizedContextCheckpoint = viewer
        .finalize_context_workflow(&prepared, summary.into(), &Words)
        .unwrap()
        .unwrap();
    let snapshot: CompactionCheckpoint = viewer.context_checkpoint(&checkpoint).unwrap();
    assert_eq!(snapshot.through_message_id, "old");
    assert!(
        operator
            .prepare_context_workflow(
                owner,
                "chat",
                transcript.clone(),
                &[],
                &[],
                &[],
                Some(&checkpoint),
                0,
                budget,
                &UncalledCounter,
            )
            .is_err()
    );
    viewer
        .prepare_context_workflow(
            owner,
            "chat",
            transcript,
            &[],
            &[],
            &[],
            Some(&checkpoint),
            0,
            budget,
            &Words,
        )
        .unwrap();
    let revoked = viewer.compact_context_with(
        &prepared,
        |_| {
            control.revoke(VIEWER).unwrap();
            Ok(summary.into())
        },
        &Words,
    );
    assert_eq!(revoked.err().unwrap().kind(), ProductErrorKind::Revoked);
    assert_eq!(
        viewer.context_checkpoint(&checkpoint).unwrap_err().kind(),
        ProductErrorKind::Revoked
    );
}
