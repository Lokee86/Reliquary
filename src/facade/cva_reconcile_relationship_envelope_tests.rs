//! Relationship reconciliation of owner publication envelopes.
use super::{read_relationship_range, replay_relationship_tail};
use crate::entity_compact_codec::publication as entity_publication;
use crate::entity_compact_contract::EntityCreation;
use crate::entity_model::EntityRecord;
use crate::owner_lifecycle::{LifecycleState, TombstoneCoordinates};
use crate::relationship_compact_codec::publication::{self, RelationshipPublication};
use crate::relationship_compact_contract::{RelationshipCreation, RelationshipDelta};
use crate::relationship_model::RelationshipRecord;
use crate::{
    Cva, EntityGlobalId, EntityId, EntityRef, MemoryId, MemoryRef, RelationshipId,
    RelationshipParticipant,
};
use reliquary_container::Container;
use reliquary_container::publication_envelope::{self, PublicationClock, PublicationEnvelope};
use std::fs;
use std::path::{Path, PathBuf};

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!(
                "cva-reconcile-relationship-envelope-{}",
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

fn record(id: u8, revision: u64, global: u64, local: u64, mutation: &str) -> RelationshipRecord {
    RelationshipRecord {
        id: RelationshipId([id; 32]),
        revision,
        kind: "supports".into(),
        participants: vec![
            RelationshipParticipant {
                entity: EntityRef {
                    owner_id: "project-owner".into(),
                    entity_id: EntityId([id.wrapping_add(10); 32]),
                },
                role: Some("source".into()),
            },
            RelationshipParticipant {
                entity: EntityRef {
                    owner_id: "user-owner".into(),
                    entity_id: EntityId([id.wrapping_add(20); 32]),
                },
                role: Some("target".into()),
            },
        ],
        evidence: vec![MemoryRef {
            owner_id: "project-owner".into(),
            memory_id: MemoryId([id.wrapping_add(30); 32]),
        }],
        summary: format!("relationship revision {revision}"),
        mutation_id: mutation.into(),
        created_at_ns: 100,
        updated_at_ns: 100 + revision as i64,
        global_version: global,
        relationship_version: local,
    }
}

fn append_allocated(container: &mut Container, value: &RelationshipPublication, time: Option<i64>) {
    let body = publication::encode(value).unwrap();
    container.set_next_transaction_time_override(time);
    container.append_publication_envelope(&body).unwrap();
}

fn append_reserved(
    container: &mut Container,
    global: u64,
    value: &RelationshipPublication,
) -> usize {
    let body = publication::encode(value).unwrap();
    let frame = publication_envelope::encode(PublicationEnvelope {
        global_version: global,
        clock: PublicationClock::BindReserved,
        body: &body,
    })
    .unwrap();
    let slot = container.chunks().unwrap().len();
    container.append(&frame).unwrap();
    slot
}

fn create_base(path: &Path, id: u8) -> RelationshipRecord {
    let mut owner = Cva::create(path).unwrap();
    let base = record(id, 1, 1, 1, "create");
    append_allocated(
        &mut owner.container,
        &RelationshipPublication::Creation(RelationshipCreation::new(base.clone()).unwrap()),
        Some(100),
    );
    owner.sync().unwrap();
    base
}

fn append_reverse_tail(path: &Path, base: &RelationshipRecord) -> (usize, usize) {
    let mut owner = Cva::open(path).unwrap();
    assert_eq!(
        owner
            .container
            .allocate_version_with_transaction_time(Some(222))
            .unwrap(),
        2
    );
    assert_eq!(
        owner
            .container
            .allocate_version_with_transaction_time(Some(333))
            .unwrap(),
        3
    );
    let mut second = base.clone();
    second.revision = 2;
    second.global_version = 3;
    second.relationship_version = 2;
    second.mutation_id = "revision-two".into();
    second.summary = "second revision".into();
    second.updated_at_ns = 222;
    let second_slot = append_reserved(
        &mut owner.container,
        3,
        &RelationshipPublication::Delta(RelationshipDelta::between(base, &second).unwrap()),
    );
    let mut third = second.clone();
    third.revision = 3;
    third.global_version = 2;
    third.relationship_version = 3;
    third.mutation_id = "revision-three".into();
    third.summary = "third revision".into();
    third.updated_at_ns = 333;
    let third_slot = append_reserved(
        &mut owner.container,
        2,
        &RelationshipPublication::Delta(RelationshipDelta::between(&second, &third).unwrap()),
    );
    owner.sync().unwrap();
    (second_slot, third_slot)
}

fn relationship_publication_slot(cva: &mut Cva, global: u64, local: u64) -> usize {
    cva.container
        .chunks()
        .unwrap()
        .iter()
        .position(|chunk| {
            let payload = cva.container.read(*chunk).unwrap();
            crate::relationship_compact_codec::publication::coordinates(&payload).unwrap()
                == Some((global, local))
        })
        .expect("accepted Relationship publication slot")
}

#[test]
fn finite_ranges_replay_reverse_reserved_envelopes_and_duplicate_is_inert() {
    let scratch = Scratch::new();
    let source_path = scratch.file("source.rel");
    let base = create_base(&source_path, 1);
    let (second_slot, third_slot) = append_reverse_tail(&source_path, &base);

    let mut source = Cva::open(&source_path).unwrap();
    assert_eq!(source.transaction_time_ns(2), Some(222));
    assert_eq!(source.transaction_time_ns(3), Some(333));
    let mut destination = Cva::create(scratch.file("destination.rel")).unwrap();
    let creation_slot = relationship_publication_slot(&mut source, 1, 1);
    let creation = read_relationship_range(&mut source, creation_slot, creation_slot + 1).unwrap();
    assert_eq!(
        replay_relationship_tail(&mut destination, creation)
            .unwrap()
            .revisions,
        1
    );

    let revision_two = read_relationship_range(&mut source, second_slot, third_slot).unwrap();
    let result = replay_relationship_tail(&mut destination, revision_two).unwrap();
    assert_eq!((result.revisions, result.duplicate_revisions), (1, 0));
    assert_eq!(destination.relationship(base.id).unwrap().revision, 2);
    assert_eq!(
        destination.relationship(base.id).unwrap().summary,
        "second revision"
    );
    assert_eq!(destination.transaction_time_ns(2), Some(333));

    let final_revision = read_relationship_range(&mut source, third_slot, third_slot + 1).unwrap();
    let result = replay_relationship_tail(&mut destination, final_revision).unwrap();
    assert_eq!((result.revisions, result.duplicate_revisions), (1, 0));
    assert_eq!(destination.relationship(base.id).unwrap().revision, 3);
    let before = destination.container.latest_version();
    let duplicate = read_relationship_range(&mut source, third_slot, third_slot + 1).unwrap();
    let result = replay_relationship_tail(&mut destination, duplicate).unwrap();
    assert_eq!((result.revisions, result.duplicate_revisions), (0, 1));
    assert_eq!(destination.container.latest_version(), before);
    assert_eq!(destination.transaction_time_ns(3), Some(222));
}

#[test]
fn terminal_envelope_replays_with_terminal_coordinates() {
    let scratch = Scratch::new();
    let source_path = scratch.file("terminal-source.rel");
    let mut source = Cva::create(&source_path).unwrap();
    let base = record(2, 1, 1, 1, "create");
    append_allocated(
        &mut source.container,
        &RelationshipPublication::Creation(RelationshipCreation::new(base.clone()).unwrap()),
        Some(101),
    );
    append_allocated(
        &mut source.container,
        &RelationshipPublication::Tombstone {
            id: base.id,
            revision: 2,
            relationship_version: 2,
        },
        Some(202),
    );
    source.sync().unwrap();
    drop(source);
    let mut source = Cva::open(&source_path).unwrap();
    let terminal_slot = source.container.chunks().unwrap().len() - 1;
    let mut destination = Cva::create(scratch.file("terminal-destination.rel")).unwrap();
    let initial = read_relationship_range(&mut source, 0, terminal_slot).unwrap();
    replay_relationship_tail(&mut destination, initial).unwrap();
    let terminal = read_relationship_range(&mut source, terminal_slot, terminal_slot + 1).unwrap();
    let result = replay_relationship_tail(&mut destination, terminal).unwrap();
    assert_eq!((result.revisions, result.duplicate_revisions), (1, 0));
    assert_eq!(
        destination.relationship_lifecycle_state(base.id).unwrap(),
        LifecycleState::Tombstoned(TombstoneCoordinates {
            global_version: Some(2),
            domain_version: Some(2),
        })
    );
}

#[test]
fn reconcile_replays_relationship_from_divergent_supported_owner_history() {
    let scratch = Scratch::new();
    let base_path = scratch.file("base.rel");
    let base = create_base(&base_path, 3);
    let left_path = scratch.file("left.rel");
    let right_path = scratch.file("right.rel");
    fs::copy(&base_path, &left_path).unwrap();
    fs::copy(&base_path, &right_path).unwrap();

    let mut right = Cva::open(&right_path).unwrap();
    let mut right_revision = base.clone();
    right_revision.revision = 2;
    right_revision.global_version = 2;
    right_revision.relationship_version = 2;
    right_revision.mutation_id = "right-branch-update".into();
    right_revision.summary = "right branch relationship update".into();
    right_revision.updated_at_ns = 202;
    append_allocated(
        &mut right.container,
        &RelationshipPublication::Delta(
            RelationshipDelta::between(&base, &right_revision).unwrap(),
        ),
        Some(202),
    );
    right.sync().unwrap();
    drop(right);

    let mut left = Cva::open(&left_path).unwrap();
    let independent = record(4, 1, 2, 2, "left-branch-create");
    append_allocated(
        &mut left.container,
        &RelationshipPublication::Creation(RelationshipCreation::new(independent.clone()).unwrap()),
        Some(303),
    );
    left.sync().unwrap();
    drop(left);

    let output = scratch.file("reconciled.rel");
    let result = Cva::reconcile(&left_path, &right_path, &output).unwrap();
    assert_eq!(result.replayed_relationship_revisions, 1);
    let reconciled = Cva::open(&output).unwrap();
    assert_eq!(
        reconciled.relationship(base.id).unwrap().summary,
        "right branch relationship update"
    );
    assert_eq!(reconciled.relationship(independent.id).unwrap().revision, 1);
    assert_eq!(reconciled.relationship_version(), 3);
    assert_eq!(reconciled.transaction_time_ns(3), Some(202));
}

#[test]
fn relationship_range_skips_valid_foreign_envelopes_and_rejects_malformed_ones() {
    let scratch = Scratch::new();
    let path = scratch.file("classification.rel");
    create_base(&path, 5);
    let mut owner = Cva::open(&path).unwrap();

    let entity = EntityRecord {
        id: EntityId([0x71; 32]),
        global_id: Some(EntityGlobalId([0x72; 16])),
        revision: 1,
        canonical_name: "foreign entity".into(),
        aliases: vec![],
        kind: "project".into(),
        summary: "foreign owner frame".into(),
        mutation_id: "foreign-entity-create".into(),
        created_at_ns: 1,
        updated_at_ns: 1,
        global_version: 2,
        entity_version: 1,
    };
    let foreign_body = entity_publication::encode(
        &entity_publication::EntityPublication::Creation(EntityCreation::new(entity).unwrap()),
    )
    .unwrap();
    owner
        .container
        .append_publication_envelope(&foreign_body)
        .unwrap();
    let tail = read_relationship_range(&mut owner, 0, usize::MAX).unwrap();
    let mut destination = Cva::create(scratch.file("foreign-destination.rel")).unwrap();
    assert_eq!(
        replay_relationship_tail(&mut destination, tail)
            .unwrap()
            .revisions,
        1
    );

    owner
        .container
        .append_publication_envelope(b"CVARNC02")
        .unwrap();
    assert!(read_relationship_range(&mut owner, 0, usize::MAX).is_err());
}
