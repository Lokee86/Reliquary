use super::*;
use crate::archive_codec::{
    ArchiveRecord,
    publication::{self, ArchivePublication},
};
use crate::{Container, Cva, ObjectRef};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!("archive-upgrade-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn frame(bytes: &mut Vec<u8>, payload: &[u8]) -> ObjectRef {
    let offset = bytes.len() as u64;
    bytes.extend_from_slice(&(payload.len() as u64).to_le_bytes());
    bytes.extend_from_slice(payload);
    let mut reference = [0; 16];
    reference[..8].copy_from_slice(&offset.to_le_bytes());
    reference[8..].copy_from_slice(&(payload.len() as u64).to_le_bytes());
    ObjectRef::from_legacy_bytes(reference)
}

fn make_v1_fixture(path: &Path) {
    let mut owner = Cva::create_project(path).unwrap();
    owner
        .append_node(
            "n".into(),
            "c".into(),
            None,
            "user".into(),
            1,
            "original body",
        )
        .unwrap();
    owner
        .archive
        .put_conversation_metadata(
            &mut owner.container,
            crate::ConversationMetadata {
                conversation_id: "c".into(),
                title: Some("fixture".into()),
                active: false,
                policy: crate::ConversationPolicy::SequentialChronological,
                policy_revision: 1,
            },
        )
        .unwrap();
    owner
        .append_branch(crate::Branch {
            id: "b".into(),
            conversation_id: "c".into(),
            leaf_node_id: "n".into(),
            canonical: true,
        })
        .unwrap();
    let file = owner
        .store_file("file.txt".into(), None, b"standalone file")
        .unwrap();
    owner
        .ingest_turn(crate::IncomingTurn {
            id: "n2".into(),
            conversation_id: "c".into(),
            parent_id: Some("n".into()),
            role: "assistant".into(),
            principal_id: None,
            timestamp_ns: 2,
            content: "turn body".into(),
            attachments: vec![crate::IncomingAttachment {
                filename: "attached.txt".into(),
                mime_type: Some("text/plain".into()),
                bytes: b"attached file".to_vec(),
            }],
            project_attachments: vec![],
        })
        .unwrap();
    owner
        .archive
        .put_fragment(
            &mut owner.container,
            crate::Fragment {
                id: crate::fragment_store::fragment_id("c", "n", "n2"),
                conversation_id: "c".into(),
                start_node_id: "n".into(),
                end_node_id: "n2".into(),
            },
        )
        .unwrap();
    owner
        .archive
        .put_episode(
            &mut owner.container,
            crate::Episode {
                id: crate::EpisodeId([0; 32]),
                conversation_id: "c".into(),
                start_node_id: "n".into(),
                end_node_id: "n2".into(),
                origin: crate::EpisodeOrigin::Import,
                boundary: crate::EpisodeBoundary::ImportEnd,
                source_through_ns: 2,
                finalized_at_ns: 3,
            },
        )
        .unwrap();
    owner
        .archive
        .put_import_receipt(
            &mut owner.container,
            crate::imports::ImportReceipt {
                schema_version: 1,
                import_key: "ab".repeat(32),
                source_namespace: "fixture".into(),
                source_format: "fixture-json".into(),
                conversations: vec![],
            },
        )
        .unwrap();
    let draft = crate::entity_candidate_test_support::memory_draft(
        "archive-fixture-memory",
        "title",
        "body",
    );
    let memory = owner
        .memories
        .publish_legacy_fixture(&mut owner.container, None, 0, draft)
        .unwrap()
        .0;
    owner
        .archive
        .link_file_to_memory(&mut owner.container, file.id, memory.id)
        .unwrap();
    owner.tombstone_conversation("c").unwrap();
    let repeat = publication::encode(&ArchivePublication::Repeat {
        archive_version: owner.archive_version() + 1,
        first_applying_archive_version: 1,
    })
    .unwrap();
    owner
        .container
        .append_publication_envelope(&repeat)
        .unwrap();
    let orphan = crate::Node {
        id: "orphan".into(),
        conversation_id: "unused".into(),
        parent_id: None,
        role: "system".into(),
        principal_id: None,
        timestamp_ns: 4,
        content_id: crate::archive_store::hash_content(b"original body"),
    };
    owner
        .container
        .append(&crate::archive_codec::encode_node(&orphan).unwrap())
        .unwrap();
    owner.sync().unwrap();
    drop(owner);

    // Explicit test-only legacy fixture construction, independent of ordinary writers.
    let mut container = Container::open(path).unwrap();
    let mut bytes = fs::read(path).unwrap()[..container.header_len as usize].to_vec();
    let mut relocated = BTreeMap::new();
    let mut applying = BTreeMap::new();
    for object in container.chunks().unwrap() {
        let payload = container.read(object).unwrap();
        if let Some(envelope) = crate::container::publication_envelope::decode(&payload).unwrap() {
            let publication = publication::decode(envelope.body, envelope.global_version)
                .unwrap()
                .expect("only Archive envelopes in fixture");
            let local = publication.archive_version();
            let mut ticket = b"CVAVERS2".to_vec();
            ticket.extend_from_slice(&envelope.global_version.to_le_bytes());
            ticket.extend_from_slice(
                &container
                    .transaction_time_ns(envelope.global_version)
                    .unwrap()
                    .to_le_bytes(),
            );
            frame(&mut bytes, &ticket);
            let record = match publication {
                ArchivePublication::Record { record, .. } => {
                    let carrier = publication::comparison_carrier(&record).unwrap();
                    let object = frame(&mut bytes, &carrier);
                    applying.insert(local, object);
                    object
                }
                ArchivePublication::Repeat {
                    first_applying_archive_version,
                    ..
                } => applying[&first_applying_archive_version],
            };
            let marker =
                crate::archive_history_codec::encode_record_version(crate::ArchiveRecordVersion {
                    global_version: envelope.global_version,
                    archive_version: local,
                    record,
                });
            relocated.insert(object, frame(&mut bytes, &marker));
        } else {
            let transformed =
                crate::storage_reclamation::relocate_payload(&payload, &relocated).unwrap();
            relocated.insert(object, frame(&mut bytes, &transformed));
        }
    }
    drop(container);
    bytes[8..10].copy_from_slice(&1_u16.to_le_bytes());
    bytes[10..12].copy_from_slice(&0_u16.to_le_bytes());
    fs::write(path, bytes).unwrap();
}

