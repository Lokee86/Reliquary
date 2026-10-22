//! Scratch compact Relationship tail tests for facade reconciliation.
use super::{read_relationship_tail, replay_relationship_tail};
use crate::container::publication_envelope;
use crate::owner_lifecycle::{LifecycleState, TombstoneCoordinates};
use crate::relationship_codec::{
    RelationshipTombstone, RelationshipVersion, encode_record, encode_tombstone, encode_version,
};
use crate::relationship_compact_codec::publication::{self, RelationshipPublication};
fn encode_creation(c: &RelationshipCreation) -> Result<Vec<u8>, crate::RelationshipError> {
    crate::relationship_compact_codec::publication::encode(
        &crate::relationship_compact_codec::publication::RelationshipPublication::Creation(
            c.clone(),
        ),
    )
}
fn encode_delta(d: &RelationshipDelta) -> Result<Vec<u8>, crate::RelationshipError> {
    crate::relationship_compact_codec::publication::encode(
        &crate::relationship_compact_codec::publication::RelationshipPublication::Delta(d.clone()),
    )
}
use crate::relationship_compact_contract::{RelationshipCreation, RelationshipDelta};
use crate::relationship_model::RelationshipRecord;
use crate::{
    Container, Cva, EntityId, EntityRef, MemoryId, MemoryRef, RelationshipId,
    RelationshipParticipant,
};
use std::fs;
use std::path::{Path, PathBuf};

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!(
                "cva-reconcile-relationship-compact-{}",
                uuid::Uuid::new_v4()
            ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn file(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn record(revision: u64, global: u64, local: u64, mutation: &str) -> RelationshipRecord {
    RelationshipRecord {
        id: RelationshipId([0x41; 32]),
        revision,
        kind: "  observes  ".into(),
        participants: vec![
            RelationshipParticipant {
                entity: EntityRef {
                    owner_id: "  external-a  ".into(),
                    entity_id: EntityId([0x11; 32]),
                },
                role: Some("  observer  ".into()),
            },
            RelationshipParticipant {
                entity: EntityRef {
                    owner_id: "external-b".into(),
                    entity_id: EntityId([0x22; 32]),
                },
                role: None,
            },
        ],
        evidence: vec![MemoryRef {
            owner_id: "  external-memory  ".into(),
            memory_id: MemoryId([0x33; 32]),
        }],
        summary: "  exact padded description  ".into(),
        mutation_id: mutation.into(),
        created_at_ns: 10,
        updated_at_ns: 20,
        global_version: global,
        relationship_version: local,
    }
}

fn reserve_tickets(container: &mut Container, times: &[i64]) {
    for (index, time) in times.iter().enumerate() {
        assert_eq!(
            container
                .allocate_version_with_transaction_time(Some(*time))
                .unwrap(),
            index as u64 + 1
        );
    }
}

fn append_version(container: &mut Container, carrier: crate::ObjectRef, global: u64, _local: u64) {
    let body = container.read(carrier).unwrap();
    let bytes = crate::container::publication_envelope::encode(
        crate::container::publication_envelope::PublicationEnvelope {
            global_version: global,
            clock: crate::container::publication_envelope::PublicationClock::BindReserved,
            body: &body,
        },
    )
    .unwrap();
    container.append(&bytes).unwrap();
}

fn raw_owner(path: &Path) -> Cva {
    let mut owner = Cva::create(path).unwrap();
    reserve_tickets(&mut owner.container, &[10_001, 10_002, 10_003]);

    let first = record(1, 1, 1, "relationship-create");
    let first_carrier = owner
        .container
        .append(&encode_creation(&RelationshipCreation::new(first).unwrap()).unwrap())
        .unwrap();
    append_version(&mut owner.container, first_carrier, 1, 1);

    let mut second = record(2, 2, 2, "relationship-revise");
    second.participants[0].role = Some("  contributor  ".into());
    second.evidence.push(MemoryRef {
        owner_id: "external-memory-b".into(),
        memory_id: MemoryId([0x44; 32]),
    });
    second.summary = "  revised description  ".into();
    second.updated_at_ns = 17;
    let first = record(1, 1, 1, "relationship-create");
    let delta = RelationshipDelta::between(&first, &second).unwrap();
    let second_carrier = owner
        .container
        .append(&encode_delta(&delta).unwrap())
        .unwrap();
    append_version(&mut owner.container, second_carrier, 2, 2);

    let tombstone = owner
        .container
        .append(&crate::relationship_compact_codec::publication::encode(&crate::relationship_compact_codec::publication::RelationshipPublication::Tombstone { id: first.id, revision: 3, relationship_version: 3 }).unwrap())
        .unwrap();
    append_version(&mut owner.container, tombstone, 3, 3);
    owner
}

#[test]
fn compact_tail_replays_exact_history_terminal_and_transaction_times() {
    let scratch = Scratch::new();
    let source_path = scratch.file("source.rel");
    drop(raw_owner(&source_path));
    let mut source = Cva::open(&source_path).unwrap();
    let source_bytes = fs::read(&source_path).unwrap();
    let first = record(1, 1, 1, "relationship-create");
    let mut second = record(2, 2, 2, "relationship-revise");
    second.participants[0].role = Some("  contributor  ".into());
    second.evidence.push(MemoryRef {
        owner_id: "external-memory-b".into(),
        memory_id: MemoryId([0x44; 32]),
    });
    second.summary = "  revised description  ".into();
    second.updated_at_ns = 17;
    let tail = read_relationship_tail(&mut source, 0).unwrap();
    assert_eq!(fs::read(&source_path).unwrap(), source_bytes);

    let destination_path = scratch.file("destination.rel");
    let mut destination = Cva::create(&destination_path).unwrap();
    let result = replay_relationship_tail(&mut destination, tail).unwrap();
    assert_eq!(result.revisions, 3);
    assert_eq!(result.duplicate_revisions, 0);
    assert_eq!(
        destination.relationships.records(),
        &[first.clone(), second.clone()]
    );
    assert_eq!(
        source.relationships.records(),
        &[first.clone(), second.clone()]
    );
    for global in 1..=3 {
        assert_eq!(
            source.transaction_time_ns(global),
            destination.transaction_time_ns(global)
        );
    }
    assert_eq!(destination.transaction_time_ns(1), Some(10_001));
    assert_eq!(destination.transaction_time_ns(2), Some(10_002));
    assert_eq!(destination.transaction_time_ns(3), Some(10_003));
    assert_eq!(
        destination.relationship_lifecycle_state(first.id).unwrap(),
        LifecycleState::Tombstoned(TombstoneCoordinates {
            global_version: Some(3),
            domain_version: Some(3),
        })
    );

    let mut publications = Vec::new();
    for chunk in destination.container.chunks().unwrap() {
        let payload = destination.container.read(chunk).unwrap();
        if let Some(envelope) = publication_envelope::decode(&payload).unwrap() {
            publications.push(
                publication::decode(envelope.body, envelope.global_version)
                    .unwrap()
                    .unwrap(),
            );
        }
    }
    assert_eq!(publications.len(), 3);
    assert!(matches!(
        &publications[0],
        RelationshipPublication::Creation(creation)
            if creation.record == first
                && creation.record.global_version == 1
                && creation.record.relationship_version == 1
    ));
    assert!(matches!(
        &publications[1],
        RelationshipPublication::Delta(delta)
            if delta.id == first.id
                && delta.previous_revision == 1
                && delta.revision == 2
                && delta.global_version == 2
                && delta.relationship_version == 2
    ));
    assert!(matches!(
        &publications[2],
        RelationshipPublication::Tombstone { id, revision: 3, relationship_version: 3 }
            if *id == first.id
    ));
}

#[test]
fn orphan_compact_relationship_carrier_is_inert_during_tail_replay() {
    let scratch = Scratch::new();
    let source_path = scratch.file("orphan-source.rel");
    let mut source = raw_owner(&source_path);

    let mut orphan = record(1, 99, 99, "orphan-create");
    orphan.id = RelationshipId([0x72; 32]);
    let orphan_bytes = encode_creation(&RelationshipCreation::new(orphan).unwrap()).unwrap();
    source.container.append(&orphan_bytes).unwrap();
    source.sync().unwrap();
    drop(source);
    let mut source = Cva::open(&source_path).unwrap();

    let tail = read_relationship_tail(&mut source, 0).unwrap();
    let destination_path = scratch.file("orphan-destination.rel");
    let mut destination = Cva::create(&destination_path).unwrap();
    let result = replay_relationship_tail(&mut destination, tail).unwrap();

    assert_eq!(result.revisions, 3);
    assert_eq!(result.duplicate_revisions, 0);
    assert_eq!(
        destination.relationships.records(),
        source.relationships.records()
    );
    assert!(
        destination
            .relationships
            .records()
            .iter()
            .all(|record| record.id != RelationshipId([0x72; 32]))
    );
}

#[test]
fn relationship_tail_rejects_pruned_payload_with_typed_unavailability() {
    use crate::relationship_model::RelationshipRetainedRevision;

    let scratch = Scratch::new();
    let path = scratch.file("pruned.rel");
    let mut owner = Cva::create(&path).unwrap();
    let record = record(1, 1, 1, "pruned-create");
    let carrier = owner
        .container
        .append(&encode_creation(&RelationshipCreation::new(record.clone()).unwrap()).unwrap())
        .unwrap();
    let global = owner
        .container
        .allocate_version_with_transaction_time(Some(20_001))
        .unwrap();
    append_version(&mut owner.container, carrier, global, 1);
    let mut retained = record.clone();
    retained.global_version = global;
    retained.relationship_version = 1;
    owner
        .relationships
        .insert_rebuilt(retained.clone())
        .unwrap();
    owner.relationships.pruned_revisions.insert((record.id, 1));
    assert!(matches!(
        owner
            .relationships
            .retained_publication_at(global, 1)
            .unwrap(),
        Some(
            crate::relationship_store::RelationshipReplayPublication::PrunedRecord {
                revision: RelationshipRetainedRevision { revision: 1, .. },
                ..
            }
        )
    ));
    assert!(matches!(
        read_relationship_tail(&mut owner, 0),
        Err(crate::CvaReconcileError::Relationships(
            crate::RelationshipError::PayloadUnavailable(
                crate::owner_lifecycle::PayloadUnavailable::Pruned
            )
        ))
    ));
}
