use super::*;
use crate::ImportWatermarks;
use reliquary_container::{ContainerIdentity, FileKind};
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
    let dir = std::env::current_dir()
        .unwrap()
        .join(format!(".epoch-start-{}", std::process::id()));
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
    let payload = crate::codec::encode_start(&malformed).unwrap();
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
    let payload = crate::codec::encode_start(&malformed).unwrap();
    let mut epochs = ImportEpochStore::default();
    assert!(epochs.ingest(&payload).is_err());
    assert!(epochs.open().is_none());
}
#[test]
fn replay_and_completion_validation_fail_closed() {
    let owner = [3; 16];
    let start = start(owner, &"a".repeat(64));
    let mut store = ImportEpochStore::default();
    let start_bytes = crate::codec::encode_start(&start).unwrap();
    let mut complete = ImportComplete {
        owner_uuid: owner,
        import_id: start.import_id.clone(),
        start_digest: format!(
            "{:x}",
            sha2::Sha256::digest(serde_json::to_vec(&start).unwrap())
        ),
        receipt_digest: "receipt".into(),
        completed_at_ns: start.started_at_ns,
        post: start.pre,
        historical_freshness_settlement_digest: "settlement".into(),
    };
    assert!(
        store
            .ingest(&crate::codec::encode_complete(&complete).unwrap())
            .is_err()
    );
    store.ingest(&start_bytes).unwrap();
    assert!(store.ingest(&start_bytes).is_err());
    let other = self::start(owner, &"b".repeat(64));
    assert!(
        store
            .ingest(&crate::codec::encode_start(&other).unwrap())
            .is_err()
    );
    assert!(store.validate_owner(Some([4; 16])).is_err());
    let mut bad = complete.clone();
    bad.start_digest = "wrong".into();
    assert!(store.check_complete(&bad).is_err());
    bad = complete.clone();
    bad.historical_freshness_settlement_digest.clear();
    assert!(store.check_complete(&bad).is_err());
    bad = complete.clone();
    bad.completed_at_ns -= 1;
    assert!(store.check_complete(&bad).is_err());
    assert!(store.check_complete(&complete).unwrap());
    store
        .ingest(&crate::codec::encode_complete(&complete).unwrap())
        .unwrap();
    assert!(!store.check_complete(&complete).unwrap());
    complete.receipt_digest = "conflict".into();
    assert!(store.check_complete(&complete).is_err());
    assert!(
        store
            .validate_receipts(&std::collections::BTreeMap::new())
            .is_err()
    );
}
#[test]
fn completion_rejects_regressed_watermarks() {
    let owner = [8; 16];
    let mut epoch = start(owner, &"c".repeat(64));
    epoch.pre.global_version = 2;
    let mut store = ImportEpochStore::default();
    store
        .ingest(&crate::codec::encode_start(&epoch).unwrap())
        .unwrap();
    let mut complete = ImportComplete {
        owner_uuid: owner,
        import_id: epoch.import_id.clone(),
        start_digest: format!(
            "{:x}",
            sha2::Sha256::digest(serde_json::to_vec(&epoch).unwrap())
        ),
        receipt_digest: "receipt".into(),
        completed_at_ns: epoch.started_at_ns,
        post: epoch.pre,
        historical_freshness_settlement_digest: "settlement".into(),
    };
    complete.post.global_version -= 1;
    assert!(store.check_complete(&complete).is_err());
    assert!(store.open().is_some());
}
