//! Entity publication-envelope admission through the REL and PHY facades.
use crate::entity_compact_codec::publication::{self, EntityPublication};
use crate::entity_compact_contract::{EntityCreation, EntityDelta};
use crate::entity_model::EntityRecord;
use crate::{Cva, EntityGlobalId, EntityId, MemoryDraft, Phylactery};
use reliquary_container::Container;
use reliquary_container::publication_envelope::{PublicationClock, PublicationEnvelope};
use std::fs;
use std::path::{Path, PathBuf};

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!(
                "entity-publication-replay-{}",
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

fn record(
    id: u8,
    name: &str,
    mutation_id: &str,
    revision: u64,
    global: u64,
    local: u64,
) -> EntityRecord {
    EntityRecord {
        id: EntityId([id; 32]),
        global_id: Some(EntityGlobalId([id.wrapping_add(80); 16])),
        revision,
        canonical_name: name.into(),
        aliases: vec![format!("{name} alias")],
        kind: "project".into(),
        summary: format!("Summary for {name}"),
        mutation_id: mutation_id.into(),
        created_at_ns: 10,
        updated_at_ns: 20 + revision as i64,
        global_version: global,
        entity_version: local,
    }
}

fn append_allocated(
    container: &mut Container,
    publication: &EntityPublication,
    time_ns: Option<i64>,
) -> u64 {
    let body = publication::encode(publication).unwrap();
    container.set_next_transaction_time_override(time_ns);
    container.append_publication_envelope(&body).unwrap().0
}

fn reserve(container: &mut Container, time_ns: Option<i64>) -> u64 {
    container.set_next_transaction_time_override(time_ns);
    container.allocate_version().unwrap()
}

fn append_reserved_binding(
    container: &mut Container,
    global: u64,
    publication: &EntityPublication,
) {
    let body = publication::encode(publication).unwrap();
    let envelope = reliquary_container::publication_envelope::encode(PublicationEnvelope {
        global_version: global,
        clock: PublicationClock::BindReserved,
        body: &body,
    })
    .unwrap();
    container.append(&envelope).unwrap();
}

fn append_creation(container: &mut Container, value: EntityRecord, time: Option<i64>) {
    append_allocated(
        container,
        &EntityPublication::Creation(EntityCreation::new(value).unwrap()),
        time,
    );
}

fn append_delta(
    container: &mut Container,
    previous: &EntityRecord,
    next: EntityRecord,
    time: Option<i64>,
) {
    let delta = EntityDelta::between(previous, &next).unwrap();
    append_allocated(container, &EntityPublication::Delta(delta), time);
}

fn assert_terminal_history<T>(owner: &T, id: EntityId, final_global: u64, final_local: u64)
where
    T: EntityOwnerView,
{
    assert_eq!(owner.entity_version(), final_local);
    assert_eq!(owner.latest_global_version(), final_global);
    assert!(owner.entity(id).is_err());
    let retained = owner.entity_retention_descriptor(id).unwrap();
    assert_eq!(retained.tombstone_revision, 3);
    assert_eq!(retained.revisions.len(), 2);
    assert_eq!(retained.revisions[1].global_version, 2);
}

trait EntityOwnerView {
    fn entity_version(&self) -> u64;
    fn latest_global_version(&self) -> u64;
    fn entity(&self, id: EntityId) -> Result<crate::Entity, crate::EntityError>;
    fn entity_retention_descriptor(
        &self,
        id: EntityId,
    ) -> Result<crate::EntityRetentionDescriptor, crate::EntityError>;
}

impl EntityOwnerView for Cva {
    fn entity_version(&self) -> u64 {
        Cva::entity_version(self)
    }
    fn latest_global_version(&self) -> u64 {
        Cva::latest_global_version(self)
    }
    fn entity(&self, id: EntityId) -> Result<crate::Entity, crate::EntityError> {
        Cva::entity(self, id)
    }
    fn entity_retention_descriptor(
        &self,
        id: EntityId,
    ) -> Result<crate::EntityRetentionDescriptor, crate::EntityError> {
        Cva::entity_retention_descriptor(self, id)
    }
}

impl EntityOwnerView for Phylactery {
    fn entity_version(&self) -> u64 {
        Phylactery::entity_version(self)
    }
    fn latest_global_version(&self) -> u64 {
        Phylactery::latest_global_version(self)
    }
    fn entity(&self, id: EntityId) -> Result<crate::Entity, crate::EntityError> {
        Phylactery::entity(self, id)
    }
    fn entity_retention_descriptor(
        &self,
        id: EntityId,
    ) -> Result<crate::EntityRetentionDescriptor, crate::EntityError> {
        Phylactery::entity_retention_descriptor(self, id)
    }
}

fn write_create_update_direct_terminal(container: &mut Container) -> EntityId {
    let first = record(11, "Entity", "entity-create", 1, 1, 1);
    append_creation(container, first.clone(), Some(101));
    let mut second = first.clone();
    second.revision = 2;
    second.global_version = 2;
    second.entity_version = 2;
    second.mutation_id = "entity-update".into();
    second.canonical_name = "Entity revised".into();
    second.updated_at_ns = 30;
    append_delta(container, &first, second, Some(202));
    append_allocated(
        container,
        &EntityPublication::DirectTombstone {
            id: first.id,
            revision: 3,
            entity_version: 3,
        },
        Some(303),
    );
    first.id
}

#[test]
fn entity_create_delta_and_direct_terminal_reopen_in_rel_and_phy() {
    let scratch = Scratch::new();
    let rel_path = scratch.file("history.rel");
    let mut rel = Cva::create(&rel_path).unwrap();
    let rel_id = write_create_update_direct_terminal(&mut rel.container);
    rel.sync().unwrap();
    drop(rel);
    let rel = Cva::open(&rel_path).unwrap();
    assert_terminal_history(&rel, rel_id, 3, 3);
    assert_eq!(rel.transaction_time_ns(1), Some(101));
    assert_eq!(rel.transaction_time_ns(2), Some(202));
    assert_eq!(rel.transaction_time_ns(3), Some(303));

    let phy_path = scratch.file("history.phy");
    let mut phy = Phylactery::create(&phy_path).unwrap();
    let phy_id = write_create_update_direct_terminal(&mut phy.container);
    phy.sync().unwrap();
    drop(phy);
    let phy = Phylactery::open(&phy_path).unwrap();
    assert_terminal_history(&phy, phy_id, 3, 3);
    assert_eq!(phy.transaction_time_ns(1), Some(101));
    assert_eq!(phy.transaction_time_ns(2), Some(202));
    assert_eq!(phy.transaction_time_ns(3), Some(303));
}

#[test]
fn merge_tombstones_reopen_with_the_complete_replacement_chain() {
    let scratch = Scratch::new();
    let path = scratch.file("merge.rel");
    let mut rel = Cva::create(&path).unwrap();
    let first = record(21, "First", "first", 1, 1, 1);
    let second = record(22, "Second", "second", 1, 2, 2);
    let third = record(23, "Third", "third", 1, 3, 3);
    append_creation(&mut rel.container, first.clone(), None);
    append_creation(&mut rel.container, second.clone(), None);
    append_creation(&mut rel.container, third.clone(), None);
    append_allocated(
        &mut rel.container,
        &EntityPublication::MergeTombstone {
            id: first.id,
            revision: 2,
            entity_version: 4,
            replacement_id: second.id,
        },
        None,
    );
    append_allocated(
        &mut rel.container,
        &EntityPublication::MergeTombstone {
            id: second.id,
            revision: 2,
            entity_version: 5,
            replacement_id: third.id,
        },
        None,
    );
    rel.sync().unwrap();
    drop(rel);

    let rel = Cva::open(&path).unwrap();
    assert_eq!(rel.entity_version(), 5);
    assert_eq!(rel.retired_entity_replacement(first.id), Some(second.id));
    assert_eq!(rel.retired_entity_replacement(second.id), Some(third.id));
    assert_eq!(rel.entity(third.id).unwrap().canonical_name, "Third");
}

#[test]
fn reserved_entity_bindings_keep_reverse_global_order_and_reserved_times() {
    let scratch = Scratch::new();
    let path = scratch.file("reserved.rel");
    let mut rel = Cva::create(&path).unwrap();
    assert_eq!(reserve(&mut rel.container, Some(111)), 1);
    assert_eq!(reserve(&mut rel.container, Some(222)), 2);

    let first = record(31, "First local revision", "first", 1, 2, 1);
    let mut second = first.clone();
    second.revision = 2;
    second.global_version = 1;
    second.entity_version = 2;
    second.mutation_id = "second".into();
    second.summary = "Second local revision".into();
    append_reserved_binding(
        &mut rel.container,
        2,
        &EntityPublication::Creation(EntityCreation::new(first.clone()).unwrap()),
    );
    append_reserved_binding(
        &mut rel.container,
        1,
        &EntityPublication::Delta(EntityDelta::between(&first, &second).unwrap()),
    );
    rel.sync().unwrap();
    drop(rel);

    let rel = Cva::open(&path).unwrap();
    let current = rel.entity(first.id).unwrap();
    assert_eq!(current.revision, 2);
    assert_eq!(current.global_version, 1);
    assert_eq!(current.entity_version, 2);
    assert_eq!(rel.latest_global_version(), 2);
    assert_eq!(rel.transaction_time_ns(1), Some(111));
    assert_eq!(rel.transaction_time_ns(2), Some(222));
}

fn invalid_file(path: &Path, publications: &[EntityPublication]) {
    let mut rel = Cva::create(path).unwrap();
    for value in publications {
        append_allocated(&mut rel.container, value, Some(10));
    }
    rel.sync().unwrap();
}

#[test]
fn invalid_entity_delta_base_local_gap_and_duplicate_terminal_reject_reopen() {
    let scratch = Scratch::new();

    let missing_path = scratch.file("missing-base.rel");
    let missing_base = record(41, "Missing", "missing", 1, 1, 1);
    let mut missing_next = missing_base.clone();
    missing_next.revision = 2;
    missing_next.global_version = 2;
    missing_next.entity_version = 2;
    missing_next.mutation_id = "missing-next".into();
    missing_next.summary = "changed".into();
    invalid_file(
        &missing_path,
        &[EntityPublication::Delta(
            EntityDelta::between(&missing_base, &missing_next).unwrap(),
        )],
    );
    assert!(Cva::open(&missing_path).is_err());

    let gap_path = scratch.file("local-gap.rel");
    invalid_file(
        &gap_path,
        &[EntityPublication::Creation(
            EntityCreation::new(record(42, "Gap", "gap", 1, 1, 2)).unwrap(),
        )],
    );
    assert!(Cva::open(&gap_path).is_err());

    let terminal_path = scratch.file("duplicate-terminal.rel");
    let base = record(43, "Terminal", "terminal", 1, 1, 1);
    let mut rel = Cva::create(&terminal_path).unwrap();
    append_creation(&mut rel.container, base.clone(), Some(10));
    append_allocated(
        &mut rel.container,
        &EntityPublication::DirectTombstone {
            id: base.id,
            revision: 2,
            entity_version: 2,
        },
        Some(20),
    );
    append_allocated(
        &mut rel.container,
        &EntityPublication::DirectTombstone {
            id: base.id,
            revision: 3,
            entity_version: 3,
        },
        Some(30),
    );
    rel.sync().unwrap();
    drop(rel);
    assert!(Cva::open(&terminal_path).is_err());
}

#[test]
fn unknown_entity_body_and_v1_envelope_reject_facade_open() {
    let scratch = Scratch::new();
    let unknown_path = scratch.file("unknown-body.rel");
    let mut rel = Cva::create(&unknown_path).unwrap();
    rel.container
        .append_publication_envelope(b"CVAENC99 unknown owner body")
        .unwrap();
    rel.sync().unwrap();
    drop(rel);
    assert!(Cva::open(&unknown_path).is_err());

    let v1_path = scratch.file("v1-envelope.rel");
    let mut rel = Cva::create(&v1_path).unwrap();
    append_creation(&mut rel.container, record(51, "V1", "v1", 1, 1, 1), None);
    rel.sync().unwrap();
    drop(rel);
    let mut bytes = fs::read(&v1_path).unwrap();
    bytes[8..10].copy_from_slice(&1_u16.to_le_bytes());
    bytes[10..12].copy_from_slice(&0_u16.to_le_bytes());
    fs::write(&v1_path, bytes).unwrap();
    assert!(Cva::open(&v1_path).is_err());
}

fn memory_draft(mutation_id: &str) -> MemoryDraft {
    MemoryDraft {
        category: "fact".into(),
        memory_type: "project".into(),
        authority_kind: "direct".into(),
        temporal_status: "current".into(),
        title: "Memory alongside Entity".into(),
        content: "Both owners use the canonical publication envelope.".into(),
        scope: "project".into(),
        lifecycle_state: "knowledge".into(),
        archived: false,
        superseded_by: None,
        parent_id: None,
        source_node_id: None,
        content_source_conversation_id: None,
        content_source_node_id: None,
        grounding_source_conversation_id: None,
        grounding_source_node_id: None,
        source_episode_id: None,
        source_time_ns: None,
        mutation_id: mutation_id.into(),
        created_at_ns: 10,
        updated_at_ns: 10,
    }
}

#[test]
fn entity_envelopes_coexist_with_ordinary_memory_writer_and_reopen() {
    let scratch = Scratch::new();
    let path = scratch.file("memory-and-entity.rel");
    let mut rel = Cva::create(&path).unwrap();
    let (memory, changed) = rel
        .publish_memory(None, 0, memory_draft("memory-create"))
        .unwrap();
    assert!(changed);
    let entity = record(61, "Adjacent Entity", "entity-create", 1, 2, 1);
    append_creation(&mut rel.container, entity.clone(), Some(20));
    rel.sync().unwrap();
    drop(rel);

    let mut rel = Cva::open(&path).unwrap();
    assert_eq!(rel.latest_global_version(), 2);
    assert_eq!(
        rel.memory(memory.id).unwrap().title,
        "Memory alongside Entity"
    );
    assert_eq!(
        rel.entity(entity.id).unwrap().canonical_name,
        "Adjacent Entity"
    );
}
