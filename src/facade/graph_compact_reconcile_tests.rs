use super::{read_graph_tail, replay_graph_tail};
use crate::graph_codec::{GraphVersionPayload, encode_version};
use crate::graph_compact_codec::{encode_batch, encode_mutation};
use crate::graph_compact_contract::GraphOrdinalMutation;
use crate::{
    Cva, CvaReconcileError, GraphRelationKind, GraphRelationOrigin, MemoryId,
    SemanticGraphRelationKind, SemanticNodeRef,
};
use std::fs;
use std::path::{Path, PathBuf};

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!("graph-compact-reconcile-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn file(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn base_owner(path: &Path) -> (Cva, [MemoryId; 2]) {
    let mut owner = Cva::create_project(path).unwrap();
    let first = crate::cva_reconcile_graph_test_support::publish_memory(&mut owner, "graph-a");
    let second = crate::cva_reconcile_graph_test_support::publish_memory(&mut owner, "graph-b");
    owner.sync().unwrap();
    (owner, [first, second])
}

fn initialize_mapping_order(owner: &mut Cva, ids: [MemoryId; 2], reverse: bool) {
    let (source, target) = if reverse {
        (ids[1], ids[0])
    } else {
        (ids[0], ids[1])
    };
    owner
        .set_memory_relation(
            source,
            target,
            GraphRelationKind::Causal,
            true,
            owner.graph_version(),
        )
        .unwrap();
}

fn append_compact_marker(
    owner: &mut Cva,
    carrier: &[u8],
    transaction_time_ns: i64,
) -> (usize, u64) {
    let start = owner.container.chunks().unwrap().len();
    let rows = crate::graph_compact_codec::decode(carrier)
        .unwrap()
        .unwrap();
    let transaction = if carrier.starts_with(b"CVAGCM") {
        crate::graph_compact_codec::publication::GraphTransaction::Single(rows[0])
    } else {
        crate::graph_compact_codec::publication::GraphTransaction::Batch(rows)
    };
    let body = crate::graph_compact_codec::publication::encode(
        &crate::graph_compact_codec::publication::GraphPublication {
            graph_version: owner.graph_version() + 1,
            transaction,
        },
    )
    .unwrap();
    owner
        .container
        .set_next_transaction_time_override(Some(transaction_time_ns));
    let (global_version, _) = owner.container.append_publication_envelope(&body).unwrap();
    (start, global_version)
}

#[test]
fn compact_tail_resolves_ordinals_in_each_source_scope_and_replays_stable_ids_and_time() {
    let scratch = Scratch::new();
    let base_path = scratch.file("base.rel");
    let (base, ids) = base_owner(&base_path);
    drop(base);

    let left_path = scratch.file("left.rel");
    let right_path = scratch.file("right.rel");
    fs::copy(&base_path, &left_path).unwrap();
    fs::copy(&base_path, &right_path).unwrap();
    let mut left = Cva::open(&left_path).unwrap();
    initialize_mapping_order(&mut left, ids, false); // ordinal 0=A, 1=B
    let mut right = Cva::open(&right_path).unwrap();
    initialize_mapping_order(&mut right, ids, true); // ordinal 0=B, 1=A

    let (left_start, left_global) = append_compact_marker(
        &mut left,
        &encode_mutation(GraphOrdinalMutation {
            source: 0,
            target: 1,
            kind: SemanticGraphRelationKind::Memory(GraphRelationKind::References),
            active: true,
            origin: GraphRelationOrigin::User,
        }),
        44_001,
    );
    let (right_start, right_global) = append_compact_marker(
        &mut right,
        &encode_mutation(GraphOrdinalMutation {
            source: 0,
            target: 1,
            kind: SemanticGraphRelationKind::Memory(GraphRelationKind::References),
            active: true,
            origin: GraphRelationOrigin::User,
        }),
        55_002,
    );

    let left_tail = read_graph_tail(&mut left, left_start).unwrap();
    let right_tail = read_graph_tail(&mut right, right_start).unwrap();
    let left_output_path = scratch.file("left-output.rel");
    let right_output_path = scratch.file("right-output.rel");
    fs::copy(&base_path, &left_output_path).unwrap();
    fs::copy(&base_path, &right_output_path).unwrap();
    let mut left_output = Cva::open(&left_output_path).unwrap();
    let mut right_output = Cva::open(&right_output_path).unwrap();

    let left_result = replay_graph_tail(&mut left_output, &left_tail).unwrap();
    let right_result = replay_graph_tail(&mut right_output, &right_tail).unwrap();
    assert_eq!((left_result.transactions, left_result.mutations), (1, 1));
    assert_eq!((right_result.transactions, right_result.mutations), (1, 1));
    let left_relation = left_output
        .semantic_graph_relations()
        .into_iter()
        .find(|relation| {
            relation.kind == SemanticGraphRelationKind::Memory(GraphRelationKind::References)
        })
        .unwrap();
    let right_relation = right_output
        .semantic_graph_relations()
        .into_iter()
        .find(|relation| {
            relation.kind == SemanticGraphRelationKind::Memory(GraphRelationKind::References)
        })
        .unwrap();
    assert_eq!(left_relation.source, SemanticNodeRef::memory(ids[0]));
    assert_eq!(left_relation.target, SemanticNodeRef::memory(ids[1]));
    assert_eq!(right_relation.source, SemanticNodeRef::memory(ids[1]));
    assert_eq!(right_relation.target, SemanticNodeRef::memory(ids[0]));
    assert_eq!(left_relation.origin, GraphRelationOrigin::User);
    assert_eq!(right_relation.origin, GraphRelationOrigin::User);
    assert_eq!(left.transaction_time_ns(left_global), Some(44_001));
    assert_eq!(
        left_output.transaction_time_ns(left_relation.global_version),
        Some(44_001)
    );
    assert_eq!(right.transaction_time_ns(right_global), Some(55_002));
    assert_eq!(
        right_output.transaction_time_ns(right_relation.global_version),
        Some(55_002)
    );
    assert_eq!(left_output.graph_version(), 1);
    assert_eq!(right_output.graph_version(), 1);
}

#[test]
fn mixed_origin_compact_batch_keeps_the_existing_reconciliation_rejection() {
    let scratch = Scratch::new();
    let path = scratch.file("mixed-origin.rel");
    let (mut owner, ids) = base_owner(&path);
    initialize_mapping_order(&mut owner, ids, false);
    let (start, _) = append_compact_marker(
        &mut owner,
        &encode_batch(&[
            GraphOrdinalMutation {
                source: 0,
                target: 1,
                kind: SemanticGraphRelationKind::Memory(GraphRelationKind::References),
                active: true,
                origin: GraphRelationOrigin::Dream,
            },
            GraphOrdinalMutation {
                source: 1,
                target: 0,
                kind: SemanticGraphRelationKind::Memory(GraphRelationKind::Causal),
                active: true,
                origin: GraphRelationOrigin::User,
            },
        ])
        .unwrap(),
        66_003,
    );
    assert!(matches!(
        read_graph_tail(&mut owner, start),
        Err(CvaReconcileError::InvalidGraphVersionRecord)
    ));
}
