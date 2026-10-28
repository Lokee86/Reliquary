use super::*;
use std::fs;
use uuid::Uuid;

fn path(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("switchboard-config-{name}-{}", Uuid::new_v4()));
    fs::create_dir_all(&dir).unwrap();
    dir.join("reliquary.cfg")
}

#[test]
fn framed_document_bytes_round_trip_and_preserve_unknown_objects() {
    let mut document = ConfigDocument::new();
    document.insert(
        "future.object",
        ConfigObject {
            schema: 41,
            flags: 7,
            payload: vec![0, 255, 1, 2],
        },
    );
    let encoded = encode_config_document(&document).unwrap();
    assert_eq!(&encoded[..8], b"CVCFG\0\r\n");
    let reopened = decode_config_document(&encoded).unwrap();
    assert_eq!(reopened, document);
    assert_eq!(encode_config_document(&reopened).unwrap(), encoded);
}

#[test]
fn malformed_documents_fail_closed() {
    assert!(matches!(
        decode_config_document(b"short"),
        Err(ConfigDocumentError::Truncated)
    ));
    let mut bytes = encode_config_document(&ConfigDocument::new()).unwrap();
    bytes[0] ^= 1;
    assert!(matches!(
        decode_config_document(&bytes),
        Err(ConfigDocumentError::InvalidMagic)
    ));
}

#[test]
fn replacement_writes_a_current_document_at_the_same_path() {
    let path = path("replace");
    let mut first = ConfigDocument::new();
    first.insert(
        "unknown",
        ConfigObject {
            schema: 9,
            flags: 3,
            payload: vec![4, 5, 6],
        },
    );
    replace_config_document(&path, &first).unwrap();
    let mut second = first.clone();
    second.insert(
        "known",
        ConfigObject {
            schema: 1,
            flags: 0,
            payload: vec![8],
        },
    );
    replace_config_document(&path, &second).unwrap();
    assert_eq!(read_config_document(&path).unwrap(), second);
}
#[test]
fn duplicate_and_unsupported_version_are_rejected() {
    let mut document = ConfigDocument::new();
    document.insert(
        "x",
        ConfigObject {
            schema: 1,
            flags: 0,
            payload: vec![9],
        },
    );
    let bytes = encode_config_document(&document).unwrap();
    let mut duplicate = bytes.clone();
    duplicate[12..16].copy_from_slice(&2_u32.to_le_bytes());
    duplicate.extend_from_slice(&bytes[16..]);
    assert!(matches!(
        decode_config_document(&duplicate),
        Err(ConfigDocumentError::DuplicateObject(_))
    ));
    let mut version = bytes.clone();
    version[8..10].copy_from_slice(&2_u16.to_le_bytes());
    assert!(matches!(
        decode_config_document(&version),
        Err(ConfigDocumentError::UnsupportedVersion { .. })
    ));
    assert!(matches!(
        decode_config_document(&bytes[..bytes.len() - 1]),
        Err(ConfigDocumentError::Truncated)
    ));
}

#[test]
fn replacement_failure_preserves_existing_destination() {
    let target = path("failure");
    fs::create_dir(&target).unwrap();
    let marker = target.join("marker");
    fs::write(&marker, b"unchanged").unwrap();
    assert!(replace_config_document(&target, &ConfigDocument::new()).is_err());
    assert_eq!(fs::read(marker).unwrap(), b"unchanged");
}
