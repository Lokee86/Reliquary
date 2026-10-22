use super::super::ArchiveRecord;
use super::{ArchivePublication, decode, encode};
use crate::conversation_metadata_codec::{
    CONVERSATION_METADATA_MAGIC, decode_conversation_metadata, encode_conversation_metadata,
};
use crate::turn_ingest_codec::{INGESTED_TURN_MAGIC, decode_ingested_turn, encode_ingested_turn};
use crate::{
    ArchiveError, ContentId, ConversationMetadata, ConversationPolicy, FileId, IngestedTurn, Node,
    StoredFile,
};

fn metadata(
    active: bool,
    title: Option<&str>,
    policy: ConversationPolicy,
    policy_revision: u64,
) -> ConversationMetadata {
    ConversationMetadata {
        conversation_id: "c".into(),
        title: title.map(str::to_owned),
        active,
        policy,
        policy_revision,
    }
}

fn publication_metadata(bytes: &[u8], global: u64) -> (u64, ConversationMetadata) {
    match decode(bytes, global).unwrap().unwrap() {
        ArchivePublication::Record {
            archive_version,
            record: ArchiveRecord::ConversationMetadata(value),
        } => (archive_version, value),
        _ => panic!("expected Archive conversation-metadata publication"),
    }
}

fn append_string(out: &mut Vec<u8>, value: &str) {
    out.extend_from_slice(&(value.len() as u32).to_le_bytes());
    out.extend_from_slice(value.as_bytes());
}

fn turn(principal_id: Option<&str>) -> IngestedTurn {
    IngestedTurn {
        node: Node {
            id: "node-1".into(),
            conversation_id: "conversation-1".into(),
            parent_id: Some("parent-1".into()),
            role: "assistant".into(),
            principal_id: principal_id.map(str::to_owned),
            timestamp_ns: -123,
            content_id: ContentId([0x11; 32]),
        },
        attachments: vec![
            StoredFile {
                id: FileId([0x21; 32]),
                content_id: ContentId([0x31; 32]),
                filename: "first.bin".into(),
                mime_type: Some("application/octet-stream".into()),
                byte_length: 17,
            },
            StoredFile {
                id: FileId([0x22; 32]),
                content_id: ContentId([0x32; 32]),
                filename: "second.txt".into(),
                mime_type: None,
                byte_length: 29,
            },
        ],
    }
}

fn publication_turn(bytes: &[u8], global: u64) -> (u64, IngestedTurn) {
    match decode(bytes, global).unwrap().unwrap() {
        ArchivePublication::Record {
            archive_version,
            record: ArchiveRecord::IngestedTurn(value),
        } => (archive_version, value),
        _ => panic!("expected Archive ingested-turn publication"),
    }
}

fn append_stored_file(out: &mut Vec<u8>, file: &StoredFile) {
    out.extend_from_slice(&file.id.0);
    out.extend_from_slice(&file.content_id.0);
    out.extend_from_slice(&file.byte_length.to_le_bytes());
    append_string(out, &file.filename);
    append_string(out, file.mime_type.as_deref().unwrap_or(""));
}

#[test]
fn metadata_publication_uses_literal_header_and_round_trips_all_current_fields() {
    let value = metadata(true, Some("T"), ConversationPolicy::VisibleAutoBranch, 2);
    let bytes = encode(&ArchivePublication::Record {
        archive_version: 4,
        record: ArchiveRecord::ConversationMetadata(value.clone()),
    })
    .unwrap();
    let mut golden = b"CVACON04".to_vec();
    golden.extend_from_slice(&4_u64.to_le_bytes());
    append_string(&mut golden, "c");
    append_string(&mut golden, "T");
    golden.extend_from_slice(&[1, 2]);
    golden.extend_from_slice(&2_u64.to_le_bytes());
    assert_eq!(bytes, golden);
    assert_eq!(bytes.get(..8), Some(b"CVACON04".as_slice()));
    let (version, decoded) = publication_metadata(&bytes, 17);
    assert_eq!(version, 4);
    assert_eq!(decoded, value);

    for active in [false, true] {
        for title in [None, Some("title")] {
            for policy in [
                ConversationPolicy::SequentialChronological,
                ConversationPolicy::VisibleAutoBranch,
            ] {
                for policy_revision in [1, 2, 91] {
                    let expected = metadata(active, title, policy, policy_revision);
                    let bytes = encode(&ArchivePublication::Record {
                        archive_version: policy_revision,
                        record: ArchiveRecord::ConversationMetadata(expected.clone()),
                    })
                    .unwrap();
                    assert_eq!(publication_metadata(&bytes, 100).1, expected);
                }
            }
        }
    }
}

#[test]
fn current_metadata_comparison_wrapper_has_literal_golden_bytes() {
    let current = metadata(true, Some("T"), ConversationPolicy::VisibleAutoBranch, 2);
    let mut golden = CONVERSATION_METADATA_MAGIC.to_vec();
    append_string(&mut golden, "c");
    append_string(&mut golden, "T");
    golden.extend_from_slice(&[1, 2]);
    golden.extend_from_slice(&2_u64.to_le_bytes());
    assert_eq!(encode_conversation_metadata(&current).unwrap(), golden);
    assert_eq!(decode_conversation_metadata(&golden).unwrap(), current);
}

