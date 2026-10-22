use crate::cva_reconcile_graph_test_support::{
    create_workspace, diverge_archive, has_relation, publish_memory, set, test_dir,
};
use crate::{Cva, GraphRelationChange, GraphRelationKind, GraphRelationOrigin};
use std::fs;

#[test]
fn reconcile_merges_disjoint_divergent_graph_relations() {
    let dir = test_dir();
    let left = dir.join("left.cva");
    let right = dir.join("right.cva");
    let output = dir.join("merged.cva");
    let ids = create_workspace(&left, &["a", "b", "c", "d"]);
    fs::copy(&left, &right).unwrap();

    let mut left_cva = Cva::open(&left).unwrap();
    diverge_archive(&mut left_cva, "left");
    set(
        &mut left_cva,
        ids[0],
        ids[1],
        GraphRelationKind::Factual,
        true,
    );
    left_cva.sync().unwrap();

    let mut right_cva = Cva::open(&right).unwrap();
    diverge_archive(&mut right_cva, "right");
    set(
        &mut right_cva,
        ids[2],
        ids[3],
        GraphRelationKind::Causal,
        true,
    );
    right_cva.sync().unwrap();

    drop(left_cva);
    drop(right_cva);
    let result = Cva::reconcile(&left, &right, &output).unwrap();
    assert_eq!(result.replayed_graph_transactions, 1);
    assert_eq!(result.replayed_graph_mutations, 1);
    assert_eq!(result.duplicate_graph_mutations, 0);
    let merged = Cva::open(output).unwrap();
    assert!(has_relation(
        &merged,
        ids[0],
        ids[1],
        GraphRelationKind::Factual
    ));
    assert!(has_relation(
        &merged,
        ids[2],
        ids[3],
        GraphRelationKind::Causal
    ));
}

#[test]
fn reconcile_preserves_user_relation_origin() {
    let dir = test_dir();
    let left = dir.join("left.cva");
    let right = dir.join("right.cva");
    let output = dir.join("merged.cva");
    let ids = create_workspace(&left, &["a", "b"]);
    fs::copy(&left, &right).unwrap();

    let mut left_cva = Cva::open(&left).unwrap();
    diverge_archive(&mut left_cva, "left");
    left_cva.sync().unwrap();

    let mut right_cva = Cva::open(&right).unwrap();
    diverge_archive(&mut right_cva, "right");
    right_cva
        .set_memory_relation_with_origin(
            ids[0],
            ids[1],
            GraphRelationKind::Factual,
            true,
            GraphRelationOrigin::User,
            right_cva.graph_version(),
        )
        .unwrap();
    right_cva.sync().unwrap();

    drop(left_cva);
    drop(right_cva);
    Cva::reconcile(&left, &right, &output).unwrap();
    let merged = Cva::open(output).unwrap();
    let relation = merged.graph_relations().into_iter().next().unwrap();
    assert_eq!(relation.origin, GraphRelationOrigin::User);
}

#[test]
fn reconcile_replays_graph_after_right_only_memory() {
    let dir = test_dir();
    let left = dir.join("left.cva");
    let right = dir.join("right.cva");
    let output = dir.join("merged.cva");
    let ids = create_workspace(&left, &["a"]);
    fs::copy(&left, &right).unwrap();

    let mut left_cva = Cva::open(&left).unwrap();
    diverge_archive(&mut left_cva, "left");
    left_cva.sync().unwrap();

    let mut right_cva = Cva::open(&right).unwrap();
    diverge_archive(&mut right_cva, "right");
    let new_id = publish_memory(&mut right_cva, "right-only");
    set(
        &mut right_cva,
        new_id,
        ids[0],
        GraphRelationKind::References,
        true,
    );
    right_cva.sync().unwrap();

    drop(left_cva);
    drop(right_cva);
    Cva::reconcile(&left, &right, &output).unwrap();
    let mut merged = Cva::open(output).unwrap();
    assert!(merged.memory(new_id).is_ok());
    assert!(has_relation(
        &merged,
        new_id,
        ids[0],
        GraphRelationKind::References
    ));
}

