//! Equivalent accepted records have import evidence independent of disk codecs.
use super::EntityReplayPublication;
use crate::entity_compact_contract::{EntityCreation, EntityDelta};
use crate::entity_model::EntityRecord;
use crate::{EntityGlobalId, EntityId};

fn legacy_payload(record: &EntityRecord) -> Vec<u8> {
    if record.global_id.is_some() {
        return crate::entity_codec::encode_record(record).unwrap();
    }
    let mut bytes = b"CVAENTR1".to_vec();
    bytes.extend(record.id.0);
    bytes.extend(record.revision.to_le_bytes());
    bytes.extend(record.created_at_ns.to_le_bytes());
    bytes.extend(record.updated_at_ns.to_le_bytes());
    fn string(bytes: &mut Vec<u8>, text: &str) {
        bytes.extend((text.len() as u32).to_le_bytes());
        bytes.extend(text.as_bytes());
    }
    string(&mut bytes, &record.canonical_name);
    bytes.extend((record.aliases.len() as u16).to_le_bytes());
    for alias in &record.aliases {
        string(&mut bytes, alias);
    }
    for text in [&record.kind, &record.summary, &record.mutation_id] {
        string(&mut bytes, text);
    }
    bytes
}

#[test]
fn legacy_and_compact_optional_identity_history_have_identical_import_evidence() {
    let mut current = EntityRecord {
        id: EntityId([6; 32]),
        global_id: None,
        revision: 1,
        canonical_name: "  accepted name  ".into(),
        aliases: vec!["Alias".into()],
        kind: "project".into(),
        summary: "Exact text".into(),
        mutation_id: "mutation-1".into(),
        created_at_ns: -10,
        updated_at_ns: 20,
        global_version: 8,
        entity_version: 1,
    };
    let mut previous = None;
    for global_id in [
        None,
        Some(EntityGlobalId([1; 16])),
        Some(EntityGlobalId([2; 16])),
        None,
    ] {
        current.global_id = global_id;
        let legacy = crate::entity_codec::decode_record(&legacy_payload(&current))
            .unwrap()
            .unwrap();
        let compact = match &previous {
            None => {
                let bytes = crate::entity_compact_codec::encode_creation(
                    &EntityCreation::new(current.clone()).unwrap(),
                )
                .unwrap();
                crate::entity_compact_codec::decode_creation(&bytes)
                    .unwrap()
                    .record
            }
            Some(prior) => {
                let delta = EntityDelta::between(prior, &current).unwrap();
                let bytes = crate::entity_compact_codec::encode_delta(&delta).unwrap();
                let decoded = crate::entity_compact_codec::decode_delta(&bytes).unwrap();
                crate::entity_compact_apply::apply_delta(prior, &decoded).unwrap()
            }
        };
        assert_eq!(
            EntityReplayPublication::Record(legacy)
                .normalized_import_evidence()
                .unwrap(),
            EntityReplayPublication::Record(compact)
                .normalized_import_evidence()
                .unwrap()
        );
        previous = Some(current.clone());
        current.revision += 1;
        current.entity_version += 1;
        current.global_version -= 1;
        current.updated_at_ns -= 1;
        current.mutation_id = format!("mutation-{}", current.revision);
    }
}
