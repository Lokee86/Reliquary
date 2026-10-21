use super::*;
use crate::archive_codec::publication::{self, ArchivePublication};
use crate::container::publication_envelope::{PublicationClock, PublicationEnvelope};

#[test]
fn native_terminal_uses_envelope_coordinates_and_exact_closure() {
    for invalid in [false, true] {
        let dir = std::env::temp_dir().join(format!(
            "archive-envelope-terminal-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test.rel");
        let mut cva = crate::Cva::create(&path).unwrap();
        cva.append_node("n".into(), "c".into(), None, "user".into(), 1, "content")
            .unwrap();
        let mut closure = cva.archive.discover_conversation_tombstone_closure("c");
        if invalid {
            closure.node_ids.clear();
        }
        let body = publication::encode(&ArchivePublication::Record {
            archive_version: 2,
            record: ArchiveRecord::ConversationTombstone(crate::ConversationTombstone {
                conversation_id: "c".into(),
                archive_version: 2,
                global_version: 999,
                closure,
            }),
        })
        .unwrap();
        let (global, _) = cva.container.append_publication_envelope(&body).unwrap();
        drop(cva);
        let result = crate::Cva::open(&path);
        if invalid {
            assert!(matches!(
                result,
                Err(crate::CvaError::Archive(
                    ArchiveError::ConflictingConversationTombstone
                ))
            ));
        } else {
            let opened = result.unwrap();
            assert_eq!(
                opened.archive.conversation_tombstones["c"].global_version,
                global
            );
            assert_eq!(opened.archive_version(), 2);
        }
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
fn rel_dispatch_reopens_native_records_and_repeat_at_their_own_objects() {
    let dir = std::env::temp_dir().join(format!("archive-envelope-open-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("test.rel");
    let mut cva = crate::Cva::create(&path).unwrap();
    cva.append_node("n".into(), "c".into(), None, "user".into(), 1, "content")
        .unwrap();
    let branch = publication::encode(&ArchivePublication::Record {
        archive_version: 2,
        record: ArchiveRecord::Branch(crate::Branch {
            id: "b".into(),
            conversation_id: "c".into(),
            leaf_node_id: "n".into(),
            canonical: true,
        }),
    })
    .unwrap();
    let (first_global, first_object) = cva.container.append_publication_envelope(&branch).unwrap();
    let updated = publication::encode(&ArchivePublication::Record {
        archive_version: 3,
        record: ArchiveRecord::Branch(crate::Branch {
            id: "b".into(),
            conversation_id: "c".into(),
            leaf_node_id: "n".into(),
            canonical: false,
        }),
    })
    .unwrap();
    cva.container.append_publication_envelope(&updated).unwrap();
    let repeat = publication::encode(&ArchivePublication::Repeat {
        archive_version: 4,
        first_applying_archive_version: 2,
    })
    .unwrap();
    let (repeat_global, repeat_object) =
        cva.container.append_publication_envelope(&repeat).unwrap();
    drop(cva);
    let mut reopened = crate::Cva::open(&path).unwrap();
    assert_eq!(reopened.archive_version(), 4);
    let versions = reopened.archive.record_versions();
    assert_eq!(versions[1].global_version, first_global);
    assert_eq!(versions[1].record, first_object);
    assert_eq!(versions[3].global_version, repeat_global);
    assert_eq!(versions[3].record, repeat_object);
    let repeated = versions[3];
    let (record, applying) = reopened
        .archive
        .read_publication(&mut reopened.container, repeated)
        .unwrap();
    assert_eq!(applying.archive_version, 2);
    assert!(matches!(
        record,
        ArchiveRecord::Branch(crate::Branch {
            canonical: true,
            ..
        })
    ));
    assert!(
        reopened
            .archive
            .branch_at(&mut reopened.container, "c", "b", 2)
            .unwrap()
            .unwrap()
            .canonical
    );
    assert!(
        !reopened
            .archive
            .branch_at(&mut reopened.container, "c", "b", 4)
            .unwrap()
            .unwrap()
            .canonical
    );
    drop(reopened);
    std::fs::remove_dir_all(dir).unwrap();
}

fn admit(
    state: &mut ArchiveOpenState,
    object: ObjectRef,
    local: u64,
    global: u64,
    first: Option<u64>,
) -> Result<bool, ArchiveError> {
    let publication = match first {
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
    let body = publication::encode(&publication)?;
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

#[test]
fn repeats_consume_cuts_without_becoming_applying_anchors() {
    let dir =
        std::env::temp_dir().join(format!("archive-envelope-replay-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut cva = crate::Cva::create(dir.join("test.rel")).unwrap();
    let object = cva.container.append(b"test object").unwrap();
    let mut state = ArchiveOpenState::new();
    assert!(admit(&mut state, object, 1, 2, None).unwrap());
    assert!(admit(&mut state, object, 2, 4, Some(1)).unwrap());
    assert_eq!(state.record_versions.len(), 2);
    assert_eq!(state.next_archive_version, 3);
    assert_eq!(state.applying_versions.len(), 1);
    assert!(admit(&mut state, object, 3, 5, Some(2)).is_err());
    assert_eq!(state.record_versions.len(), 2);
    assert_eq!(state.next_archive_version, 3);
    assert!(admit(&mut state, object, 3, 5, Some(1)).unwrap());
    drop(cva);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn local_gaps_and_reversed_globals_leave_cut_unchanged() {
    let dir = std::env::temp_dir().join(format!("archive-envelope-order-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut cva = crate::Cva::create(dir.join("test.rel")).unwrap();
    let object = cva.container.append(b"test object").unwrap();
    let mut state = ArchiveOpenState::new();
    assert!(admit(&mut state, object, 2, 2, None).is_err());
    assert_eq!(state.next_archive_version, 1);
    admit(&mut state, object, 1, 4, None).unwrap();
    assert!(admit(&mut state, object, 2, 3, Some(1)).is_err());
    assert_eq!(state.next_archive_version, 2);
    assert_eq!(state.record_versions.len(), 1);
    drop(cva);
    std::fs::remove_dir_all(dir).unwrap();
}
