use crate::entity_resolution_test_support::rel_with_three_mentions;
use crate::{
    Cva, CvaReconcileError, EntityId, EntityResolutionReason, MemoryEntityResolutionStatus,
};
use std::fs;

#[test]
fn linear_reconcile_preserves_entity_resolution_state() {
    let (base, _, keys, base_path) = rel_with_three_mentions();
    base.sync().unwrap();
    drop(base);

    let right = sibling_path("right.rel");
    let output = sibling_path("linear-output.rel");
    fs::copy(&base_path, &right).unwrap();
    let mut newer = Cva::open(&right).unwrap();
    newer
        .put_entity_resolved(
            keys[0],
            0,
            EntityId([1; 32]),
            EntityResolutionReason::ContextMatch,
            10,
        )
        .unwrap();
    newer.sync().unwrap();
    drop(newer);

    Cva::reconcile(&base_path, &right, &output).unwrap();
    let merged = Cva::open(&output).unwrap();
    assert!(matches!(
        merged.entity_resolution(keys[0]).unwrap().status,
        MemoryEntityResolutionStatus::Resolved { .. }
    ));
}

#[test]
fn divergent_resolution_state_is_not_guessed() {
    let (base, _, keys, base_path) = rel_with_three_mentions();
    base.sync().unwrap();
    drop(base);

    let left = sibling_path("left.rel");
    let right = sibling_path("right-diverged.rel");
    let output = sibling_path("conflict-output.rel");
    fs::copy(&base_path, &left).unwrap();
    fs::copy(&base_path, &right).unwrap();

    let mut left_rel = Cva::open(&left).unwrap();
    left_rel
        .put_entity_resolved(
            keys[0],
            0,
            EntityId([1; 32]),
            EntityResolutionReason::ContextMatch,
            10,
        )
        .unwrap();
    left_rel.sync().unwrap();

    let mut right_rel = Cva::open(&right).unwrap();
    right_rel
        .put_entity_rejected(keys[0], 0, EntityResolutionReason::GenericRole, 10)
        .unwrap();
    right_rel.sync().unwrap();
    drop(left_rel);
    drop(right_rel);

    assert!(matches!(
        Cva::reconcile(&left, &right, &output),
        Err(CvaReconcileError::UnsupportedSemanticOwner(
            "divergent Entity resolution state"
        ))
    ));
}

fn sibling_path(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "reliquary-entity-resolution-reconcile-{}-{name}",
        uuid::Uuid::new_v4()
    ))
}
