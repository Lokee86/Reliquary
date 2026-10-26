use super::{EntityReplayPublication, EntityStore};
use crate::entity_model::EntityRecord;
use crate::{EntityError, EntityGlobalId, EntityId, EntityRetainedRevision};

fn record(global_version: u64, entity_version: u64) -> EntityRecord {
    EntityRecord {
        id: EntityId([1; 32]),
        global_id: Some(EntityGlobalId([2; 16])),
        revision: 1,
        canonical_name: "Private entity name".into(),
        aliases: vec!["Private alias".into()],
        kind: "project".into(),
        summary: "Private summary".into(),
        mutation_id: "entity-create".into(),
        created_at_ns: 10,
        updated_at_ns: 11,
        global_version,
        entity_version,
    }
}

#[test]
fn retained_lookup_marks_pruned_payload_and_evidence_omits_coordinates_and_text() {
    let mut store = EntityStore::empty();
    let full_record = record(3, 1);
    store.insert_rebuilt(full_record.clone()).unwrap();
    let full = store.retained_publication_at(3, 1).unwrap().unwrap();
    assert!(matches!(full, EntityReplayPublication::Record(_)));

    store.pruned_revisions.insert((full_record.id, 1));
    let pruned = store.retained_publication_at(3, 1).unwrap().unwrap();
    assert!(matches!(
        pruned,
        EntityReplayPublication::PrunedRecord { id, revision }
            if id == full_record.id && revision.revision == 1
    ));
    assert!(matches!(
        store.publication_at(3, 1),
        Err(EntityError::PayloadUnavailable(
            crate::EntityPayloadUnavailable::Pruned
        ))
    ));

    let full_evidence = EntityReplayPublication::Record(full_record.clone())
        .normalized_import_evidence()
        .unwrap();
    let mut relocated = full_record.clone();
    relocated.global_version = 30;
    relocated.entity_version = 9;
    assert_eq!(
        EntityReplayPublication::Record(relocated)
            .normalized_import_evidence()
            .unwrap(),
        full_evidence
    );

    let retained = EntityRetainedRevision {
        revision: 1,
        global_version: 3,
        entity_version: 1,
        global_id: full_record.global_id,
        kind: full_record.kind.clone(),
        created_at_ns: full_record.created_at_ns,
        updated_at_ns: full_record.updated_at_ns,
        mutation_id: full_record.mutation_id.clone(),
    };
    let pruned_evidence = EntityReplayPublication::PrunedRecord {
        id: full_record.id,
        revision: retained.clone(),
    }
    .normalized_import_evidence()
    .unwrap();
    let mut relocated_retained = retained;
    relocated_retained.global_version = 300;
    relocated_retained.entity_version = 90;
    assert_eq!(
        EntityReplayPublication::PrunedRecord {
            id: full_record.id,
            revision: relocated_retained,
        }
        .normalized_import_evidence()
        .unwrap(),
        pruned_evidence
    );
    assert_ne!(full_evidence, pruned_evidence);
    let encoded = String::from_utf8(pruned_evidence).unwrap();
    assert!(encoded.contains("pruned"));
    assert!(!encoded.contains("Private entity name"));
    assert!(!encoded.contains("Private alias"));
    assert!(!encoded.contains("Private summary"));
}
