use crate::graph_compact_codec::{decode, encode_batch, encode_mutation};
use crate::graph_compact_contract::GraphOrdinalMutation;
use crate::{GraphRelationOrigin, SemanticGraphRelationKind};

fn kind(code: u16) -> SemanticGraphRelationKind {
    SemanticGraphRelationKind::from_code(code).unwrap()
}

fn mutation(
    source: u32,
    target: u32,
    kind_code: u16,
    active: bool,
    origin: GraphRelationOrigin,
) -> GraphOrdinalMutation {
    GraphOrdinalMutation {
        source,
        target,
        kind: kind(kind_code),
        active,
        origin,
    }
}

fn literal_body(source: u32, target: u32, kind_code: u16, active: u8, origin: u8) -> [u8; 12] {
    let source = source.to_le_bytes();
    let target = target.to_le_bytes();
    let kind = kind_code.to_le_bytes();
    [
        source[0], source[1], source[2], source[3], target[0], target[1], target[2], target[3],
        kind[0], kind[1], active, origin,
    ]
}

#[test]
fn independent_literal_mutation_layout_and_every_relation_kind() {
    let codes = [1, 2, 3, 4, 5, 6, 7, 8, 100, 101];
    for code in codes {
        // Independently laid-out bytes: 8-byte magic, LE u32 endpoints,
        // LE u16 semantic kind, active byte, origin byte.
        let mut literal = b"CVAGCM01".to_vec();
        literal.extend_from_slice(&literal_body(0x0102_0304, 0xfedc_ba98, code, 1, 3));
        assert_eq!(literal.len(), 20);
        assert_eq!(
            decode(&literal).unwrap(),
            Some(vec![mutation(
                0x0102_0304,
                0xfedc_ba98,
                code,
                true,
                GraphRelationOrigin::Perception,
            )])
        );
        assert_eq!(
            encode_mutation(mutation(
                0x0102_0304,
                0xfedc_ba98,
                code,
                true,
                GraphRelationOrigin::Perception,
            )),
            literal
        );
    }
}

#[test]
fn literal_batch_preserves_each_mutation_and_permuted_origins() {
    let first = mutation(0, u32::MAX, 8, false, GraphRelationOrigin::Dream);
    let second = mutation(u32::MAX, 0, 100, true, GraphRelationOrigin::User);
    let third = mutation(
        0x1122_3344,
        0x5566_7788,
        101,
        false,
        GraphRelationOrigin::Perception,
    );
    let mut literal = b"CVAGCB01".to_vec();
    literal.extend_from_slice(&3_u32.to_le_bytes());
    literal.extend_from_slice(&literal_body(0, u32::MAX, 8, 0, 1));
    literal.extend_from_slice(&literal_body(u32::MAX, 0, 100, 1, 2));
    literal.extend_from_slice(&literal_body(0x1122_3344, 0x5566_7788, 101, 0, 3));
    assert_eq!(literal.len(), 12 + 12 * 3);
    assert_eq!(decode(&literal).unwrap(), Some(vec![first, second, third]));
    assert_eq!(encode_batch(&[first, second, third]).unwrap(), literal);
}

#[test]
fn zero_and_max_ordinals_and_repeated_identical_mutations_are_preserved() {
    let repeated = mutation(0, u32::MAX, 1, true, GraphRelationOrigin::Dream);
    let encoded = encode_batch(&[repeated, repeated]).unwrap();
    assert_eq!(encoded.len(), 12 + 12 * 2);
    assert_eq!(decode(&encoded).unwrap(), Some(vec![repeated, repeated]));
}

#[test]
fn empty_batch_is_a_valid_unmarked_carrier_shape() {
    let literal = b"CVAGCB01\0\0\0\0";
    assert_eq!(literal.len(), 12);
    assert_eq!(decode(literal).unwrap(), Some(Vec::new()));
    assert_eq!(encode_batch(&[]).unwrap(), literal);
}

#[test]
fn every_recognized_proper_prefix_and_trailing_or_wrong_count_bytes_fail_closed() {
    let frames = [
        encode_mutation(mutation(4, 9, 100, true, GraphRelationOrigin::User)),
        encode_batch(&[
            mutation(4, 9, 100, true, GraphRelationOrigin::User),
            mutation(9, 4, 5, false, GraphRelationOrigin::Dream),
        ])
        .unwrap(),
        b"CVAGCB01\0\0\0\0".to_vec(),
    ];
    for frame in frames {
        for end in 0..frame.len() {
            let prefix = &frame[..end];
            if end < 6 {
                assert_eq!(
                    decode(prefix).unwrap(),
                    None,
                    "unrelated short prefix {end}"
                );
            } else {
                assert!(decode(prefix).is_err(), "recognized truncated prefix {end}");
            }
        }
        let mut trailing = frame.clone();
        trailing.push(0);
        assert!(decode(&trailing).is_err());
    }

    let mut wrong_count =
        encode_batch(&[mutation(0, 1, 1, true, GraphRelationOrigin::Dream)]).unwrap();
    wrong_count[8..12].copy_from_slice(&2_u32.to_le_bytes());
    assert!(decode(&wrong_count).is_err());
    let mut impossible_count = b"CVAGCB01".to_vec();
    impossible_count.extend_from_slice(&u32::MAX.to_le_bytes());
    assert!(decode(&impossible_count).is_err());
}

#[test]
fn unknown_version_kind_active_origin_and_unrelated_payloads_are_distinguished() {
    let valid = encode_mutation(mutation(0, u32::MAX, 1, true, GraphRelationOrigin::Dream));
    for unknown in [
        b"CVAGCM02".as_slice(),
        b"CVAGCB02",
        b"CVAGCM99",
        b"CVAGCB99",
    ] {
        assert!(decode(unknown).is_err());
    }
    assert_eq!(decode(b"CVAGCX01payload").unwrap(), None);
    assert_eq!(decode(b"unrelated Graph data").unwrap(), None);

    let mut unknown_kind = valid.clone();
    unknown_kind[16..18].copy_from_slice(&9_u16.to_le_bytes());
    assert!(decode(&unknown_kind).is_err());
    let mut unknown_semantic_kind = valid.clone();
    unknown_semantic_kind[16..18].copy_from_slice(&99_u16.to_le_bytes());
    assert!(decode(&unknown_semantic_kind).is_err());
    let mut bad_active = valid.clone();
    bad_active[18] = 2;
    assert!(decode(&bad_active).is_err());
    let mut bad_origin = valid;
    bad_origin[19] = 0;
    assert!(decode(&bad_origin).is_err());
}
