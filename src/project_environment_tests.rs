use crate::{
    Cva, GatewayRuntime, ProjectRepositoryKind, ProjectRepositoryManagement, ProjectRepositoryMode,
    ReliquaryRuntimeRoutes,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use uuid::Uuid;

fn temp_dir(label: &str) -> PathBuf {
    let path =
        std::env::temp_dir().join(format!("reliquary-project-env-{label}-{}", Uuid::new_v4()));
    fs::create_dir_all(&path).unwrap();
    path
}

fn rel_path(label: &str) -> PathBuf {
    temp_dir(label).join("project.rel")
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

fn init_git(root: &Path) -> String {
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

#[test]
fn managed_lore_bootstrap_records_repository_correlation() {
    let project = temp_dir("managed-lore");
    fs::write(project.join("brief.txt"), b"brief").unwrap();
    let rel = rel_path("managed-lore-rel");
    let host = host();
    let cva = host
        .create_project_environment_rel(&rel, &project, ProjectRepositoryMode::ManagedLore)
        .unwrap();

    let correlation = cva.latest_project_revision_correlation().unwrap();
    assert_eq!(
        correlation.project_revision.repository.kind,
        ProjectRepositoryKind::Lore
    );
    assert_eq!(
        correlation.repository_management,
        ProjectRepositoryManagement::WarlockManaged
    );
    assert_eq!(correlation.project_revision.repository.project_path, ".");
    assert!(
        !correlation
            .project_revision
            .repository
            .repository_id
            .is_empty()
    );
    assert!(!correlation.project_revision.revision.is_empty());
    assert!(project.join(".lore").is_dir());
    assert!(
        fs::read_to_string(project.join(".loreignore"))
            .unwrap()
            .lines()
            .any(|line| line.trim() == ".warlock/")
    );
}

#[test]
fn manual_lore_checkpoint_is_explicit_and_updates_revision() {
    let project = temp_dir("manual-lore");
    fs::write(project.join("brief.txt"), b"initial").unwrap();
    let rel = rel_path("manual-lore-rel");
    let host = host();
    let cva = host
        .create_project_environment_rel(&rel, &project, ProjectRepositoryMode::ManualLore)
        .unwrap();
    let owner = cva.owner_id().unwrap();
    let before = cva.latest_project_revision_correlation().unwrap();
    host.mount_rel(crate::InteractionRuntime::new(cva)).unwrap();
    let instance = project_instance(&host, &owner);

    fs::write(project.join("later.txt"), b"later").unwrap();
    let revision = host
        .checkpoint_manual_project_for(&instance, &owner, &project)
        .unwrap();
    let after = host
        .latest_project_revision_correlation_for(&instance, &owner)
        .unwrap()
        .unwrap();

    assert_eq!(
        after.repository_management,
        ProjectRepositoryManagement::External
    );
    assert_ne!(
        after.project_revision.revision,
        before.project_revision.revision
    );
    assert_eq!(after.project_revision, revision);
}

#[test]
fn git_bootstrap_preserves_head_and_index_and_assigns_stable_identity() {
    let project = temp_dir("git");
    let head = init_git(&project);
    fs::write(project.join("staged.txt"), b"user staged change").unwrap();
    git(&project, &["add", "staged.txt"]);
    let index_before = git(&project, &["write-tree"]);
    let rel = rel_path("git-rel");
    let host = host();

    let cva = host
        .create_project_environment_rel(&rel, &project, ProjectRepositoryMode::Git)
        .unwrap();
    let correlation = cva.latest_project_revision_correlation().unwrap();

    assert_eq!(
        correlation.project_revision.repository.kind,
        ProjectRepositoryKind::Git
    );
    assert_eq!(correlation.project_revision.revision, head);
    assert_eq!(git(&project, &["rev-parse", "HEAD"]), head);
    assert_eq!(git(&project, &["write-tree"]), index_before);
    assert_eq!(
        correlation.project_revision.repository.repository_id,
        git(
            &project,
            &["config", "--local", "--get", "warlock.repositoryId"]
        )
    );
}

#[test]
fn project_adoption_preserves_owner_and_existing_rel_content() {
    let project = temp_dir("adopt-project");
    fs::write(project.join("brief.txt"), b"project").unwrap();
    let source = rel_path("adopt-source");
    let mut original = Cva::create_project(&source).unwrap();
    let owner = original.owner_id().unwrap();
    original
        .append_node(
            "turn".into(),
            "conversation".into(),
            None,
            "user".into(),
            1,
            "preserve me",
        )
        .unwrap();
    original.sync().unwrap();
    drop(original);

    let host = host();
    let adoption = host.inspect_project_adoption_source(&source).unwrap();
    assert_eq!(adoption.owner_id, owner);
    let mut adopted = host
        .adopt_project_environment_rel(&adoption, &project, ProjectRepositoryMode::ManagedLore)
        .unwrap();

    assert_eq!(adopted.owner_id().as_deref(), Some(owner.as_str()));
    assert!(adopted.latest_project_revision_correlation().is_some());
    assert!(
        adopted
            .conversation_turns("conversation", "turn")
            .unwrap()
            .iter()
            .any(|turn| turn.content == "preserve me")
    );
}

#[test]
fn lore_attachment_reference_reads_exact_historical_bytes() {
    let project = temp_dir("exact-lore");
    let rel = rel_path("exact-lore-rel");
    let host = host();
    let cva = host
        .create_project_environment_rel(&rel, &project, ProjectRepositoryMode::ManagedLore)
        .unwrap();
    let owner = cva.owner_id().unwrap();
    host.mount_rel(crate::InteractionRuntime::new(cva)).unwrap();
    let instance = project_instance(&host, &owner);

    let external = temp_dir("exact-lore-source").join("evidence.txt");
    fs::write(&external, b"original bytes").unwrap();
    let file = host
        .ingest_project_attachment_for(&instance, &owner, &project, &external)
        .unwrap();
    let reference = host
        .project_file_ref_for(&instance, &owner, file.id)
        .unwrap()
        .unwrap();
    let materialized = project.join(&reference.path);
    fs::write(&materialized, b"mutated working bytes").unwrap();

    assert_eq!(
        host.resolved_file_bytes_for(&instance, &owner, Some(&project), file.id)
            .unwrap(),
        b"original bytes"
    );
}

#[test]
fn git_attachment_snapshot_does_not_move_head_and_reads_exact_bytes() {
    let project = temp_dir("exact-git");
    let head = init_git(&project);
    let rel = rel_path("exact-git-rel");
    let host = host();
    let cva = host
        .create_project_environment_rel(&rel, &project, ProjectRepositoryMode::Git)
        .unwrap();
    let owner = cva.owner_id().unwrap();
    host.mount_rel(crate::InteractionRuntime::new(cva)).unwrap();
    let instance = project_instance(&host, &owner);

    let external = temp_dir("exact-git-source").join("evidence.txt");
    fs::write(&external, b"snapshot bytes").unwrap();
    let file = host
        .ingest_project_attachment_for(&instance, &owner, &project, &external)
        .unwrap();
    let reference = host
        .project_file_ref_for(&instance, &owner, file.id)
        .unwrap()
        .unwrap();

    assert_eq!(git(&project, &["rev-parse", "HEAD"]), head);
    fs::write(project.join(&reference.path), b"later bytes").unwrap();
    assert_eq!(
        host.resolved_file_bytes_for(&instance, &owner, Some(&project), file.id)
            .unwrap(),
        b"snapshot bytes"
    );
}

#[test]
fn repository_identity_mismatch_is_rejected() {
    let project = temp_dir("identity");
    init_git(&project);
    let rel = rel_path("identity-rel");
    let host = host();
    let cva = host
        .create_project_environment_rel(&rel, &project, ProjectRepositoryMode::Git)
        .unwrap();
    let owner = cva.owner_id().unwrap();
    host.mount_rel(crate::InteractionRuntime::new(cva)).unwrap();
    let instance = project_instance(&host, &owner);

    git(
        &project,
        &[
            "config",
            "--local",
            "warlock.repositoryId",
            "different-repository",
        ],
    );
    let error = host
        .validate_project_environment_for(&instance, &owner, &project)
        .unwrap_err()
        .to_string();
    assert!(error.contains("Project repository identity mismatch"));
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
