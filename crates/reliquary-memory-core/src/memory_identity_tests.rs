use crate::memory_codec_values::{read_optional_source_ref, write_optional_source_ref};
use crate::memory_model::{memory_body_bytes, memory_body_id, memory_id};
use crate::{MemorySourceRef, SourceEpisodeId};

#[test]
fn memory_identity_and_body_bytes_match_current_literal_vectors() {
    assert_eq!(
        memory_id("identity-vector-1").0,
        [
            0x76, 0xab, 0xfa, 0xff, 0x44, 0x29, 0xdd, 0x7c, 0xc8, 0x4f, 0xfd, 0x53, 0xe4, 0x73,
            0x57, 0x65, 0xe9, 0x62, 0xd7, 0x06, 0x42, 0xd6, 0x78, 0x50, 0xfd, 0x59, 0x2b, 0xcc,
            0x34, 0x65, 0x49, 0x51,
        ]
    );
    assert_eq!(
        memory_body_bytes("a", "bc"),
        [
            1_u64.to_le_bytes().as_slice(),
            b"a".as_slice(),
            2_u64.to_le_bytes().as_slice(),
            b"bc".as_slice()
        ]
        .concat()
    );
    assert_eq!(
        memory_body_id("a", "bc").0,
        [
            0x9a, 0x8a, 0xcc, 0xa1, 0xb6, 0xc6, 0xc0, 0xbe, 0xfd, 0x3f, 0xbc, 0x75, 0x6a, 0xed,
            0x62, 0x5d, 0xa9, 0x98, 0xc9, 0x98, 0xf7, 0x25, 0x2e, 0x73, 0x8c, 0x4e, 0xf0, 0x61,
            0x90, 0x6b, 0x9b, 0x21,
        ]
    );
}

#[test]
fn foreign_episode_reference_round_trips_as_the_same_32_persisted_bytes() {
    let episode = SourceEpisodeId(std::array::from_fn(|index| 0xa0 + index as u8));
    let source = MemorySourceRef {
        owner_id: "owner".into(),
        principal_id: None,
        source_episode_id: episode,
        source_node_id: "node".into(),
        content_source_conversation_id: None,
        content_source_node_id: None,
        grounding_source_conversation_id: None,
        grounding_source_node_id: None,
    };
    let mut bytes = Vec::new();
    write_optional_source_ref(&mut bytes, Some(&source)).unwrap();
    assert_eq!(&bytes[11..43], &(0xa0_u8..=0xbf).collect::<Vec<_>>());
    let mut cursor = 0;
    let decoded = read_optional_source_ref(&bytes, &mut cursor, true)
        .unwrap()
        .unwrap();
    assert_eq!(cursor, bytes.len());
    assert_eq!(
        decoded.source_episode_id.0,
        (0xa0_u8..=0xbf).collect::<Vec<_>>().as_slice()
    );
    assert_eq!(decoded, source);
}
