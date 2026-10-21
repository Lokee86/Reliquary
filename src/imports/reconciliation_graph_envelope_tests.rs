use super::*;
use crate::graph_compact_codec::publication::{self, GraphPublication, GraphTransaction};
use crate::graph_compact_contract::GraphOrdinalMutation;
use crate::{
    EpisodeBoundary, EpisodeConfig, EpisodeOrigin, GraphRelationKind, GraphRelationOrigin,
    InsomniaPriority, SemanticGraphRelationKind, SemanticNodeRef,
};
use std::{fs, path::PathBuf};

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!("graph-envelope-interval-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn fixture(
    mode: u8,
) -> (
    Scratch,
    PathBuf,
    PathBuf,
    usize,
    Vec<u8>,
    Option<i64>,
    Vec<GraphOrdinalMutation>,
) {
    let scratch = Scratch::new();
    let source = scratch.0.join("source.rel");
    let target = scratch.0.join("target.rel");
    let mut owner = Cva::create_project(&source).unwrap();
    let mut ids = Vec::new();
    for i in 0..3 {
        ids.push(
            owner
                .publish_memory(
                    None,
                    0,
                    crate::entity_candidate_test_support::memory_draft(
                        &format!("memory-{i}"),
                        "title",
                        "body",
                    ),
                )
                .unwrap()
                .0
                .id,
        );
    }
    assert_eq!(owner.graph.stats().nodes, 0);
    owner.sync().unwrap();
    fs::copy(&source, &target).unwrap();
    for (ordinal, id) in ids.iter().enumerate() {
        owner
            .container
            .append(&crate::graph_codec::encode_node(
                crate::graph_codec::GraphNodePayload {
                    semantic_node: SemanticNodeRef::memory(*id),
                    node_id: arcana::NodeId(ordinal as u32),
                },
            ))
            .unwrap();
    }
    let row = GraphOrdinalMutation {
        source: 0,
        target: 1,
        kind: SemanticGraphRelationKind::Memory(GraphRelationKind::Factual),
        active: true,
        origin: GraphRelationOrigin::User,
    };
    let mut rows = vec![
        row,
        GraphOrdinalMutation {
            source: 2,
            target: 0,
            kind: SemanticGraphRelationKind::Memory(GraphRelationKind::Causal),
            ..row
        },
    ];
    rows.sort_by_key(|row| ids[row.source as usize].0);
    rows.reverse();
    let (transaction, time) = match mode {
        0 => (GraphTransaction::Single(row), Some(-123)),
        1 => (GraphTransaction::Batch(rows.clone()), None),
        2 => (GraphTransaction::Batch(vec![row]), Some(77)),
        _ => {
            rows[1].origin = GraphRelationOrigin::Dream;
            (GraphTransaction::Batch(rows.clone()), Some(99))
        }
    };
    let expected = match &transaction {
        GraphTransaction::Single(row) => vec![*row],
        GraphTransaction::Batch(rows) => rows.clone(),
    };
    let body = publication::encode(&GraphPublication {
        graph_version: 1,
        transaction,
    })
    .unwrap();
    let reserved = owner
        .container
        .allocate_version_with_transaction_time(time)
        .unwrap();
    owner
        .container
        .allocate_version_with_transaction_time(Some(500))
        .unwrap();
    let index = owner.container.chunks().unwrap().len();
    let payload = crate::container::publication_envelope::encode(
        crate::container::publication_envelope::PublicationEnvelope {
            global_version: reserved,
            clock: crate::container::publication_envelope::PublicationClock::BindReserved,
            body: &body,
        },
    )
    .unwrap();
    owner.container.append(&payload).unwrap();
    owner.sync().unwrap();
    drop(owner);
    (scratch, source, target, index, payload, time, expected)
}

fn empty_import(namespace: &str) -> crate::imports::NativeImport {
    crate::imports::NativeImport {
        schema_version: 1,
        source_namespace: namespace.into(),
        source_format: "graph-partial-dedup-test-v1".into(),
        conversations: Vec::new(),
    }
}

fn append_native_graph_batch(cva: &mut Cva, rows: Vec<GraphOrdinalMutation>) {
    let body = publication::encode(&GraphPublication {
        graph_version: cva.graph_version() + 1,
        transaction: GraphTransaction::Batch(rows),
    })
    .unwrap();
    cva.container.append_publication_envelope(&body).unwrap();
}

fn graph_transaction_in_epoch(
    cva: &mut Cva,
    epoch: &super::CompletedEpoch,
) -> (
    bool,
    Vec<crate::graph_codec::GraphMutationPayload>,
    Option<i64>,
) {
    let chunks = cva.container.chunks().unwrap();
    for object in chunks.into_iter().take(epoch.end).skip(epoch.first) {
        let payload = cva.container.read(object).unwrap();
        let Some(version) = publication::version_at(object, &payload).unwrap() else {
            continue;
        };
        let mutation = cva.container.read(version.mutation).unwrap();
        if let Some((single, rows)) = cva.graph.decode_stored_transaction(&mutation).unwrap() {
            return (
                single,
                rows,
                cva.transaction_time_ns(version.global_version),
            );
        }
    }
    panic!("completed import interval has no Graph transaction");
}

