use crate::relationship_codec::RelationshipTombstone;
use crate::relationship_model::RelationshipRecord;
use crate::relationship_store::{RelationshipReplayPublication, RelationshipStore};
use crate::{EntityId, EntityRef, RelationshipId, RelationshipMemoryRef, RelationshipParticipant};
use crate::{RelationshipError, RelationshipPayloadUnavailable};

fn record(revision: u64, global: u64, local: u64, mutation: &str) -> RelationshipRecord {
    RelationshipRecord {
        id: RelationshipId([0xA7; 32]),
        revision,
        kind: "  observes  ".into(),
        participants: vec![
            RelationshipParticipant {
                entity: EntityRef {
                    owner_id: "  external-entity  ".into(),
                    entity_id: EntityId([0x11; 32]),
                },
                role: Some("  exact role  ".into()),
            },
            RelationshipParticipant {
                entity: EntityRef {
                    owner_id: "remote-entity".into(),
                    entity_id: EntityId([0x22; 32]),
                },
                role: None,
            },
        ],
        evidence: vec![RelationshipMemoryRef {
            owner_id: "  external-memory  ".into(),
            memory_id: [0x33; 32],
        }],
        summary: "  exact description  ".into(),
        mutation_id: mutation.into(),
        created_at_ns: 10,
        updated_at_ns: 20,
        global_version: global,
        relationship_version: local,
    }
}

#[test]
fn exact_coordinate_lookup_and_normalized_evidence_retain_semantics_only() {
    let mut store = RelationshipStore::empty();
    let original = record(1, 17, 1, "legacy-mutation");
    store.insert_rebuilt(original.clone()).unwrap();

    assert!(matches!(
        store.retained_publication_at(17, 1).unwrap(),
        Some(RelationshipReplayPublication::Record(ref value)) if value == &original
    ));
    assert!(store.publication_at(17, 2).unwrap().is_none());
    assert!(store.retained_publication_at(18, 1).unwrap().is_none());

    let evidence = RelationshipReplayPublication::Record(original.clone())
        .normalized_import_evidence()
        .unwrap();
    let mut relocated = original.clone();
    relocated.global_version = 170;
    relocated.relationship_version = 30;
    assert_eq!(
        RelationshipReplayPublication::Record(relocated.clone())
            .normalized_import_evidence()
            .unwrap(),
        evidence,
        "only publication coordinates are omitted"
    );

    let mut changed = relocated.clone();
    changed.kind = "supports".into();
    assert_ne!(
        RelationshipReplayPublication::Record(changed)
            .normalized_import_evidence()
            .unwrap(),
        evidence
    );
    let mut changed = relocated.clone();
    changed.participants[0].role = Some("other exact role".into());
    assert_ne!(
        RelationshipReplayPublication::Record(changed)
            .normalized_import_evidence()
            .unwrap(),
        evidence
    );
    let mut changed = relocated.clone();
    changed.evidence[0].memory_id = [0x55; 32];
    assert_ne!(
        RelationshipReplayPublication::Record(changed)
            .normalized_import_evidence()
            .unwrap(),
        evidence
    );
    let mut changed = relocated.clone();
    changed.updated_at_ns += 1;
    assert_ne!(
        RelationshipReplayPublication::Record(changed)
            .normalized_import_evidence()
            .unwrap(),
        evidence
    );
    let mut changed = relocated.clone();
    changed.created_at_ns += 1;
    assert_ne!(
        RelationshipReplayPublication::Record(changed)
            .normalized_import_evidence()
            .unwrap(),
        evidence
    );
    let mut changed = relocated.clone();
    changed.mutation_id = "different mutation".into();
    assert_ne!(
        RelationshipReplayPublication::Record(changed)
            .normalized_import_evidence()
            .unwrap(),
        evidence
    );
    let mut changed = relocated.clone();
    changed.summary = "different authored summary".into();
    assert_ne!(
        RelationshipReplayPublication::Record(changed)
            .normalized_import_evidence()
            .unwrap(),
        evidence
    );

    let mut authored_empty = relocated;
    authored_empty.summary.clear();
    let authored_empty_evidence = RelationshipReplayPublication::Record(authored_empty.clone())
        .normalized_import_evidence()
        .unwrap();
    assert_ne!(authored_empty_evidence, evidence);

    store
        .insert_tombstone_rebuilt(
            RelationshipTombstone {
                id: original.id,
                revision: 2,
            },
            20,
            2,
        )
        .unwrap();
    assert!(matches!(
        store.publication_at(20, 2).unwrap(),
        Some(RelationshipReplayPublication::Tombstone { id, revision: 2 }) if id == original.id
    ));

    store
        .pruned_revisions
        .insert((original.id, original.revision));
    let retained = match store.retained_publication_at(17, 1).unwrap().unwrap() {
        RelationshipReplayPublication::PrunedRecord { id, revision } => {
            assert_eq!(id, original.id);
            revision
        }
        other => panic!("expected retained prune descriptor, got {other:?}"),
    };
    assert!(matches!(
        store.publication_at(17, 1),
        Err(RelationshipError::PayloadUnavailable(
            RelationshipPayloadUnavailable::Pruned
        ))
    ));
    let pruned_evidence = RelationshipReplayPublication::PrunedRecord {
        id: original.id,
        revision: retained.clone(),
    }
    .normalized_import_evidence()
    .unwrap();
    let mut relocated_retained = retained;
    relocated_retained.global_version = 1_700;
    relocated_retained.relationship_version = 300;
    assert_eq!(
        RelationshipReplayPublication::PrunedRecord {
            id: original.id,
            revision: relocated_retained,
        }
        .normalized_import_evidence()
        .unwrap(),
        pruned_evidence,
        "pruned structural evidence also omits only coordinates"
    );
    assert_ne!(authored_empty_evidence, pruned_evidence);
    let pruned_json = String::from_utf8(pruned_evidence).unwrap();
    assert!(pruned_json.contains("pruned"));
    assert!(pruned_json.contains("  observes  "));
    assert!(pruned_json.contains("  exact role  "));
    assert!(pruned_json.contains("  external-memory  "));
    assert!(
        pruned_json.contains("null"),
        "the missing optional role remains explicit"
    );
    assert!(!pruned_json.contains("exact description"));
}
