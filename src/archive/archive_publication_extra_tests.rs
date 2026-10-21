use super::{ArchivePublication, decode, encode};
use crate::archive_codec::ArchiveRecord;
use crate::imports::ImportReceipt;
use crate::{
    ArchiveError, Episode, EpisodeBoundary, EpisodeId, EpisodeOrigin, FileId, FileMemoryLink,
    MemoryId,
};

fn episode() -> Episode {
    Episode {
        id: EpisodeId([0xA5; 32]),
        conversation_id: "conv-é".into(),
        start_node_id: "start-01".into(),
        end_node_id: "end-02".into(),
        origin: EpisodeOrigin::Import,
        boundary: EpisodeBoundary::ImportEnd,
        source_through_ns: -123_456_789,
        finalized_at_ns: 987_654_321,
    }
}

fn receipt() -> ImportReceipt {
    ImportReceipt {
        schema_version: 1,
        import_key: "ab".repeat(32),
        source_namespace: "provider/account".into(),
        source_format: "native-json".into(),
        conversations: vec![],
    }
}

fn append_string(out: &mut Vec<u8>, value: &str) {
    out.extend_from_slice(&(value.len() as u32).to_le_bytes());
    out.extend_from_slice(value.as_bytes());
}

#[test]
fn episode_publication_fields_keep_tags_times_and_literal_text() {
    let value = episode();
    let bytes = encode(&ArchivePublication::Record {
        archive_version: 19,
        record: ArchiveRecord::Episode(value.clone()),
    })
    .unwrap();
    let mut expected = b"CVAEPI02".to_vec();
    expected.extend_from_slice(&19_u64.to_le_bytes());
    expected.extend_from_slice(&[0xA5; 32]);
    expected.extend_from_slice(&[2, 4]);
    expected.extend_from_slice(&(-123_456_789_i64).to_le_bytes());
    expected.extend_from_slice(&987_654_321_i64.to_le_bytes());
    append_string(&mut expected, "conv-é");
    append_string(&mut expected, "start-01");
    append_string(&mut expected, "end-02");
    assert_eq!(bytes, expected);
    assert!(matches!(
        decode(&bytes, 700).unwrap(),
        Some(ArchivePublication::Record {
            archive_version: 19,
            record: ArchiveRecord::Episode(decoded),
        }) if decoded == value
    ));
}

#[test]
fn import_receipt_publication_uses_exact_json_and_rejects_bad_identity() {
    let value = receipt();
    let bytes = encode(&ArchivePublication::Record {
        archive_version: 7,
        record: ArchiveRecord::ImportReceipt(value.clone()),
    })
    .unwrap();
    let expected_json = format!(
        r#"{{"schema_version":1,"import_key":"{}","source_namespace":"provider/account","source_format":"native-json","conversations":[]}}"#,
        "ab".repeat(32)
    );
    let mut expected = b"CVAIMP02".to_vec();
    expected.extend_from_slice(&7_u64.to_le_bytes());
    expected.extend_from_slice(expected_json.as_bytes());
    assert_eq!(bytes, expected);
    assert!(matches!(
        decode(&bytes, 701).unwrap(),
        Some(ArchivePublication::Record {
            archive_version: 7,
            record: ArchiveRecord::ImportReceipt(decoded),
        }) if decoded == value
    ));

    for invalid in [
        ImportReceipt {
            schema_version: 2,
            ..receipt()
        },
        ImportReceipt {
            import_key: "z".repeat(64),
            ..receipt()
        },
        ImportReceipt {
            import_key: "a".repeat(63),
            ..receipt()
        },
        ImportReceipt {
            source_namespace: String::new(),
            ..receipt()
        },
        ImportReceipt {
            source_format: String::new(),
            ..receipt()
        },
    ] {
        assert!(
            encode(&ArchivePublication::Record {
                archive_version: 1,
                record: ArchiveRecord::ImportReceipt(invalid),
            })
            .is_err()
        );
    }

    let invalid_json = format!(
        r#"{{"schema_version":2,"import_key":"{}","source_namespace":"n","source_format":"f","conversations":[]}}"#,
        "ab".repeat(32)
    );
    let mut invalid_body = b"CVAIMP02".to_vec();
    invalid_body.extend_from_slice(&1_u64.to_le_bytes());
    invalid_body.extend_from_slice(invalid_json.as_bytes());
    assert!(decode(&invalid_body, 702).is_err());
    let mut trailing_json = bytes.clone();
    trailing_json.push(b'x');
    assert!(decode(&trailing_json, 702).is_err());
    let mut unknown_version = bytes;
    unknown_version[..8].copy_from_slice(b"CVAIMP03");
    assert!(decode(&unknown_version, 702).is_err());
}

#[test]
fn file_memory_publication_has_fixed_literal_body_and_rejects_truncation() {
    let value = FileMemoryLink {
        file_id: FileId([0x11; 32]),
        memory_id: MemoryId([0x22; 32]),
    };
    let bytes = encode(&ArchivePublication::Record {
        archive_version: 5,
        record: ArchiveRecord::FileMemoryLink(value),
    })
    .unwrap();
    assert_eq!(bytes.len(), 80);
    assert_eq!(&bytes[..8], b"CVAFME02");
    assert_eq!(&bytes[8..16], &5_u64.to_le_bytes());
    assert_eq!(&bytes[16..48], &[0x11; 32]);
    assert_eq!(&bytes[48..80], &[0x22; 32]);
    assert!(matches!(
        decode(&bytes, 703).unwrap(),
        Some(ArchivePublication::Record {
            archive_version: 5,
            record: ArchiveRecord::FileMemoryLink(decoded),
        }) if decoded == value
    ));
    assert!(decode(&bytes[..79], 703).is_err());
}

#[test]
fn archive_publication_rejects_episode_trailing_and_unknown_family_version() {
    let valid = encode(&ArchivePublication::Record {
        archive_version: 1,
        record: ArchiveRecord::Episode(episode()),
    })
    .unwrap();
    let mut trailing = valid.clone();
    trailing.push(0);
    assert!(decode(&trailing, 704).is_err());
    let mut unknown_version = valid;
    unknown_version[..8].copy_from_slice(b"CVAEPI03");
    assert!(matches!(
        decode(&unknown_version, 704),
        Err(ArchiveError::CorruptRecord(
            "unsupported Archive publication version"
        ))
    ));
    assert!(decode(b"arbitrary body", 704).unwrap().is_none());
}