fn partial_or_full_duplicate_completed_batch(existing_members: usize) {
    let scratch = Scratch::new();
    let base_path = scratch.0.join("base.rel");
    let left_path = scratch.0.join("left.rel");
    let right_path = scratch.0.join("right.rel");
    let merged_path = scratch.0.join("merged.rel");
    let repeated_path = scratch.0.join("repeated.rel");

    let mut base = Cva::create(&base_path).unwrap();
    let mut parent = None;
    for index in 0..8 {
        let user = format!("u{index}");
        let assistant = format!("a{index}");
        base.append_node(
            user.clone(),
            "shared-conversation".into(),
            parent.clone(),
            "user".into(),
            10 + index as i64 * 2,
            &format!("Graph interval provenance fact {index}."),
        )
        .unwrap();
        base.append_node(
            assistant.clone(),
            "shared-conversation".into(),
            Some(user),
            "assistant".into(),
            11 + index as i64 * 2,
            "Acknowledged.",
        )
        .unwrap();
        parent = Some(assistant);
    }
    let episode = base
        .materialize_path_episodes(
            "shared-conversation",
            parent.as_deref().unwrap(),
            EpisodeConfig::default(),
            EpisodeOrigin::Live,
            Some((EpisodeBoundary::Inactivity, 100)),
        )
        .unwrap()
        .created
        .remove(0);
    base.queue_insomnia_episode(episode.id, InsomniaPriority::Live, 101)
        .unwrap();
    let ids = crate::insomnia::completion_tests::process(&mut base, 3, 110)
        .created
        .into_iter()
        .map(|memory| memory.id)
        .collect::<Vec<_>>();
    assert_eq!(ids.len(), 3, "live completion supplies proven Memory nodes");
    for (ordinal, id) in ids.iter().enumerate() {
        base.container
            .append(&crate::graph_codec::encode_node(
                crate::graph_codec::GraphNodePayload {
                    semantic_node: SemanticNodeRef::memory(*id),
                    node_id: arcana::NodeId(ordinal as u32),
                },
            ))
            .unwrap();
    }
    base.sync().unwrap();
    drop(base);
    fs::copy(&base_path, &left_path).unwrap();
    fs::copy(&base_path, &right_path).unwrap();

    let rows = [(0, 1), (1, 2)]
        .into_iter()
        .map(|(source, target)| GraphOrdinalMutation {
            source,
            target,
            kind: SemanticGraphRelationKind::Memory(GraphRelationKind::Factual),
            active: true,
            origin: GraphRelationOrigin::User,
        })
        .collect::<Vec<_>>();

    let mut left = Cva::open(&left_path).unwrap();
    for (source, target) in [(ids[0], ids[1]), (ids[1], ids[2])]
        .into_iter()
        .take(existing_members)
    {
        left.set_memory_relation_with_origin(
            source,
            target,
            GraphRelationKind::Factual,
            true,
            GraphRelationOrigin::User,
            left.graph_version(),
        )
        .unwrap();
    }
    left.sync().unwrap();
    drop(left);

    let mut right = Cva::open(&right_path).unwrap();
    right
        .import_native(&empty_import("partial-duplicate-epoch"), 100)
        .unwrap();
    right.with_import_write_scope(|right| append_native_graph_batch(right, rows));
    right.sync().unwrap();
    drop(right);
    let mut right = Cva::open(&right_path).unwrap();
    let import_id = right.open_import_epoch().unwrap().import_id.clone();
    crate::imports::tests::finish_current_import(&mut right, 110);
    right.sync().unwrap();
    drop(right);

    let mut source = Cva::open(&right_path).unwrap();
    let source_epoch = super::inventory(&mut source).unwrap();
    let source_epoch = source_epoch.get(&import_id).unwrap();
    let source_evidence = source_epoch.owner_evidence.clone();
    let source_transaction = graph_transaction_in_epoch(&mut source, source_epoch);
    assert!(!source_transaction.0, "the source publication is a batch");
    assert_eq!(source_transaction.1.len(), 2);
    assert_eq!(
        (
            source_transaction.1[0].source,
            source_transaction.1[0].target
        ),
        (
            SemanticNodeRef::memory(ids[0]),
            SemanticNodeRef::memory(ids[1])
        )
    );
    assert_eq!(
        (
            source_transaction.1[1].source,
            source_transaction.1[1].target
        ),
        (
            SemanticNodeRef::memory(ids[1]),
            SemanticNodeRef::memory(ids[2])
        )
    );
    drop(source);
    let before_left = fs::read(&left_path).unwrap();
    let before_right = fs::read(&right_path).unwrap();
    Cva::reconcile(&left_path, &right_path, &merged_path).unwrap();
    let mut merged = Cva::open(&merged_path).unwrap();
    let graph_transactions = merged.graph.transaction_global_versions().len();
    assert_eq!(
        merged.graph.stats().active_relations,
        2,
        "both edges are effective"
    );
    let merged_epoch = super::inventory(&mut merged).unwrap();
    let merged_epoch = merged_epoch.get(&import_id).unwrap();
    let merged_evidence = &merged_epoch.owner_evidence;
    assert!(
        *merged_evidence == source_evidence,
        "reconciled import retains its original normalized Graph batch evidence"
    );
    let merged_transaction = graph_transaction_in_epoch(&mut merged, merged_epoch);
    assert_eq!(merged_transaction, source_transaction);
    drop(merged);
    assert_eq!(fs::read(&left_path).unwrap(), before_left);
    assert_eq!(fs::read(&right_path).unwrap(), before_right);

    let result = Cva::reconcile(&merged_path, &right_path, &repeated_path);
    assert!(
        result.is_ok(),
        "retrying the same source epoch should retain completed-epoch authority: {result:?}"
    );
    assert!(repeated_path.exists());
    assert_eq!(fs::read(&left_path).unwrap(), before_left);
    assert_eq!(fs::read(&right_path).unwrap(), before_right);
    assert!(graph_transactions >= 1);
}

