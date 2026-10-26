use crate::project_history_tests::{lore_revision, rel_path};
use crate::{Cva, CvaReconcileError};
use std::fs;

fn append_user_node(rel: &mut Cva, id: &str, timestamp: i64) {
    rel.append_node(
        id.into(),
        format!("{id}-conversation"),
        None,
        "user".into(),
        timestamp,
        id,
    )
    .unwrap();
}

fn correlated_base() -> (std::path::PathBuf, std::path::PathBuf) {
    let left = rel_path();
    let right = rel_path();
    let mut base = Cva::create_project(&left).unwrap();
    base.correlate_project_revision(lore_revision("repo-1", "revision-1"))
        .unwrap();
    base.sync().unwrap();
    drop(base);
    fs::copy(&left, &right).unwrap();
    (left, right)
}

#[test]
fn divergent_reconcile_recorrelates_latest_prefix_extended_project_revision() {
    let (left, right) = correlated_base();
    let output = rel_path();

    let mut left_rel = Cva::open_project(&left).unwrap();
    append_user_node(&mut left_rel, "left", 1);
    left_rel.sync().unwrap();
    drop(left_rel);

    let mut right_rel = Cva::open_project(&right).unwrap();
    append_user_node(&mut right_rel, "right", 2);
    right_rel
        .correlate_project_revision(lore_revision("repo-1", "revision-2"))
        .unwrap();
    right_rel.sync().unwrap();
    drop(right_rel);

    Cva::reconcile(&left, &right, &output).unwrap();
    let merged = Cva::open_project(&output).unwrap();
    let history = merged.project_revision_correlations();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].sequence, 1);
    assert_eq!(history[0].project_revision.revision, "revision-2");
    assert_eq!(history[0].rel_cut, merged.current_rel_semantic_cut());
}

#[test]
fn divergent_reconcile_keeps_repository_only_right_advance() {
    let (left, right) = correlated_base();
    let output = rel_path();

    let mut left_rel = Cva::open_project(&left).unwrap();
    append_user_node(&mut left_rel, "left", 1);
    left_rel.sync().unwrap();
    drop(left_rel);

    let mut right_rel = Cva::open_project(&right).unwrap();
    right_rel
        .correlate_project_revision(lore_revision("repo-1", "revision-2"))
        .unwrap();
    right_rel.sync().unwrap();
    drop(right_rel);

    let result = Cva::reconcile(&left, &right, &output).unwrap();
    assert!(result.canonical_change_required);
    let merged = Cva::open_project(&output).unwrap();
    let history = merged.project_revision_correlations();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].project_revision.revision, "revision-2");
    assert_eq!(history[0].rel_cut, merged.current_rel_semantic_cut());
}

#[test]
fn divergent_reconcile_accepts_matching_repository_history_with_different_rel_cuts() {
    let (left, right) = correlated_base();
    let output = rel_path();

    let mut left_rel = Cva::open_project(&left).unwrap();
    append_user_node(&mut left_rel, "left", 1);
    left_rel
        .correlate_project_revision(lore_revision("repo-1", "revision-2"))
        .unwrap();
    left_rel.sync().unwrap();
    drop(left_rel);

    let mut right_rel = Cva::open_project(&right).unwrap();
    append_user_node(&mut right_rel, "right-1", 2);
    append_user_node(&mut right_rel, "right-2", 3);
    right_rel
        .correlate_project_revision(lore_revision("repo-1", "revision-2"))
        .unwrap();
    right_rel.sync().unwrap();
    drop(right_rel);

    let left_cut = Cva::open_project(&left)
        .unwrap()
        .latest_project_revision_correlation()
        .unwrap()
        .rel_cut;
    let right_cut = Cva::open_project(&right)
        .unwrap()
        .latest_project_revision_correlation()
        .unwrap()
        .rel_cut;
    assert_ne!(left_cut, right_cut);

    Cva::reconcile(&left, &right, &output).unwrap();
    let merged = Cva::open_project(&output).unwrap();
    let history = merged.project_revision_correlations();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].project_revision.revision, "revision-2");
    assert_eq!(history[0].rel_cut, merged.current_rel_semantic_cut());
}

#[test]
fn divergent_reconcile_rejects_divergent_project_history() {
    let (left, right) = correlated_base();
    let output = rel_path();

    for (path, node, revision) in [
        (&left, "left", "revision-left"),
        (&right, "right", "revision-right"),
    ] {
        let mut rel = Cva::open_project(path).unwrap();
        append_user_node(&mut rel, node, 2);
        rel.correlate_project_revision(lore_revision("repo-1", revision))
            .unwrap();
        rel.sync().unwrap();
    }

    assert!(matches!(
        Cva::reconcile(&left, &right, &output),
        Err(CvaReconcileError::UnsupportedSemanticOwner(
            "divergent project-revision correlation history"
        ))
    ));
}
