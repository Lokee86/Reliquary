//! Scratch owner replay retry and semantic import-evidence tests.
use crate::{Cva, EntityId, EntityRef, RelationshipId, RelationshipParticipant};
use reliquary_perception_store::RelationshipError;
use reliquary_perception_store::RelationshipMemoryRef;
use reliquary_perception_store::relationship_model::RelationshipRecord;
use reliquary_perception_store::relationship_store::RelationshipReplayPublication;
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
