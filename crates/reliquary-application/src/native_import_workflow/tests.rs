use super::*;
use reliquary_container::ReliquaryScope;
use reliquary_owner_session::{
    CompletionCheck, GraphAdmission, records::GroupedInsomniaCompletion,
};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(1);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".test-tmp");
        fs::create_dir_all(&dir).unwrap();
        Self(dir.join(format!(
            "import-{}-{}.rel",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )))
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}
fn batch(owner: [u8; 16]) -> NativeImportBatch {
    let source = ImportReceipt {
        schema_version: 1,
        owner_uuid: owner,
        import_key: [0x3a; 32],
        source_namespace: "fixture/account".into(),
        source_format: "normalized-v1".into(),
        source_digest: [0x3a; 32],
        receipt_digest: [0; 32],
        conversations: vec![reliquary_owner_session::model::ImportedConversation {
            source_id: "source-conversation".into(),
            conversation_id: "conversation".into(),
            title: None,
            messages: vec![reliquary_owner_session::model::ImportedMessage {
                source_id: "source-turn".into(),
                revision: "1".into(),
                source_parent_id: None,
                replaces: None,
                deleted: false,
                node_id: Some("imported-turn".into()),
                fingerprint: [0x34; 32],
                artifacts: vec![],
            }],
            branches: vec![reliquary_owner_session::model::ImportedBranch {
                source_id: "source-main".into(),
                branch_id: "main".into(),
                leaf_node_id: Some("imported-turn".into()),
                canonical: true,
            }],
        }],
    };
    NativeImportBatch {
        source,
        turns: vec![(
            TurnInput {
                id: "imported-turn".into(),
                conversation_id: "conversation".into(),
                parent_id: None,
                role: "user".into(),
                principal_id: Some("alice".into()),
                timestamp_ns: 1,
                content: "imported evidence".into(),
                attachments: vec![],
                project_attachments: vec![],
            },
            Branch {
                id: "main".into(),
                conversation_id: "conversation".into(),
                leaf_node_id: "imported-turn".into(),
                canonical: true,
            },
        )],
        episodes: vec![NativeImportEpisode {
            conversation_id: "conversation".into(),
            start_node_id: "imported-turn".into(),
            end_node_id: "imported-turn".into(),
            finalized_at_ns: 11,
        }],
        started_at_ns: 10,
    }
}

