use super::*;
use crate::{ProjectRepositoryRef, ProjectRevisionRef};
use reliquary_container::{ContainerIdentity, ReliquaryScope};
use std::{
    fs,
    path::{Path, PathBuf},
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
            .join(format!("archive-{}-{suffix}", std::process::id()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn file(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn turn(id: &str, parent_id: Option<&str>, timestamp_ns: i64, content: &str) -> TurnInput {
    TurnInput {
        id: id.into(),
        conversation_id: "conversation-1".into(),
        parent_id: parent_id.map(str::to_owned),
        role: if id.starts_with("user") {
            "user"
        } else {
            "assistant"
        }
        .into(),
        principal_id: Some("principal-7".into()),
        timestamp_ns,
        content: content.to_owned(),
        attachments: Vec::new(),
        project_attachments: Vec::new(),
    }
}

#[test]
fn file_identity_matches_f03_reference_vector() {
    let id = super::file_id("report.txt", Some("text/plain"), ContentId([0x10; 32]), 123);
    let actual: String = id.iter().map(|byte| format!("{byte:02x}")).collect();
    assert_eq!(
        actual,
        "f88c8992a4741a09f22417bc8fe4ca378ba21ab9d66ae19caeab0e552f6c7a88"
    );
}

#[test]
fn accepted_turns_branches_episodes_and_owner_identity_reopen_exactly() {
    let temp = TestDirectory::new();
    let path = temp.file("archive.rel");
    let mut container = Container::create(
        &path,
        ContainerIdentity::reliquary(ReliquaryScope::Project),
        [0xA5; 16],
    )
    .unwrap();
    let owner_id = container.owner_id();
    let mut archive = Archive::empty();

    assert!(
        archive
            .accept_turn(&mut container, turn("user-root", None, 10, "prompt\0bytes"))
            .unwrap()
            .inserted
    );
    assert!(
        archive
            .accept_turn(
                &mut container,
                turn("assistant-left", Some("user-root"), 20, "left reply"),
            )
            .unwrap()
            .inserted
    );
    assert!(
        archive
            .accept_turn(
                &mut container,
                turn("assistant-right", Some("user-root"), 30, "right reply"),
            )
            .unwrap()
            .inserted
    );
    let mut non_turn = turn("tool-event", Some("user-root"), 35, "tool evidence");
    non_turn.role = "tool".into();
    let non_turn = archive.accept_turn(&mut container, non_turn).unwrap();
    assert_eq!(non_turn.activity_position, None);
    assert_eq!(non_turn.rel_turn_count, 3);
    assert!(
        archive
            .accept_turn(
                &mut container,
                turn("user-followup", Some("assistant-left"), 40, "follow up"),
            )
            .unwrap()
            .inserted
    );

    assert_eq!(archive.rel_turn_count(), 4);
    assert_eq!(
        archive
            .accepted_turn_sources()
            .iter()
            .map(|source| (source.node_id.as_str(), source.activity_position))
            .collect::<Vec<_>>(),
        [
            ("user-root", Some(1)),
            ("assistant-left", Some(2)),
            ("assistant-right", Some(3)),
            ("tool-event", None),
            ("user-followup", Some(4)),
        ]
    );
    assert_eq!(
        archive.activity_position_for_turn("conversation-1", "assistant-right"),
        Some(3)
    );
    let retry = archive
        .accept_turn(&mut container, turn("user-root", None, 10, "prompt\0bytes"))
        .unwrap();
    assert!(!retry.inserted);
    assert_eq!(retry.activity_position, Some(1));
    assert_eq!(retry.rel_turn_count, 4);

    assert!(
        archive
            .put_branch(
                &mut container,
                Branch {
                    id: "main".into(),
                    conversation_id: "conversation-1".into(),
                    leaf_node_id: "assistant-left".into(),
                    canonical: true,
                },
            )
            .unwrap()
    );
    assert!(
        archive
            .put_branch(
                &mut container,
                Branch {
                    id: "alternate".into(),
                    conversation_id: "conversation-1".into(),
                    leaf_node_id: "assistant-right".into(),
                    canonical: false,
                },
            )
            .unwrap()
    );
    assert!(
        archive
            .put_branch(
                &mut container,
                Branch {
                    id: "main".into(),
                    conversation_id: "conversation-1".into(),
                    leaf_node_id: "user-followup".into(),
                    canonical: true,
                },
            )
            .unwrap()
    );
    let episode_id = archive
        .put_episode(
            &mut container,
            "conversation-1",
            "assistant-left",
            "user-followup",
            EpisodeOrigin::Live,
            EpisodeBoundary::Explicit,
            50,
        )
        .unwrap();
    container.sync().unwrap();
    drop(archive);
    drop(container);

    let reopened_container = Container::open_read_only_as(&path, FileKind::Reliquary).unwrap();
    assert_eq!(reopened_container.owner_id(), owner_id);
    let reopened = Archive::open(&reopened_container).unwrap();
    assert_eq!(reopened.owner_id(), Some(owner_id));
    assert_eq!(reopened.node_count(), 5);
    assert_eq!(reopened.rel_turn_count(), 4);
    assert_eq!(
        reopened.activity_position_for_turn("conversation-1", "user-followup"),
        Some(4)
    );
    assert_eq!(
        reopened
            .accepted_turn_sources()
            .iter()
            .map(|source| (source.node_id.as_str(), source.activity_position))
            .collect::<Vec<_>>(),
        [
            ("user-root", Some(1)),
            ("assistant-left", Some(2)),
            ("assistant-right", Some(3)),
            ("tool-event", None),
            ("user-followup", Some(4)),
        ]
    );
    assert_eq!(reopened.branches().len(), 2);
    let fragments = reopened
        .branch_fragment_windows(
            "conversation-1",
            "main",
            crate::FragmentConfig {
                turns: 2,
                overlap: 1,
            },
            true,
        )
        .unwrap();
    assert_eq!(
        fragments
            .iter()
            .map(|f| (f.start_node_id.as_str(), f.end_node_id.as_str()))
            .collect::<Vec<_>>(),
        [
            ("user-root", "assistant-left"),
            ("assistant-left", "user-followup")
        ]
    );
    assert_eq!(reopened.episodes().len(), 1);
    assert_eq!(reopened.episode(episode_id).unwrap().finalized_at_ns, 50);
    let episode_turns = reopened.episode_turns(episode_id).unwrap().unwrap();
    assert_eq!(
        episode_turns
            .iter()
            .map(|turn| turn.node_id.as_str())
            .collect::<Vec<_>>(),
        ["assistant-left", "user-followup"]
    );
    assert_eq!(episode_turns[0].content, "left reply");

    let main = reopened.branch_turns("conversation-1", "main").unwrap();
    assert_eq!(
        main.iter()
            .map(|turn| turn.node_id.as_str())
            .collect::<Vec<_>>(),
        ["user-root", "assistant-left", "user-followup"]
    );
    assert_eq!(main[0].content, "prompt\0bytes");
    assert_eq!(main[1].role, "assistant");
    assert_eq!(main[1].principal_id.as_deref(), Some("principal-7"));
    assert_eq!(main[2].timestamp_ns, 40);
    let alternate = reopened
        .branch_turns("conversation-1", "alternate")
        .unwrap();
    assert_eq!(
        alternate
            .iter()
            .map(|turn| turn.node_id.as_str())
            .collect::<Vec<_>>(),
        ["user-root", "assistant-right"]
    );
    assert_eq!(alternate[1].content, "right reply");
}

#[test]
fn accepted_source_attachments_and_source_timestamp_survive_reopen() {
    let temp = TestDirectory::new();
    let path = temp.file("source-attachments.rel");
    let mut container = Container::create(
        &path,
        ContainerIdentity::reliquary(ReliquaryScope::Project),
        [0xA6; 16],
    )
    .unwrap();
    let mut archive = Archive::empty();
    let mut input = turn("user-with-file", None, -123, "message");
    input.attachments.push(AttachmentInput {
        filename: "evidence.bin".into(),
        mime_type: Some("application/octet-stream".into()),
        bytes: vec![0, 1, 2, 255],
    });
    let accepted = archive.accept_turn(&mut container, input.clone()).unwrap();
    assert!(accepted.inserted);
    let files = archive.files_for_source("conversation-1", "user-with-file");
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].filename, "evidence.bin");
    assert_eq!(
        files[0].mime_type.as_deref(),
        Some("application/octet-stream")
    );
    assert_eq!(files[0].bytes, vec![0, 1, 2, 255]);
    assert_eq!(
        archive.accept_turn(&mut container, input).unwrap().inserted,
        false
    );

    let reopened = Archive::open(&container).unwrap();
    assert_eq!(
        reopened.files_for_source("conversation-1", "user-with-file"),
        files
    );
    assert_eq!(
        reopened
            .node("conversation-1", "user-with-file")
            .unwrap()
            .timestamp_ns,
        -123
    );
}

#[test]
fn duplicate_source_attachment_rejected_before_append() {
    let temp = TestDirectory::new();
    let path = temp.file("duplicate-source-attachments.rel");
    let mut container = Container::create(
        &path,
        ContainerIdentity::reliquary(ReliquaryScope::Project),
        [0xA7; 16],
    )
    .unwrap();
    let mut archive = Archive::empty();
    let mut input = turn("user-duplicate-file", None, 1, "message");
    let attachment = AttachmentInput {
        filename: "same.bin".into(),
        mime_type: None,
        bytes: vec![7, 8],
    };
    input.attachments = vec![attachment.clone(), attachment];
    let before = container.read_frames().unwrap().len();
    assert!(matches!(
        archive.accept_turn(&mut container, input),
        Err(ArchiveError::DuplicateAttachment)
    ));
    assert_eq!(container.read_frames().unwrap().len(), before);
}

#[test]
fn attachment_content_is_published_once_and_shared_by_source_links() {
    let temp = TestDirectory::new();
    let path = temp.file("deduplicated-source-attachments.rel");
    let mut container = Container::create(
        &path,
        ContainerIdentity::reliquary(ReliquaryScope::Project),
        [0xA8; 16],
    )
    .unwrap();
    let mut archive = Archive::empty();
    let bytes = vec![4, 5, 6];
    let mut first = turn("user-file-a", None, 1, "first");
    first.attachments.push(AttachmentInput {
        filename: "shared.bin".into(),
        mime_type: None,
        bytes: bytes.clone(),
    });
    archive.accept_turn(&mut container, first).unwrap();
    let mut second = turn("user-file-b", Some("user-file-a"), 2, "second");
    second.attachments.push(AttachmentInput {
        filename: "shared.bin".into(),
        mime_type: None,
        bytes: bytes.clone(),
    });
    archive.accept_turn(&mut container, second).unwrap();
    let blob_count = container
        .read_frames()
        .unwrap()
        .iter()
        .filter(|frame| frame.payload.starts_with(b"RELFILE1"))
        .count();
    assert_eq!(blob_count, 1);
    let reopened = Archive::open(&container).unwrap();
    assert_eq!(
        reopened.files_for_source("conversation-1", "user-file-a")[0].bytes,
        bytes
    );
    assert_eq!(
        reopened.files_for_source("conversation-1", "user-file-b")[0].bytes,
        bytes
    );
}

#[test]
fn project_attachment_preserves_immutable_provenance_without_copying_vcs_bytes() {
    let temp = TestDirectory::new();
    let path = temp.file("project-source-attachment.rel");
    let mut container = Container::create(
        &path,
        ContainerIdentity::reliquary(ReliquaryScope::Project),
        [0xA9; 16],
    )
    .unwrap();
    let mut archive = Archive::empty();
    let content_id = ContentId([0x42; 32]);
    let reference = ProjectFileRef {
        revision: ProjectRevisionRef {
            repository: ProjectRepositoryRef {
                kind: ProjectRepositoryKind::Git,
                repository_id: "repo-17".into(),
                project_path: "workspace/app".into(),
            },
            revision: "a1b2c3d4".into(),
        },
        path: "workspace/app/src/main.rs".into(),
        content_hash: Some(content_id.0),
    };
    let mut input = turn("user-project-file", None, 51, "review this");
    input.project_attachments.push(ProjectAttachmentInput {
        filename: "main.rs".into(),
        mime_type: Some("text/rust".into()),
        content_id,
        byte_length: 31,
        reference: reference.clone(),
    });
    archive.accept_turn(&mut container, input).unwrap();
    let attached = archive.files_for_source("conversation-1", "user-project-file");
    assert_eq!(attached.len(), 1);
    assert_eq!(attached[0].project_reference.as_ref(), Some(&reference));
    assert!(attached[0].bytes.is_empty());

    let reopened = Archive::open(&container).unwrap();
    let attached = reopened.files_for_source("conversation-1", "user-project-file");
    assert_eq!(attached.len(), 1);
    assert_eq!(attached[0].project_reference.as_ref(), Some(&reference));
    assert!(attached[0].bytes.is_empty());
}

#[test]
fn project_attachment_must_stay_within_project_subtree() {
    let temp = TestDirectory::new();
    let path = temp.file("invalid-project-source-attachment.rel");
    let mut container = Container::create(
        &path,
        ContainerIdentity::reliquary(ReliquaryScope::Project),
        [0xAA; 16],
    )
    .unwrap();
    let mut archive = Archive::empty();
    let mut input = turn("user-outside-project", None, 52, "review this");
    input.project_attachments.push(ProjectAttachmentInput {
        filename: "outside.rs".into(),
        mime_type: None,
        content_id: ContentId([0x43; 32]),
        byte_length: 9,
        reference: ProjectFileRef {
            revision: ProjectRevisionRef {
                repository: ProjectRepositoryRef {
                    kind: ProjectRepositoryKind::Lore,
                    repository_id: "repo-18".into(),
                    project_path: "workspace/app".into(),
                },
                revision: "rev-1".into(),
            },
            path: "other/outside.rs".into(),
            content_hash: None,
        },
    });
    let before = container.read_frames().unwrap().len();
    assert!(matches!(
        archive.accept_turn(&mut container, input),
        Err(ArchiveError::InvalidField(_))
    ));
    assert_eq!(container.read_frames().unwrap().len(), before);
}

#[test]
fn retries_are_idempotent_and_conflicts_do_not_publish() {
    let temp = TestDirectory::new();
    let path = temp.file("idempotency.rel");
    let mut container = Container::create(
        &path,
        ContainerIdentity::reliquary(ReliquaryScope::Project),
        [0xB6; 16],
    )
    .unwrap();
    let mut archive = Archive::empty();
    let root = turn("root", None, 1, "accepted");
    assert!(
        archive
            .accept_turn(&mut container, root.clone())
            .unwrap()
            .inserted
    );
    let frame_count = container.read_frames().unwrap().len();
    assert!(!archive.accept_turn(&mut container, root).unwrap().inserted);
    assert_eq!(container.read_frames().unwrap().len(), frame_count);

    let conflict = archive.accept_turn(&mut container, turn("root", None, 1, "changed"));
    assert!(matches!(conflict, Err(ArchiveError::ConflictingNode)));
    let missing_parent = archive.accept_turn(
        &mut container,
        turn("orphan", Some("missing"), 2, "not accepted"),
    );
    assert!(matches!(missing_parent, Err(ArchiveError::MissingParent)));
    assert_eq!(container.read_frames().unwrap().len(), frame_count);
}

#[test]
fn archive_state_cannot_be_written_to_a_different_container_owner() {
    let temp = TestDirectory::new();
    let first_path = temp.file("first.rel");
    let second_path = temp.file("second.rel");
    let mut first = Container::create(
        &first_path,
        ContainerIdentity::reliquary(ReliquaryScope::Project),
        [0xC7; 16],
    )
    .unwrap();
    let mut archive = Archive::empty();
    archive
        .accept_turn(&mut first, turn("root", None, 1, "owner one"))
        .unwrap();
    let mut second = Container::create(
        &second_path,
        ContainerIdentity::reliquary(ReliquaryScope::Project),
        [0xD8; 16],
    )
    .unwrap();

    assert!(matches!(
        archive.accept_turn(&mut second, turn("child", Some("root"), 2, "wrong owner")),
        Err(ArchiveError::WrongOwner)
    ));
    assert!(second.read_frames().unwrap().is_empty());
}

#[test]
fn fresh_projection_replays_existing_history_before_accepting() {
    let temp = TestDirectory::new();
    let path = temp.file("existing.rel");
    let mut container = Container::create(
        &path,
        ContainerIdentity::reliquary(ReliquaryScope::Project),
        [0xE1; 16],
    )
    .unwrap();
    let mut first = Archive::empty();
    first
        .accept_turn(&mut container, turn("root", None, 1, "original"))
        .unwrap();
    let before = container.read_frames().unwrap().len();
    let mut fresh = Archive::empty();
    assert!(matches!(
        fresh.accept_turn(&mut container, turn("root", None, 1, "conflict")),
        Err(ArchiveError::ConflictingNode)
    ));
    assert_eq!(container.read_frames().unwrap().len(), before);
    let accepted = fresh
        .accept_turn(&mut container, turn("child", Some("root"), 2, "next"))
        .unwrap();
    assert_eq!(accepted.activity_position, Some(2));
    assert_eq!(Archive::open(&container).unwrap().node_count(), 2);
}

#[test]
fn archive_rejects_other_rel_scope_with_same_owner_id() {
    let temp = TestDirectory::new();
    let mut first = Container::create(
        &temp.file("project.rel"),
        ContainerIdentity::reliquary(ReliquaryScope::Project),
        [0xED; 16],
    )
    .unwrap();
    let mut archive = Archive::empty();
    archive
        .accept_turn(&mut first, turn("root", None, 1, "first"))
        .unwrap();
    let mut other = Container::create(
        &temp.file("organization.rel"),
        ContainerIdentity::reliquary(ReliquaryScope::Organization),
        [0xED; 16],
    )
    .unwrap();
    assert!(matches!(
        archive.accept_turn(&mut other, turn("other", None, 2, "wrong")),
        Err(ArchiveError::WrongOwner)
    ));
    assert!(matches!(
        archive.validate_owner(&other),
        Err(ArchiveError::WrongOwner)
    ));
    assert!(other.read_frames().unwrap().is_empty());
}
