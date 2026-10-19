use super::*;
use serde_json::json;

fn projection() -> OwnerProjection {
    OwnerProjection::new([7; 16], "memory", 3, None).unwrap()
}

#[test]
fn row_and_recursive_object_order_are_canonical() {
    let mut left = projection();
    let mut right = projection();
    left.insert(
        "record",
        b"b",
        json!({"z": [2, 1], "a": {"x": 9, "b": null}}),
    )
    .unwrap();
    left.insert("record", b"a", json!({"revision": 1})).unwrap();
    right
        .insert("record", b"a", json!({"revision": 1}))
        .unwrap();
    right
        .insert(
            "record",
            b"b",
            json!({"a": {"b": null, "x": 9}, "z": [2, 1]}),
        )
        .unwrap();
    assert_eq!(left.fingerprint().unwrap(), right.fingerprint().unwrap());
    assert_eq!(
        canonical_value(&json!({"z": [2, 1], "a": {"x": 9, "b": null}})).unwrap(),
        br#"{"a":{"b":null,"x":9},"z":[2,1]}"#
    );
}

#[test]
fn duplicate_and_invalid_rows_leave_projection_unchanged() {
    let mut owner = projection();
    owner
        .insert("record", b"id", json!({"revision": 1}))
        .unwrap();
    let before = owner.fingerprint().unwrap();
    assert!(
        owner
            .insert("record", b"id", json!({"revision": 2}))
            .is_err()
    );
    assert!(
        owner
            .insert("record", b"other", json!({"nested": [1.25]}))
            .is_err()
    );
    assert!(owner.insert("", b"other", Value::Null).is_err());
    assert_eq!(before, owner.fingerprint().unwrap());
}

#[test]
fn semantic_content_and_order_changes_are_detected() {
    let fingerprint = |value| {
        let mut owner = projection();
        owner.insert("record", b"id", value).unwrap();
        owner.fingerprint().unwrap().sha256
    };
    assert_ne!(fingerprint(json!([1, 2])), fingerprint(json!([2, 1])));
    assert_ne!(fingerprint(Value::Null), fingerprint(json!("")));
    assert_ne!(
        fingerprint(json!({})),
        fingerprint(json!({"optional": null}))
    );
    assert_ne!(fingerprint(json!(u64::MAX)), fingerprint(json!(i64::MAX)));
    assert_ne!(fingerprint(json!("é")), fingerprint(json!("e\u{301}")));
}

#[test]
fn owner_domain_and_cut_coordinates_are_bound_to_digest() {
    let base = projection().fingerprint().unwrap();
    for other in [
        OwnerProjection::new([8; 16], "memory", 3, None).unwrap(),
        OwnerProjection::new([7; 16], "entity", 3, None).unwrap(),
        OwnerProjection::new([7; 16], "memory", 4, None).unwrap(),
        OwnerProjection::new([7; 16], "memory", 3, Some(0)).unwrap(),
    ] {
        assert_ne!(base.sha256, other.fingerprint().unwrap().sha256);
    }
    assert!(OwnerProjection::new([0; 16], "", 0, None).is_err());
}

#[test]
fn length_framing_prevents_family_key_boundary_aliases() {
    let mut left = projection();
    let mut right = projection();
    left.insert("ab", b"c", json!(0)).unwrap();
    right.insert("a", b"bc", json!(0)).unwrap();
    assert_ne!(
        left.fingerprint().unwrap().sha256,
        right.fingerprint().unwrap().sha256
    );
}

#[test]
fn empty_projection_matches_independent_sha256_vector() {
    let digest = projection().fingerprint().unwrap().sha256;
    // Generated independently with Python hashlib and explicit little-endian framing.
    let expected = "8d713b0e9efa2ef12fb9eef2878e7bdfba1c217a3b738f284c7edcf4e0ee000a";
    let actual: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(actual, expected);
}
