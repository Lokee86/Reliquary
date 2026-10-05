use crate::conversation_metadata_codec::{
    LEGACY_CONVERSATION_METADATA_MAGIC, decode_conversation_metadata,
};
use crate::{Cva, IncomingTurn};
use std::fs;
use std::path::PathBuf;

fn test_path() -> PathBuf {
    let unique = uuid::Uuid::new_v4();
    let dir = std::env::temp_dir().join(format!("continuity-conversations-{unique}"));
    fs::create_dir_all(&dir).unwrap();
    dir.join("archive.cva")
}

fn turn(id: &str, parent: Option<&str>, timestamp_ns: i64, content: &str) -> IncomingTurn {
    IncomingTurn {
        id: id.into(),
        conversation_id: "c1".into(),
        parent_id: parent.map(str::to_owned),
        role: "user".into(),
        principal_id: None,
        timestamp_ns,
        content: content.into(),
        attachments: Vec::new(),
        project_attachments: Vec::new(),
    }
}

#[test]
fn conversation_summary_and_path_survive_reopen() {
    let path = test_path();
    let mut cva = Cva::create(&path).unwrap();
    cva.ingest_turn(turn("u1", None, 10, "First")).unwrap();
    cva.ingest_turn(turn("u2", Some("u1"), 20, "Second"))
        .unwrap();
    cva.sync().unwrap();
    drop(cva);

    let mut reopened = Cva::open(path).unwrap();
    let summaries = reopened.conversation_summaries();
    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].conversation_id, "c1");
    assert_eq!(summaries[0].title, None);
    assert!(!summaries[0].active);
    assert_eq!(summaries[0].leaf_node_ids, vec!["u2"]);
    assert_eq!(summaries[0].turn_count, 2);
    assert_eq!(summaries[0].latest_timestamp_ns, 20);

    let turns = reopened.conversation_turns("c1", "u2").unwrap();
    assert_eq!(turns.len(), 2);
    assert_eq!(turns[0].content, "First");
    assert_eq!(turns[1].content, "Second");
}

#[test]
fn legacy_conversation_metadata_decodes_inactive() {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&LEGACY_CONVERSATION_METADATA_MAGIC);
    bytes.extend_from_slice(&2u32.to_le_bytes());
    bytes.extend_from_slice(b"c1");
    bytes.extend_from_slice(&5u32.to_le_bytes());
    bytes.extend_from_slice(b"Title");

    let metadata = decode_conversation_metadata(&bytes).unwrap();
    assert_eq!(metadata.conversation_id, "c1");
    assert_eq!(metadata.title.as_deref(), Some("Title"));
    assert!(!metadata.active);
}

#[test]
fn conversation_title_requires_existing_conversation() {
    let path = test_path();
    let mut cva = Cva::create(path).unwrap();
    assert!(matches!(
        cva.set_conversation_title("missing", "Title".into()),
        Err(crate::ArchiveError::MissingConversation)
    ));
}

#[test]
fn conversation_title_round_trips_and_is_idempotent() {
    let path = test_path();
    let mut cva = Cva::create(&path).unwrap();
    cva.ingest_turn(turn("u1", None, 10, "First")).unwrap();
    assert!(
        cva.set_conversation_title("c1", "Imported title".into())
            .unwrap()
    );
    assert!(
        !cva.set_conversation_title("c1", "Imported title".into())
            .unwrap()
    );
    cva.sync().unwrap();
    drop(cva);

    let reopened = Cva::open(path).unwrap();
    assert_eq!(
        reopened.conversation_summaries()[0].title.as_deref(),
        Some("Imported title")
    );
    assert_eq!(
        reopened
            .conversation_metadata("c1")
            .and_then(|metadata| metadata.title.as_deref()),
        Some("Imported title")
    );
}

