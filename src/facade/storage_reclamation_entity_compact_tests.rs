//! Storage reclamation retains accepted compact Entity history and drops inert carriers.
fn encode_creation(c: &EntityCreation) -> Result<Vec<u8>, crate::EntityError> {
    crate::entity_compact_codec::publication::encode(
        &crate::entity_compact_codec::publication::EntityPublication::Creation(c.clone()),
    )
}
fn encode_delta(d: &EntityDelta) -> Result<Vec<u8>, crate::EntityError> {
    crate::entity_compact_codec::publication::encode(
        &crate::entity_compact_codec::publication::EntityPublication::Delta(d.clone()),
    )
}
use crate::entity_compact_contract::{EntityCreation, EntityDelta};
use crate::entity_model::EntityRecord;
use crate::owner_lifecycle::{LifecycleState, TombstoneCoordinates};
use crate::{Cva, EntityGlobalId, EntityId, Phylactery};
use reliquary_container::Container;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!("reclaim-compact-entity-{}", uuid::Uuid::new_v4()));
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

fn record(id: u8, mutation_id: &str, revision: u64, global: u64, local: u64) -> EntityRecord {
    EntityRecord {
        id: EntityId([id; 32]),
        global_id: Some(EntityGlobalId([id.wrapping_add(40); 16])),
        revision,
        canonical_name: format!("Entity {id}"),
        aliases: vec![format!("Alias {id}")],
        kind: "project".into(),
        summary: format!("Summary {id} revision {revision}"),
        mutation_id: mutation_id.into(),
        created_at_ns: 10,
        updated_at_ns: 20 + revision as i64,
        global_version: global,
        entity_version: local,
    }
}

fn append_publication(container: &mut Container, payload: &[u8], global: u64, _local: u64) {
    let bytes = reliquary_container::publication_envelope::encode(
        reliquary_container::publication_envelope::PublicationEnvelope {
            global_version: global,
            clock: reliquary_container::publication_envelope::PublicationClock::BindReserved,
            body: payload,
        },
    )
    .unwrap();
    container.append(&bytes).unwrap();
}

