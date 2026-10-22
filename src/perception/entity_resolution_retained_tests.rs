use super::*;
use sha2::Digest;
use std::{fs, path::PathBuf};
fn scratch() -> PathBuf {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".test-tmp")
        .join(format!("retained-resolution-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&p).unwrap();
    p
}
#[test]
fn retained_resolution_codec_keeps_every_inert_byte_and_rejects_damage() {
    let mut original = b"CVAERS02".to_vec();
    original.extend((0..410).map(|v| (v % 256) as u8));
    let encoded = retained::encode(&original).unwrap();
    assert_eq!(
        retained::decode(&encoded).unwrap(),
        Some(original.as_slice())
    );
    assert_eq!(encoded.len(), original.len() + 44);
    let mut store = crate::entity_resolution_store::EntityResolutionStore::default();
    assert!(store.ingest(&original).is_err());
    store.ingest(&encoded).unwrap();
    assert!(store.records().is_empty());
    for end in 6..encoded.len() {
        assert!(retained::decode(&encoded[..end]).is_err());
    }
    for index in [7, 8, 12, encoded.len() - 1] {
        let mut bad = encoded.clone();
        bad[index] ^= 1;
        assert!(store.ingest(&bad).is_err());
        assert!(store.records().is_empty());
    }
    let mut bad = encoded.clone();
    bad[19] = b'3';
    let end = bad.len() - 32;
    let hash = sha2::Sha256::digest(&bad[..end]);
    bad[end..].copy_from_slice(&hash);
    assert!(retained::decode(&bad).is_err());
}

#[test]
fn retained_literal_evidence_opens_in_both_current_owner_kinds_without_projection() {
    let root = scratch();
    let encoded = retained::encode(b"CVAERS02exact inert evidence").unwrap();
    for phy in [false, true] {
        let path = root.join(format!("current-{phy}"));
        if phy {
            let mut owner = crate::Phylactery::create(&path).unwrap();
            owner.container.append(&encoded).unwrap();
            owner.sync().unwrap();
        } else {
            let mut owner = crate::Cva::create(&path).unwrap();
            owner.container.append(&encoded).unwrap();
            owner.sync().unwrap();
        }
        let before = fs::read(&path).unwrap();
        if phy {
            let owner = crate::Phylactery::open(&path).unwrap();
            assert!(owner.entity_resolution_records().is_empty());
        } else {
            let owner = crate::Cva::open(&path).unwrap();
            assert!(owner.entity_resolution_records().is_empty());
        }
        assert_eq!(fs::read(&path).unwrap(), before);
    }
}
