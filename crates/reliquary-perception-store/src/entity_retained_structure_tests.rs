use super::encode_record_structure;
use crate::entity_model::EntityRecord;
use crate::{EntityGlobalId, EntityId};

fn record(global_id: Option<EntityGlobalId>) -> EntityRecord {
    EntityRecord {
        id: EntityId([0x11; 32]),
        global_id,
        revision: 7,
        canonical_name: "Sensitive name".into(),
        aliases: vec!["Sensitive alias".into()],
        kind: "project".into(),
        summary: "Sensitive summary".into(),
        mutation_id: "stable-mutation-id".into(),
        created_at_ns: -21,
        updated_at_ns: 34,
        global_version: 100,
        entity_version: 55,
    }
}

fn redact_for_legacy_descriptor(mut record: EntityRecord) -> EntityRecord {
    record.canonical_name = "[pruned]".into();
    record.aliases.clear();
    record.summary.clear();
    record
}

#[test]
fn some_uuid_encoding_preserves_the_existing_descriptor_hash_input() {
    let original = record(Some(EntityGlobalId([0x22; 16])));
    let legacy_redacted = redact_for_legacy_descriptor(original.clone());
    assert_eq!(
        encode_record_structure(&original).unwrap(),
        crate::entity_codec::encode_record(&legacy_redacted).unwrap()
    );
}

#[test]
fn none_uuid_has_explicit_r1_structure_and_retains_only_structural_fields() {
    let original = record(None);
    let encoded = encode_record_structure(&original).unwrap();
    let mut expected = b"CVAENTR1".to_vec();
    expected.extend_from_slice(&[0x11; 32]);
    expected.extend_from_slice(&7_u64.to_le_bytes());
    expected.extend_from_slice(&(-21_i64).to_le_bytes());
    expected.extend_from_slice(&34_i64.to_le_bytes());
    push_string(&mut expected, "[pruned]");
    expected.extend_from_slice(&0_u16.to_le_bytes());
    push_string(&mut expected, "project");
    push_string(&mut expected, "");
    push_string(&mut expected, "stable-mutation-id");
    assert_eq!(encoded, expected);
    assert!(
        !encoded
            .windows("Sensitive name".len())
            .any(|window| window == b"Sensitive name")
    );
    assert!(
        !encoded
            .windows("Sensitive alias".len())
            .any(|window| window == b"Sensitive alias")
    );
    assert!(
        !encoded
            .windows("Sensitive summary".len())
            .any(|window| window == b"Sensitive summary")
    );
}

#[test]
fn retained_structure_binds_kind_mutation_and_string_lengths() {
    let original = record(None);
    let baseline = encode_record_structure(&original).unwrap();

    let mut changed_mutation = original.clone();
    changed_mutation.mutation_id = "different-mutation".into();
    assert_ne!(
        encode_record_structure(&changed_mutation).unwrap(),
        baseline
    );

    let mut changed_kind = original.clone();
    changed_kind.kind = "person".into();
    assert_ne!(encode_record_structure(&changed_kind).unwrap(), baseline);

    let mutation = b"stable-mutation-id";
    assert!(
        baseline
            .windows(mutation.len())
            .any(|window| window == mutation)
    );
    let mutation_offset = baseline
        .windows(mutation.len())
        .position(|window| window == mutation)
        .unwrap();
    assert_eq!(
        u32::from_le_bytes(
            baseline[mutation_offset - 4..mutation_offset]
                .try_into()
                .unwrap()
        ) as usize,
        mutation.len()
    );
}

fn push_string(out: &mut Vec<u8>, value: &str) {
    out.extend_from_slice(&(value.len() as u32).to_le_bytes());
    out.extend_from_slice(value.as_bytes());
}