#[test]
fn reconcile_preserves_atomic_graph_batch() {
    let dir = test_dir();
    let left = dir.join("left.cva");
    let right = dir.join("right.cva");
    let output = dir.join("merged.cva");
    let ids = create_workspace(&left, &["a", "b", "c", "d"]);
    fs::copy(&left, &right).unwrap();
    let mut left_cva = Cva::open(&left).unwrap();
    diverge_archive(&mut left_cva, "left");
    left_cva.sync().unwrap();

    let mut right_cva = Cva::open(&right).unwrap();
    diverge_archive(&mut right_cva, "right");
    right_cva
        .set_memory_relations(
            &[
                GraphRelationChange {
                    source: ids[0],
                    target: ids[1],
                    kind: GraphRelationKind::Factual,
                    active: true,
                },
                GraphRelationChange {
                    source: ids[2],
                    target: ids[3],
                    kind: GraphRelationKind::Causal,
                    active: true,
                },
            ],
            0,
        )
        .unwrap();
    right_cva.sync().unwrap();

    drop(left_cva);
    drop(right_cva);
    let result = Cva::reconcile(&left, &right, &output).unwrap();
    assert_eq!(result.replayed_graph_transactions, 1);
    assert_eq!(result.replayed_graph_mutations, 2);
    let merged = Cva::open(output).unwrap();
    assert_eq!(merged.graph_version(), 1);
    assert_eq!(merged.graph_stats().active_relations, 2);
}

#[test]
fn reconcile_deduplicates_same_divergent_graph_history() {
    let dir = test_dir();
    let left = dir.join("left.cva");
    let right = dir.join("right.cva");
    let output = dir.join("merged.cva");
    let ids = create_workspace(&left, &["a", "b", "c", "d"]);
    fs::copy(&left, &right).unwrap();

    let mut left_cva = Cva::open(&left).unwrap();
    diverge_archive(&mut left_cva, "left");
    set(
        &mut left_cva,
        ids[0],
        ids[1],
        GraphRelationKind::Factual,
        true,
    );
    left_cva.sync().unwrap();

    let mut right_cva = Cva::open(&right).unwrap();
    diverge_archive(&mut right_cva, "right");
    set(
        &mut right_cva,
        ids[0],
        ids[1],
        GraphRelationKind::Factual,
        true,
    );
    set(
        &mut right_cva,
        ids[2],
        ids[3],
        GraphRelationKind::Causal,
        true,
    );
    right_cva.sync().unwrap();

    drop(left_cva);
    drop(right_cva);
    let result = Cva::reconcile(&left, &right, &output).unwrap();
    assert_eq!(result.duplicate_graph_mutations, 1);
    assert_eq!(result.replayed_graph_mutations, 1);
    let merged = Cva::open(output).unwrap();
    assert_eq!(merged.graph_stats().active_relations, 2);
    assert_eq!(merged.graph_version(), 2);
}

#[test]
fn reconcile_preserves_graph_retraction() {
    let dir = test_dir();
    let left = dir.join("left.cva");
    let right = dir.join("right.cva");
    let output = dir.join("merged.cva");
    let ids = create_workspace(&left, &["a", "b"]);
    let mut base = Cva::open(&left).unwrap();
    set(&mut base, ids[0], ids[1], GraphRelationKind::Factual, true);
    base.sync().unwrap();
    drop(base);
    fs::copy(&left, &right).unwrap();

    let mut left_cva = Cva::open(&left).unwrap();
    diverge_archive(&mut left_cva, "left");
    left_cva.sync().unwrap();
    let mut right_cva = Cva::open(&right).unwrap();
    diverge_archive(&mut right_cva, "right");
    set(
        &mut right_cva,
        ids[0],
        ids[1],
        GraphRelationKind::Factual,
        false,
    );
    right_cva.sync().unwrap();

    drop(left_cva);
    drop(right_cva);
    Cva::reconcile(&left, &right, &output).unwrap();
    let merged = Cva::open(output).unwrap();
    assert!(!has_relation(
        &merged,
        ids[0],
        ids[1],
        GraphRelationKind::Factual
    ));
    assert_eq!(merged.graph_version(), 2);
    assert_eq!(merged.graph_stats().relation_mutations, 2);
}

#[test]
fn absorbed_graph_copy_is_semantic_noop() {
    let dir = test_dir();
    let left = dir.join("left.cva");
    let right = dir.join("right.cva");
    let merged = dir.join("merged.cva");
    let second = dir.join("second.cva");
    let ids = create_workspace(&left, &["a", "b", "c", "d"]);
    fs::copy(&left, &right).unwrap();
    let mut left_cva = Cva::open(&left).unwrap();
    diverge_archive(&mut left_cva, "left");
    set(
        &mut left_cva,
        ids[0],
        ids[1],
        GraphRelationKind::Factual,
        true,
    );
    left_cva.sync().unwrap();
    let mut right_cva = Cva::open(&right).unwrap();
    diverge_archive(&mut right_cva, "right");
    set(
        &mut right_cva,
        ids[2],
        ids[3],
        GraphRelationKind::Causal,
        true,
    );
    right_cva.sync().unwrap();

    drop(left_cva);
    drop(right_cva);
    Cva::reconcile(&left, &right, &merged).unwrap();
    let before = fs::read(&merged).unwrap();
    let result = Cva::reconcile(&merged, &right, &second).unwrap();
    assert!(!result.canonical_change_required);
    assert_eq!(fs::read(&second).unwrap(), before);
}
