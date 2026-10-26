use crate::{ArchiveError, Branch, Cva, Node};
use reliquary_archive::archive_codec::{ArchiveRecord, decode_record, encode_node};
use reliquary_archive::archive_store::hash_content;
use std::fs;
use std::path::PathBuf;
fn test_path() -> PathBuf {
    let unique = uuid::Uuid::new_v4();
    let dir = std::env::temp_dir().join(format!("continuity-archive-{unique}"));
    fs::create_dir_all(&dir).unwrap();
    dir.join("archive.cva")
}

#[test]
fn shared_branch_prefix_and_content_are_stored_once() {
    let path = test_path();
    let mut archive = Cva::create(&path).unwrap();
    archive
        .append_node(
            "n1".into(),
            "c1".into(),
            None,
            "user".into(),
            1,
            "same body",
        )
        .unwrap();
    archive
        .append_node(
            "n2".into(),
            "c1".into(),
            Some("n1".into()),
            "assistant".into(),
            2,
            "shared prefix",
        )
        .unwrap();
    archive
        .append_node(
            "a".into(),
            "c1".into(),
            Some("n2".into()),
            "user".into(),
            3,
            "same body",
        )
        .unwrap();
    archive
        .append_node(
            "b".into(),
            "c1".into(),
            Some("n2".into()),
            "user".into(),
            4,
            "other body",
        )
        .unwrap();
    archive
        .append_branch(Branch {
            id: "a".into(),
            conversation_id: "c1".into(),
            leaf_node_id: "a".into(),
            canonical: true,
        })
        .unwrap();
    archive
        .append_branch(Branch {
            id: "b".into(),
            conversation_id: "c1".into(),
            leaf_node_id: "b".into(),
            canonical: false,
        })
        .unwrap();
    assert_eq!(archive.stats().nodes, 4);
    assert_eq!(archive.stats().branches, 2);
    assert_eq!(archive.stats().content_objects, 3);
    archive.sync().unwrap();
    drop(archive);

    let mut reopened = Cva::open(&path).unwrap();
    assert_eq!(reopened.stats().content_objects, 3);
    let a = reopened.branch_turns("c1", "a").unwrap();
    let b = reopened.branch_turns("c1", "b").unwrap();
    assert_eq!(
        a.iter().map(|t| t.node_id.as_str()).collect::<Vec<_>>(),
        ["n1", "n2", "a"]
    );
    assert_eq!(
        b.iter().map(|t| t.node_id.as_str()).collect::<Vec<_>>(),
        ["n1", "n2", "b"]
    );
    assert_eq!(a[0].content, "same body");
    assert_eq!(a[2].content, "same body");
}

#[test]
fn binary_content_round_trips_across_reopen() {
    let path = test_path();
    let bytes = vec![0x00, 0xff, 0x80, 0x41, 0x00, 0xfe, 0x7f];
    let id = hash_content(&bytes);

    let mut archive = Cva::create(&path).unwrap();
    archive
        .archive
        .put_content_bytes(&mut archive.container, id, &bytes)
        .unwrap();
    assert_eq!(
        archive
            .archive
            .content_bytes(&mut archive.container, id)
            .unwrap(),
        bytes
    );
    assert!(matches!(
        archive.archive.content(&mut archive.container, id),
        Err(ArchiveError::InvalidUtf8)
    ));
    archive.sync().unwrap();
    drop(archive);

    let mut reopened = Cva::open(&path).unwrap();
    assert_eq!(
        reopened
            .archive
            .content_bytes(&mut reopened.container, id)
            .unwrap(),
        bytes
    );
}

#[test]
fn identical_node_append_is_idempotent() {
    let path = test_path();
    let mut archive = Cva::create(path).unwrap();
    archive
        .append_node("n1".into(), "c1".into(), None, "user".into(), 1, "hello")
        .unwrap();
    archive
        .append_node("n1".into(), "c1".into(), None, "user".into(), 1, "hello")
        .unwrap();
    assert_eq!(archive.stats().nodes, 1);
    assert_eq!(archive.stats().content_objects, 1);
}

#[test]
fn principal_id_round_trips_and_current_node_preserves_none() {
    let path = test_path();
    let principal = "phy-00000000-0000-0000-0000-000000000001".to_owned();
    let mut archive = Cva::create(&path).unwrap();
    let node = archive
        .append_node_with_principal(
            "n1".into(),
            "c1".into(),
            None,
            "user".into(),
            Some(principal.clone()),
            1,
            "hello",
        )
        .unwrap();
    assert_eq!(node.principal_id.as_deref(), Some(principal.as_str()));
    archive.sync().unwrap();
    drop(archive);

    let mut reopened = Cva::open(&path).unwrap();
    let turns = reopened.conversation_turns("c1", "n1").unwrap();
    assert_eq!(turns[0].principal_id.as_deref(), Some(principal.as_str()));

    let legacy = Node {
        id: "legacy".into(),
        conversation_id: "legacy-c".into(),
        parent_id: None,
        role: "user".into(),
        principal_id: None,
        timestamp_ns: 2,
        content_id: node.content_id,
    };
    let mut bytes = encode_node(&legacy).unwrap();

    let ArchiveRecord::Node(decoded) = decode_record(&bytes).unwrap() else {
        panic!("legacy node did not decode as a node");
    };
    assert_eq!(decoded.principal_id, None);
}

#[test]
fn unversioned_semantic_record_is_inert_on_reopen() {
    let path = test_path();
    let mut archive = Cva::create(&path).unwrap();
    let first = archive
        .append_node("n1".into(), "c1".into(), None, "user".into(), 1, "hello")
        .unwrap();
    let orphan = Node {
        id: "orphan".into(),
        conversation_id: "c1".into(),
        parent_id: Some("n1".into()),
        role: "assistant".into(),
        principal_id: None,
        timestamp_ns: 2,
        content_id: first.content_id,
    };
    archive
        .container
        .append(&encode_node(&orphan).unwrap())
        .unwrap();
    archive.sync().unwrap();
    drop(archive);

    let reopened = Cva::open(path).unwrap();
    assert_eq!(reopened.stats().nodes, 1);
    assert!(reopened.archive.source_node("c1", "orphan").is_none());
}
