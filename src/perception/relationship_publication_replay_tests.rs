//! Relationship publication-envelope replay and history admission through REL and PHY.
use crate::container::publication_envelope::{PublicationClock, PublicationEnvelope};
use crate::relationship_compact_codec::publication::{self, RelationshipPublication};
use crate::relationship_compact_contract::{RelationshipCreation, RelationshipDelta};
use crate::relationship_model::RelationshipRecord;
use crate::{
    Container, Cva, EntityId, EntityRef, MemoryId, MemoryRef, Phylactery, RelationshipId,
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
                "relationship-publication-replay-{}",
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

fn reserve(container: &mut Container, time: i64) -> u64 {
    container.set_next_transaction_time_override(Some(time));
    container.allocate_version().unwrap()
}

fn append_reserved(container: &mut Container, global: u64, value: &RelationshipPublication) {
    let body = publication::encode(value).unwrap();
    let bytes = crate::container::publication_envelope::encode(PublicationEnvelope {
        global_version: global,
        clock: PublicationClock::BindReserved,
        body: &body,
    })
    .unwrap();
    container.append(&bytes).unwrap();
}

trait RelationshipOwner {
    fn relationship_version(&self) -> u64;
    fn latest_global(&self) -> u64;
    fn relationship(
        &self,
        id: RelationshipId,
    ) -> Result<crate::Relationship, crate::RelationshipError>;
    fn retention(
        &self,
        id: RelationshipId,
    ) -> Result<crate::RelationshipRetentionDescriptor, crate::RelationshipError>;
    fn time(&self, global: u64) -> Option<i64>;
}

impl RelationshipOwner for Cva {
    fn relationship_version(&self) -> u64 {
        Cva::relationship_version(self)
    }
    fn latest_global(&self) -> u64 {
        self.container.latest_version()
    }
    fn relationship(
        &self,
        id: RelationshipId,
    ) -> Result<crate::Relationship, crate::RelationshipError> {
        Cva::relationship(self, id)
    }
    fn retention(
        &self,
        id: RelationshipId,
    ) -> Result<crate::RelationshipRetentionDescriptor, crate::RelationshipError> {
        Cva::relationship_retention_descriptor(self, id)
    }
    fn time(&self, global: u64) -> Option<i64> {
        self.transaction_time_ns(global)
    }
}

impl RelationshipOwner for Phylactery {
    fn relationship_version(&self) -> u64 {
        Phylactery::relationship_version(self)
    }
    fn latest_global(&self) -> u64 {
        self.container.latest_version()
    }
    fn relationship(
        &self,
        id: RelationshipId,
    ) -> Result<crate::Relationship, crate::RelationshipError> {
        Phylactery::relationship(self, id)
    }
    fn retention(
        &self,
        id: RelationshipId,
    ) -> Result<crate::RelationshipRetentionDescriptor, crate::RelationshipError> {
        Phylactery::relationship_retention_descriptor(self, id)
    }
    fn time(&self, global: u64) -> Option<i64> {
        self.transaction_time_ns(global)
    }
}

fn assert_terminal_history(owner: &impl RelationshipOwner, id: RelationshipId) {
    assert_eq!(owner.relationship_version(), 3);
    assert_eq!(owner.latest_global(), 3);
    assert!(owner.relationship(id).is_err());
    let descriptor = owner.retention(id).unwrap();
    assert_eq!(descriptor.tombstone_revision, 3);
    assert_eq!(descriptor.tombstone_relationship_version, 3);
    assert_eq!(descriptor.revisions.len(), 2);
    assert_eq!(descriptor.revisions[0].revision, 1);
    assert_eq!(descriptor.revisions[1].revision, 2);
    assert_eq!(descriptor.revisions[0].global_version, 1);
    assert_eq!(descriptor.revisions[0].relationship_version, 1);
    assert_eq!(descriptor.revisions[1].global_version, 2);
    assert_eq!(descriptor.revisions[1].relationship_version, 2);
    assert_eq!(descriptor.revisions[0].kind, "supports");
    assert_eq!(descriptor.revisions[0].participants.len(), 2);
    assert_eq!(
        descriptor.revisions[0].participants[0].role.as_deref(),
        Some("source")
    );
    assert_eq!(descriptor.revisions[0].evidence.len(), 1);
    assert_eq!(descriptor.revisions[0].created_at_ns, 100);
    assert_eq!(descriptor.revisions[1].updated_at_ns, 202);
    assert_eq!(descriptor.revisions[1].mutation_id, "update");
}

fn create_update_delete(container: &mut Container) -> RelationshipId {
    let first = record(41, 1, 1, 1, "create");
    append_allocated(
        container,
        &RelationshipPublication::Creation(RelationshipCreation::new(first.clone()).unwrap()),
        Some(101),
    );
    let mut second = first.clone();
    second.revision = 2;
    second.global_version = 2;
    second.relationship_version = 2;
    second.mutation_id = "update".into();
    second.summary = "updated relationship".into();
    second.updated_at_ns = 202;
    append_allocated(
        container,
        &RelationshipPublication::Delta(RelationshipDelta::between(&first, &second).unwrap()),
        Some(202),
    );
    append_allocated(
        container,
        &RelationshipPublication::Tombstone {
            id: first.id,
            revision: 3,
            relationship_version: 3,
        },
        Some(303),
    );
    first.id
}

#[test]
fn create_delta_and_terminal_reopen_with_history_in_rel_and_phy() {
    let scratch = Scratch::new();
    let rel_path = scratch.file("history.rel");
    let mut rel = Cva::create(&rel_path).unwrap();
    let rel_id = create_update_delete(&mut rel.container);
    rel.sync().unwrap();
    drop(rel);
    let rel = Cva::open(&rel_path).unwrap();
    assert_terminal_history(&rel, rel_id);
    assert_eq!(rel.time(1), Some(101));
    assert_eq!(rel.time(2), Some(202));
    assert_eq!(rel.time(3), Some(303));

    let phy_path = scratch.file("history.phy");
    let mut phy = Phylactery::create(&phy_path).unwrap();
    let phy_id = create_update_delete(&mut phy.container);
    phy.sync().unwrap();
    drop(phy);
    let phy = Phylactery::open(&phy_path).unwrap();
    assert_terminal_history(&phy, phy_id);
    assert_eq!(phy.time(1), Some(101));
    assert_eq!(phy.time(2), Some(202));
    assert_eq!(phy.time(3), Some(303));
}

#[test]
fn reverse_reserved_globals_preserve_dense_relationship_order_and_times() {
    let scratch = Scratch::new();
    let path = scratch.file("reverse-reserved.rel");
    let mut rel = Cva::create(&path).unwrap();
    assert_eq!(reserve(&mut rel.container, 111), 1);
    assert_eq!(reserve(&mut rel.container, 222), 2);
    let first = record(51, 1, 2, 1, "create");
    let mut second = first.clone();
    second.revision = 2;
    second.global_version = 1;
    second.relationship_version = 2;
    second.mutation_id = "update".into();
    second.summary = "revision two".into();
    append_reserved(
        &mut rel.container,
        2,
        &RelationshipPublication::Creation(RelationshipCreation::new(first.clone()).unwrap()),
    );
    append_reserved(
        &mut rel.container,
        1,
        &RelationshipPublication::Delta(RelationshipDelta::between(&first, &second).unwrap()),
    );
    rel.sync().unwrap();
    drop(rel);
    let rel = Cva::open(&path).unwrap();
    assert_eq!(rel.relationship_version(), 2);
    let current = rel.relationship(first.id).unwrap();
    assert_eq!(current.revision, 2);
    assert_eq!(current.global_version, 1);
    assert_eq!(current.relationship_version, 2);
    assert_eq!(rel.time(1), Some(111));
    assert_eq!(rel.time(2), Some(222));
}

fn write_publications(path: &Path, values: &[RelationshipPublication]) {
    let mut rel = Cva::create(path).unwrap();
    for value in values {
        append_allocated(&mut rel.container, value, None);
    }
    rel.sync().unwrap();
}

#[test]
fn missing_predecessor_duplicate_local_position_and_resurrection_reject_open() {
    let scratch = Scratch::new();
    let base = record(61, 1, 1, 1, "create");
    let mut next = base.clone();
    next.revision = 2;
    next.global_version = 2;
    next.relationship_version = 2;
    next.mutation_id = "update".into();

    let missing = scratch.file("missing-predecessor.rel");
    write_publications(
        &missing,
        &[RelationshipPublication::Delta(
            RelationshipDelta::between(&base, &next).unwrap(),
        )],
    );
    assert!(Cva::open(&missing).is_err());

    let duplicate = scratch.file("duplicate-local.rel");
    write_publications(
        &duplicate,
        &[
            RelationshipPublication::Creation(
                RelationshipCreation::new(record(62, 1, 1, 1, "create-a")).unwrap(),
            ),
            RelationshipPublication::Creation(
                RelationshipCreation::new(record(63, 1, 2, 1, "create-b")).unwrap(),
            ),
        ],
    );
    assert!(Cva::open(&duplicate).is_err());

    let gap = scratch.file("local-gap.rel");
    write_publications(
        &gap,
        &[RelationshipPublication::Creation(
            RelationshipCreation::new(record(64, 1, 1, 2, "gap")).unwrap(),
        )],
    );
    assert!(Cva::open(&gap).is_err());

    let resurrection = scratch.file("resurrection.rel");
    write_publications(
        &resurrection,
        &[
            RelationshipPublication::Creation(
                RelationshipCreation::new(record(65, 1, 1, 1, "create")).unwrap(),
            ),
            RelationshipPublication::Tombstone {
                id: RelationshipId([65; 32]),
                revision: 2,
                relationship_version: 2,
            },
            RelationshipPublication::Creation(
                RelationshipCreation::new(record(65, 1, 3, 3, "resurrect")).unwrap(),
            ),
        ],
    );
    assert!(Cva::open(&resurrection).is_err());
}

#[test]
fn zero_global_coordinate_is_rejected_by_publication_decoder() {
    let value = RelationshipPublication::Creation(
        RelationshipCreation::new(record(71, 1, 1, 1, "create")).unwrap(),
    );
    let body = publication::encode(&value).unwrap();
    assert!(publication::decode(&body, 0).is_err());
}