#[test]
fn archive_conversion_preserves_every_family_repeat_cut_and_source_bytes() {
    let scratch = Scratch::new();
    let source = scratch.0.join("source.rel");
    let destination = scratch.0.join("unpublished.rel");
    make_v1_fixture(&source);
    let before = fs::read(&source).unwrap();
    let modified = fs::metadata(&source).unwrap().modified().unwrap();
    let mut old = Cva::open_v1_read_only(&source).unwrap();
    let plan = plan_storage_upgrade(StorageUpgradeRequest {
        source: source.clone(),
        destination: destination.clone(),
    })
    .unwrap();
    let staged = stage_archive_envelope_storage_upgrade(&plan).unwrap();
    let mut new = Cva::open(staged.test_path()).unwrap();
    assert_eq!(old.archive_version(), 11);
    assert_eq!(new.archive_version(), 11);
    let mut families = std::collections::BTreeSet::new();
    for local in 1..=11 {
        let a = *old.archive.record_version(local).unwrap();
        let b = *new.archive.record_version(local).unwrap();
        let (a_record, a_origin) = old.archive.read_publication(&mut old.container, a).unwrap();
        let (b_record, b_origin) = new.archive.read_publication(&mut new.container, b).unwrap();
        assert_eq!(
            publication::comparison_carrier(&a_record).unwrap(),
            publication::comparison_carrier(&b_record).unwrap()
        );
        assert_eq!(
            (a.global_version, a_origin.archive_version),
            (b.global_version, b_origin.archive_version)
        );
        assert_eq!(
            old.transaction_time_ns(a.global_version),
            new.transaction_time_ns(b.global_version)
        );
        let payload = new.container.read(b.record).unwrap();
        let envelope = crate::container::publication_envelope::decode(&payload)
            .unwrap()
            .unwrap();
        assert_eq!(
            envelope.clock,
            crate::container::publication_envelope::PublicationClock::BindReserved
        );
        families.insert(envelope.body[..8].to_vec());
    }
    assert_eq!(families.len(), 11);
    let mut orphan_count = 0;
    for object in new.container.chunks().unwrap() {
        let payload = new.container.read(object).unwrap();
        assert!(!payload.starts_with(b"CVAAREC1"));
        if let ArchiveRecord::Node(node) = crate::archive_codec::decode_record(&payload).unwrap() {
            assert_eq!(node.id, "orphan");
            orphan_count += 1;
        }
    }
    assert_eq!(orphan_count, 1);
    assert_eq!(old.owner_uuid(), new.owner_uuid());
    assert_eq!(old.latest_global_version(), new.latest_global_version());
    assert_eq!(fs::read(&source).unwrap(), before);
    assert_eq!(fs::metadata(&source).unwrap().modified().unwrap(), modified);
    assert!(!destination.exists());
    drop(new);
    drop(old);
    drop(staged);
    assert!(fs::read_dir(&scratch.0).unwrap().all(|entry| {
        !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".storage-upgrade-")
    }));
}

#[test]
fn invalid_archive_local_history_returns_no_stage_and_changes_no_source() {
    let scratch = Scratch::new();
    let source = scratch.0.join("invalid.rel");
    let destination = scratch.0.join("unpublished.rel");
    make_v1_fixture(&source);
    let mut container = Container::open_v1_read_only(&source).unwrap();
    let marker = container
        .chunks()
        .unwrap()
        .into_iter()
        .find(|object| {
            crate::archive_history_codec::decode_record_version(&container.read(*object).unwrap())
                .unwrap()
                .is_some()
        })
        .unwrap();
    let offset = u64::from_le_bytes(marker.legacy_bytes()[..8].try_into().unwrap()) as usize;
    drop(container);
    let mut bytes = fs::read(&source).unwrap();
    bytes[offset + 24..offset + 32].copy_from_slice(&2_u64.to_le_bytes());
    fs::write(&source, &bytes).unwrap();
    let modified = fs::metadata(&source).unwrap().modified().unwrap();
    let plan = plan_storage_upgrade(StorageUpgradeRequest {
        source: source.clone(),
        destination: destination.clone(),
    })
    .unwrap();
    assert!(stage_archive_envelope_storage_upgrade(&plan).is_err());
    assert_eq!(fs::read(&source).unwrap(), bytes);
    assert_eq!(fs::metadata(&source).unwrap().modified().unwrap(), modified);
    assert!(!destination.exists());
    assert!(fs::read_dir(&scratch.0).unwrap().all(|entry| {
        !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".storage-upgrade-")
    }));
}
