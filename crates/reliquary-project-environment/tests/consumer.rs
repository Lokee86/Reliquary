use reliquary_archive::{
    ProjectRepositoryKind, ProjectRepositoryManagement, ProjectRepositoryRef,
    ProjectRevisionCorrelation, ProjectRevisionRef, RelSemanticCut,
};
use reliquary_project_environment::{ProjectRepositoryMode, checkpoint_manual_lore};
use std::path::Path;

#[test]
fn modes_and_checkpoint_gate_are_available_to_consumers() {
    let mut correlation = ProjectRevisionCorrelation {
        sequence: 1,
        rel_cut: RelSemanticCut {
            global_version: 0,
            archive_version: 0,
            memory_version: 0,
        },
        project_revision: ProjectRevisionRef {
            repository: ProjectRepositoryRef {
                kind: ProjectRepositoryKind::Lore,
                repository_id: "repo".into(),
                project_path: "".into(),
            },
            revision: "rev".into(),
        },
        repository_management: ProjectRepositoryManagement::WarlockManaged,
    };
    assert_eq!(
        ProjectRepositoryMode::from_correlation(&correlation),
        ProjectRepositoryMode::ManagedLore
    );
    assert!(
        checkpoint_manual_lore(Path::new("."), &correlation)
            .unwrap_err()
            .contains("only for manually managed")
    );
    correlation.repository_management = ProjectRepositoryManagement::External;
    assert_eq!(
        ProjectRepositoryMode::from_correlation(&correlation),
        ProjectRepositoryMode::ManualLore
    );
    correlation.project_revision.repository.kind = ProjectRepositoryKind::Git;
    assert_eq!(
        ProjectRepositoryMode::from_correlation(&correlation),
        ProjectRepositoryMode::Git
    );
    assert!(checkpoint_manual_lore(Path::new("."), &correlation).is_err());
}

#[test]
fn consumer_prepares_a_real_git_repository() {
    use reliquary_project_environment::prepare_project_repository;
    use std::{fs, process::Command};
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.project-environment-test")
        .join(uuid::Uuid::new_v4().to_string());
    fs::create_dir_all(&dir).unwrap();
    let git = |args: &[&str]| {
        let status = Command::new("git")
            .arg("-C")
            .arg(&dir)
            .args(args)
            .status()
            .unwrap();
        assert!(status.success());
    };
    git(&["init", "-q"]);
    git(&[
        "-c",
        "user.name=Test",
        "-c",
        "user.email=test@example.invalid",
        "commit",
        "--allow-empty",
        "-qm",
        "initial",
    ]);
    let prepared = prepare_project_repository(&dir, ProjectRepositoryMode::Git).unwrap();
    assert_eq!(
        prepared.revision().repository.kind,
        ProjectRepositoryKind::Git
    );
    assert!(!prepared.revision().repository.repository_id.is_empty());
    assert_eq!(prepared.revision().repository.project_path, ".");
    fn make_writable(path: &Path) {
        for entry in fs::read_dir(path).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_dir() {
                make_writable(&path);
            } else {
                let mut permissions = entry.metadata().unwrap().permissions();
                if permissions.readonly() {
                    permissions.set_readonly(false);
                    fs::set_permissions(path, permissions).unwrap();
                }
            }
        }
    }
    make_writable(&dir);
    fs::remove_dir_all(&dir).unwrap();
}
