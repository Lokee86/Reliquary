use crate::{JsonMasterKeyStore, MASTER_KEY_BYTES, MasterKeyStore};
use std::fs;
use std::path::PathBuf;

fn test_path(name: &str) -> PathBuf {
    let unique = uuid::Uuid::new_v4();
    let dir = std::env::temp_dir().join(format!("continuity-master-key-{unique}"));
    fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

#[test]
fn json_store_generates_and_reloads_one_stable_key() {
    let path = test_path("master-key.json");
    let store = JsonMasterKeyStore::new(&path);
    let first = store.load_or_create().unwrap();
    let second = store.load_or_create().unwrap();

    assert_eq!(first, second);
    assert_eq!(first.bytes().len(), MASTER_KEY_BYTES);
    assert_eq!(store.path(), path.as_path());

    let json: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    assert_eq!(json["version"], 1);
    assert_eq!(json["master_key_hex"].as_str().unwrap().len(), 64);
}

#[test]
fn separately_created_stores_get_different_keys() {
    let left = JsonMasterKeyStore::new(test_path("left.json"));
    let right = JsonMasterKeyStore::new(test_path("right.json"));
    assert_ne!(
        left.load_or_create().unwrap(),
        right.load_or_create().unwrap()
    );
}

#[test]
fn malformed_or_wrong_sized_keys_are_rejected() {
    let malformed = test_path("malformed.json");
    fs::write(&malformed, b"not json").unwrap();
    assert!(
        JsonMasterKeyStore::new(&malformed)
            .load_or_create()
            .is_err()
    );

    let short = test_path("short.json");
    fs::write(&short, br#"{"version":1,"master_key_hex":"abcd"}"#).unwrap();
    assert!(JsonMasterKeyStore::new(&short).load_or_create().is_err());
}

#[test]
fn debug_output_never_contains_key_material() {
    let store = JsonMasterKeyStore::new(test_path("debug.json"));
    let key = store.load_or_create().unwrap();
    assert_eq!(format!("{key:?}"), "MasterKey([REDACTED])");
}
