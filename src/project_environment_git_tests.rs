use crate::{GatewayRuntime, InteractionRuntime, ProjectRepositoryMode, ReliquaryRuntimeRoutes};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use uuid::Uuid;

fn temp_dir(label: &str) -> PathBuf {
    let path =
        std::env::temp_dir().join(format!("reliquary-project-git-{label}-{}", Uuid::new_v4()));
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

fn git(root: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_string()
}

fn init_repo(root: &Path) -> String {
    git(root, &["init", "-q"]);
    git(
        root,
        &["config", "user.email", "reliquary-tests@example.invalid"],
    );
    git(root, &["config", "user.name", "Reliquary Tests"]);
    fs::write(root.join("base.txt"), b"base").unwrap();
    git(root, &["add", "."]);
    git(root, &["commit", "-qm", "initial"]);
    git(root, &["rev-parse", "HEAD"])
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
        .create_project_environment_rel(&rel, project, ProjectRepositoryMode::Git)
        .unwrap();
    let owner = cva.owner_id().unwrap();
    let head = cva
        .latest_project_revision_correlation()
        .unwrap()
        .project_revision
        .revision;
    host.mount_rel(InteractionRuntime::new(cva)).unwrap();
    let instance = project_instance(&host, &owner);
    (host, instance, owner, head)
}

#[test]
fn git_upload_reuses_exact_tracked_file_without_branch_or_index_mutation() {
    let root = temp_dir("dedupe");
    init_repo(&root);
    fs::create_dir_all(root.join("docs")).unwrap();
    fs::write(root.join("docs/plan.pdf"), b"tracked plan bytes").unwrap();
    git(&root, &["add", "."]);
    git(&root, &["commit", "-qm", "tracked"]);
    let head = git(&root, &["rev-parse", "HEAD"]);
    let (host, instance, owner, _) = mounted_project(&root);
    fs::write(root.join("staged.txt"), b"user staged change").unwrap();
    git(&root, &["add", "staged.txt"]);
    let index_before = git(&root, &["write-tree"]);
    let external = temp_dir("dedupe-source").join("plan.pdf");
    fs::write(&external, b"tracked plan bytes").unwrap();

    let file = host
        .ingest_project_attachment_for(&instance, &owner, &root, &external)
        .unwrap();
    let reference = host
        .project_file_ref_for(&instance, &owner, file.id)
        .unwrap()
        .unwrap();

    assert_eq!(reference.path, "docs/plan.pdf");
    assert_eq!(reference.revision.revision, head);
    assert_eq!(git(&root, &["rev-parse", "HEAD"]), head);
    assert_eq!(git(&root, &["write-tree"]), index_before);
    assert!(!root.join("warlock/uploads").exists());
}

#[test]
fn git_upload_snapshot_preserves_branch_index_and_snapshot_ref() {
    let root = temp_dir("snapshot");
    let head = init_repo(&root);
    let (host, instance, owner, _) = mounted_project(&root);
    fs::write(root.join("staged.txt"), b"staged user change").unwrap();
    git(&root, &["add", "staged.txt"]);
    let index_before = git(&root, &["write-tree"]);
    let external = temp_dir("snapshot-source").join("evidence.txt");
    fs::write(&external, b"exact upload bytes").unwrap();

    let file = host
        .ingest_project_attachment_for(&instance, &owner, &root, &external)
        .unwrap();
    let reference = host
        .project_file_ref_for(&instance, &owner, file.id)
        .unwrap()
        .unwrap();

    assert_eq!(reference.path, "warlock/uploads/evidence.txt");
    assert_ne!(reference.revision.revision, head);
    assert_eq!(git(&root, &["rev-parse", "HEAD"]), head);
    assert_eq!(git(&root, &["write-tree"]), index_before);
    assert_eq!(
        host.resolved_file_bytes_for(&instance, &owner, Some(&root), file.id)
            .unwrap(),
        b"exact upload bytes"
    );
    let snapshot_ref = format!("refs/warlock/snapshots/{}", reference.revision.revision);
    assert_eq!(
        git(&root, &["rev-parse", &snapshot_ref]),
        reference.revision.revision
    );
}

#[test]
fn git_historical_upload_survives_edit_delete_and_gc() {
    let root = temp_dir("history");
    init_repo(&root);
    let (host, instance, owner, _) = mounted_project(&root);
    let external = temp_dir("history-source").join("evidence.txt");
    fs::write(&external, b"historical git bytes").unwrap();
    let file = host
        .ingest_project_attachment_for(&instance, &owner, &root, &external)
        .unwrap();
    let reference = host
        .project_file_ref_for(&instance, &owner, file.id)
        .unwrap()
        .unwrap();
    let working = root.join(&reference.path);

    fs::write(&working, b"later edit").unwrap();
    assert_eq!(
        host.resolved_file_bytes_for(&instance, &owner, Some(&root), file.id)
            .unwrap(),
        b"historical git bytes"
    );
    fs::remove_file(&working).unwrap();
    git(&root, &["gc", "--prune=now"]);
    assert_eq!(
        host.resolved_file_bytes_for(&instance, &owner, Some(&root), file.id)
            .unwrap(),
        b"historical git bytes"
    );
}

#[test]
fn git_upload_requires_existing_repository_identity_without_reassigning_it() {
    let root = temp_dir("missing-id");
    init_repo(&root);
    let (host, instance, owner, _) = mounted_project(&root);
    git(
        &root,
        &["config", "--local", "--unset", "warlock.repositoryId"],
    );
    let external = temp_dir("missing-id-source").join("evidence.txt");
    fs::write(&external, b"evidence").unwrap();

    let error = host
        .ingest_project_attachment_for(&instance, &owner, &root, &external)
        .unwrap_err()
        .to_string();

    assert!(error.contains("missing its Warlock repository identity"));
    let output = Command::new("git")
        .arg("-C")
        .arg(&root)
        .args(["config", "--local", "--get", "warlock.repositoryId"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(!root.join("warlock/uploads").exists());
}

#[test]
fn git_nested_project_upload_stays_inside_project_subtree() {
    let root = temp_dir("nested");
    init_repo(&root);
    let project = root.join("client/project");
    fs::create_dir_all(&project).unwrap();
    let (host, instance, owner, _) = mounted_project(&project);
    let correlation = host
        .latest_project_revision_correlation_for(&instance, &owner)
        .unwrap()
        .unwrap();
    let external = temp_dir("nested-source").join("plan.pdf");
    fs::write(&external, b"nested bytes").unwrap();

    let file = host
        .ingest_project_attachment_for(&instance, &owner, &project, &external)
        .unwrap();
    let reference = host
        .project_file_ref_for(&instance, &owner, file.id)
        .unwrap()
        .unwrap();

    assert_eq!(
        correlation.project_revision.repository.project_path,
        "client/project"
    );
    assert_eq!(reference.path, "client/project/warlock/uploads/plan.pdf");
    assert_eq!(
        fs::read(project.join("warlock/uploads/plan.pdf")).unwrap(),
        b"nested bytes"
    );
    assert_eq!(
        host.resolved_file_bytes_for(&instance, &owner, Some(&project), file.id)
            .unwrap(),
        b"nested bytes"
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
