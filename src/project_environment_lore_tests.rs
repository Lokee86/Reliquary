use crate::{GatewayRuntime, InteractionRuntime, ProjectRepositoryMode, ReliquaryRuntimeRoutes};
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

fn temp_dir(label: &str) -> PathBuf {
    let path =
        std::env::temp_dir().join(format!("reliquary-project-lore-{label}-{}", Uuid::new_v4()));
    fs::create_dir_all(&path).unwrap();
    path
}

fn host() -> GatewayRuntime {
    GatewayRuntime::new(
        temp_dir("gateway-state"),
        ReliquaryRuntimeRoutes::default(),
        crate::InsomniaWorkerConfig::default(),
        crate::EpisodePolicy::default(),
    )
}

fn mounted_project(
    project: &Path,
) -> (
    GatewayRuntime,
    std::sync::Arc<crate::InstanceRuntime>,
    String,
    String,
) {
    let rel = temp_dir("rel").join("project.rel");
    let host = host();
    let cva = host
        .create_project_environment_rel(&rel, project, ProjectRepositoryMode::ManagedLore)
        .unwrap();
    let owner = cva.owner_id().unwrap();
    let revision = cva
        .latest_project_revision_correlation()
        .unwrap()
        .project_revision
        .revision;
    host.mount_rel(InteractionRuntime::new(cva)).unwrap();
    let instance = project_instance(&host, &owner);
    (host, instance, owner, revision)
}

#[test]
fn lore_upload_reuses_exact_tracked_file_without_new_revision() {
    let project = temp_dir("dedupe");
    fs::create_dir_all(project.join("docs")).unwrap();
    fs::write(project.join("docs/plan.pdf"), b"tracked plan bytes").unwrap();
    let (host, instance, owner, initial) = mounted_project(&project);
    let external = temp_dir("dedupe-source").join("plan.pdf");
    fs::write(&external, b"tracked plan bytes").unwrap();

    let file = host
        .ingest_project_attachment_for(&instance, &owner, &project, &external)
        .unwrap();
    let reference = host
        .project_file_ref_for(&instance, &owner, file.id)
        .unwrap()
        .unwrap();
    let correlation = host
        .latest_project_revision_correlation_for(&instance, &owner)
        .unwrap()
        .unwrap();

    assert_eq!(reference.path, "docs/plan.pdf");
    assert_eq!(reference.revision.revision, initial);
    assert_eq!(correlation.project_revision.revision, initial);
    assert!(!project.join("warlock/uploads").exists());
}

#[test]
fn lore_upload_requires_existing_repository_without_replacing_it() {
    let source_project = temp_dir("missing-repo-source-project");
    let (host, instance, owner, _) = mounted_project(&source_project);
    let correlation = host
        .latest_project_revision_correlation_for(&instance, &owner)
        .unwrap()
        .unwrap();
    let project = temp_dir("missing-repo");
    let external = temp_dir("missing-repo-source").join("evidence.txt");
    fs::write(&external, b"evidence").unwrap();

    let error =
        crate::project_environment::prepare_upload(&correlation, &project, &external).unwrap_err();

    assert!(error.contains("managed Lore repository is missing"));
    assert!(!project.join(".lore").exists());
}

#[test]
fn lore_new_upload_creates_revision_and_exact_historical_file() {
    let project = temp_dir("new");
    let (host, instance, owner, initial) = mounted_project(&project);
    let bytes = b"new context bytes";
    let external = temp_dir("new-source").join("plan.pdf");
    fs::write(&external, bytes).unwrap();

    let file = host
        .ingest_project_attachment_for(&instance, &owner, &project, &external)
        .unwrap();
    let reference = host
        .project_file_ref_for(&instance, &owner, file.id)
        .unwrap()
        .unwrap();

    assert_eq!(reference.path, "warlock/uploads/plan.pdf");
    assert_ne!(reference.revision.revision, initial);
    assert_eq!(file.byte_length, bytes.len() as u64);
    assert_eq!(fs::read(project.join(&reference.path)).unwrap(), bytes);
    assert!(project.join("warlock/generated").is_dir());
    assert_eq!(
        host.resolved_file_bytes_for(&instance, &owner, Some(&project), file.id)
            .unwrap(),
        bytes
    );
}