#[test]
fn completed_native_graph_batch_partial_duplicate_preserves_authority_on_retry() {
    partial_or_full_duplicate_completed_batch(1);
}

#[test]
fn completed_native_graph_batch_fully_duplicate_preserves_authority_on_retry() {
    partial_or_full_duplicate_completed_batch(2);
}

#[test]
fn native_graph_intervals_preserve_order_batch_shape_time_evidence_and_duplicate_noop() {
    for mode in 0..3 {
        let (_scratch, path, target, index, payload, time, expected) = fixture(mode);
        let mut source = Cva::open(&path).unwrap();
        let mut output = Cva::open(&target).unwrap();
        super::validate_replay_interval(&mut source, index).unwrap();
        let mut coordinates =
            crate::imports::reconciliation_interval::Coordinates::shared(&mut source, index)
                .unwrap();
        crate::imports::reconciliation_interval::replay_frame(
            &mut source,
            &mut output,
            index,
            &payload,
            &mut coordinates,
        )
        .unwrap();
        assert_eq!(coordinates.graph_transactions, 1);
        assert_eq!(coordinates.graph_mutations, expected.len());
        assert_eq!(output.graph_version(), 1);
        assert_eq!(output.memory_graph_version(), 1);
        let global = output.graph.transaction_global_versions()[0];
        assert_eq!(output.transaction_time_ns(global), time);
        let source_object = source.container.chunks().unwrap()[index];
        let version = publication::version_at(source_object, &payload)
            .unwrap()
            .unwrap();
        let expected_evidence = super::canonical_graph_publication(&mut source, version).unwrap();
        let mut found = None;
        for object in output.container.chunks().unwrap() {
            let bytes = output.container.read(object).unwrap();
            if let Some(version) = publication::version_at(object, &bytes).unwrap() {
                found = Some(version);
            }
        }
        let target_version = found.unwrap();
        let bytes = output.container.read(target_version.mutation).unwrap();
        let (single, values) = output
            .graph
            .decode_stored_transaction(&bytes)
            .unwrap()
            .unwrap();
        assert_eq!(single, mode == 0);
        assert_eq!(values.len(), expected.len());
        assert_eq!(
            output.graph.normalized_import_evidence(&bytes).unwrap(),
            expected_evidence
        );
        output.sync().unwrap();
        let before = fs::read(&target).unwrap();
        crate::imports::reconciliation_interval::replay_frame(
            &mut source,
            &mut output,
            index,
            &payload,
            &mut coordinates,
        )
        .unwrap();
        output.sync().unwrap();
        assert_eq!(fs::read(&target).unwrap(), before);
        assert_eq!(coordinates.graph_transactions, 1);
        assert_eq!(coordinates.duplicate_graph_mutations, expected.len());
    }
}

#[test]
fn admitted_mixed_origin_graph_interval_rejects_replay_without_destination_mutation() {
    let (_scratch, path, target, index, payload, _, _) = fixture(3);
    let mut source = Cva::open(&path).unwrap();
    let mut output = Cva::open(&target).unwrap();
    let before = fs::read(&target).unwrap();
    let mut coordinates =
        crate::imports::reconciliation_interval::Coordinates::shared(&mut source, index).unwrap();
    assert!(
        crate::imports::reconciliation_interval::replay_frame(
            &mut source,
            &mut output,
            index,
            &payload,
            &mut coordinates
        )
        .is_err()
    );
    output.sync().unwrap();
    assert_eq!(output.graph_version(), 0);
    assert_eq!(fs::read(&target).unwrap(), before);
}