#[test]
fn conversation_active_round_trips_and_title_updates_preserve_it() {
    let path = test_path();
    let mut cva = Cva::create(&path).unwrap();
    cva.ingest_turn(turn("u1", None, 10, "First")).unwrap();
    assert!(cva.set_conversation_active("c1", true).unwrap());
    assert!(!cva.set_conversation_active("c1", true).unwrap());
    assert!(
        cva.set_conversation_title("c1", "Active title".into())
            .unwrap()
    );
    cva.sync().unwrap();
    drop(cva);

    let reopened = Cva::open(path).unwrap();
    let metadata = reopened.conversation_metadata("c1").unwrap();
    assert!(metadata.active);
    assert_eq!(metadata.title.as_deref(), Some("Active title"));
    assert!(reopened.conversation_summaries()[0].active);
}

#[test]
fn conversation_summary_exposes_multiple_durable_leaves() {
    let path = test_path();
    let mut cva = Cva::create(path).unwrap();
    cva.ingest_turn(turn("root", None, 10, "Root")).unwrap();
    cva.ingest_turn(turn("older", Some("root"), 20, "Older branch"))
        .unwrap();
    cva.ingest_turn(turn("newer", Some("root"), 30, "Newer branch"))
        .unwrap();

    let summary = cva.conversation_summaries().remove(0);
    assert_eq!(summary.leaf_node_ids, vec!["newer", "older"]);
    assert_eq!(summary.turn_count, 3);
    assert_eq!(
        cva.conversation_turns("c1", "older").unwrap()[1].content,
        "Older branch"
    );
}

#[test]
fn branch_start_markers_follow_every_durable_fork_on_selected_path() {
    let path = test_path();
    let mut cva = Cva::create(path).unwrap();
    cva.ingest_turn(turn("root", None, 10, "Root")).unwrap();
    cva.ingest_turn(turn("left", Some("root"), 20, "Left"))
        .unwrap();
    cva.ingest_turn(turn("right", Some("root"), 30, "Right"))
        .unwrap();
    cva.ingest_turn(turn("right-a", Some("right"), 40, "Right A"))
        .unwrap();
    cva.ingest_turn(turn("right-b", Some("right"), 50, "Right B"))
        .unwrap();

    assert_eq!(
        cva.conversation_branch_start_node_ids("c1", "left")
            .unwrap(),
        vec!["left"]
    );
    assert_eq!(
        cva.conversation_branch_start_node_ids("c1", "right-a")
            .unwrap(),
        vec!["right", "right-a"]
    );
}

#[test]
fn conversation_policy_defaults_and_round_trips_without_changing_other_metadata() {
    use crate::ConversationPolicy::{SequentialChronological, VisibleAutoBranch};
    let path = test_path();
    let mut cva = Cva::create(&path).unwrap();
    cva.ingest_turn(turn("root", None, 10, "Root")).unwrap();
    let before = cva.archive_version();
    assert_eq!(cva.conversation_policy("c1"), (SequentialChronological, 1));
    assert!(
        !cva.set_conversation_policy("c1", SequentialChronological)
            .unwrap()
    );
    assert_eq!(cva.archive_version(), before);
    assert!(matches!(
        cva.set_conversation_policy("missing", VisibleAutoBranch),
        Err(crate::ArchiveError::MissingConversation)
    ));
    assert!(
        cva.set_conversation_policy("c1", VisibleAutoBranch)
            .unwrap()
    );
    assert_eq!(cva.archive_version(), before + 1);
    assert!(
        !cva.set_conversation_policy("c1", VisibleAutoBranch)
            .unwrap()
    );
    cva.set_conversation_title("c1", "Managed title".into())
        .unwrap();
    cva.set_conversation_active("c1", true).unwrap();
    assert_eq!(cva.conversation_policy("c1"), (VisibleAutoBranch, 2));
    cva.sync().unwrap();
    drop(cva);
    let mut reopened = Cva::open(&path).unwrap();
    assert_eq!(reopened.conversation_policy("c1"), (VisibleAutoBranch, 2));
    let metadata = reopened.conversation_metadata("c1").unwrap();
    assert_eq!(metadata.title.as_deref(), Some("Managed title"));
    assert!(metadata.active);
    assert!(
        reopened
            .set_conversation_policy("c1", SequentialChronological)
            .unwrap()
    );
    assert_eq!(
        reopened.conversation_policy("c1"),
        (SequentialChronological, 3)
    );
    reopened.sync().unwrap();
    drop(reopened);
    assert_eq!(
        Cva::open(path).unwrap().conversation_policy("c1"),
        (SequentialChronological, 3)
    );
}

