use super::*;
use reliquary_archive::TurnInput;
use reliquary_container::ReliquaryScope;
use reliquary_graph::{NodeRef, RelationChange, RelationKind, RelationOrigin};
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
