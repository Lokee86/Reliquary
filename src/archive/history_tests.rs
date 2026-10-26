use crate::{ArchiveError, Branch, Cva, CvaError};
use reliquary_container::Container;
use std::fs;
use std::path::PathBuf;

fn test_path(name: &str) -> PathBuf {
    let unique = uuid::Uuid::new_v4();
    let dir = std::env::temp_dir().join(format!("continuity-history-{unique}"));
    fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

fn append(archive: &mut Cva, conversation: &str, id: &str, parent: Option<&str>) {
    archive
        .append_node(
            id.into(),
            conversation.into(),
            parent.map(str::to_owned),
            "user".into(),
            1,
            id,
        )
        .unwrap();
}

fn branch(conversation: &str, id: &str, leaf: &str) -> Branch {
    Branch {
        id: id.into(),
        conversation_id: conversation.into(),
        leaf_node_id: leaf.into(),
        canonical: true,
    }
}

fn parent<'a>(cva: &'a Cva, conversation: &str, id: &str) -> Option<&'a str> {
    cva.archive
        .nodes
        .get(conversation, id)
        .unwrap()
        .parent_id
        .as_deref()
}

fn current_leaf<'a>(cva: &'a Cva, conversation: &str, branch_id: &str) -> &'a str {
    let branch = cva.archive.branches.get(conversation, branch_id).unwrap();
    branch.leaf_node_id.as_str()
}

#[test]
fn archive_rejects_container_without_archive_format_marker() {
    let path = test_path("unmarked.cva");
    let container = Container::create_with_identity(
        &path,
        reliquary_container::ContainerIdentity {
            file_kind: reliquary_container::FileKind::Reliquary,
            scope: Some(reliquary_container::ReliquaryScopeKind::Project),
        },
    )
    .unwrap();
    container.sync().unwrap();
    drop(container);
    assert!(matches!(
        Cva::open(path),
        Err(CvaError::Archive(ArchiveError::MissingArchiveFormat))
    ));
}

#[test]
fn unrelated_conversations_share_clocks_not_ancestry() {
    let path = test_path("concurrent.cva");
    let mut archive = Cva::create(path).unwrap();
    append(&mut archive, "a", "a1", None);
    append(&mut archive, "b", "b1", None);
    append(&mut archive, "a", "a2", Some("a1"));
    append(&mut archive, "b", "b2", Some("b1"));

    let versions = archive.record_versions();
    assert_eq!(versions.len(), 4);
    assert_eq!(
        versions
            .iter()
            .map(|v| v.archive_version)
            .collect::<Vec<_>>(),
        [1, 2, 3, 4]
    );
    assert_eq!(parent(&archive, "a", "a2"), Some("a1"));
    assert_eq!(parent(&archive, "b", "b2"), Some("b1"));
}

#[test]
fn global_and_archive_clocks_are_independent() {
    let path = test_path("clocks.cva");
    let mut archive = Cva::create(path).unwrap();
    append(&mut archive, "a", "a1", None);
    let unrelated_global = archive.container.allocate_version().unwrap();
    append(&mut archive, "a", "a2", Some("a1"));

    assert_eq!(unrelated_global, 2);
    assert_eq!(archive.record_versions()[0].global_version, 1);
    assert_eq!(archive.record_versions()[0].archive_version, 1);
    assert_eq!(archive.record_versions()[1].global_version, 3);
    assert_eq!(archive.record_versions()[1].archive_version, 2);
}

#[test]
fn archive_versions_survive_reopen() {
    let path = test_path("reopen.cva");
    let mut archive = Cva::create(&path).unwrap();
    append(&mut archive, "a", "a1", None);
    append(&mut archive, "b", "b1", None);
    archive.append_branch(branch("a", "main", "a1")).unwrap();
    let latest = archive.archive_version();
    archive.sync().unwrap();
    drop(archive);

    let reopened = Cva::open(path).unwrap();
    assert_eq!(reopened.archive_version(), latest);
    assert_eq!(reopened.record_versions().len(), latest as usize);
}

#[test]
fn branch_heads_are_append_only_revisions() {
    let path = test_path("heads.cva");
    let mut archive = Cva::create(&path).unwrap();
    append(&mut archive, "a", "a1", None);
    append(&mut archive, "a", "a2", Some("a1"));
    archive.append_branch(branch("a", "main", "a1")).unwrap();
    let first_head = archive.archive_version();
    archive.append_branch(branch("a", "main", "a2")).unwrap();

    assert_eq!(
        archive
            .branch_at("a", "main", first_head)
            .unwrap()
            .unwrap()
            .leaf_node_id,
        "a1"
    );
    assert_eq!(current_leaf(&archive, "a", "main"), "a2");
    archive.sync().unwrap();
    drop(archive);

    let mut reopened = Cva::open(path).unwrap();
    assert_eq!(current_leaf(&reopened, "a", "main"), "a2");
    assert_eq!(
        reopened
            .branch_at("a", "main", first_head)
            .unwrap()
            .unwrap()
            .leaf_node_id,
        "a1"
    );
}

#[test]
fn existing_branch_head_cannot_jump_backwards() {
    let path = test_path("rewind-head.cva");
    let mut archive = Cva::create(path).unwrap();
    append(&mut archive, "a", "a1", None);
    append(&mut archive, "a", "a2", Some("a1"));
    archive.append_branch(branch("a", "main", "a2")).unwrap();

    assert!(matches!(
        archive.append_branch(branch("a", "main", "a1")),
        Err(ArchiveError::InvalidBranchRevision)
    ));
}

#[test]
fn old_conversation_point_can_start_a_new_local_branch() {
    let path = test_path("revive.cva");
    let mut archive = Cva::create(path).unwrap();
    append(&mut archive, "a", "a1", None);
    append(&mut archive, "a", "a2", Some("a1"));
    append(&mut archive, "a", "a3", Some("a2"));
    archive
        .append_branch(branch("a", "original", "a3"))
        .unwrap();

    archive.append_branch(branch("a", "revived", "a1")).unwrap();
    append(&mut archive, "a", "a2b", Some("a1"));
    archive
        .append_branch(branch("a", "revived", "a2b"))
        .unwrap();

    let original = archive.branch_turns("a", "original").unwrap();
    let revived = archive.branch_turns("a", "revived").unwrap();
    assert_eq!(
        original
            .iter()
            .map(|t| t.node_id.as_str())
            .collect::<Vec<_>>(),
        ["a1", "a2", "a3"]
    );
    assert_eq!(
        revived
            .iter()
            .map(|t| t.node_id.as_str())
            .collect::<Vec<_>>(),
        ["a1", "a2b"]
    );
}