#[test]
fn import_restarts_then_finishes_scoped_work_and_settlement_once() {
    let path = Temp::new();
    let owner = [0x62; 16];
    let input = batch(owner);
    let (id, episode);
    {
        let mut writer = RelWriter::create_path(&path.0, ReliquaryScope::Project, owner).unwrap();
        let (mut live_turn, mut live_branch) = input.turns[0].clone();
        live_turn.id = "live-turn".into();
        live_turn.conversation_id = "live-conversation".into();
        live_turn.timestamp_ns = 100;
        live_branch.conversation_id = live_turn.conversation_id.clone();
        live_branch.leaf_node_id = live_turn.id.clone();
        writer
            .accept_turn_and_advance_branch(live_turn, live_branch)
            .unwrap();
        let staged = stage_native_import(&mut writer, input.clone()).unwrap();
        let timeline = import_timeline(&writer).unwrap();
        assert_eq!(timeline.activity_order[0].node_id, "live-turn");
        assert_eq!(timeline.activity_order[1].node_id, "imported-turn");
        id = staged.import_id;
        episode = staged.episodes[0];
        assert!(writer.complete_native_import(&id, 12).is_err());
        assert!(matches!(
            writer.read_session().import_status(&id),
            Some(ImportEpochStatus::Open(_))
        ));
    }
    let grant = GraphAdmission::for_replay(owner, [], [], [], [], []);
    let mut writer = RelWriter::open_path(&path.0, grant.clone(), [], []).unwrap();
    let before = fs::metadata(&path.0).unwrap().len();
    assert_eq!(
        stage_native_import(&mut writer, input.clone())
            .unwrap()
            .episodes,
        vec![episode]
    );
    assert_eq!(fs::metadata(&path.0).unwrap().len(), before);
    let claim = writer
        .claim_native_import_episode(&id, episode, "worker", 20, 100)
        .unwrap();
    assert_eq!(claim.turns[0].content, "imported evidence");
    let completion = GroupedInsomniaCompletion {
        episode_id: episode.0,
        attempt: claim.work.attempt_count,
        started_at_ns: claim.started_at_ns,
        completed_at_ns: 21,
        transaction_time_ns: 21,
        extractor_model: "fixture".into(),
        extractor_version: "v1".into(),
        rejected_count: 0,
        memory_ids: vec![],
        external_memory_refs: vec![],
        global_version_start: writer.next_global_version().unwrap(),
        bodies: vec![],
        memory_records: vec![],
        routing_metadata: vec![],
        freshness_birth_turn: None,
        freshness_policy_version: None,
        freshness_birth_memory_ids: vec![],
    };
    assert_eq!(
        writer
            .commit_native_import_completion_with_routed_receipts(
                &id,
                claim.work.lease_token.unwrap(),
                completion,
                21,
                &[],
            )
            .unwrap(),
        CompletionCheck::Apply
    );
    let frozen = settle_native_import(&mut writer, &id).unwrap();
    assert_eq!(frozen.import_id, id);
    assert!(
        writer
            .read_session()
            .native_import_settlement(&id)
            .is_some()
    );
    assert!(settle_native_import(&mut writer, &id).is_ok());
    drop(writer);
    // Simulate a restart after the settlement sync but before completion.
    let mut writer = RelWriter::open_path(&path.0, grant.clone(), [], []).unwrap();
    let sealed = stage_native_import(&mut writer, input.clone()).unwrap();
    assert!(!sealed.already_completed);
    assert_eq!(sealed.episodes, vec![episode]);
    assert_eq!(
        writer.read_session().native_import_settlement(&id),
        Some(&frozen)
    );
    let (accepted, did_write) = writer.complete_native_import(&id, 22).unwrap();
    assert!(did_write);
    assert!(
        writer
            .read_session()
            .native_import_settlement(&id)
            .is_some()
    );
    assert_eq!(
        writer.complete_native_import(&id, 22).unwrap(),
        (accepted.clone(), false)
    );
    drop(writer);
    let mut reopened = RelWriter::open_path(&path.0, grant, [], []).unwrap();
    assert!(matches!(reopened.read_session().import_status(&id),
        Some(ImportEpochStatus::Completed { complete, .. }) if complete == accepted));
    assert!(
        stage_native_import(&mut reopened, input)
            .unwrap()
            .already_completed
    );
    assert_eq!(
        reopened.complete_native_import(&id, 30).unwrap(),
        (accepted, false)
    );
}

#[test]
fn import_rejects_episode_evidence_outside_its_source_receipt() {
    let path = Temp::new();
    let owner = [0x63; 16];
    let mut input = batch(owner);
    let mut writer = RelWriter::create_path(&path.0, ReliquaryScope::Project, owner).unwrap();

    let (mut prior_turn, mut prior_branch) = input.turns[0].clone();
    prior_turn.id = "prior-turn".into();
    prior_branch.leaf_node_id = prior_turn.id.clone();
    writer
        .accept_turn_and_advance_branch(prior_turn, prior_branch)
        .unwrap();

    input.turns[0].0.parent_id = Some("prior-turn".into());
    input.episodes[0].start_node_id = "prior-turn".into();
    let staged = stage_native_import(&mut writer, input).unwrap();
    let before = fs::metadata(&path.0).unwrap().len();
    let error = writer
        .claim_native_import_episode(&staged.import_id, staged.episodes[0], "worker", 20, 100)
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("path escapes its receipt source set")
    );
    let error = settle_native_import(&mut writer, &staged.import_id).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("path escapes its receipt source set")
    );
    assert_eq!(fs::metadata(&path.0).unwrap().len(), before);
}
