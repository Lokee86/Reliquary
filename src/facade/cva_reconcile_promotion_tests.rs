use crate::{Cva, CvaReconcileError};
use std::fs;
use std::path::{Path, PathBuf};

fn test_dir() -> PathBuf {
    let unique = uuid::Uuid::new_v4();
    let dir = std::env::temp_dir().join(format!("continuity-cva-promotion-{unique}"));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn create_workspace(path: &Path) {
    Cva::create_project(path).unwrap().sync().unwrap();
}

fn append(path: &Path, id: &str, content: &str) {
    let mut cva = Cva::open(path).unwrap();
    cva.append_node(
        id.into(),
        format!("{id}-conversation"),
        None,
        "user".into(),
        1,
        content,
    )
    .unwrap();
    cva.sync().unwrap();
    drop(cva);
}

fn promotion_artifacts(dir: &Path) -> Vec<String> {
    fs::read_dir(dir)
        .unwrap()
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| name.ends_with("merge.tmp") || name.ends_with("merge.bak"))
        .collect()
}

#[test]
fn reconcile_and_promote_replaces_canonical_after_validation() {
    let dir = test_dir();
    let canonical = dir.join("project.cva");
    let conflicted = dir.join("project-conflicted.cva");
    create_workspace(&canonical);
    fs::copy(&canonical, &conflicted).unwrap();
    append(&canonical, "left", "left state");
    append(&conflicted, "right", "right state");

    let result = Cva::reconcile_and_promote(&canonical, &conflicted).unwrap();
    assert_eq!(
        result.comparison.owner_id,
        Cva::open(&canonical).unwrap().owner_id().unwrap()
    );
    assert!(result.canonical_change_required);
    let promoted = Cva::open(&canonical).unwrap();
    assert_eq!(promoted.stats().nodes, 2);
    let preserved_conflict = Cva::open(&conflicted).unwrap();
    assert_eq!(preserved_conflict.stats().nodes, 1);
    assert!(promotion_artifacts(&dir).is_empty());
}

#[test]
fn failed_reconciliation_never_changes_canonical() {
    let dir = test_dir();
    let canonical = dir.join("project.cva");
    let conflicted = dir.join("project-conflicted.cva");
    create_workspace(&canonical);
    fs::copy(&canonical, &conflicted).unwrap();
    append(&canonical, "same", "left state");
    append(&conflicted, "same", "right state");
    let before = fs::read(&canonical).unwrap();

    assert!(Cva::reconcile_and_promote(&canonical, &conflicted).is_err());
    assert_eq!(fs::read(&canonical).unwrap(), before);
    assert_eq!(Cva::open(&canonical).unwrap().stats().nodes, 1);
    assert!(promotion_artifacts(&dir).is_empty());
}

#[test]
fn already_absorbed_conflicted_copy_is_a_noop() {
    let dir = test_dir();
    let canonical = dir.join("project.cva");
    let conflicted = dir.join("project-conflicted.cva");
    create_workspace(&canonical);
    fs::copy(&canonical, &conflicted).unwrap();
    append(&canonical, "left", "left state");
    append(&conflicted, "right", "right state");

    Cva::reconcile_and_promote(&canonical, &conflicted).unwrap();
    let before = fs::read(&canonical).unwrap();
    let result = Cva::reconcile_and_promote(&canonical, &conflicted).unwrap();

    assert!(!result.canonical_change_required);
    assert!(!result.vector_rebuild_required);
    assert_eq!(fs::read(&canonical).unwrap(), before);
    assert_eq!(Cva::open(&canonical).unwrap().stats().nodes, 2);
    assert!(promotion_artifacts(&dir).is_empty());
}

#[test]
fn promotion_rejects_the_same_physical_file() {
    let dir = test_dir();
    let canonical = dir.join("project.cva");
    create_workspace(&canonical);
    assert!(matches!(
        Cva::reconcile_and_promote(&canonical, &canonical),
        Err(CvaReconcileError::PromotionPathsMustDiffer)
    ));
}
