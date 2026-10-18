use super::*;
use crate::imports::{ImportCompletionOutcome, NativeImport};
use crate::{
    Cva, EpisodeId, HistoricalReconstructionOptions, InsomniaExtractor, InsomniaWorkState,
    SimulatedGeneralEndpoint,
};
use serde_json::{Map, json};
use std::{fs, path::PathBuf};

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let path = std::env::current_dir()
            .unwrap()
            .join(".test-tmp")
            .join(uuid::Uuid::new_v4().to_string());
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn rel(&self) -> PathBuf {
        self.0.join("sequential-imports.rel")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn imported(namespace: &str) -> NativeImport {
    let mut messages = Vec::new();
    let mut parent = None;
    for index in 0..8 {
        let user = format!("u{index}");
        let assistant = format!("a{index}");
        messages.push(ImportMessage {
            id: user.clone(),
            revision: Some("1".into()),
            parent_id: parent.clone(),
            replaces: None,
            deleted: false,
            role: "user".into(),
            timestamp_ns: 10 + index as i64 * 2,
            content: format!("{namespace} imported fact {index}."),
            artifacts: vec![],
        });
        messages.push(ImportMessage {
            id: assistant.clone(),
            revision: Some("1".into()),
            parent_id: Some(user),
            replaces: None,
            deleted: false,
            role: "assistant".into(),
            timestamp_ns: 11 + index as i64 * 2,
            content: "Acknowledged.".into(),
            artifacts: vec![],
        });
        parent = Some(assistant);
    }
    NativeImport {
        schema_version: 1,
        source_namespace: namespace.into(),
        source_format: "sequential-completion-test-v1".into(),
        conversations: vec![ImportConversation {
            id: "conversation".into(),
            title: None,
            messages,
            branches: vec![ImportBranch {
                id: "main".into(),
                leaf_message_id: parent.unwrap(),
                canonical: true,
            }],
        }],
    }
}

fn hex_id(id: &[u8; 32]) -> String {
    id.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn complete_import_episode(cva: &mut Cva, now_ns: i64) {
    let scope = cva.import_epoch_scope().expect("open import scope");
    assert_eq!(scope.episodes.len(), 1);
    cva.container.set_owner_write_scope(true);
    let claim = cva
        .claim_insomnia_episode("sequential-import-test", now_ns, 1_000)
        .unwrap()
        .expect("queued Import Episode");
    assert!(scope.episodes.contains(&claim.episode_id.0));

    let turns = cva.episode_turns(claim.episode_id).unwrap();
    let mut turn_responses = Map::new();
    for turn in turns.iter().filter(|turn| turn.role == "user") {
        turn_responses.insert(
            turn.node_id.clone(),
            json!([{
                "disposition": "omit",
                "authority_kind": "none",
                "category": "none",
                "type": "none",
                "lifecycle": "none",
                "proposition": "",
                "authority_source_node_id": "",
                "grounding_source_node_id": "",
                "reason": "zero-output sequential completion fixture"
            }]),
        );
    }
    assert_eq!(turn_responses.len(), 8);
    let extractor = InsomniaExtractor::new(SimulatedGeneralEndpoint::new(
        "sequential-import-test",
        vec![json!({"turns": turn_responses, "evidence_requests": []})],
    ));
    let result = cva
        .process_claimed_insomnia_episode(&claim, &extractor, "private", now_ns, now_ns + 1)
        .unwrap();
    cva.container.set_owner_write_scope(false);
    assert!(result.created.is_empty());
    assert_eq!(
        cva.insomnia_work(EpisodeId(*scope.episodes.iter().next().unwrap()))
            .unwrap()
            .state,
        InsomniaWorkState::Complete
    );
    let completed_scope = cva.import_epoch_scope().unwrap();
    let output = completed_scope
        .outputs
        .get(completed_scope.episodes.iter().next().unwrap())
        .expect("durable zero-output Insomnia completion");
    assert!(output.memory_ids.is_empty());
    assert!(output.external_memory_refs.is_empty());
    assert!(output.records.is_empty());
}

fn settle_and_complete(cva: &mut Cva, import_id: &str, now_ns: i64) -> ImportComplete {
    let report = cva
        .reconstruct_historical_freshness(HistoricalReconstructionOptions::default())
        .unwrap();
    assert!(report.committed, "append an accepted scoped settlement");
    let settlement = cva
        .freshness
        .import_settlement(report.import_id)
        .expect("accepted settlement for current import");
    assert!(settlement.epoch.is_some());
    assert!(settlement.encode().starts_with(b"CVAFRS05"));
    assert_eq!(
        settlement
            .import_id
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>(),
        import_id
    );
    assert!(
        cva.native_import_readiness(import_id)
            .unwrap()
            .blocker
            .is_none()
    );
    let ImportCompletionOutcome::Completed {
        complete,
        committed,
    } = cva.complete_native_import(import_id, now_ns).unwrap()
    else {
        panic!("ready nonempty import remained pending");
    };
    assert!(committed);
    complete
}

#[test]
fn completed_nonempty_imports_sequence_across_live_activity_and_reopen_without_duplicates() {
    let fixture = Fixture::new();
    let path = fixture.rel();
    let mut cva = Cva::create(&path).unwrap();

    let a_receipt = cva.import_native(&imported("sequential/A"), 100).unwrap();
    assert!(!a_receipt.conversations.is_empty());
    let a_start = cva.open_import_epoch().unwrap();
    complete_import_episode(&mut cva, 110);
    let a_complete = settle_and_complete(&mut cva, &a_start.import_id, 120);
    assert!(cva.open_import_epoch().is_none());
    let a_completed_prefix = fs::read(&path).unwrap();
    let a_settlement = cva
        .freshness
        .import_settlements()
        .into_iter()
        .find(|settlement| {
            settlement
                .import_id
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
                == a_start.import_id
        })
        .cloned()
        .expect("A accepted settlement");
    assert_eq!(a_complete.post.global_version, cva.latest_global_version());

    // A real, ordinary owner write proves A reopened admission. Import B's start
    // must bind the resulting canonical owner state while retaining A's cut.
    cva.append_node(
        "between-imports".into(),
        "live".into(),
        None,
        "user".into(),
        130,
        "ordinary activity between completed imports",
    )
    .unwrap();
    let between_activity = (
        cva.latest_global_version(),
        cva.archive_version(),
        cva.memory_version(),
        cva.rel_turn_count(),
    );
    assert!(fs::read(&path).unwrap().starts_with(&a_completed_prefix));

    let b_receipt = cva.import_native(&imported("sequential/B"), 140).unwrap();
    assert!(!b_receipt.conversations.is_empty());
    let b_start = cva.open_import_epoch().unwrap();
    let b_scope = cva.import_epoch_scope().unwrap();
    assert_ne!(a_start.import_id, b_start.import_id);
    assert_eq!(b_start.pre.global_version, between_activity.0);
    assert_eq!(b_start.pre.archive_version, between_activity.1);
    assert_eq!(b_start.pre.memory_version, between_activity.2);
    assert!(b_start.pre.global_version > a_complete.post.global_version);
    assert!(b_start.pre.archive_version > a_complete.post.archive_version);
    assert_eq!(b_scope.base_cut, between_activity.3);
    assert!(b_scope.base_cut >= a_settlement.settlement_cut);

    complete_import_episode(&mut cva, 150);
    let b_complete = settle_and_complete(&mut cva, &b_start.import_id, 160);
    assert!(cva.open_import_epoch().is_none());
    let settlements = cva.freshness.import_settlements();
    assert_eq!(settlements.len(), 2);
    assert_eq!(settlements[0].import_id, a_settlement.import_id);
    assert_eq!(hex_id(&settlements[1].import_id), b_start.import_id);
    assert!(settlements[1].settlement_cut > settlements[0].settlement_cut);
    assert!(settlements[1].base_cut >= settlements[0].settlement_cut);
    assert_ne!(settlements[0].import_id, settlements[1].import_id);
    assert_eq!(b_complete.post.archive_version, cva.archive_version());
    assert_eq!(
        cva.import_epoch_statuses().len(),
        2,
        "both nonempty epochs have durable completion markers"
    );

    // Retry B after a normal write and reopen; neither accepted completion nor
    // accepted settlement may be duplicated or erase later owner activity.
    cva.append_node(
        "after-imports".into(),
        "live".into(),
        None,
        "user".into(),
        170,
        "ordinary activity after completed imports",
    )
    .unwrap();
    let post_activity = (
        cva.latest_global_version(),
        cva.archive_version(),
        cva.memory_version(),
        cva.rel_turn_count(),
    );
    let status_count = cva.import_epoch_statuses().len();
    drop(cva);

    let mut reopened = Cva::open(&path).unwrap();
    assert_eq!(reopened.import_epoch_statuses().len(), 2);
    assert_eq!(reopened.freshness.import_settlements().len(), 2);
    let bytes_before_retry = fs::read(&path).unwrap();
    assert_eq!(
        reopened
            .complete_native_import(&b_start.import_id, 180)
            .unwrap(),
        ImportCompletionOutcome::Completed {
            complete: b_complete,
            committed: false,
        }
    );
    assert_eq!(
        (
            reopened.latest_global_version(),
            reopened.archive_version(),
            reopened.memory_version(),
            reopened.rel_turn_count(),
        ),
        post_activity
    );
    assert_eq!(reopened.import_epoch_statuses().len(), status_count);
    assert_eq!(reopened.freshness.import_settlements().len(), 2);
    assert_eq!(fs::read(&path).unwrap(), bytes_before_retry);
}
