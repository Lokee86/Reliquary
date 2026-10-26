use super::*;
use sha2::Digest;
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
