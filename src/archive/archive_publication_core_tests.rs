use crate::archive_codec::ArchiveRecord;
use crate::archive_codec::publication::{ArchivePublication, decode, encode};
use crate::{
    Branch, ContentId, ConversationTombstone, ConversationTombstoneClosure, FileId, Fragment,
    FragmentId, Node, StoredFile,
};

fn string_field(out: &mut Vec<u8>, value: &str) {
    out.extend_from_slice(&(value.len() as u32).to_le_bytes());
    out.extend_from_slice(value.as_bytes());
}

fn record(archive_version: u64, value: ArchiveRecord) -> ArchivePublication {
    ArchivePublication::Record {
        archive_version,
        record: value,
    }
}

#[test]
fn node_and_branch_envelopes_have_literal_owner_local_fields_and_no_global() {
    let node = Node {
        id: "n".into(),
        conversation_id: "c".into(),
        parent_id: Some("p".into()),
        role: "user".into(),
        principal_id: Some("u".into()),
        timestamp_ns: -2,
        content_id: ContentId([0x11; 32]),
    };
    let body = encode(&record(7, ArchiveRecord::Node(node.clone()))).unwrap();
    let mut expected = b"CVANOD02".to_vec();
    expected.extend_from_slice(&7_u64.to_le_bytes());
    expected.extend_from_slice(&(-2_i64).to_le_bytes());
    expected.extend_from_slice(&[0x11; 32]);
    for field in ["n", "c", "p", "user", "u"] {
        string_field(&mut expected, field);
    }
    assert_eq!(body, expected);
    match decode(&body, 99).unwrap().unwrap() {
        ArchivePublication::Record {
            archive_version,
            record: ArchiveRecord::Node(decoded),
        } => {
            assert_eq!(archive_version, 7);
            assert_eq!(decoded, node);
        }
        _ => panic!("expected Node publication"),
    }

    let branch = Branch {
        id: "b".into(),
        conversation_id: "c".into(),
        leaf_node_id: "n".into(),
        canonical: true,
    };
    let body = encode(&record(8, ArchiveRecord::Branch(branch.clone()))).unwrap();
    let mut expected = b"CVABRC02".to_vec();
    expected.extend_from_slice(&8_u64.to_le_bytes());
    expected.push(1);
    for field in ["b", "c", "n"] {
        string_field(&mut expected, field);
    }
    assert_eq!(body, expected);
    match decode(&body, 100).unwrap().unwrap() {
        ArchivePublication::Record {
            archive_version,
            record: ArchiveRecord::Branch(decoded),
        } => {
            assert_eq!(archive_version, 8);
            assert_eq!(decoded, branch);
        }
        _ => panic!("expected Branch publication"),
    }
}

