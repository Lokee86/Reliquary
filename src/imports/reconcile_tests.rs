use super::*;
use crate::Cva;
use crate::imports::{
    ImportBranch, ImportConversation, ImportEpochStatus, ImportMessage, NativeImport,
};
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

    fn rel(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn input(namespace: &str) -> NativeImport {
    NativeImport {
        schema_version: 1,
        source_namespace: namespace.into(),
        source_format: "reconciliation-test-v1".into(),
        conversations: vec![ImportConversation {
            id: "session".into(),
            title: Some("Imported".into()),
            messages: vec![
                message("u", None, "user", "question"),
                message("a", Some("u"), "assistant", "answer"),
            ],
            branches: vec![ImportBranch {
                id: "main".into(),
                leaf_message_id: "a".into(),
                canonical: true,
            }],
        }],
    }
}

fn empty_input(namespace: &str) -> NativeImport {
    NativeImport {
        schema_version: 1,
        source_namespace: namespace.into(),
        source_format: "reconciliation-test-v1".into(),
        conversations: Vec::new(),
    }
}

fn message(id: &str, parent_id: Option<&str>, role: &str, content: &str) -> ImportMessage {
    ImportMessage {
        id: id.into(),
        revision: Some("1".into()),
        parent_id: parent_id.map(str::to_owned),
        replaces: None,
        deleted: false,
        role: role.into(),
        timestamp_ns: 42,
        content: content.into(),
        artifacts: vec![],
    }
}

fn import_and_finish(cva: &mut Cva, namespace: &str, now_ns: i64) -> String {
    cva.import_native(&input(namespace), now_ns).unwrap();
    let start = cva.open_import_epoch().unwrap().clone();
    crate::imports::tests::finish_current_import(cva, now_ns + 10);
    assert!(cva.open_import_epoch().is_none());
    start.import_id
}

fn status(cva: &Cva, import_id: &str) -> ImportEpochStatus {
    cva.import_epoch_statuses()
        .into_iter()
        .find(|status| match status {
            ImportEpochStatus::Open(start) | ImportEpochStatus::Completed { start, .. } => {
                start.import_id == import_id
            }
        })
        .expect("epoch status")
}

fn completed(
    cva: &Cva,
    import_id: &str,
) -> (crate::imports::ImportStart, crate::imports::ImportComplete) {
    match status(cva, import_id) {
        ImportEpochStatus::Completed { start, complete } => (start, complete),
        ImportEpochStatus::Open(_) => panic!("epoch must be completed"),
    }
}

fn append_live_node(cva: &mut Cva, id: &str, at_ns: i64) {
    cva.append_node(
        id.into(),
        "reconcile-test-live".into(),
        None,
        "user".into(),
        at_ns,
        &format!("live branch node {id}"),
    )
    .unwrap();
}

fn hex_id(id: &[u8; 32]) -> String {
    id.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[test]
fn divergent_completed_zero_output_epochs_replay_with_destination_watermarks() {
    let f = Fixture::new();
    let left_path = f.rel("left.rel");
    let right_path = f.rel("right.rel");
    let merged_path = f.rel("merged.rel");
    let repeated_path = f.rel("repeated.rel");
    let mut seed = Cva::create(&left_path).unwrap();
    let seed_id = import_and_finish(&mut seed, "common/seed", 100);
    seed.sync().unwrap();
    drop(seed);
    fs::copy(&left_path, &right_path).unwrap();

    let mut left = Cva::open(&left_path).unwrap();
    let left_id = import_and_finish(&mut left, "branch/left", 200);
    let left_final = crate::imports::tests::current_watermarks(&left);
    let left_insomnia = left
        .episodes()
        .into_iter()
        .map(|episode| {
            (
                episode.id,
                left.insomnia_work(episode.id).unwrap().clone(),
                left.insomnia_attempts(episode.id),
            )
        })
        .collect::<Vec<_>>();
    left.sync().unwrap();
    drop(left);

    let mut right = Cva::open(&right_path).unwrap();
    let right_id = import_and_finish(&mut right, "branch/right", 200);
    let (_, source_right_complete) = completed(&right, &right_id);
    let source_settlement = right
        .freshness
        .import_settlements()
        .into_iter()
        .find(|settlement| hex_id(&settlement.import_id) == right_id)
        .cloned()
        .unwrap();
    let source_scope = source_settlement.epoch.as_ref().unwrap().clone();
    right.sync().unwrap();
    drop(right);

    Cva::reconcile(&left_path, &right_path, &merged_path).unwrap();
    let mut merged = Cva::open(&merged_path).unwrap();
    assert_eq!(merged.import_epoch_statuses().len(), 3);
    assert_eq!(merged.freshness.import_settlements().len(), 3);
    for (episode_id, expected_work, expected_attempts) in left_insomnia {
        assert_eq!(merged.insomnia_work(episode_id), Some(&expected_work));
        assert_eq!(merged.insomnia_attempts(episode_id), expected_attempts);
    }
    assert!(matches!(
        status(&merged, &seed_id),
        ImportEpochStatus::Completed { .. }
    ));
    assert!(matches!(
        status(&merged, &left_id),
        ImportEpochStatus::Completed { .. }
    ));
    let (destination_start, destination_complete) = completed(&merged, &right_id);
    assert_eq!(destination_start.pre, left_final);
    assert_eq!(
        destination_complete.post.global_version,
        merged.latest_global_version()
    );
    assert_eq!(
        destination_complete.post.archive_version,
        merged.archive_version()
    );
    assert_eq!(destination_complete.import_id, right_id);
    assert_ne!(destination_complete.post, source_right_complete.post);
    let destination_settlement = merged
        .freshness
        .import_settlements()
        .into_iter()
        .find(|settlement| hex_id(&settlement.import_id) == right_id)
        .cloned()
        .unwrap();
    let destination_scope = destination_settlement.epoch.as_ref().unwrap();
    assert_ne!(destination_scope.start_prefix, source_scope.start_prefix);
    assert_ne!(destination_scope.freeze_prefix, source_scope.freeze_prefix);
    assert_eq!(
        destination_scope.start_evidence_digest,
        destination_settlement.start_evidence_digest
    );
    merged
        .validate_import_settlement_scope(destination_scope)
        .unwrap();

    let bytes_before_retry = fs::read(&merged_path).unwrap();
    drop(merged);
    let mut reopened = Cva::open(&merged_path).unwrap();
    assert_eq!(reopened.import_epoch_statuses().len(), 3);
    assert_eq!(reopened.freshness.import_settlements().len(), 3);
    assert_eq!(
        reopened.complete_native_import(&right_id, 999).unwrap(),
        ImportCompletionOutcome::Completed {
            complete: destination_complete,
            committed: false,
        }
    );
    assert_eq!(fs::read(&merged_path).unwrap(), bytes_before_retry);

    let repeated = Cva::reconcile(&merged_path, &right_path, &repeated_path).unwrap();
    assert!(!repeated.canonical_change_required);
    assert_eq!(fs::read(&repeated_path).unwrap(), bytes_before_retry);
    let repeated_cva = Cva::open(&repeated_path).unwrap();
    assert_eq!(repeated_cva.import_epoch_statuses().len(), 3);
    assert_eq!(repeated_cva.freshness.import_settlements().len(), 3);
}

#[test]
fn identical_completed_epoch_is_deduplicated_while_archive_tails_merge() {
    let f = Fixture::new();
    let left_path = f.rel("left.rel");
    let right_path = f.rel("right.rel");
    let merged_path = f.rel("merged.rel");
    let mut seed = Cva::create(&left_path).unwrap();
    import_and_finish(&mut seed, "common/seed", 100);
    seed.sync().unwrap();
    drop(seed);
    fs::copy(&left_path, &right_path).unwrap();

    let mut left = Cva::open(&left_path).unwrap();
    let shared_id = import_and_finish(&mut left, "same/epoch", 200);
    append_live_node(&mut left, "left-live", 300);
    left.sync().unwrap();
    drop(left);

    let mut right = Cva::open(&right_path).unwrap();
    let right_id = import_and_finish(&mut right, "same/epoch", 200);
    assert_eq!(right_id, shared_id);
    append_live_node(&mut right, "right-live", 301);
    right.sync().unwrap();
    drop(right);

    Cva::reconcile(&left_path, &right_path, &merged_path).unwrap();
    let mut merged = Cva::open(&merged_path).unwrap();
    assert_eq!(merged.import_epoch_statuses().len(), 2);
    assert_eq!(merged.freshness.import_settlements().len(), 2);
    assert!(matches!(
        status(&merged, &shared_id),
        ImportEpochStatus::Completed { .. }
    ));
    assert_eq!(
        merged
            .conversation_turns("reconcile-test-live", "left-live")
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        merged
            .conversation_turns("reconcile-test-live", "right-live")
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn divergent_reconciliation_refuses_open_epoch_and_preserves_inputs() {
    let f = Fixture::new();
    let left_path = f.rel("left.rel");
    let right_path = f.rel("right.rel");
    let merged_path = f.rel("merged.rel");
    let mut seed = Cva::create(&left_path).unwrap();
    import_and_finish(&mut seed, "common/seed", 100);
    seed.sync().unwrap();
    drop(seed);
    fs::copy(&left_path, &right_path).unwrap();

    let mut left = Cva::open(&left_path).unwrap();
    import_and_finish(&mut left, "branch/left", 200);
    left.import_native(&input("open/left"), 300).unwrap();
    left.sync().unwrap();
    drop(left);
    let mut right = Cva::open(&right_path).unwrap();
    import_and_finish(&mut right, "branch/right", 200);
    right.import_native(&input("open/right"), 300).unwrap();
    right.sync().unwrap();
    drop(right);

    let before_left = fs::read(&left_path).unwrap();
    let before_right = fs::read(&right_path).unwrap();
    assert!(matches!(
        Cva::reconcile(&left_path, &right_path, &merged_path),
        Err(crate::CvaReconcileError::UnsupportedSemanticOwner(
            "open import epoch in divergent reconciliation"
        ))
    ));
    assert!(!merged_path.exists());
    assert_eq!(fs::read(&left_path).unwrap(), before_left);
    assert_eq!(fs::read(&right_path).unwrap(), before_right);
}

#[test]
fn same_import_id_with_different_accepted_epoch_evidence_conflicts_before_output() {
    let f = Fixture::new();
    let left_path = f.rel("left.rel");
    let right_path = f.rel("right.rel");
    let merged_path = f.rel("merged.rel");
    let mut seed = Cva::create(&left_path).unwrap();
    import_and_finish(&mut seed, "common/seed", 100);
    seed.sync().unwrap();
    drop(seed);
    fs::copy(&left_path, &right_path).unwrap();

    let mut left = Cva::open(&left_path).unwrap();
    let left_id = import_and_finish(&mut left, "same/conflicting-epoch", 200);
    left.sync().unwrap();
    drop(left);
    let mut right = Cva::open(&right_path).unwrap();
    let right_id = import_and_finish(&mut right, "same/conflicting-epoch", 220);
    right.sync().unwrap();
    drop(right);
    assert_eq!(
        left_id, right_id,
        "same normalized input has the same import ID"
    );

    let before_left = fs::read(&left_path).unwrap();
    let before_right = fs::read(&right_path).unwrap();
    assert!(matches!(
        Cva::reconcile(&left_path, &right_path, &merged_path),
        Err(crate::CvaReconcileError::Conflict(
            crate::CvaReconcileConflict::ImportEpoch { import_id }
        )) if import_id == left_id
    ));
    assert!(!merged_path.exists());
    assert_eq!(fs::read(&left_path).unwrap(), before_left);
    assert_eq!(fs::read(&right_path).unwrap(), before_right);
}

#[test]
fn multiple_right_only_epochs_replay_across_live_activity_and_empty_import() {
    let f = Fixture::new();
    let left_path = f.rel("left.rel");
    let right_path = f.rel("right.rel");
    let merged_path = f.rel("merged.rel");
    let mut seed = Cva::create(&left_path).unwrap();
    let seed_id = import_and_finish(&mut seed, "common/seed", 100);
    seed.sync().unwrap();
    drop(seed);
    fs::copy(&left_path, &right_path).unwrap();

    let mut left = Cva::open(&left_path).unwrap();
    let left_id = import_and_finish(&mut left, "branch/left", 200);
    left.sync().unwrap();
    drop(left);

    let mut right = Cva::open(&right_path).unwrap();
    let first_id = import_and_finish(&mut right, "branch/right-first", 200);
    append_live_node(&mut right, "between-right-imports", 300);
    let second_id = import_and_finish(&mut right, "branch/right-second", 310);
    right
        .import_native(&empty_input("branch/right-empty"), 420)
        .unwrap();
    let empty_start = right.open_import_epoch().unwrap().clone();
    crate::imports::tests::finish_current_import(&mut right, 430);
    let empty_id = empty_start.import_id;
    assert!(right.open_import_epoch().is_none());
    right.sync().unwrap();
    drop(right);

    Cva::reconcile(&left_path, &right_path, &merged_path).unwrap();
    let mut merged = Cva::open(&merged_path).unwrap();
    assert_eq!(merged.import_epoch_statuses().len(), 5);
    assert_eq!(merged.freshness.import_settlements().len(), 5);
    for import_id in [&seed_id, &left_id, &first_id, &second_id, &empty_id] {
        assert!(matches!(
            status(&merged, import_id),
            ImportEpochStatus::Completed { .. }
        ));
    }
    assert_eq!(
        merged
            .conversation_turns("reconcile-test-live", "between-right-imports")
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn valid_insomnia_journal_history_is_refused_without_mutating_sources() {
    let f = Fixture::new();
    let left_path = f.rel("left.rel");
    let right_path = f.rel("right.rel");
    let merged_path = f.rel("merged.rel");
    let mut seed = Cva::create(&left_path).unwrap();
    import_and_finish(&mut seed, "common/seed", 100);
    seed.sync().unwrap();
    drop(seed);
    fs::copy(&left_path, &right_path).unwrap();

    let mut left = Cva::open(&left_path).unwrap();
    import_and_finish(&mut left, "branch/left", 200);
    left.sync().unwrap();
    drop(left);

    let mut right = Cva::open(&right_path).unwrap();
    right.import_native(&input("branch/right"), 200).unwrap();
    let scope = right.import_epoch_scope().unwrap();
    let episode_id = crate::EpisodeId(*scope.episodes.iter().next().unwrap());
    right.with_import_write_scope(|right| {
        let claim = right
            .claim_insomnia_episode("reconcile-journal-test", 210, 1_000)
            .unwrap()
            .unwrap();
        assert_eq!(claim.episode_id, episode_id);
        right
            .terminal_insomnia_episode(
                episode_id,
                claim.lease_token.unwrap(),
                210,
                211,
                "retry retained journal history".into(),
            )
            .unwrap();
        assert_eq!(
            right
                .retry_terminal_insomnia_episode(episode_id, 212)
                .unwrap()
                .state,
            crate::InsomniaWorkState::Pending
        );
    });
    crate::imports::tests::finish_current_import(&mut right, 220);
    assert_eq!(
        right.insomnia_work(episode_id).unwrap().state,
        crate::InsomniaWorkState::Complete
    );
    let has_journal_record = right.container.chunks().unwrap().into_iter().any(|chunk| {
        right
            .container
            .read(chunk)
            .unwrap()
            .starts_with(b"CVAINSW1")
    });
    assert!(
        has_journal_record,
        "retained journal records remain in the completed epoch"
    );
    right.sync().unwrap();
    drop(right);

    let before_left = fs::read(&left_path).unwrap();
    let before_right = fs::read(&right_path).unwrap();
    assert!(matches!(
        Cva::reconcile(&left_path, &right_path, &merged_path),
        Err(crate::CvaReconcileError::UnsupportedSemanticOwner(
            "unhandled owner payload in import interval"
        ))
    ));
    assert!(!merged_path.exists());
    assert_eq!(fs::read(&left_path).unwrap(), before_left);
    assert_eq!(fs::read(&right_path).unwrap(), before_right);
}