fn build_history(path: &Path) -> (Vec<EntityRecord>, Vec<Option<i64>>) {
    let mut container = Container::open(path).unwrap();
    let a1 = record(1, "a-create", 1, 1, 1);
    let a1_bytes = encode_creation(&EntityCreation::new(a1.clone()).unwrap()).unwrap();
    container
        .allocate_version_with_transaction_time(Some(1_001))
        .unwrap();
    append_publication(&mut container, &a1_bytes, 1, 1);

    let mut a2 = a1.clone();
    a2.revision = 2;
    a2.global_version = 2;
    a2.entity_version = 2;
    a2.mutation_id = "a-revise".into();
    a2.canonical_name = "Entity 1 revised".into();
    a2.summary = "Updated Entity 1".into();
    let a2_bytes = encode_delta(&EntityDelta::between(&a1, &a2).unwrap()).unwrap();
    container
        .allocate_version_with_transaction_time(Some(1_002))
        .unwrap();
    append_publication(&mut container, &a2_bytes, 2, 2);

    let b1 = record(2, "b-create", 1, 3, 3);
    let b1_bytes = encode_creation(&EntityCreation::new(b1.clone()).unwrap()).unwrap();
    container
        .allocate_version_with_transaction_time(Some(1_003))
        .unwrap();
    append_publication(&mut container, &b1_bytes, 3, 3);

    let mut b2 = b1.clone();
    b2.revision = 2;
    b2.global_version = 4;
    b2.entity_version = 4;
    b2.mutation_id = "b-revise".into();
    b2.aliases = vec!["New B alias".into()];
    let b2_bytes = encode_delta(&EntityDelta::between(&b1, &b2).unwrap()).unwrap();
    container
        .allocate_version_with_transaction_time(Some(1_004))
        .unwrap();
    append_publication(&mut container, &b2_bytes, 4, 4);

    container
        .allocate_version_with_transaction_time(Some(1_005))
        .unwrap();
    let body = crate::entity_compact_codec::publication::encode(
        &crate::entity_compact_codec::publication::EntityPublication::MergeTombstone {
            id: a1.id,
            revision: 3,
            entity_version: 5,
            replacement_id: b1.id,
        },
    )
    .unwrap();
    append_publication(&mut container, &body, 5, 5);

    let c1 = record(3, "c-create", 1, 6, 6);
    let c1_bytes = encode_creation(&EntityCreation::new(c1.clone()).unwrap()).unwrap();
    container
        .allocate_version_with_transaction_time(Some(1_006))
        .unwrap();
    append_publication(&mut container, &c1_bytes, 6, 6);

    let mut c2 = c1.clone();
    c2.revision = 2;
    c2.global_version = 7;
    c2.entity_version = 7;
    c2.mutation_id = "c-revise".into();
    c2.summary = "Updated Entity 3".into();
    let c2_bytes = encode_delta(&EntityDelta::between(&c1, &c2).unwrap()).unwrap();
    container
        .allocate_version_with_transaction_time(Some(1_007))
        .unwrap();
    append_publication(&mut container, &c2_bytes, 7, 7);

    container
        .allocate_version_with_transaction_time(Some(1_008))
        .unwrap();
    let body = crate::entity_compact_codec::publication::encode(
        &crate::entity_compact_codec::publication::EntityPublication::DirectTombstone {
            id: c1.id,
            revision: 3,
            entity_version: 8,
        },
    )
    .unwrap();
    append_publication(&mut container, &body, 8, 8);

    // These structurally valid compact carriers have no matching version
    // markers. Reclamation should classify them as unpublished backing data.
    let mut orphan = record(9, "orphan-create", 1, 109, 109);
    let orphan_creation = encode_creation(&EntityCreation::new(orphan.clone()).unwrap()).unwrap();
    container.append(&orphan_creation).unwrap();
    orphan.revision = 2;
    orphan.global_version = 110;
    orphan.entity_version = 110;
    orphan.mutation_id = "orphan-delta".into();
    orphan.summary = "Never published".into();
    container
        .append(
            &encode_delta(
                &EntityDelta::between(&record(9, "orphan-create", 1, 109, 109), &orphan).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
    container.sync().unwrap();

    let records = vec![a1, a2, b1, b2, c1, c2];
    let times = (1..=8).map(|global| Some(1_000 + global as i64)).collect();
    (records, times)
}

fn file_hash(path: &Path) -> [u8; 32] {
    Sha256::digest(fs::read(path).unwrap()).into()
}

fn payloads(container: &Container) -> Vec<Vec<u8>> {
    let mut result = Vec::new();
    let source = container.read_source().unwrap();
    let mut cursor = source.physical_cursor().unwrap();
    while let Some(frame) = cursor.next_frame().unwrap() {
        if let Some(envelope) =
            reliquary_container::publication_envelope::decode(&frame.payload).unwrap()
        {
            result.push(envelope.body.to_vec());
        } else {
            result.push(frame.payload);
        }
    }
    result
}

fn assert_compact_counts(container: &Container) {
    let payloads = payloads(container);
    assert_eq!(
        payloads
            .iter()
            .filter(|p| p.starts_with(b"CVAENC02"))
            .count(),
        3
    );
    assert_eq!(
        payloads
            .iter()
            .filter(|p| p.starts_with(b"CVAEND02"))
            .count(),
        3
    );
    assert_eq!(
        payloads
            .iter()
            .filter(|p| p.starts_with(b"CVAETM02"))
            .count(),
        1
    );
    assert_eq!(
        payloads
            .iter()
            .filter(|p| p.starts_with(b"CVAETD02"))
            .count(),
        1
    );
    assert!(
        !payloads
            .iter()
            .any(|p| p.starts_with(b"CVAENTR1") || p.starts_with(b"CVAENTR2"))
    );
    assert!(!payloads.iter().any(|p| {
        p.starts_with(b"CVAENC02")
            && p.get(8..40)
                .is_some_and(|id| id.iter().all(|byte| *byte == 9))
    }));
}

fn assert_records_and_times(
    records: &[EntityRecord],
    times: &[Option<i64>],
    actual: &[EntityRecord],
    transaction_time: impl Fn(u64) -> Option<i64>,
) {
    assert_eq!(actual, records);
    for record in records {
        assert_eq!(
            transaction_time(record.global_version),
            times[(record.global_version - 1) as usize]
        );
    }
}

#[test]
fn rel_reclamation_retains_compact_entity_revisions_and_terminals() {
    let scratch = Scratch::new();
    let source = scratch.file("source.rel");
    let output = scratch.file("reclaimed.rel");
    drop(Cva::create(&source).unwrap());
    let (expected, times) = build_history(&source);
    let original_hash = file_hash(&source);
    let mut rel = Cva::open(&source).unwrap();
    assert_eq!(rel.entities.records(), expected.as_slice());
    let retained = rel.reclaim_storage(&output).unwrap();
    assert!(retained.unpublished_backing_chunks >= 2);
    assert_eq!(file_hash(&source), original_hash);
    assert_eq!(rel.entities.records(), expected.as_slice());
    let reopened = Cva::open(&output).unwrap();
    assert_records_and_times(&expected, &times, reopened.entities.records(), |global| {
        reopened.transaction_time_ns(global)
    });
    assert_eq!(reopened.entity_version(), 8);
    assert_eq!(reopened.latest_global_version(), 8);
    assert_eq!(
        reopened.retired_entity_replacement(EntityId([1; 32])),
        Some(EntityId([2; 32]))
    );
    assert_eq!(reopened.retired_entity_replacement(EntityId([3; 32])), None);
    assert!(matches!(
        reopened.entity_lifecycle_state(EntityId([1; 32])).unwrap(),
        LifecycleState::Tombstoned(TombstoneCoordinates {
            global_version: Some(5),
            domain_version: Some(5)
        })
    ));
    assert!(matches!(
        reopened.entity_lifecycle_state(EntityId([3; 32])).unwrap(),
        LifecycleState::Tombstoned(TombstoneCoordinates {
            global_version: Some(8),
            domain_version: Some(8)
        })
    ));
    assert_compact_counts(&reopened.container);
}

#[test]
fn phy_reclamation_retains_compact_entity_revisions_and_terminals() {
    let scratch = Scratch::new();
    let source = scratch.file("source.phy");
    let output = scratch.file("reclaimed.phy");
    drop(Phylactery::create(&source).unwrap());
    let (expected, times) = build_history(&source);
    let original_hash = file_hash(&source);
    let mut phy = Phylactery::open(&source).unwrap();
    assert_eq!(phy.entities.records(), expected.as_slice());
    let retained = phy.reclaim_storage(&output).unwrap();
    assert!(retained.unpublished_backing_chunks >= 2);
    assert_eq!(file_hash(&source), original_hash);
    assert_eq!(phy.entities.records(), expected.as_slice());
    let reopened = Phylactery::open(&output).unwrap();
    assert_records_and_times(&expected, &times, reopened.entities.records(), |global| {
        reopened.transaction_time_ns(global)
    });
    assert_eq!(reopened.entity_version(), 8);
    assert_eq!(reopened.latest_global_version(), 8);
    assert_eq!(
        reopened.retired_entity_replacement(EntityId([1; 32])),
        Some(EntityId([2; 32]))
    );
    assert_eq!(reopened.retired_entity_replacement(EntityId([3; 32])), None);
    assert!(matches!(
        reopened.entity_lifecycle_state(EntityId([1; 32])).unwrap(),
        LifecycleState::Tombstoned(TombstoneCoordinates {
            global_version: Some(5),
            domain_version: Some(5)
        })
    ));
    assert!(matches!(
        reopened.entity_lifecycle_state(EntityId([3; 32])).unwrap(),
        LifecycleState::Tombstoned(TombstoneCoordinates {
            global_version: Some(8),
            domain_version: Some(8)
        })
    ));
    assert_compact_counts(&reopened.container);
}