#[test]
fn fragment_file_and_tombstone_roundtrip_and_tombstone_uses_decode_global() {
    let fragment = Fragment {
        id: FragmentId([0x22; 32]),
        conversation_id: "c".into(),
        start_node_id: "a".into(),
        end_node_id: "z".into(),
    };
    let body = encode(&record(3, ArchiveRecord::Fragment(fragment.clone()))).unwrap();
    let mut expected = b"CVAFRA02".to_vec();
    expected.extend_from_slice(&3_u64.to_le_bytes());
    expected.extend_from_slice(&[0x22; 32]);
    for field in ["c", "a", "z"] {
        string_field(&mut expected, field);
    }
    assert_eq!(body, expected);
    match decode(&body, 41).unwrap().unwrap() {
        ArchivePublication::Record {
            archive_version: 3,
            record: ArchiveRecord::Fragment(decoded),
        } => assert_eq!(decoded, fragment),
        _ => panic!("expected Fragment publication"),
    }

    let file = StoredFile {
        id: FileId([0x33; 32]),
        content_id: ContentId([0x44; 32]),
        filename: "f.txt".into(),
        mime_type: Some("text/plain".into()),
        byte_length: 12,
    };
    let body = encode(&record(4, ArchiveRecord::File(file.clone()))).unwrap();
    let mut expected = b"CVAFIL02".to_vec();
    expected.extend_from_slice(&4_u64.to_le_bytes());
    expected.extend_from_slice(&[0x33; 32]);
    expected.extend_from_slice(&[0x44; 32]);
    expected.extend_from_slice(&12_u64.to_le_bytes());
    string_field(&mut expected, "f.txt");
    string_field(&mut expected, "text/plain");
    assert_eq!(body, expected);
    match decode(&body, 42).unwrap().unwrap() {
        ArchivePublication::Record {
            archive_version: 4,
            record: ArchiveRecord::File(decoded),
        } => assert_eq!(decoded, file),
        _ => panic!("expected File publication"),
    }

    let tombstone = ConversationTombstone {
        conversation_id: "c".into(),
        closure: ConversationTombstoneClosure {
            node_ids: vec!["n".into()],
            branch_ids: vec!["b".into()],
            fragment_ids: vec![FragmentId([0x55; 32])],
            episode_ids: vec![crate::EpisodeId([0x66; 32])],
            file_ids: vec![FileId([0x77; 32])],
            metadata_archive_versions: vec![2, 5],
        },
        archive_version: 9,
        global_version: 17,
    };
    let body = encode(&record(
        9,
        ArchiveRecord::ConversationTombstone(tombstone.clone()),
    ))
    .unwrap();
    let mut expected = b"CVACTM02".to_vec();
    expected.extend_from_slice(&9_u64.to_le_bytes());
    string_field(&mut expected, "c");
    for values in [&["n"][..], &["b"][..]] {
        expected.extend_from_slice(&(values.len() as u32).to_le_bytes());
        for value in values {
            string_field(&mut expected, value);
        }
    }
    expected.extend_from_slice(&1_u32.to_le_bytes());
    expected.extend_from_slice(&[0x55; 32]);
    expected.extend_from_slice(&1_u32.to_le_bytes());
    expected.extend_from_slice(&[0x66; 32]);
    expected.extend_from_slice(&1_u32.to_le_bytes());
    expected.extend_from_slice(&[0x77; 32]);
    expected.extend_from_slice(&2_u32.to_le_bytes());
    expected.extend_from_slice(&2_u64.to_le_bytes());
    expected.extend_from_slice(&5_u64.to_le_bytes());
    assert_eq!(body, expected);
    match decode(&body, 101).unwrap().unwrap() {
        ArchivePublication::Record {
            archive_version: 9,
            record: ArchiveRecord::ConversationTombstone(decoded),
        } => {
            assert_eq!(decoded.archive_version, 9);
            assert_eq!(decoded.global_version, 101);
            assert_eq!(decoded.conversation_id, tombstone.conversation_id);
            assert_eq!(decoded.closure, tombstone.closure);
        }
        _ => panic!("expected tombstone publication"),
    }
}

#[test]
fn repeat_body_has_explicit_local_and_first_applying_local() {
    let publication = ArchivePublication::Repeat {
        archive_version: 12,
        first_applying_archive_version: 4,
    };
    let body = encode(&publication).unwrap();
    let mut expected = b"CVAARP02".to_vec();
    expected.extend_from_slice(&12_u64.to_le_bytes());
    expected.extend_from_slice(&4_u64.to_le_bytes());
    assert_eq!(body, expected);
    match decode(&body, 65).unwrap().unwrap() {
        ArchivePublication::Repeat {
            archive_version,
            first_applying_archive_version,
        } => {
            assert_eq!(archive_version, 12);
            assert_eq!(first_applying_archive_version, 4);
        }
        _ => panic!("expected repeat publication"),
    }
}

