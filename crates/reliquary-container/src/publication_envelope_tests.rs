use super::{PublicationClock, PublicationEnvelope, decode, encode};
use crate::ContainerError;

const BODY: &[u8] = b"OWNER001\x00\xffopaque\0payload";

fn error_is_invalid_version<T>(result: Result<T, ContainerError>) {
    assert!(matches!(result, Err(ContainerError::InvalidVersionRecord)));
}

fn literal(global: u64, tag: u8, timestamp: Option<i64>, body: &[u8]) -> Vec<u8> {
    // Independent expected-wire construction; it does not call the codec.
    let mut bytes = b"CVAPUB01".to_vec();
    bytes.extend_from_slice(&global.to_le_bytes());
    bytes.push(tag);
    if let Some(timestamp) = timestamp {
        bytes.extend_from_slice(&timestamp.to_le_bytes());
    }
    bytes.extend_from_slice(body);
    bytes
}

#[test]
fn literal_clock_tags_roundtrip_with_exact_fields_and_borrowed_body() {
    let cases = [
        (
            PublicationEnvelope {
                global_version: 1,
                clock: PublicationClock::Allocate {
                    transaction_time_ns: None,
                },
                body: BODY,
            },
            literal(1, 0, None, BODY),
        ),
        (
            PublicationEnvelope {
                global_version: 0x0102_0304_0506_0708,
                clock: PublicationClock::Allocate {
                    transaction_time_ns: Some(i64::MIN),
                },
                body: BODY,
            },
            literal(0x0102_0304_0506_0708, 1, Some(i64::MIN), BODY),
        ),
        (
            PublicationEnvelope {
                global_version: u64::MAX,
                clock: PublicationClock::BindReserved,
                body: BODY,
            },
            literal(u64::MAX, 2, None, BODY),
        ),
    ];

    for (envelope, expected) in cases {
        assert_eq!(encode(envelope).unwrap(), expected);
        let decoded = decode(&expected).unwrap().unwrap();
        assert_eq!(decoded.global_version, envelope.global_version);
        assert_eq!(decoded.clock, envelope.clock);
        assert_eq!(decoded.body, BODY);
        assert!(std::ptr::eq(
            decoded.body.as_ptr(),
            expected[17 + usize::from(expected[16] == 1) * 8..].as_ptr()
        ));
        assert_eq!(encode(decoded).unwrap(), expected);
    }
}

#[test]
fn literal_timestamp_boundaries_and_opaque_owner_tail_are_preserved() {
    for timestamp in [i64::MIN, -1, 0, 1, i64::MAX] {
        let expected = literal(7, 1, Some(timestamp), BODY);
        let decoded = decode(&expected).unwrap().unwrap();
        assert_eq!(
            decoded.clock,
            PublicationClock::Allocate {
                transaction_time_ns: Some(timestamp)
            }
        );
        assert_eq!(decoded.body, BODY);
        assert_eq!(encode(decoded).unwrap(), expected);
    }

    let mut body_with_arbitrary_tail = b"OWNER002".to_vec();
    body_with_arbitrary_tail.extend_from_slice(&[0, 0xFE, 0xFF, b'C', b'V', b'A']);
    let expected = literal(9, 2, None, &body_with_arbitrary_tail);
    let decoded = decode(&expected).unwrap().unwrap();
    assert_eq!(decoded.body, body_with_arbitrary_tail);
    assert_eq!(encode(decoded).unwrap(), expected);
}

#[test]
fn rejects_zero_global_short_or_nested_body() {
    error_is_invalid_version(encode(PublicationEnvelope {
        global_version: 0,
        clock: PublicationClock::Allocate {
            transaction_time_ns: None,
        },
        body: BODY,
    }));
    error_is_invalid_version(decode(&literal(0, 0, None, BODY)));

    for body in [&b""[..], b"1234567", b"CVAPUB01nested"] {
        error_is_invalid_version(encode(PublicationEnvelope {
            global_version: 1,
            clock: PublicationClock::BindReserved,
            body,
        }));
    }
    error_is_invalid_version(decode(&literal(1, 0, None, b"short")));
    error_is_invalid_version(decode(&literal(1, 0, None, b"CVAPUB01nested")));
}

#[test]
fn short_outer_prefixes_are_unrelated_until_the_full_dispatch_prefix() {
    for length in 0..6 {
        assert_eq!(decode(&b"CVAPUB"[..length]).unwrap(), None);
    }
    for prefix_len in [6, 7] {
        error_is_invalid_version(decode(&b"CVAPUB01"[..prefix_len]));
    }
    assert_eq!(decode(b"NOTAPUB1unrelated owner payload").unwrap(), None);
}

#[test]
fn rejects_unknown_envelope_version_tag_and_truncated_timestamp() {
    let mut unknown_version = literal(1, 0, None, BODY);
    unknown_version[..8].copy_from_slice(b"CVAPUB02");
    error_is_invalid_version(decode(&unknown_version));

    for tag in [3, u8::MAX] {
        let mut unknown_tag = literal(1, 0, None, BODY);
        unknown_tag[16] = tag;
        error_is_invalid_version(decode(&unknown_tag));
    }

    let mut timestamp_header = b"CVAPUB01".to_vec();
    timestamp_header.extend_from_slice(&1_u64.to_le_bytes());
    timestamp_header.push(1);
    timestamp_header.extend_from_slice(&0_i64.to_le_bytes());
    for prefix_len in 17..25 {
        error_is_invalid_version(decode(&timestamp_header[..prefix_len]));
    }
}
