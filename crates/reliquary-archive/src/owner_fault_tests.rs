use crate::archive_activity::overwrite_activity_position_for_test;
use crate::archive_codec::ArchiveRecord;
use crate::archive_codec::publication::{self, ArchivePublication};
use crate::archive_rebuild::ArchiveOpenState;
use crate::{Archive, ArchiveError};
use reliquary_container::publication_envelope::{PublicationClock, PublicationEnvelope};
use reliquary_container::{Container, ObjectRef};

fn publication(local: u64, first: Option<u64>) -> Vec<u8> {
    let value = match first {
        Some(first_applying_archive_version) => ArchivePublication::Repeat {
            archive_version: local,
            first_applying_archive_version,
        },
        None => ArchivePublication::Record {
            archive_version: local,
            record: ArchiveRecord::Branch(crate::Branch {
                id: "b".into(),
                conversation_id: "c".into(),
                leaf_node_id: "n".into(),
                canonical: true,
            }),
        },
    };
    publication::encode(&value).unwrap()
}

fn admit(
    state: &mut ArchiveOpenState,
    object: ObjectRef,
    local: u64,
    global: u64,
    first: Option<u64>,
) -> Result<bool, ArchiveError> {
    let body = publication(local, first);
    state.ingest_publication_envelope(
        object,
        PublicationEnvelope {
            global_version: global,
            clock: PublicationClock::BindReserved,
            body: &body,
        },
        global,
    )
}

fn seed_format(state: &mut ArchiveOpenState, marker_object: ObjectRef) {
    let marker = crate::archive_history_codec::encode_archive_format();
    state.ingest(marker_object, &marker, 0).unwrap();
}

fn fixture_container() -> (std::path::PathBuf, Container, ObjectRef, ObjectRef) {
    let path = std::env::current_dir()
        .unwrap()
        .join(format!(".archive-owner-fault-{}.rel", uuid::Uuid::new_v4()));
    let mut container = Container::create(&path).unwrap();
    let marker = crate::archive_history_codec::encode_archive_format();
    let marker_object = container.append(&marker).unwrap();
    let object = container.append(b"test object").unwrap();
    (path, container, marker_object, object)
}

#[test]
fn local_archive_version_exhaustion_publishes_no_content_or_node() {
    let path = std::env::current_dir().unwrap().join(format!(
        ".archive-version-fault-{}.rel",
        uuid::Uuid::new_v4()
    ));
    let mut container = Container::create(&path).unwrap();
    let mut archive = Archive::empty();
    archive.next_archive_version = u64::MAX;
    let chunks = container.chunks().unwrap();

    let result = archive.append_node(
        &mut container,
        "n".into(),
        "c".into(),
        None,
        "user".into(),
        None,
        1,
        "body",
    );
    assert!(matches!(result, Err(ArchiveError::ArchiveVersionExhausted)));
    assert_eq!(container.chunks().unwrap(), chunks);
    assert_eq!(archive.archive_version(), u64::MAX - 1);
    assert!(archive.source_node("c", "n").is_none());

    drop(container);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn corrupted_activity_position_breaks_preservation_comparison() {
    let left_path = std::env::current_dir().unwrap().join(format!(
        ".archive-activity-left-{}.rel",
        uuid::Uuid::new_v4()
    ));
    let right_path = std::env::current_dir().unwrap().join(format!(
        ".archive-activity-right-{}.rel",
        uuid::Uuid::new_v4()
    ));
    let mut left_container = Container::create(&left_path).unwrap();
    let mut right_container = Container::create(&right_path).unwrap();
    let mut left = Archive::empty();
    let mut right = Archive::empty();
    for (archive, container) in [
        (&mut left, &mut left_container),
        (&mut right, &mut right_container),
    ] {
        archive
            .append_node(
                container,
                "u".into(),
                "c".into(),
                None,
                "user".into(),
                None,
                1,
                "body",
            )
            .unwrap();
    }

    assert!(left.activity_order_matches(&right));
    assert!(left.activity_history_matches(&right));
    overwrite_activity_position_for_test(&mut right.activity, 0, 7);
    assert!(!left.activity_order_matches(&right));
    assert!(!left.activity_history_matches(&right));

    drop(left_container);
    drop(right_container);
    std::fs::remove_file(left_path).unwrap();
    std::fs::remove_file(right_path).unwrap();
}

#[test]
fn repeats_consume_cuts_without_becoming_applying_anchors() {
    let (path, container, marker, object) = fixture_container();
    let mut state = ArchiveOpenState::new();
    seed_format(&mut state, marker);
    assert!(admit(&mut state, object, 1, 2, None).unwrap());
    assert!(admit(&mut state, object, 2, 4, Some(1)).unwrap());
    assert!(admit(&mut state, object, 3, 5, Some(2)).is_err());
    assert!(admit(&mut state, object, 3, 5, Some(1)).unwrap());
    let archive = state.finish().unwrap();
    assert_eq!(archive.archive_version(), 3);
    assert_eq!(archive.record_versions().len(), 3);
    assert_eq!(archive.record_version(3).unwrap().archive_version, 3);
    drop(container);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn local_gaps_and_reversed_globals_leave_cut_unchanged() {
    let (path, container, marker, object) = fixture_container();
    let mut state = ArchiveOpenState::new();
    seed_format(&mut state, marker);
    assert!(admit(&mut state, object, 2, 2, None).is_err());
    assert!(admit(&mut state, object, 1, 4, None).unwrap());
    assert!(admit(&mut state, object, 2, 3, Some(1)).is_err());
    let archive = state.finish().unwrap();
    assert_eq!(archive.archive_version(), 1);
    assert_eq!(archive.record_versions().len(), 1);
    drop(container);
    std::fs::remove_file(path).unwrap();
}
