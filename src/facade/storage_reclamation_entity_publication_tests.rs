//! Entity publication envelopes remain live during reclamation.
use reliquary_container::publication_envelope::PublicationClock;
use reliquary_perception_store::entity_compact_codec::publication::{self, EntityPublication};
fn encode_creation(c: &EntityCreation) -> Result<Vec<u8>, reliquary_perception_store::EntityError> {
    reliquary_perception_store::entity_compact_codec::publication::encode(
        &reliquary_perception_store::entity_compact_codec::publication::EntityPublication::Creation(
            c.clone(),
        ),
    )
}
fn encode_delta(d: &EntityDelta) -> Result<Vec<u8>, reliquary_perception_store::EntityError> {
    reliquary_perception_store::entity_compact_codec::publication::encode(
        &reliquary_perception_store::entity_compact_codec::publication::EntityPublication::Delta(
            d.clone(),
        ),
    )
}
use crate::owner_lifecycle::{HistoricalPayload, LifecycleTarget};
use crate::{Cva, EntityId, HistoryPruneRequest, Phylactery};
use reliquary_container::{Container, FileKind};
use reliquary_perception_store::entity_compact_contract::{EntityCreation, EntityDelta};
use reliquary_perception_store::entity_model::EntityRecord;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!("reclaim-entity-envelope-{}", uuid::Uuid::new_v4()));
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

fn record(revision: u64, global: u64, local: u64) -> EntityRecord {
    EntityRecord {
        id: EntityId([0x71; 32]),
        global_id: None,
        revision,
        canonical_name: format!("Retained Entity revision {revision}"),
        aliases: vec![format!("Alias {revision}")],
        kind: "project".into(),
        summary: format!("Retained description {}", "s".repeat(2_000)),
        mutation_id: format!("entity-mutation-{revision}"),
        created_at_ns: -50,
        updated_at_ns: -40 + revision as i64,
        global_version: global,
        entity_version: local,
    }
}

fn append(container: &mut Container, value: EntityPublication, time: Option<i64>) -> Vec<u8> {
    let body = publication::encode(&value).unwrap();
    container.set_next_transaction_time_override(time);
    let (_, object) = container.append_publication_envelope(&body).unwrap();
    container.read(object).unwrap()
}

fn envelope_payloads(path: &Path) -> Vec<Vec<u8>> {
    let mut container = Container::open(path).unwrap();
    container
        .chunks()
        .unwrap()
        .into_iter()
        .filter_map(|object| {
            let bytes = container.read(object).unwrap();
            bytes.starts_with(b"CVAPUB01").then_some(bytes)
        })
        .collect()
}

fn add_unpublished_current_bodies(path: &Path) {
    let mut container = Container::open(path).unwrap();
    let mut orphan = record(1, 900, 900);
    orphan.id = EntityId([0x72; 32]);
    let creation = EntityCreation::new(orphan.clone()).unwrap();
    container
        .append(&encode_creation(&creation).unwrap())
        .unwrap();
    let mut revised = orphan.clone();
    revised.revision = 2;
    revised.global_version = 901;
    revised.entity_version = 901;
    revised.mutation_id = "unpublished-revision".into();
    let delta = EntityDelta::between(&orphan, &revised).unwrap();
    container.append(&encode_delta(&delta).unwrap()).unwrap();
    container.sync().unwrap();
}

fn file_hash(path: &Path) -> [u8; 32] {
    Sha256::digest(fs::read(path).unwrap()).into()
}

fn make_pruned_history(path: &Path, kind: FileKind) -> (Vec<Option<i64>>, Vec<Vec<u8>>) {
    match kind {
        FileKind::Reliquary => drop(Cva::create_rel_with_uuid(path, [0x89; 16], None).unwrap()),
        FileKind::Phylactery => drop(Phylactery::create(path).unwrap()),
    }
    let mut container = Container::open(path).unwrap();
    let first = record(1, 1, 1);
    let first_bytes = append(
        &mut container,
        EntityPublication::Creation(EntityCreation::new(first.clone()).unwrap()),
        Some(-111),
    );
    let mut second = first.clone();
    second.revision = 2;
    second.global_version = 2;
    second.entity_version = 2;
    second.mutation_id = "entity-mutation-2".into();
    second.canonical_name = "Retained Entity revised".into();
    second.summary = format!("Retained revised description {}", "t".repeat(2_000));
    let second_bytes = append(
        &mut container,
        EntityPublication::Delta(EntityDelta::between(&first, &second).unwrap()),
        None,
    );
    let terminal = append(
        &mut container,
        EntityPublication::DirectTombstone {
            id: first.id,
            revision: 3,
            entity_version: 3,
        },
        Some(0),
    );
    container.sync().unwrap();
    drop(container);

    let expected_envelopes = vec![first_bytes, second_bytes, terminal];
    match kind {
        FileKind::Reliquary => {
            let mut owner = Cva::open(path).unwrap();
            let pruned = path.with_extension("pruned.rel");
            owner
                .prune_tombstoned_history(
                    &pruned,
                    HistoryPruneRequest {
                        targets: vec![LifecycleTarget::Entity(first.id)],
                    },
                )
                .unwrap();
            drop(owner);
            fs::remove_file(path).unwrap();
            fs::rename(pruned, path).unwrap();
        }
        FileKind::Phylactery => {
            let mut owner = Phylactery::open(path).unwrap();
            let pruned = path.with_extension("pruned.phy");
            owner
                .prune_tombstoned_history(
                    &pruned,
                    HistoryPruneRequest {
                        targets: vec![LifecycleTarget::Entity(first.id)],
                    },
                )
                .unwrap();
            drop(owner);
            fs::remove_file(path).unwrap();
            fs::rename(pruned, path).unwrap();
        }
    }
    let pruned_envelopes = envelope_payloads(path);
    assert_eq!(pruned_envelopes.len(), 3);
    assert!(pruned_envelopes.iter().any(|bytes| {
        reliquary_container::publication_envelope::decode(bytes)
            .unwrap()
            .is_some_and(|envelope| {
                publication::decode(envelope.body, envelope.global_version)
                    .unwrap()
                    .is_some_and(|value| match value {
                        EntityPublication::Creation(creation) => {
                            creation.description_proof.is_some()
                        }
                        EntityPublication::Delta(delta) => delta.description_proof.is_some(),
                        _ => false,
                    })
            })
    }));
    assert_ne!(expected_envelopes[0], pruned_envelopes[0]);
    (vec![Some(-111), None, Some(0)], pruned_envelopes)
}

