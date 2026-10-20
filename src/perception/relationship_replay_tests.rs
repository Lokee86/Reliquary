//! Scratch owner replay retry and semantic import-evidence tests.
use crate::relationship_codec::RelationshipTombstone;
use crate::relationship_model::RelationshipRecord;
use crate::relationship_store::{RelationshipReplayPublication, RelationshipStore};
use crate::{
    Cva, EntityId, EntityRef, MemoryId, MemoryRef, RelationshipError, RelationshipId,
    RelationshipParticipant,
};
use std::fs;
use std::path::{Path, PathBuf};

fn test_path(name: &str) -> PathBuf {
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".test-tmp")
        .join(format!(
            "relationship-replay-evidence-{}",
            uuid::Uuid::new_v4()
        ));
    fs::create_dir_all(&directory).unwrap();
    directory.join(name)
}

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
        evidence: vec![MemoryRef {
            owner_id: "  external-memory  ".into(),
            memory_id: MemoryId([0x33; 32]),
        }],
        summary: "  exact description  ".into(),
        mutation_id: mutation.into(),
        created_at_ns: 10,
        updated_at_ns: 20,
        global_version: global,
        relationship_version: local,
    }
}

fn assert_no_write(path: &Path, owner: &Cva, bytes_before: &[u8], clocks_before: (u64, u64)) {
    assert_eq!(fs::read(path).unwrap(), bytes_before);
    assert_eq!(
        (
            owner.relationship_version(),
            owner.container.latest_version()
        ),
        clocks_before
    );
}

#[test]
fn exact_older_mutation_retry_is_inert_and_semantic_conflicts_do_not_append() {
    let path = test_path("retry-and-conflict.rel");
    let mut owner = Cva::create(&path).unwrap();
    let first = record(1, 90, 7, "mutation-one");
    assert!(
        owner
            .relationships
            .replay_publication(
                &mut owner.container,
                RelationshipReplayPublication::Record(first.clone()),
            )
            .unwrap()
    );

    let mut second = record(2, 91, 12, "mutation-two");
    second.summary = "  second revision  ".into();
    second.participants[0].role = Some("  revised role  ".into());
    assert!(
        owner
            .relationships
            .replay_publication(
                &mut owner.container,
                RelationshipReplayPublication::Record(second.clone()),
            )
            .unwrap()
    );
    assert_eq!(owner.relationship_version(), 2);

    let bytes_before = fs::read(&path).unwrap();
    let clocks_before = (
        owner.relationship_version(),
        owner.container.latest_version(),
    );
    assert!(
        !owner
            .relationships
            .replay_publication(
                &mut owner.container,
                RelationshipReplayPublication::Record(first.clone()),
            )
            .unwrap()
    );
    assert_no_write(&path, &owner, &bytes_before, clocks_before);

    let mut summary_conflict = first.clone();
    summary_conflict.summary = "different semantic description".into();
    assert!(matches!(
        owner.relationships.replay_publication(
            &mut owner.container,
            RelationshipReplayPublication::Record(summary_conflict),
        ),
        Err(RelationshipError::MutationConflict)
    ));
    assert_no_write(&path, &owner, &bytes_before, clocks_before);

    let mut participant_conflict = first.clone();
    participant_conflict.participants[0].role = Some("different role".into());
    assert!(matches!(
        owner.relationships.replay_publication(
            &mut owner.container,
            RelationshipReplayPublication::Record(participant_conflict),
        ),
        Err(RelationshipError::MutationConflict)
    ));
    assert_no_write(&path, &owner, &bytes_before, clocks_before);

    let tombstone = RelationshipReplayPublication::Tombstone {
        id: first.id,
        revision: 3,
    };
    assert!(
        owner
            .relationships
            .replay_publication(&mut owner.container, tombstone.clone())
            .unwrap()
    );
    let after_tombstone = fs::read(&path).unwrap();
    let tombstone_clocks = (
        owner.relationship_version(),
        owner.container.latest_version(),
    );
    assert!(
        !owner
            .relationships
            .replay_publication(&mut owner.container, tombstone)
            .unwrap()
    );
    assert_no_write(&path, &owner, &after_tombstone, tombstone_clocks);
    assert!(matches!(
        owner.relationships.replay_publication(
            &mut owner.container,
            RelationshipReplayPublication::Tombstone {
                id: first.id,
                revision: 4,
            },
        ),
        Err(RelationshipError::TerminalTombstone)
    ));
    assert!(matches!(
        owner.relationships.replay_publication(
            &mut owner.container,
            RelationshipReplayPublication::Record({
                let mut resurrection = second;
                resurrection.revision = 3;
                resurrection.mutation_id = "attempt-resurrection".into();
                resurrection
            }),
        ),
        Err(RelationshipError::TerminalTombstone)
    ));
    assert_no_write(&path, &owner, &after_tombstone, tombstone_clocks);
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
    changed.evidence[0].memory_id = MemoryId([0x55; 32]);
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
            crate::owner_lifecycle::PayloadUnavailable::Pruned
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
