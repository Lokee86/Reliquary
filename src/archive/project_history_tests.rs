use crate::{Cva, ProjectRepositoryKind, ProjectRepositoryRef, ProjectRevisionRef};
use std::path::PathBuf;
use uuid::Uuid;

pub(crate) fn rel_path() -> PathBuf {
    std::env::temp_dir().join(format!("project-history-{}.prj.rel", Uuid::new_v4()))
}

pub(crate) fn lore_revision(id: &str, revision: &str) -> ProjectRevisionRef {
    ProjectRevisionRef {
        repository: ProjectRepositoryRef {
            kind: ProjectRepositoryKind::Lore,
            repository_id: id.into(),
            project_path: ".".into(),
        },
        revision: revision.into(),
    }
}

#[test]
fn project_revision_correlation_survives_reopen() {
    let path = rel_path();
    let mut cva = Cva::create_project(&path).unwrap();
    let (first, changed) = cva
        .correlate_project_revision(lore_revision("repo-1", "revision-1"))
        .unwrap();
    assert!(changed);
    assert_eq!(first.sequence, 1);
    assert_eq!(cva.project_revision_correlations(), vec![first.clone()]);
    cva.sync().unwrap();
    drop(cva);

    let reopened = Cva::open_project(&path).unwrap();
    assert_eq!(reopened.latest_project_revision_correlation(), Some(first));
}

#[test]
fn identical_project_revision_correlation_is_idempotent() {
    let path = rel_path();
    let mut cva = Cva::create_project(&path).unwrap();
    let revision = lore_revision("repo-1", "revision-1");
    let (first, changed) = cva.correlate_project_revision(revision.clone()).unwrap();
    assert!(changed);
    let (second, changed) = cva.correlate_project_revision(revision).unwrap();
    assert!(!changed);
    assert_eq!(first, second);
    assert_eq!(cva.project_revision_correlations().len(), 1);
}

#[test]
fn project_revision_correlation_requires_stable_repository_identity() {
    let path = rel_path();
    let mut cva = Cva::create_project(&path).unwrap();
    cva.correlate_project_revision(lore_revision("repo-1", "revision-1"))
        .unwrap();

    let mut moved = lore_revision("repo-1", "revision-2");
    moved.repository.project_path = "nested/project".into();
    cva.correlate_project_revision(moved).unwrap();

    assert!(
        cva.correlate_project_revision(lore_revision("repo-2", "revision-3"))
            .is_err()
    );
    let mut changed_kind = lore_revision("repo-1", "revision-3");
    changed_kind.repository.kind = ProjectRepositoryKind::Git;
    assert!(cva.correlate_project_revision(changed_kind).is_err());
    assert_eq!(cva.project_revision_correlations().len(), 2);
}

#[test]
fn project_revision_correlation_accepts_rel_labels_and_rejects_unsafe_path() {
    let org_path = rel_path();
    let mut org = Cva::create_organization(&org_path).unwrap();
    assert!(
        org.correlate_project_revision(lore_revision("repo-1", "revision-1"))
            .is_ok()
    );

    let project_path = rel_path();
    let mut project = Cva::create_project(&project_path).unwrap();
    for unsafe_path in ["../other", "C:/other", "foo//bar", "foo/./bar", "/other"] {
        let mut revision = lore_revision("repo-1", "revision-1");
        revision.repository.project_path = unsafe_path.into();
        assert!(project.correlate_project_revision(revision).is_err());
    }
}

#[test]
fn reopen_rejects_non_contiguous_project_correlation_sequence() {
    let path = rel_path();
    let mut cva = Cva::create_project(&path).unwrap();
    let record = crate::ProjectRevisionCorrelation {
        sequence: 2,
        rel_cut: cva.current_rel_semantic_cut(),
        project_revision: lore_revision("repo-1", "revision-1"),
        repository_management: crate::ProjectRepositoryManagement::WarlockManaged,
    };
    let payload = crate::project_history_codec::encode(&record).unwrap();
    cva.container.append(&payload).unwrap();
    cva.sync().unwrap();
    drop(cva);

    assert!(matches!(
        Cva::open_project(&path),
        Err(crate::CvaError::ProjectHistory(_))
    ));
}

#[test]
fn explicit_external_repository_management_survives_reopen() {
    let path = rel_path();
    let mut cva = Cva::create_project(&path).unwrap();
    let (record, changed) = cva
        .correlate_project_revision_with_management(
            lore_revision("repo-1", "revision-1"),
            crate::ProjectRepositoryManagement::External,
        )
        .unwrap();
    assert!(changed);
    assert_eq!(
        record.repository_management,
        crate::ProjectRepositoryManagement::External
    );
    drop(cva);

    let reopened = Cva::open_project(&path).unwrap();
    assert_eq!(
        reopened
            .latest_project_revision_correlation()
            .unwrap()
            .repository_management,
        crate::ProjectRepositoryManagement::External
    );
}
