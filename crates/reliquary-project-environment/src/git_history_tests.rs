use super::*;
use reliquary_archive::{
    ProjectFileRef, ProjectRepositoryKind, ProjectRepositoryRef, ProjectRevisionRef,
};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!("git-history-{}-{suffix}", std::process::id()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
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
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

fn initialize_repository(path: &Path) -> String {
    git(path, &["init", "-q"]);
    git(path, &["config", "user.name", "Project Environment Tests"]);
    git(
        path,
        &[
            "config",
            "user.email",
            "project-environment@example.invalid",
        ],
    );
    git(
        path,
        &[
            "config",
            "--local",
            "warlock.repositoryId",
            "git-test-owner",
        ],
    );
    fs::create_dir_all(path.join("client/project/docs")).unwrap();
    fs::write(path.join("client/project/docs/history.bin"), [0, 1, 2, 255]).unwrap();
    fs::write(path.join("outside.txt"), b"outside project subtree").unwrap();
    git(path, &["add", "."]);
    git(path, &["commit", "-qm", "initial project state"]);
    git(path, &["rev-parse", "HEAD"])
}

#[test]
fn reads_exact_nested_project_file_at_historical_git_revision() {
    let temp = TestDirectory::new();
    let root = temp.path().join("repo");
    fs::create_dir_all(&root).unwrap();
    let revision = initialize_repository(&root);
    let expected = [0, 1, 2, 255];
    let digest: [u8; 32] = Sha256::digest(expected).into();
    let index_before = git(&root, &["write-tree"]);

    fs::write(
        root.join("client/project/docs/history.bin"),
        b"later working copy",
    )
    .unwrap();
    let current_head = git(&root, &["rev-parse", "HEAD"]);
    let bytes = read_git_revision_file(
        &root.join("client/project"),
        "git-test-owner",
        "client/project",
        &revision,
        "client/project/docs/history.bin",
        Some(digest),
    )
    .unwrap();

    assert_eq!(bytes, expected);
    assert_eq!(git(&root, &["rev-parse", "HEAD"]), current_head);
    assert_eq!(git(&root, &["write-tree"]), index_before);

    // Warlock-owned Git snapshots store tree object IDs directly, while
    // ordinary project revisions store commit IDs.
    let tree_revision = git(&root, &["rev-parse", &format!("{revision}^{{tree}}")]);
    let snapshot_bytes = read_git_revision_file(
        &root.join("client/project"),
        "git-test-owner",
        "client/project",
        &tree_revision,
        "client/project/docs/history.bin",
        Some(digest),
    )
    .unwrap();
    assert_eq!(snapshot_bytes, expected);

    let reference = ProjectFileRef {
        revision: ProjectRevisionRef {
            repository: ProjectRepositoryRef {
                kind: ProjectRepositoryKind::Git,
                repository_id: "git-test-owner".into(),
                project_path: "client/project".into(),
            },
            revision,
        },
        path: "client/project/docs/history.bin".into(),
        content_hash: Some(digest),
    };
    assert_eq!(
        read_project_file_reference(&root.join("client/project"), &reference).unwrap(),
        expected
    );
}

#[test]
fn archive_project_reference_requires_git_authority_and_matching_identity() {
    let temp = TestDirectory::new();
    let root = temp.path().join("repo");
    fs::create_dir_all(&root).unwrap();
    let revision = initialize_repository(&root);
    let mut reference = ProjectFileRef {
        revision: ProjectRevisionRef {
            repository: ProjectRepositoryRef {
                kind: ProjectRepositoryKind::Lore,
                repository_id: "git-test-owner".into(),
                project_path: "client/project".into(),
            },
            revision: revision.clone(),
        },
        path: "client/project/docs/history.bin".into(),
        content_hash: None,
    };
    assert!(
        read_project_file_reference(&root, &reference)
            .unwrap_err()
            .contains("not backed by Git")
    );

    reference.revision.repository.kind = ProjectRepositoryKind::Git;
    reference.revision.repository.repository_id = "some-other-repository".into();
    assert!(
        read_project_file_reference(&root, &reference)
            .unwrap_err()
            .contains("identity does not match")
    );

    reference.revision.repository.repository_id = "git-test-owner".into();
    reference.path = "client/project/../outside.txt".into();
    assert!(
        read_project_file_reference(&root, &reference)
            .unwrap_err()
            .contains("project repository-relative path")
    );
}

#[test]
fn rejects_missing_or_changed_repository_identity_without_assigning_one() {
    let temp = TestDirectory::new();
    let root = temp.path().join("repo");
    fs::create_dir_all(&root).unwrap();
    let revision = initialize_repository(&root);
    git(
        &root,
        &["config", "--local", "--unset", "warlock.repositoryId"],
    );

    let error = read_git_revision_file(
        &root,
        "git-test-owner",
        ".",
        &revision,
        "client/project/docs/history.bin",
        None,
    )
    .unwrap_err();
    assert!(error.contains("missing its Warlock repository identity"));
    let config = Command::new("git")
        .arg("-C")
        .arg(&root)
        .args(["config", "--local", "--get", REPOSITORY_ID_KEY])
        .output()
        .unwrap();
    assert_eq!(config.status.code(), Some(1));
}

#[test]
fn rejects_wrong_owner_bad_digest_and_paths_outside_nested_project() {
    let temp = TestDirectory::new();
    let root = temp.path().join("repo");
    fs::create_dir_all(&root).unwrap();
    let revision = initialize_repository(&root);

    let wrong_owner = read_git_revision_file(
        &root,
        "another-owner",
        "client/project",
        &revision,
        "client/project/docs/history.bin",
        None,
    )
    .unwrap_err();
    assert!(wrong_owner.contains("repository identity does not match"));

    let outside_subtree = read_git_revision_file(
        &root,
        "git-test-owner",
        "client/project",
        &revision,
        "outside.txt",
        None,
    )
    .unwrap_err();
    assert!(outside_subtree.contains("outside the associated project subtree"));

    let traversal = read_git_revision_file(
        &root,
        "git-test-owner",
        "client/project",
        &revision,
        "client/project/../outside.txt",
        None,
    )
    .unwrap_err();
    assert!(traversal.contains("not a normalized repository-relative path"));

    let mismatch = read_git_revision_file(
        &root,
        "git-test-owner",
        ".",
        &revision,
        "client/project/docs/history.bin",
        Some([0xff; 32]),
    )
    .unwrap_err();
    assert!(mismatch.contains("content hash mismatch"));
}