#[test]
fn legacy_conversation_policy_reopens_and_upgrades_in_the_same_archive_owner() {
    use crate::ConversationPolicy::{SequentialChronological, VisibleAutoBranch};
    for (magic, active) in [(*b"CVACONV1", false), (*b"CVACONV2", true)] {
        let path = test_path();
        let mut cva = Cva::create(&path).unwrap();
        cva.ingest_turn(turn("root", None, 10, "Root")).unwrap();
        let mut bytes = magic.to_vec();
        bytes.extend_from_slice(&2u32.to_le_bytes());
        bytes.extend_from_slice(b"c1");
        bytes.extend_from_slice(&5u32.to_le_bytes());
        bytes.extend_from_slice(b"Title");
        if magic == *b"CVACONV2" {
            bytes.push(1);
        }
        let record = cva.container.append(&bytes).unwrap();
        cva.archive
            .publish_record(&mut cva.container, record)
            .unwrap();
        cva.sync().unwrap();
        drop(cva);
        let mut reopened = Cva::open(&path).unwrap();
        assert_eq!(
            reopened.conversation_policy("c1"),
            (SequentialChronological, 1)
        );
        assert_eq!(reopened.conversation_metadata("c1").unwrap().active, active);
        reopened
            .set_conversation_policy("c1", VisibleAutoBranch)
            .unwrap();
        reopened.sync().unwrap();
        drop(reopened);
        let upgraded = Cva::open(path).unwrap();
        assert_eq!(upgraded.conversation_policy("c1"), (VisibleAutoBranch, 2));
        let metadata = upgraded.conversation_metadata("c1").unwrap();
        assert_eq!(metadata.title.as_deref(), Some("Title"));
        assert_eq!(metadata.active, active);
    }
}

#[test]
fn conversation_policy_codec_fails_closed_on_unknown_or_incomplete_state() {
    use crate::conversation_metadata_codec::encode_conversation_metadata;
    let metadata = crate::ConversationMetadata::new("c1");
    let bytes = encode_conversation_metadata(&metadata).unwrap();
    for end in 0..bytes.len() {
        assert!(decode_conversation_metadata(&bytes[..end]).is_err());
    }
    let mut invalid_policy = bytes.clone();
    let policy_offset = invalid_policy.len() - 9;
    invalid_policy[policy_offset] = 99;
    assert!(decode_conversation_metadata(&invalid_policy).is_err());
    let mut zero_revision = bytes.clone();
    let revision_offset = zero_revision.len() - 8;
    zero_revision[revision_offset..].fill(0);
    assert!(decode_conversation_metadata(&zero_revision).is_err());
    let mut trailing = bytes;
    trailing.push(0);
    assert!(decode_conversation_metadata(&trailing).is_err());
    let mut exhausted = metadata;
    exhausted.policy_revision = u64::MAX;
    let path = test_path();
    let mut cva = Cva::create(path).unwrap();
    cva.ingest_turn(turn("root", None, 10, "Root")).unwrap();
    cva.archive
        .put_conversation_metadata(&mut cva.container, exhausted)
        .unwrap();
    let version = cva.archive_version();
    assert!(
        cva.set_conversation_policy("c1", crate::ConversationPolicy::VisibleAutoBranch)
            .is_err()
    );
    assert_eq!(cva.archive_version(), version);
    assert_eq!(cva.conversation_policy("c1").1, u64::MAX);
}
