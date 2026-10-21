use super::*;
use crate::graph_compact_codec::publication::{self, GraphPublication, GraphTransaction};
use crate::graph_compact_contract::GraphOrdinalMutation;
use crate::{GraphRelationKind, GraphRelationOrigin, SemanticGraphRelationKind, SemanticNodeRef};
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
