use super::*;
use crate::staged_copy::StagedCopy;
use reliquary_archive::{ImportReceipt, TurnInput};
use reliquary_container::ReliquaryScope;
use reliquary_graph::{NodeRef, RelationChange, RelationKind, RelationOrigin};
use reliquary_memory_core::{
    MemoryRecord, encode_grouped_creation, memory_body_bytes, memory_body_id, memory_id,
};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(1);

struct Temp(PathBuf);

impl Temp {
    fn new(tag: &str) -> Self {
        let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join(".test-tmp");
        fs::create_dir_all(&directory).unwrap();
        Self(directory.join(format!(
            "semantic-reconcile-{tag}-{}-{}.rel",
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

fn draft(mutation: &str, source_node: Option<&str>) -> MemoryDraft {
    MemoryDraft {
        title: mutation.into(),
        content: format!("content for {mutation}"),
        category: "fact".into(),
        memory_type: "semantic".into(),
        authority_kind: "owner".into(),
        temporal_status: "current".into(),
        scope: "connection".into(),
        lifecycle_state: "active".into(),
        archived: false,
        parent_id: None,
        superseded_by: None,
        source_node_id: source_node.map(str::to_owned),
        content_source_conversation_id: source_node.map(|_| "conversation".into()),
        content_source_node_id: source_node.map(str::to_owned),
        grounding_source_conversation_id: None,
        grounding_source_node_id: None,
        source_episode_id: None,
        source_ref: None,
        source_time_ns: Some(10),
        temporal_inference: None,
        mutation_id: mutation.into(),
        created_at_ns: 10,
        updated_at_ns: 10,
    }
}

fn append_branch(path: &Path, node: &str, content: &str, mutations: &[&str]) {
    let mut container = Container::open_writer_as(path, FileKind::Reliquary).unwrap();
    let mut archive = Archive::open(&container).unwrap();
    archive
        .accept_turn(
            &mut container,
            TurnInput {
                id: node.into(),
                conversation_id: "conversation".into(),
                parent_id: None,
                role: "user".into(),
                principal_id: Some("principal".into()),
                timestamp_ns: 10,
                content: content.into(),
                attachments: Vec::new(),
                project_attachments: Vec::new(),
            },
        )
        .unwrap();
    let mut memories = MemoryStore::open(&container).unwrap();
    let mut freshness = FreshnessStore::open(&container).unwrap();
    let mut ids = Vec::new();
    for mutation in mutations {
        let source = (*mutation != "duplicate").then_some(node);
        let published = memories
            .publish(&mut container, None, 0, draft(mutation, source))
            .unwrap();
        freshness
            .create_birth(
                &mut container,
                FreshnessMemoryId(published.revision.record.id.0),
                1,
                true,
            )
            .unwrap();
        ids.push(published.revision.record.id);
    }
    if ids.len() == 2 {
        let admission =
            GraphAdmission::for_changes(owner_id(&container), ids.iter().map(|id| id.0), []);
        let mut graph = GraphStore::open(&container, &admission).unwrap();
        graph
            .put(
                &mut container,
                &admission,
                &[RelationChange {
                    source: NodeRef::Memory(ids[0].0),
                    target: NodeRef::Memory(ids[1].0),
                    kind: RelationKind::Topical,
                    active: true,
                    origin: RelationOrigin::User,
                }],
            )
            .unwrap();
    }
    container.sync().unwrap();
}

fn owner_pair() -> (Temp, Temp, [u8; 16]) {
    let left = Temp::new("left");
    let right = Temp::new("right");
    let owner = [91; 16];
    drop(RelWriter::create_path(&left.0, ReliquaryScope::Connection, owner).unwrap());
    fs::copy(&left.0, &right.0).unwrap();
    (left, right, owner)
}

fn owner_id(container: &Container) -> [u8; 16] {
    container.owner_id()
}

fn complete_empty_import(path: &Path, owner: [u8; 16], key_byte: u8, started_at_ns: i64) -> String {
    let admission = GraphAdmission::for_replay(owner, [], [], [], [], []);
    let mut writer = RelWriter::open_path(path, admission, [], []).unwrap();
    let key = format!("{key_byte:02x}").repeat(32);
    let ImportEpochStatus::Open(start) = writer
        .start_native_import(&key, "reconciliation/fixture", "empty-v1", started_at_ns)
        .unwrap()
    else {
        panic!("new fixture import must be open");
    };
    writer
        .commit_native_import_receipt(
            &start.import_id,
            ImportReceipt {
                schema_version: 1,
                owner_uuid: owner,
                import_key: [key_byte; 32],
                source_namespace: "reconciliation/fixture".into(),
                source_format: "empty-v1".into(),
                source_digest: [key_byte; 32],
                conversations: Vec::new(),
                receipt_digest: [0; 32],
            },
        )
        .unwrap();
    writer
        .complete_native_import(&start.import_id, started_at_ns + 1)
        .unwrap();
    start.import_id
}

fn complete_grouped_memory_import(
    path: &Path,
    owner: [u8; 16],
    key_byte: u8,
) -> (String, MemoryId, reliquary_archive::EpisodeId) {
    let admission = GraphAdmission::for_replay(owner, [], [], [], [], []);
    let mut writer = RelWriter::open_path(path, admission, [], []).unwrap();
    let key = format!("{key_byte:02x}").repeat(32);
    let ImportEpochStatus::Open(start) = writer
        .start_native_import(&key, "reconciliation/grouped", "grouped-v1", 200)
        .unwrap()
    else {
        panic!("new grouped fixture import must be open");
    };
    writer
        .accept_native_import_turn(
            &start.import_id,
            TurnInput {
                id: "grouped-turn".into(),
                conversation_id: "grouped-conversation".into(),
                parent_id: None,
                role: "user".into(),
                principal_id: Some("alice".into()),
                timestamp_ns: 200,
                content: "grouped import evidence".into(),
                attachments: vec![],
                project_attachments: vec![],
            },
            reliquary_archive::Branch {
                id: "main".into(),
                conversation_id: "grouped-conversation".into(),
                leaf_node_id: "grouped-turn".into(),
                canonical: true,
            },
        )
        .unwrap();
    let episode = writer
        .finalize_native_import_episode(
            &start.import_id,
            "grouped-conversation",
            "grouped-turn",
            "grouped-turn",
            201,
        )
        .unwrap();
    writer
        .commit_native_import_receipt(
            &start.import_id,
            ImportReceipt {
                schema_version: 1,
                owner_uuid: owner,
                import_key: [key_byte; 32],
                source_namespace: "reconciliation/grouped".into(),
                source_format: "grouped-v1".into(),
                source_digest: [key_byte; 32],
                conversations: vec![crate::model::ImportedConversation {
                    source_id: "source-conversation".into(),
                    conversation_id: "grouped-conversation".into(),
                    title: None,
                    messages: vec![crate::model::ImportedMessage {
                        source_id: "source-turn".into(),
                        revision: "1".into(),
                        source_parent_id: None,
                        replaces: None,
                        deleted: false,
                        node_id: Some("grouped-turn".into()),
                        fingerprint: [0x44; 32],
                        artifacts: vec![],
                    }],
                    branches: vec![crate::model::ImportedBranch {
                        source_id: "source-main".into(),
                        branch_id: "main".into(),
                        leaf_node_id: Some("grouped-turn".into()),
                        canonical: true,
                    }],
                }],
                receipt_digest: [0; 32],
            },
        )
        .unwrap();
    let claim = writer
        .claim_native_import_episode(&start.import_id, episode, "fixture", 202, 100)
        .unwrap();
    let mutation = "reconciled-grouped-memory";
    let id = memory_id(mutation);
    let body_id = memory_body_id("grouped title", "grouped body");
    let record = MemoryRecord {
        id,
        revision: 1,
        body_id,
        category: "fact".into(),
        memory_type: "semantic".into(),
        authority_kind: "direct".into(),
        temporal_status: "current".into(),
        scope: "project".into(),
        lifecycle_state: "knowledge".into(),
        archived: false,
        parent_id: None,
        superseded_by: None,
        source_node_id: Some("grouped-turn".into()),
        content_source_conversation_id: Some("grouped-conversation".into()),
        content_source_node_id: Some("grouped-turn".into()),
        grounding_source_conversation_id: None,
        grounding_source_node_id: None,
        source_episode_id: Some(episode.0),
        source_ref: None,
        source_time_ns: Some(200),
        temporal_inference: None,
        mutation_id: mutation.into(),
        created_at_ns: 203,
        updated_at_ns: 203,
        memory_version: writer.read_session().memory_version() + 1,
    };
    writer
        .commit_native_import_completion_with_routed_receipts(
            &start.import_id,
            claim.work.lease_token.unwrap(),
            GroupedInsomniaCompletion {
                episode_id: episode.0,
                attempt: claim.work.attempt_count,
                started_at_ns: claim.started_at_ns,
                completed_at_ns: 203,
                transaction_time_ns: 203,
                extractor_model: "fixture".into(),
                extractor_version: "v1".into(),
                rejected_count: 0,
                memory_ids: vec![id.0],
                external_memory_refs: vec![],
                global_version_start: writer.next_global_version().unwrap(),
                bodies: vec![crate::records::GroupedMemoryBody {
                    id: reliquary_memory_core::MemoryBodyId(body_id.0),
                    bytes: memory_body_bytes("grouped title", "grouped body"),
                }],
                memory_records: vec![encode_grouped_creation(&record).unwrap()],
                routing_metadata: vec![],
                freshness_birth_turn: Some(writer.read_session().rel_turn_count()),
                freshness_policy_version: Some(1),
                freshness_birth_memory_ids: vec![id.0],
            },
            203,
            &[],
        )
        .unwrap();
    writer
        .complete_native_import(&start.import_id, 204)
        .unwrap();
    (start.import_id, id, episode)
}

#[test]
fn compares_all_four_relations_and_replays_independent_semantics() {
    let (left, right, owner) = owner_pair();
    let identical = compare_rel_histories(&left.0, &right.0).unwrap();
    assert_eq!(identical.relation, RelHistoryRelation::Identical);

    append_branch(&left.0, "left-node", "left", &["left-memory", "duplicate"]);
    assert_eq!(
        compare_rel_histories(&left.0, &right.0).unwrap().relation,
        RelHistoryRelation::LeftExtendsRight
    );
    append_branch(
        &right.0,
        "right-node",
        "right",
        &["right-memory", "duplicate"],
    );
    let comparison = compare_rel_histories(&left.0, &right.0).unwrap();
    assert_eq!(comparison.relation, RelHistoryRelation::Diverged);

    let output = Temp::new("output");
    let ids = ["left-memory", "right-memory", "duplicate"].map(reliquary_memory_core::memory_id);
    let graph = GraphAdmission::for_replay(owner, ids.map(|id| id.0), [], [], [], []);
    let report =
        reconcile_divergent_rel_copy(&left.0, &right.0, &output.0, &graph, &[], &[]).unwrap();
    assert_eq!(report.comparison, comparison);
    assert_eq!(report.replayed_memory_revisions, 1);
    assert!(report.duplicate_memory_revisions >= 1);
    assert_eq!(report.replayed_graph_mutations, 1);
    assert_eq!(report.replayed_freshness_records, 1);
    let merged = RelSession::open_path(&output.0, &graph, [], []).unwrap();
    assert_eq!(merged.accepted_turn_sources().len(), 2);
    assert_eq!(merged.memory_ids_page(None, 16).unwrap().1.len(), 3);
    assert_eq!(merged.common.graph.active_relations().count(), 2);
    let merged_cut = merged.current_historical_cut();
    let merged_proof = merged.historical_proof(owner, &merged_cut).unwrap();
    drop(merged);

    let repeated = Temp::new("repeated");
    let repeated_report =
        reconcile_divergent_rel_copy(&output.0, &right.0, &repeated.0, &graph, &[], &[]).unwrap();
    assert_eq!(repeated_report.replayed_memory_revisions, 0);
    assert_eq!(repeated_report.replayed_graph_mutations, 0);
    assert_eq!(repeated_report.replayed_freshness_records, 0);
    assert_eq!(fs::read(&repeated.0).unwrap(), fs::read(&output.0).unwrap());
    let repeated_session = RelSession::open_path(&repeated.0, &graph, [], []).unwrap();
    assert_eq!(
        repeated_session
            .historical_proof(owner, &merged_cut)
            .unwrap(),
        merged_proof
    );
}

#[test]
fn conflicting_archive_identity_is_typed_and_leaves_every_path_untouched() {
    let (left, right, owner) = owner_pair();
    append_branch(&left.0, "same-node", "left content", &["left-memory"]);
    append_branch(&right.0, "same-node", "right content", &["right-memory"]);
    let left_before = fs::read(&left.0).unwrap();
    let right_before = fs::read(&right.0).unwrap();
    let output = Temp::new("conflict");
    let graph = GraphAdmission::for_replay(owner, [], [], [], [], []);
    let error =
        reconcile_divergent_rel_copy(&left.0, &right.0, &output.0, &graph, &[], &[]).unwrap_err();
    assert!(matches!(error, RelReconciliationError::SemanticConflict(_)));
    assert!(!output.0.exists());
    assert_eq!(fs::read(&left.0).unwrap(), left_before);
    assert_eq!(fs::read(&right.0).unwrap(), right_before);
}

#[test]
fn conflicting_completed_import_authority_is_typed_and_leaves_every_path_untouched() {
    let (left, right, owner) = owner_pair();
    let left_id = complete_empty_import(&left.0, owner, 0xa0, 100);
    let right_id = complete_empty_import(&right.0, owner, 0xa0, 200);
    assert_eq!(left_id, right_id);
    let left_before = fs::read(&left.0).unwrap();
    let right_before = fs::read(&right.0).unwrap();
    let output = Temp::new("import-conflict");
    let graph = GraphAdmission::for_replay(owner, [], [], [], [], []);

    let error =
        reconcile_divergent_rel_copy(&left.0, &right.0, &output.0, &graph, &[], &[]).unwrap_err();

    assert!(matches!(error, RelReconciliationError::SemanticConflict(_)));
    assert!(!output.0.exists());
    assert_eq!(fs::read(&left.0).unwrap(), left_before);
    assert_eq!(fs::read(&right.0).unwrap(), right_before);
}

#[test]
fn publication_collision_does_not_replace_an_intervening_destination() {
    let destination = Temp::new("publication-collision");
    let staged = StagedCopy::create(&destination.0).unwrap();
    fs::write(&staged.path, b"verified reconciliation").unwrap();
    fs::write(&destination.0, b"third-party owner").unwrap();

    let error = publish_reconciled_copy(&staged, &destination.0).unwrap_err();

    assert_eq!(error, RelReconciliationError::DestinationExists);
    assert_eq!(fs::read(&destination.0).unwrap(), b"third-party owner");
    assert_eq!(fs::read(&staged.path).unwrap(), b"verified reconciliation");
}

#[test]
fn completed_empty_import_epoch_replays_with_destination_watermarks() {
    let (left, right, owner) = owner_pair();
    let left_id = complete_empty_import(&left.0, owner, 0xa1, 100);
    let right_id = complete_empty_import(&right.0, owner, 0xb2, 200);
    let graph = GraphAdmission::for_replay(owner, [], [], [], [], []);
    let right_source = RelSession::open_path(&right.0, &graph, [], []).unwrap();
    let Some(ImportEpochStatus::Completed {
        start: source_start,
        ..
    }) = right_source.import_status(&right_id)
    else {
        panic!("right fixture import must be complete");
    };
    drop(right_source);

    let output = Temp::new("import-output");
    reconcile_divergent_rel_copy(&left.0, &right.0, &output.0, &graph, &[], &[]).unwrap();
    let merged = RelSession::open_path(&output.0, &graph, [], []).unwrap();
    assert!(matches!(
        merged.import_status(&left_id),
        Some(ImportEpochStatus::Completed { .. })
    ));
    let Some(ImportEpochStatus::Completed {
        start: destination_start,
        complete: destination_complete,
    }) = merged.import_status(&right_id)
    else {
        panic!("replayed import must be complete");
    };
    assert_ne!(destination_start.pre, source_start.pre);
    assert_eq!(destination_start.pre.archive_version, Some(1));
    assert_eq!(
        destination_complete.post.archive_version,
        Some(merged.archive_version())
    );
    drop(merged);

    let before = fs::read(&output.0).unwrap();
    let repeated = Temp::new("import-repeated");
    reconcile_divergent_rel_copy(&output.0, &right.0, &repeated.0, &graph, &[], &[]).unwrap();
    assert_eq!(fs::read(&repeated.0).unwrap(), before);
}

#[test]
fn shared_completed_epoch_is_skipped_before_a_right_only_epoch() {
    let (left, right, owner) = owner_pair();
    let shared = complete_empty_import(&left.0, owner, 0xb3, 100);
    fs::copy(&left.0, &right.0).unwrap();
    append_branch(&left.0, "left-after-shared", "left divergence", &[]);
    let right_only = complete_empty_import(&right.0, owner, 0xb4, 200);
    let graph = GraphAdmission::for_replay(owner, [], [], [], [], []);
    let output = Temp::new("shared-then-right-import");

    reconcile_divergent_rel_copy(&left.0, &right.0, &output.0, &graph, &[], &[]).unwrap();

    let merged = RelSession::open_path(&output.0, &graph, [], []).unwrap();
    for id in [shared, right_only] {
        assert!(matches!(
            merged.import_status(&id),
            Some(ImportEpochStatus::Completed { .. })
        ));
    }
    assert!(
        merged
            .archive_node("conversation", "left-after-shared")
            .is_some()
    );
}

#[test]
fn multiple_completed_import_epochs_replay_across_live_activity() {
    let (left, right, owner) = owner_pair();
    let left_id = complete_empty_import(&left.0, owner, 0xc1, 100);
    let first = complete_empty_import(&right.0, owner, 0xd2, 200);
    append_branch(&right.0, "between-imports", "live between imports", &[]);
    let second = complete_empty_import(&right.0, owner, 0xe3, 300);
    let graph = GraphAdmission::for_replay(owner, [], [], [], [], []);
    let output = Temp::new("multiple-import-output");
    reconcile_divergent_rel_copy(&left.0, &right.0, &output.0, &graph, &[], &[]).unwrap();
    let merged = RelSession::open_path(&output.0, &graph, [], []).unwrap();
    for id in [left_id, first, second] {
        assert!(matches!(
            merged.import_status(&id),
            Some(ImportEpochStatus::Completed { .. })
        ));
    }
    assert!(
        merged
            .archive_node("conversation", "between-imports")
            .is_some()
    );
}

#[test]
fn completed_grouped_memory_epoch_replays_through_destination_owners() {
    let (left, right, owner) = owner_pair();
    complete_empty_import(&left.0, owner, 0xf1, 100);
    let (right_id, memory, episode) = complete_grouped_memory_import(&right.0, owner, 0xf2);
    let graph = GraphAdmission::for_replay(owner, [memory.0], [], [], [], []);
    let output = Temp::new("grouped-import-output");
    reconcile_divergent_rel_copy(&left.0, &right.0, &output.0, &graph, &[], &[]).unwrap();
    let merged = RelSession::open_path(&output.0, &graph, [], []).unwrap();
    assert!(matches!(
        merged.import_status(&right_id),
        Some(ImportEpochStatus::Completed { .. })
    ));
    assert_eq!(merged.current_memory(memory).unwrap().record.id, memory);
    assert_eq!(
        merged
            .insomnia_work(WorkEpisodeId(episode.0))
            .unwrap()
            .state,
        reliquary_insomnia_store::InsomniaWorkState::Complete
    );
    assert!(
        merged
            .freshness_record(FreshnessMemoryId(memory.0))
            .unwrap()
            .is_some()
    );
    drop(merged);
    let merged_bytes = fs::read(&output.0).unwrap();
    let repeated = Temp::new("grouped-import-idempotent");
    let repeated_report =
        reconcile_divergent_rel_copy(&output.0, &right.0, &repeated.0, &graph, &[], &[]).unwrap();
    assert_eq!(repeated_report.replayed_memory_revisions, 0);
    assert_eq!(repeated_report.replayed_freshness_records, 0);
    assert_eq!(fs::read(&repeated.0).unwrap(), merged_bytes);
}
#[test]
fn unrelated_live_insomnia_work_must_not_disappear_during_import_merge() {
    let (left, right, owner) = owner_pair();
    append_branch(&left.0, "left-live", "left branch", &[]);
    let graph = GraphAdmission::for_replay(owner, [], [], [], [], []);
    let episode = {
        let mut writer = RelWriter::open_path(&right.0, graph.clone(), [], []).unwrap();
        writer
            .accept_turn_and_advance_branch(
                TurnInput {
                    id: "right-live".into(),
                    conversation_id: "conversation".into(),
                    parent_id: None,
                    role: "user".into(),
                    principal_id: Some("principal".into()),
                    timestamp_ns: 10,
                    content: "right branch".into(),
                    attachments: vec![],
                    project_attachments: vec![],
                },
                reliquary_archive::Branch {
                    id: "right-main".into(),
                    conversation_id: "conversation".into(),
                    leaf_node_id: "right-live".into(),
                    canonical: true,
                },
            )
            .unwrap();
        let episode = writer
            .finalize_episode(
                "conversation",
                "right-live",
                "right-live",
                reliquary_archive::EpisodeOrigin::Live,
                reliquary_archive::EpisodeBoundary::Explicit,
                11,
            )
            .unwrap();
        writer
            .queue_and_claim_insomnia_episode(episode, "worker", 12, 100)
            .unwrap();
        episode
    };
    let right_before = fs::read(&right.0).unwrap();
    let left_before = fs::read(&left.0).unwrap();
    assert!(
        RelSession::open_path(&right.0, &graph, [], [])
            .unwrap()
            .insomnia_work(WorkEpisodeId(episode.0))
            .is_some()
    );
    let output = Temp::new("live-insomnia-conflict");
    let result = reconcile_divergent_rel_copy(&left.0, &right.0, &output.0, &graph, &[], &[]);
    assert!(
        result.is_err(),
        "reconciliation must not silently discard live Insomnia journal"
    );
    assert!(!output.0.exists());
    assert_eq!(fs::read(&right.0).unwrap(), right_before);
    assert_eq!(fs::read(&left.0).unwrap(), left_before);
}