#[test]
fn lore_nested_project_upload_stays_inside_project_subtree() {
    let root = temp_dir("nested-root");
    let bootstrap_rel = temp_dir("nested-bootstrap-rel").join("root.rel");
    host()
        .create_project_environment_rel(&bootstrap_rel, &root, ProjectRepositoryMode::ManagedLore)
        .unwrap();
    let project = root.join("client/project");
    fs::create_dir_all(&project).unwrap();
    let (host, instance, owner, _) = mounted_project(&project);
    let external = temp_dir("nested-source").join("plan.pdf");
    fs::write(&external, b"nested lore bytes").unwrap();

    let file = host
        .ingest_project_attachment_for(&instance, &owner, &project, &external)
        .unwrap();
    let reference = host
        .project_file_ref_for(&instance, &owner, file.id)
        .unwrap()
        .unwrap();

    assert_eq!(reference.path, "client/project/warlock/uploads/plan.pdf");
    assert_eq!(
        fs::read(project.join("warlock/uploads/plan.pdf")).unwrap(),
        b"nested lore bytes"
    );
    assert!(!root.join("warlock/uploads").exists());
}

#[test]
fn lore_historical_read_survives_edit_delete_and_fails_closed_without_repository() {
    let project = temp_dir("history");
    let (host, instance, owner, _) = mounted_project(&project);
    let original = b"historical bytes";
    let external = temp_dir("history-source").join("evidence.txt");
    fs::write(&external, original).unwrap();
    let file = host
        .ingest_project_attachment_for(&instance, &owner, &project, &external)
        .unwrap();
    let reference = host
        .project_file_ref_for(&instance, &owner, file.id)
        .unwrap()
        .unwrap();
    let working = project.join(&reference.path);

    fs::write(&working, b"later edit").unwrap();
    assert_eq!(
        host.resolved_file_bytes_for(&instance, &owner, Some(&project), file.id)
            .unwrap(),
        original
    );
    fs::remove_file(&working).unwrap();
    assert_eq!(
        host.resolved_file_bytes_for(&instance, &owner, Some(&project), file.id)
            .unwrap(),
        original
    );

    fs::remove_dir_all(project.join(".lore")).unwrap();
    let error = host
        .resolved_file_bytes_for(&instance, &owner, Some(&project), file.id)
        .unwrap_err()
        .to_string();
    assert!(error.contains("managed Lore repository is missing"));
    assert!(!project.join(".lore").exists());
}

#[test]
fn lore_upload_filename_collision_allocates_deterministic_sibling() {
    let project = temp_dir("collision");
    fs::create_dir_all(project.join("warlock/uploads")).unwrap();
    fs::create_dir_all(project.join("warlock/generated")).unwrap();
    fs::write(project.join("warlock/uploads/plan.pdf"), b"different bytes").unwrap();
    fs::write(project.join("warlock/generated/plan.pdf"), b"new bytes").unwrap();
    let (host, instance, owner, _) = mounted_project(&project);
    let external = temp_dir("collision-source").join("plan.pdf");
    fs::write(&external, b"new bytes").unwrap();

    let file = host
        .ingest_project_attachment_for(&instance, &owner, &project, &external)
        .unwrap();
    let reference = host
        .project_file_ref_for(&instance, &owner, file.id)
        .unwrap()
        .unwrap();

    assert_eq!(reference.path, "warlock/uploads/plan-2.pdf");
    assert_eq!(
        fs::read(project.join("warlock/uploads/plan-2.pdf")).unwrap(),
        b"new bytes"
    );
}

fn project_instance(
    gateway: &GatewayRuntime,
    owner: &str,
) -> std::sync::Arc<crate::InstanceRuntime> {
    gateway
        .attach_instance(
            "project-test".into(),
            None,
            std::collections::BTreeMap::from([(
                owner.to_owned(),
                std::collections::BTreeSet::from([
                    crate::OwnerGrant::ViewRel,
                    crate::OwnerGrant::ManageProject,
                ]),
            )]),
        )
        .unwrap()
}