#[test]
fn rel_and_phy_reclamation_keeps_entity_envelopes_proofs_and_terminals() {
    for kind in [FileKind::Reliquary, FileKind::Phylactery] {
        let scratch = Scratch::new();
        let source = scratch.file(if kind == FileKind::Reliquary {
            "source.rel"
        } else {
            "source.phy"
        });
        let output = scratch.file(if kind == FileKind::Reliquary {
            "reclaimed.rel"
        } else {
            "reclaimed.phy"
        });
        let (times, expected_envelopes) = make_pruned_history(&source, kind);
        add_unpublished_current_bodies(&source);
        let source_hash = file_hash(&source);
        let before = envelope_payloads(&source);
        assert!(before.iter().any(|bytes| bytes != &expected_envelopes[0]));

        let report = match kind {
            FileKind::Reliquary => Cva::open(&source)
                .unwrap()
                .reclaim_storage(&output)
                .unwrap(),
            FileKind::Phylactery => Phylactery::open(&source)
                .unwrap()
                .reclaim_storage(&output)
                .unwrap(),
        };
        assert_eq!(file_hash(&source), source_hash);
        assert!(report.unpublished_backing_chunks >= 2);
        assert_eq!(envelope_payloads(&output), before);

        let bytes = fs::read(&output).unwrap();
        assert!(!bytes.windows(8).any(|window| window == b"CVAENC01"));
        assert!(!bytes.windows(8).any(|window| window == b"CVAEND01"));
        match kind {
            FileKind::Reliquary => {
                let owner = Cva::open(&output).unwrap();
                assert_eq!(owner.entity_version(), 3);
                assert_eq!(owner.latest_global_version(), 3);
                assert_eq!(owner.transaction_time_ns(1), times[0]);
                assert_eq!(owner.transaction_time_ns(2), times[1]);
                assert_eq!(owner.transaction_time_ns(3), times[2]);
                assert!(matches!(
                    owner
                        .historical_entity_revision(EntityId([0x71; 32]), 1)
                        .unwrap(),
                    HistoricalPayload::Pruned { .. }
                ));
                assert!(matches!(
                    owner
                        .historical_entity_revision(EntityId([0x71; 32]), 2)
                        .unwrap(),
                    HistoricalPayload::Pruned { .. }
                ));
            }
            FileKind::Phylactery => {
                let owner = Phylactery::open(&output).unwrap();
                assert_eq!(owner.entity_version(), 3);
                assert_eq!(owner.latest_global_version(), 3);
                assert_eq!(owner.transaction_time_ns(1), times[0]);
                assert_eq!(owner.transaction_time_ns(2), times[1]);
                assert_eq!(owner.transaction_time_ns(3), times[2]);
                assert!(matches!(
                    owner
                        .historical_entity_revision(EntityId([0x71; 32]), 1)
                        .unwrap(),
                    HistoricalPayload::Pruned { .. }
                ));
                assert!(matches!(
                    owner
                        .historical_entity_revision(EntityId([0x71; 32]), 2)
                        .unwrap(),
                    HistoricalPayload::Pruned { .. }
                ));
            }
        }
    }
}

#[test]
fn reclamation_scan_and_relocation_validate_entity_publication_envelopes() {
    let publication = EntityPublication::DirectTombstone {
        id: EntityId([0x73; 32]),
        revision: 2,
        entity_version: 1,
    };
    let body = publication::encode(&publication).unwrap();
    let envelope = reliquary_container::publication_envelope::encode(
        reliquary_container::publication_envelope::PublicationEnvelope {
            global_version: 1,
            clock: PublicationClock::Allocate {
                transaction_time_ns: Some(7),
            },
            body: &body,
        },
    )
    .unwrap();
    assert_eq!(
        super::relocate_payload(&envelope, &Default::default()).unwrap(),
        envelope
    );
    let mut scan = super::ReclamationScan::default();
    let object = reliquary_container::ObjectRef::from_legacy_bytes([9; 16]);
    scan.ingest(object, &envelope).unwrap();

    let malformed_body = b"CVAETD02truncated";
    let malformed = reliquary_container::publication_envelope::encode(
        reliquary_container::publication_envelope::PublicationEnvelope {
            global_version: 1,
            clock: PublicationClock::Allocate {
                transaction_time_ns: Some(7),
            },
            body: malformed_body,
        },
    )
    .unwrap();
    assert!(scan.ingest(object, &malformed).is_err());
    assert!(super::relocate_payload(&malformed, &Default::default()).is_err());

    let foreign = reliquary_container::publication_envelope::encode(
        reliquary_container::publication_envelope::PublicationEnvelope {
            global_version: 2,
            clock: PublicationClock::Allocate {
                transaction_time_ns: None,
            },
            body: b"FOREIGN1opaque",
        },
    )
    .unwrap();
    assert!(scan.ingest(object, &foreign).is_err());
    assert!(super::relocate_payload(&foreign, &Default::default()).is_err());
}
