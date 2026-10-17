use super::*;
use crate::{ContainerIdentity, FileKind, imports::ImportWatermarks};
use std::{fs, path::Path};

fn start(owner_uuid: [u8; 16], import_key: &str) -> ImportStart {
    ImportStart {
        owner_uuid,
        import_id: import_id(owner_uuid, import_key),
        import_key: import_key.to_owned(),
        source_namespace: "agent/account".into(),
        source_format: "structured-agent-v1".into(),
        started_at_ns: 1,
        pre: ImportWatermarks::default(),
    }
}

fn container(path: &Path, owner_uuid: [u8; 16]) -> Container {
    Container::create_with_identity_and_uuid(
        path,
        ContainerIdentity {
            file_kind: FileKind::Reliquary,
            scope: None,
        },
        owner_uuid,
    )
    .unwrap()
}

#[test]
fn import_id_has_a_stable_owner_scoped_vector() {
    let owner = [0x11; 16];
    let key = "a".repeat(64);
    assert_eq!(
        import_id(owner, &key),
        "2161ec3c1b52f6feb67e7df8793e5267111b19244f9d05fc40d86721b71d120b"
    );
    assert_ne!(import_id([0x12; 16], &key), import_id(owner, &key));
    let other_key = format!("{}b", &key[..63]);
    assert_ne!(import_id(owner, &other_key), import_id(owner, &key));
}

#[test]
fn append_start_rejects_invalid_identity_before_durable_write_or_fence() {
    let dir = std::env::temp_dir().join(format!("epoch-start-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&dir).unwrap();
    let path = dir.join("owner.rel");
    let owner = [0x21; 16];
    let mut container = container(&path, owner);
    let initial_len = fs::metadata(&path).unwrap().len();
    let mut epochs = ImportEpochStore::default();

    let malformed_key = start(owner, "not-a-canonical-import-key");
    assert!(epochs.append_start(&mut container, malformed_key).is_err());
    assert_eq!(fs::metadata(&path).unwrap().len(), initial_len);
    assert!(epochs.open().is_none());
    assert!(epochs.statuses().is_empty());
    assert!(container.append(b"admission remains open").is_ok());

    let mut wrong_owner = start(owner, &"a".repeat(64));
    wrong_owner.owner_uuid = [0x22; 16];
    wrong_owner.import_id = import_id(wrong_owner.owner_uuid, &wrong_owner.import_key);
    let before_wrong_owner = fs::metadata(&path).unwrap().len();
    assert!(epochs.append_start(&mut container, wrong_owner).is_err());
    assert_eq!(fs::metadata(&path).unwrap().len(), before_wrong_owner);
    assert!(epochs.open().is_none());
    assert!(epochs.statuses().is_empty());
    drop(container);
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn replay_rejects_noncanonical_start_identity() {
    let owner = [0x31; 16];
    let malformed = start(owner, "short");
    let payload = super::super::epoch_codec::encode_start(&malformed).unwrap();
    let mut epochs = ImportEpochStore::default();
    assert!(epochs.ingest(&payload).is_err());
    assert!(epochs.open().is_none());
    assert!(epochs.statuses().is_empty());
}

#[test]
fn replay_rejects_invalid_source_text() {
    let owner = [0x41; 16];
    let key = "c".repeat(64);
    let mut malformed = start(owner, &key);
    malformed.source_namespace = "bad\0namespace".into();
    let payload = super::super::epoch_codec::encode_start(&malformed).unwrap();
    let mut epochs = ImportEpochStore::default();
    assert!(epochs.ingest(&payload).is_err());
    assert!(epochs.open().is_none());
}