#[test]
fn metadata_publication_rejects_unknown_flags_zero_revisions_and_bad_framing() {
    let valid = encode(&ArchivePublication::Record {
        archive_version: 1,
        record: ArchiveRecord::ConversationMetadata(metadata(
            true,
            Some("T"),
            ConversationPolicy::VisibleAutoBranch,
            1,
        )),
    })
    .unwrap();
    for end in 6..valid.len() {
        assert!(decode(&valid[..end], 1).is_err(), "truncated at {end}");
    }
    let mut trailing = valid.clone();
    trailing.push(0);
    assert!(decode(&trailing, 1).is_err());

    let mut active_flag = valid.clone();
    active_flag[26] = 2;
    assert!(decode(&active_flag, 1).is_err());
    let mut policy_tag = valid.clone();
    policy_tag[27] = 99;
    assert!(decode(&policy_tag, 1).is_err());
    let mut zero_revision = valid.clone();
    let revision_start = zero_revision.len() - 8;
    zero_revision[revision_start..].fill(0);
    assert!(decode(&zero_revision, 1).is_err());
    let mut zero_local = valid.clone();
    zero_local[8..16].fill(0);
    assert!(decode(&zero_local, 1).is_err());
    assert!(decode(&valid, 0).is_err());

    let mut unsupported = valid.clone();
    unsupported[..8].copy_from_slice(b"CVACON99");
    assert!(decode(&unsupported, 1).is_err());
    assert!(matches!(
        encode(&ArchivePublication::Record {
            archive_version: 1,
            record: ArchiveRecord::ConversationMetadata(metadata(
                false,
                None,
                ConversationPolicy::SequentialChronological,
                0,
            )),
        }),
        Err(ArchiveError::CorruptRecord(
            "zero conversation policy revision"
        ))
    ));
}

#[test]
fn ingested_turn_publication_preserves_principal_and_ordered_attachment_fields() {
    for principal_id in [None, Some("principal-1")] {
        let expected = turn(principal_id);
        let bytes = encode(&ArchivePublication::Record {
            archive_version: 7,
            record: ArchiveRecord::IngestedTurn(expected.clone()),
        })
        .unwrap();
        assert_eq!(bytes.get(..8), Some(b"CVATUR03".as_slice()));
        assert_eq!(&bytes[8..16], &7_u64.to_le_bytes());
        let (archive_version, decoded) = publication_turn(&bytes, 31);
        assert_eq!(archive_version, 7);
        assert_eq!(decoded, expected);
        assert_eq!(decoded.attachments[0].filename, "first.bin");
        assert_eq!(
            decoded.attachments[0].mime_type.as_deref(),
            Some("application/octet-stream")
        );
        assert_eq!(decoded.attachments[1].filename, "second.txt");
        assert_eq!(decoded.attachments[1].mime_type, None);
    }

    let expected = turn(None);
    let bytes = encode(&ArchivePublication::Record {
        archive_version: 7,
        record: ArchiveRecord::IngestedTurn(expected.clone()),
    })
    .unwrap();
    let mut golden = b"CVATUR03".to_vec();
    golden.extend_from_slice(&7_u64.to_le_bytes());
    golden.extend_from_slice(&(-123_i64).to_le_bytes());
    golden.extend_from_slice(&[0x11; 32]);
    append_string(&mut golden, "node-1");
    append_string(&mut golden, "conversation-1");
    append_string(&mut golden, "parent-1");
    append_string(&mut golden, "assistant");
    append_string(&mut golden, "");
    golden.extend_from_slice(&2_u32.to_le_bytes());
    append_stored_file(&mut golden, &expected.attachments[0]);
    append_stored_file(&mut golden, &expected.attachments[1]);
    assert_eq!(bytes, golden);
}

#[test]
fn current_ingested_turn_comparison_wrapper_has_literal_golden_bytes() {
    let expected = IngestedTurn {
        node: Node {
            id: "n".into(),
            conversation_id: "c".into(),
            parent_id: None,
            role: "user".into(),
            principal_id: None,
            timestamp_ns: 1,
            content_id: ContentId([3; 32]),
        },
        attachments: vec![],
    };
    let mut v2_golden = INGESTED_TURN_MAGIC.to_vec();
    v2_golden.extend_from_slice(&1_i64.to_le_bytes());
    v2_golden.extend_from_slice(&[3; 32]);
    append_string(&mut v2_golden, "n");
    append_string(&mut v2_golden, "c");
    append_string(&mut v2_golden, "");
    append_string(&mut v2_golden, "user");
    append_string(&mut v2_golden, "");
    v2_golden.extend_from_slice(&0_u32.to_le_bytes());
    assert_eq!(encode_ingested_turn(&expected).unwrap(), v2_golden);
    assert_eq!(decode_ingested_turn(&v2_golden).unwrap(), expected);

    let current = encode(&ArchivePublication::Record {
        archive_version: 1,
        record: ArchiveRecord::IngestedTurn(expected.clone()),
    })
    .unwrap();
    assert_eq!(current.get(..8), Some(b"CVATUR03".as_slice()));
    assert!(matches!(
        decode(&current, 2).unwrap(),
        Some(ArchivePublication::Record {
            archive_version: 1,
            record: ArchiveRecord::IngestedTurn(canonicalized),
        }) if canonicalized == expected && canonicalized.node.principal_id.is_none()
    ));
}

#[test]
fn ingested_turn_publication_rejects_bad_tags_truncation_and_trailing_bytes() {
    let valid = encode(&ArchivePublication::Record {
        archive_version: 2,
        record: ArchiveRecord::IngestedTurn(turn(None)),
    })
    .unwrap();
    for end in 6..valid.len() {
        assert!(decode(&valid[..end], 1).is_err(), "truncated at {end}");
    }
    let mut trailing = valid.clone();
    trailing.push(0);
    assert!(decode(&trailing, 1).is_err());
    let mut unknown_tag = valid;
    unknown_tag[..8].copy_from_slice(b"CVATUR99");
    assert!(decode(&unknown_tag, 1).is_err());
}
