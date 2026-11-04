use super::*;
use reliquary_archive::{ProjectRepositoryKind, ProjectRepositoryRef};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!(
                "git-snapshot-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
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

fn initialize_repo(root: &Path) {
    git(root, &["init", "-q"]);
    git(root, &["config", "user.name", "Project Environment Tests"]);
    git(
        root,
        &[
            "config",
            "user.email",
            "project-environment@example.invalid",
        ],
    );
    git(
        root,
        &[
            "config",
            "--local",
            "warlock.repositoryId",
            "snapshot-test-owner",
        ],
    );
    fs::create_dir_all(root.join("client/project")).unwrap();
    fs::write(root.join("client/project/tracked.txt"), b"base").unwrap();
    git(root, &["add", "."]);
    git(root, &["commit", "-qm", "initial"]);
}

#[test]
fn current_revision_and_tree_snapshot_preserve_head_and_user_index() {
    let temp = TestDirectory::new();
    let root = temp.0.join("repo");
    fs::create_dir_all(&root).unwrap();
    initialize_repo(&root);
    let project = root.join("client/project");
    let expected = ProjectRepositoryRef {
        kind: ProjectRepositoryKind::Git,
        repository_id: "snapshot-test-owner".into(),
        project_path: "client/project".into(),
    };
    let before_head = git(&root, &["rev-parse", "HEAD"]);
    let before_index = git(&root, &["write-tree"]);
    let current = current_git_revision(&project, &expected).unwrap();
    assert_eq!(current.revision, before_head);

    let contents = [0, 1, 2, 255];
    let snapshot = snapshot_git_project_file(
        &project,
        &expected,
        "client/project/snapshots/evidence.bin",
        &contents,
    )
    .unwrap();
    assert_eq!(snapshot.repository, current.repository);
    assert_eq!(git(&root, &["rev-parse", "HEAD"]), before_head);
    assert_eq!(git(&root, &["write-tree"]), before_index);

    let cat = Command::new("git")
        .arg("-C")
        .arg(&root)
        .args([
            "cat-file",
            "blob",
            &format!(
                "{}:client/project/snapshots/evidence.bin",
                snapshot.revision
            ),
        ])
        .output()
        .unwrap();
    assert!(cat.status.success());
    assert_eq!(cat.stdout, contents);
    assert_eq!(
        git(
            &root,
            &["rev-parse", &format!("{}^{{tree}}", snapshot.revision)]
        ),
        snapshot.revision
    );
}

#[test]
fn current_revision_and_snapshot_reject_wrong_repository_or_scope() {
    let temp = TestDirectory::new();
    let root = temp.0.join("repo");
    fs::create_dir_all(&root).unwrap();
    initialize_repo(&root);
    let project = root.join("client/project");
    let mut expected = ProjectRepositoryRef {
        kind: ProjectRepositoryKind::Git,
        repository_id: "wrong-owner".into(),
        project_path: "client/project".into(),
    };
    assert!(
        current_git_revision(&project, &expected)
            .unwrap_err()
            .contains("identity does not match")
    );
    expected.repository_id = "snapshot-test-owner".into();
    assert!(
        snapshot_git_project_file(&project, &expected, "outside.txt", b"x")
            .unwrap_err()
            .contains("outside the associated project subtree")
    );
}