#[test]
fn codec_rejects_malformed_or_unsupported_archive_bodies() {
    let node = Node {
        id: "n".into(),
        conversation_id: "c".into(),
        parent_id: None,
        role: "user".into(),
        principal_id: None,
        timestamp_ns: 1,
        content_id: ContentId([0; 32]),
    };
    let body = encode(&record(1, ArchiveRecord::Node(node))).unwrap();
    for prefix_len in [6, 7] {
        assert!(decode(&body[..prefix_len], 1).is_err());
    }
    let mut trailing = body.clone();
    trailing.push(0);
    assert!(decode(&trailing, 1).is_err());
    let mut unknown = body.clone();
    unknown[7] = b'3';
    assert!(decode(&unknown, 1).is_err());
    assert!(decode(&body, 0).is_err());
    let mut zero_local = body.clone();
    zero_local[8..16].copy_from_slice(&0_u64.to_le_bytes());
    assert!(decode(&zero_local, 1).is_err());

    let mut no_principal = b"CVANOD02".to_vec();
    no_principal.extend_from_slice(&2_u64.to_le_bytes());
    no_principal.extend_from_slice(&1_i64.to_le_bytes());
    no_principal.extend_from_slice(&[0; 32]);
    for field in ["n", "c", "", "user", ""] {
        string_field(&mut no_principal, field);
    }
    match decode(&no_principal, 1).unwrap().unwrap() {
        ArchivePublication::Record {
            record: ArchiveRecord::Node(decoded),
            ..
        } => {
            assert_eq!(decoded.principal_id, None);
            assert_eq!(decoded.parent_id, None);
        }
        _ => panic!("expected Node publication"),
    }

    let legacy_decoded = crate::Node {
        id: "literal-none".into(),
        conversation_id: "c".into(),
        parent_id: None,
        role: "user".into(),
        principal_id: None,
        timestamp_ns: 1,
        content_id: crate::ContentId([0xA5; 32]),
    };
    let canonical = encode(&record(3, ArchiveRecord::Node(legacy_decoded))).unwrap();
    match decode(&canonical, 1).unwrap().unwrap() {
        ArchivePublication::Record {
            record: ArchiveRecord::Node(decoded),
            ..
        } => {
            assert_eq!(decoded.principal_id, None);
        }
        _ => panic!("expected canonicalized Node publication"),
    }

    let mut malformed_branch = b"CVABRC02".to_vec();
    malformed_branch.extend_from_slice(&1_u64.to_le_bytes());
    malformed_branch.push(2);
    assert!(decode(&malformed_branch, 1).is_err());

    for first in [0, 5, 6] {
        let repeat = ArchivePublication::Repeat {
            archive_version: 5,
            first_applying_archive_version: first,
        };
        assert!(encode(&repeat).is_err());
        let mut bytes = b"CVAARP02".to_vec();
        bytes.extend_from_slice(&5_u64.to_le_bytes());
        bytes.extend_from_slice(&first.to_le_bytes());
        assert!(decode(&bytes, 1).is_err());
    }
    assert!(decode(b"CVACONT1", 1).is_err());
    assert!(decode(b"CVAAFMT2", 1).is_err());
}

#[test]
fn standalone_node_and_branch_wrappers_keep_their_legacy_bytes() {
    let node = Node {
        id: "n".into(),
        conversation_id: "c".into(),
        parent_id: None,
        role: "user".into(),
        principal_id: None,
        timestamp_ns: -1,
        content_id: ContentId([0xAB; 32]),
    };
    let mut expected = b"CVANODE2".to_vec();
    expected.extend_from_slice(&(-1_i64).to_le_bytes());
    expected.extend_from_slice(&[0xAB; 32]);
    for field in ["n", "c", "", "user", ""] {
        string_field(&mut expected, field);
    }
    assert_eq!(crate::archive_codec::encode_node(&node).unwrap(), expected);

    let branch = Branch {
        id: "b".into(),
        conversation_id: "c".into(),
        leaf_node_id: "n".into(),
        canonical: false,
    };
    let mut expected = b"CVABRCH1".to_vec();
    expected.push(0);
    for field in ["b", "c", "n"] {
        string_field(&mut expected, field);
    }
    assert_eq!(
        crate::archive_codec::encode_branch(&branch).unwrap(),
        expected
    );
}
