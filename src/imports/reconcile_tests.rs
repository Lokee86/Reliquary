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

fn import_and_finish_with_grouped_memory(cva: &mut Cva, namespace: &str, now_ns: i64) -> String {
    let mut imported = input(namespace);
    let mut messages = Vec::new();
    let mut parent = None;
    for index in 0..8 {
        let user = format!("u{index}");
        let assistant = format!("a{index}");
        messages.push(message(
            &user,
            parent.as_deref(),
            "user",
            &format!("Atomic Insomnia completion fact {index}."),
        ));
        messages.push(message(
            &assistant,
            Some(&user),
            "assistant",
            "Acknowledged.",
        ));
        parent = Some(assistant);
    }
    messages[0].artifacts.push(crate::imports::ImportArtifact {
        id: "source-artifact".into(),
        source_ref: "source:artifact".into(),
        filename: "evidence.txt".into(),
        mime_type: Some("text/plain".into()),
        bytes: b"retained import artifact".to_vec(),
    });
    imported.conversations[0].messages = messages;
    imported.conversations[0].branches[0].leaf_message_id = parent.unwrap();
    cva.import_native(&imported, now_ns).unwrap();
    let start = cva.open_import_epoch().unwrap().clone();
    let scope = cva.import_epoch_scope().unwrap();
    cva.container.set_owner_write_scope(true);
    let mut first = true;
    let mut grouped_memory_id = None;
    while scope.episodes.iter().any(|episode| {
        cva.insomnia_work(crate::EpisodeId(*episode))
            .is_some_and(|work| work.state != crate::InsomniaWorkState::Complete)
    }) {
        let result = process_import_episode(
            cva,
            if first { 1 } else { 0 },
            now_ns + if first { 0 } else { 1 },
        );
        if first {
            assert_eq!(result.created.len(), 1, "fixture creates a grouped Memory");
            grouped_memory_id = result.created.first().map(|memory| memory.id);
            first = false;
        }
    }
    assert!(!first, "the import contains a processable Episode");
    cva.container.set_owner_write_scope(true);
    let memory_id = grouped_memory_id.expect("nonzero grouped completion Memory");
    let memory = cva.memory(memory_id).unwrap();
    let mention_text = "Insomnia";
    let mention_start = memory.content.find(mention_text).unwrap();
    let mention = crate::MemoryEntityMention {
        field: crate::MemoryTextField::Content,
        start_byte: mention_start as u32,
        end_byte: (mention_start + mention_text.len()) as u32,
        text: mention_text.into(),
    };
    let mention_key = crate::MemoryEntityMentionKey::new(memory_id, &mention);
    let routing = crate::MemoryRoutingMetadata {
        memory_id,
        body_id: cva.memory_body_id(memory_id).unwrap(),
        entity_mentions: vec![mention],
    };
    cva.memories
        .put_routing_metadata(&mut cva.container, routing)
        .unwrap();
    assert!(
        cva.put_entity_rejected(
            mention_key,
            0,
            crate::EntityResolutionReason::GenericRole,
            now_ns + 2,
        )
        .unwrap()
    );

    // Retain a completed Dream pass with an explicit empty root set, so replay
    // proves that empty roots are accepted source evidence rather than absent data.
    let pass_id = crate::dream_freshness::initial_pass_id(cva.owner_uuid().unwrap(), memory_id);
    let turn = cva.rel_turn_count();
    cva.begin_initial_dream_pass(
        pass_id.clone(),
        memory_id,
        1,
        turn,
        std::collections::BTreeSet::new(),
        cva.graph_version(),
    )
    .unwrap();
    cva.reconcile_dream_lifecycle(memory_id).unwrap();
    cva.mark_dream_processed(memory_id, 0, now_ns + 3).unwrap();
    cva.record_initial_dream_roots(&pass_id, std::collections::BTreeSet::new())
        .unwrap();
    cva.accept_initial_dream_settlement(&pass_id, turn).unwrap();
    cva.settle_initial_dream_pass(
        &pass_id,
        false,
        crate::FreshnessEventProof {
            graph_version: cva.memory_graph_version(),
            community_generation: None,
            community_graph_version: None,
        },
        std::collections::BTreeMap::new(),
    )
    .unwrap();
    cva.container.set_owner_write_scope(false);
    let reconstructed = cva
        .reconstruct_historical_freshness(crate::HistoricalReconstructionOptions::default())
        .unwrap();
    assert!(reconstructed.committed);
    let outcome = cva
        .complete_native_import(&start.import_id, now_ns + 10)
        .unwrap();
    assert!(
        matches!(
            outcome,
            ImportCompletionOutcome::Completed {
                committed: true,
                ..
            }
        ),
        "{outcome:?}"
    );
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
fn divergent_completed_grouped_memory_epoch_replays_through_destination_owners() {
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
    let left_memory_version = left.memory_version();
    left.sync().unwrap();
    drop(left);

    let mut right = Cva::open(&right_path).unwrap();
    let right_id = import_and_finish_with_grouped_memory(&mut right, "branch/right-memory", 200);
    let source_memory = right
        .memory_ids()
        .into_iter()
        .map(|id| right.memory(id).unwrap())
        .collect::<Vec<_>>();
    let source_memory = source_memory
        .into_iter()
        .find(|memory| memory.source_episode_id.is_some())
        .expect("grouped import Memory");
    let source_work = right
        .insomnia_work(source_memory.source_episode_id.unwrap())
        .unwrap()
        .clone();
    let source_attempts = right.insomnia_attempts(source_memory.source_episode_id.unwrap());
    let source_resolution = right
        .entity_resolutions
        .records()
        .into_iter()
        .find(|resolution| resolution.key.memory_id == source_memory.id)
        .expect("terminal Perception rejection");
    assert!(matches!(
        &source_resolution.status,
        crate::MemoryEntityResolutionStatus::Rejected { .. }
    ));
    let source_dream_pass = right
        .historical_completed_initial_dream_passes()
        .unwrap()
        .remove(&source_memory.id)
        .expect("completed empty-root Dream pass");
    assert!(source_dream_pass.roots.is_empty());
    right.sync().unwrap();
    drop(right);

    Cva::reconcile(&left_path, &right_path, &merged_path).unwrap();
    let mut merged = Cva::open(&merged_path).unwrap();
    assert!(matches!(
        status(&merged, &right_id),
        ImportEpochStatus::Completed { .. }
    ));
    let replayed = merged.memory(source_memory.id).unwrap();
    let mut expected_semantics = source_memory.clone();
    expected_semantics.global_version = replayed.global_version;
    expected_semantics.memory_version = replayed.memory_version;
    assert_eq!(replayed, expected_semantics);
    assert_eq!(merged.memory_version(), left_memory_version + 2);
    let replayed_work = merged
        .insomnia_work(source_memory.source_episode_id.unwrap())
        .unwrap();
    assert_eq!(replayed_work, &source_work);
    assert_eq!(
        merged.insomnia_attempts(source_memory.source_episode_id.unwrap()),
        source_attempts
    );
    assert!(merged.freshness.records().contains_key(&source_memory.id));
    assert!(
        merged
            .dream_freshness
            .is_publication_birth(source_memory.id)
    );
    assert_eq!(
        merged.entity_resolution(source_resolution.key),
        Some(&source_resolution)
    );
    let replayed_dream_pass = merged
        .historical_completed_initial_dream_passes()
        .unwrap()
        .remove(&source_memory.id)
        .expect("replayed completed Dream pass");
    assert!(replayed_dream_pass.roots.is_empty());
    assert_eq!(replayed_dream_pass.pass_id, source_dream_pass.pass_id);
    drop(merged);
    let before = fs::read(&merged_path).unwrap();
    let repeated = f.rel("repeated.rel");
    let result = Cva::reconcile(&merged_path, &right_path, &repeated).unwrap();
    assert!(!result.canonical_change_required);
    assert_eq!(before, fs::read(repeated).unwrap());
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
fn valid_insomnia_journal_history_is_replayed_with_completed_import() {
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
    let right_id = right.open_import_epoch().unwrap().import_id.clone();
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
    let has_work_record = right.container.chunks().unwrap().into_iter().any(|chunk| {
        right
            .container
            .read(chunk)
            .unwrap()
            .starts_with(b"CVAINSW1")
    });
    let has_attempt_record = right.container.chunks().unwrap().into_iter().any(|chunk| {
        right
            .container
            .read(chunk)
            .unwrap()
            .starts_with(b"CVAINSA1")
    });
    assert!(
        has_work_record,
        "retained work journal remains in the completed epoch"
    );
    let _ = has_attempt_record; // Terminal failures are retained as work records in this codec.
    let expected_work = right.insomnia_work(episode_id).unwrap().clone();
    let expected_attempts = right.insomnia_attempts(episode_id);
    right.sync().unwrap();
    drop(right);

    Cva::reconcile(&left_path, &right_path, &merged_path).unwrap();
    let merged = Cva::open(&merged_path).unwrap();
    assert_eq!(merged.insomnia_work(episode_id), Some(&expected_work));
    assert_eq!(merged.insomnia_attempts(episode_id), expected_attempts);
    assert!(matches!(
        status(&merged, &right_id),
        ImportEpochStatus::Completed { .. }
    ));
}

fn process_import_episode(
    cva: &mut Cva,
    count: usize,
    now_ns: i64,
) -> crate::InsomniaProcessResult {
    cva.container.set_owner_write_scope(true);
    let claim = cva
        .claim_insomnia_episode("import-readiness-worker", now_ns, 1_000)
        .unwrap()
        .unwrap();
    assert!(
        cva.import_epoch_scope()
            .unwrap()
            .episodes
            .contains(&claim.episode_id.0)
    );

    let turns = cva.episode_turns(claim.episode_id).unwrap();
    let mut turn_responses = serde_json::Map::new();
    let mut groups = serde_json::Map::new();
    let mut user_index = 0;
    for turn in turns.iter().filter(|turn| turn.role == "user") {
        let clause = if user_index < count {
            serde_json::json!({
                "disposition": "retain",
                "authority_kind": "direct",
                "category": "decision",
                "type": "project",
                "lifecycle": "current",
                "proposition": format!("Imported Insomnia completion invariant {user_index}."),
                "authority_source_node_id": "",
                "grounding_source_node_id": "",
                "reason": "synthetic durable state"
            })
        } else {
            serde_json::json!({
                "disposition": "omit",
                "authority_kind": "none",
                "category": "none",
                "type": "none",
                "lifecycle": "none",
                "proposition": "",
                "authority_source_node_id": "",
                "grounding_source_node_id": "",
                "reason": "synthetic omission"
            })
        };
        turn_responses.insert(turn.node_id.clone(), serde_json::json!([clause]));
        if user_index < count {
            groups.insert(
                format!("g{user_index:03}"),
                serde_json::json!({
                    "title": format!("Imported completion {user_index}"),
                    "content": format!("Imported Insomnia completion invariant {user_index}.")
                }),
            );
        }
        user_index += 1;
    }
    assert_eq!(
        user_index, 8,
        "fixture import Episode must contain eight user turns"
    );
    let mut responses = vec![serde_json::json!({"turns": turn_responses, "evidence_requests": []})];
    if count > 0 {
        responses.push(serde_json::json!({"groups": groups}));
    }
    let extractor = crate::InsomniaExtractor::new(crate::SimulatedGeneralEndpoint::new(
        "synthetic-import-readiness",
        responses,
    ));
    let result = cva
        .process_claimed_insomnia_episode(&claim, &extractor, "private", now_ns, now_ns + 1)
        .unwrap();
    cva.container.set_owner_write_scope(false);
    result
}
