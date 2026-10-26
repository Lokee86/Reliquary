use super::*;
use crate::{GraphRelationKind, GraphRelationOrigin, SemanticGraphRelationKind};

fn row(
    source: u32,
    target: u32,
    active: bool,
    origin: GraphRelationOrigin,
) -> GraphOrdinalMutation {
    GraphOrdinalMutation {
        source,
        target,
        kind: SemanticGraphRelationKind::Memory(GraphRelationKind::Factual),
        active,
        origin,
    }
}

#[test]
fn coordinate_adapter_uses_envelope_self_reference_and_ignores_retired_links() {
    let object = reliquary_container::ObjectRef::from_legacy_bytes([7; 16]);
    let carrier = reliquary_container::ObjectRef::from_legacy_bytes([3; 16]);
    let body = encode(&GraphPublication {
        graph_version: 4,
        transaction: GraphTransaction::Single(row(0, 1, true, GraphRelationOrigin::Dream)),
    })
    .unwrap();
    let framed = reliquary_container::publication_envelope::encode(
        reliquary_container::publication_envelope::PublicationEnvelope {
            global_version: 9,
            clock: reliquary_container::publication_envelope::PublicationClock::Allocate {
                transaction_time_ns: None,
            },
            body: &body,
        },
    )
    .unwrap();
    let version = version_at(object, &framed).unwrap().unwrap();
    assert_eq!(version.global_version, 9);
    assert_eq!(version.graph_version, 4);
    assert_eq!(version.mutation, object);
    let old = crate::graph_codec::GraphVersionPayload {
        global_version: 9,
        graph_version: 4,
        mutation: carrier,
    };
    assert!(
        version_at(object, &crate::graph_codec::encode_version(old))
            .unwrap()
            .is_none()
    );
    assert!(version_at(object, b"unrelated").unwrap().is_none());
}

#[test]
fn single_literal_has_local_and_ordinal_fields_without_global_or_old_wrapper() {
    let value = row(0, 9, true, GraphRelationOrigin::Dream);
    let publication = GraphPublication {
        graph_version: 7,
        transaction: GraphTransaction::Single(value),
    };
    let mut expected = b"CVAGCM02".to_vec();
    expected.extend_from_slice(&7_u64.to_le_bytes());
    expected.extend_from_slice(&0_u32.to_le_bytes());
    expected.extend_from_slice(&9_u32.to_le_bytes());
    expected.extend_from_slice(&2_u16.to_le_bytes());
    expected.extend_from_slice(&[1, 1]);
    assert_eq!(encode(&publication).unwrap(), expected);
    assert_eq!(decode(&expected, 123).unwrap(), Some(publication));
    assert!(
        super::super::decode(&expected).is_err(),
        "bare body is not a standalone carrier"
    );
}

#[test]
fn every_wire_kind_origin_and_active_value_round_trips() {
    for code in (1..=8).chain([100, 101]) {
        for origin in [
            GraphRelationOrigin::Dream,
            GraphRelationOrigin::User,
            GraphRelationOrigin::Perception,
        ] {
            for active in [false, true] {
                let publication = GraphPublication {
                    graph_version: 1,
                    transaction: GraphTransaction::Single(GraphOrdinalMutation {
                        source: 0,
                        target: u32::MAX,
                        kind: SemanticGraphRelationKind::from_code(code).unwrap(),
                        active,
                        origin,
                    }),
                };
                let bytes = encode(&publication).unwrap();
                assert_eq!(&bytes[24..26], &code.to_le_bytes());
                assert_eq!(decode(&bytes, 1).unwrap(), Some(publication));
            }
        }
    }
}

#[test]
fn batch_keeps_order_individual_origins_and_one_member_batch_shape() {
    let values = vec![
        row(3, 4, false, GraphRelationOrigin::User),
        row(0, 1, true, GraphRelationOrigin::Dream),
    ];
    let publication = GraphPublication {
        graph_version: 8,
        transaction: GraphTransaction::Batch(values.clone()),
    };
    let mut expected = b"CVAGCB02".to_vec();
    expected.extend_from_slice(&8_u64.to_le_bytes());
    expected.extend_from_slice(&2_u32.to_le_bytes());
    for value in &values {
        expected.extend_from_slice(&value.source.to_le_bytes());
        expected.extend_from_slice(&value.target.to_le_bytes());
        expected.extend_from_slice(&value.kind.code().to_le_bytes());
        expected.push(u8::from(value.active));
        expected.push(value.origin.code());
    }
    assert_eq!(encode(&publication).unwrap(), expected);
    assert_eq!(decode(&expected, 19).unwrap(), Some(publication));
    let one = GraphPublication {
        graph_version: 1,
        transaction: GraphTransaction::Batch(vec![values[0]]),
    };
    let decoded = decode(&encode(&one).unwrap(), 1).unwrap().unwrap();
    assert!(!decoded.is_single());
    assert_eq!(decoded, one);
}

#[test]
fn malformed_flags_kinds_origins_clocks_versions_counts_and_trailing_bytes_reject() {
    let publication = GraphPublication {
        graph_version: 1,
        transaction: GraphTransaction::Single(row(0, 1, true, GraphRelationOrigin::Dream)),
    };
    let valid = encode(&publication).unwrap();
    for len in 6..valid.len() {
        assert!(decode(&valid[..len], 1).is_err(), "prefix {len}");
    }
    for (offset, byte) in [(6, b'9'), (26, 2), (27, 0), (27, 4)] {
        let mut bytes = valid.clone();
        bytes[offset] = byte;
        assert!(decode(&bytes, 1).is_err());
    }
    let mut bytes = valid.clone();
    bytes[24..26].copy_from_slice(&u16::MAX.to_le_bytes());
    assert!(decode(&bytes, 1).is_err());
    let mut bytes = valid.clone();
    bytes[8..16].fill(0);
    assert!(decode(&bytes, 1).is_err());
    assert!(decode(&valid, 0).is_err());
    let mut bytes = valid.clone();
    bytes.push(0);
    assert!(decode(&bytes, 1).is_err());
    assert!(
        encode(&GraphPublication {
            graph_version: 0,
            transaction: publication.transaction.clone()
        })
        .is_err()
    );
    assert!(
        encode(&GraphPublication {
            graph_version: 1,
            transaction: GraphTransaction::Batch(vec![])
        })
        .is_err()
    );
    for count in [0, 1, u32::MAX] {
        let mut bytes = b"CVAGCB02".to_vec();
        bytes.extend_from_slice(&1_u64.to_le_bytes());
        bytes.extend_from_slice(&count.to_le_bytes());
        assert!(decode(&bytes, 1).is_err());
    }
    let mut batch = encode(&GraphPublication {
        graph_version: 2,
        transaction: GraphTransaction::Batch(vec![row(0, 1, true, GraphRelationOrigin::User)]),
    })
    .unwrap();
    batch.push(0);
    assert!(decode(&batch, 1).is_err());
    for bytes in [
        b"CVAGFMT1".as_slice(),
        b"CVAGNODE",
        b"CVAGNOD2",
        b"CVAGVER1",
        b"CVAGBL01",
        b"CVAGCM01",
        b"CVAGCB01",
    ] {
        assert!(decode(bytes, 1).is_err());
    }
    assert!(decode(b"CVAMC002", 1).unwrap().is_none());
}
