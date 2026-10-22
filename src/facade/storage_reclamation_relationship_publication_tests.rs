//! Relationship publication envelopes remain live during reclamation.
use crate::container::publication_envelope::{self, PublicationClock, PublicationEnvelope};
use crate::owner_lifecycle::{LifecycleState, TombstoneCoordinates};
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
use crate::relationship_model::{RelationshipParticipant, RelationshipRecord};
use crate::{
    Container, ContainerError, Cva, EntityId, EntityRef, FileKind, MemoryId, MemoryRef, Phylactery,
    RelationshipId,
};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!(
                "reclaim-relationship-envelope-{}",
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
        kind: "  observes  ".into(),
        participants: vec![
            RelationshipParticipant {
                entity: EntityRef {
                    owner_id: "  external-entity  ".into(),
                    entity_id: EntityId([id.wrapping_add(1); 32]),
                },
                role: Some("  exact role  ".into()),
            },
            RelationshipParticipant {
                entity: EntityRef {
                    owner_id: "remote-entity".into(),
                    entity_id: EntityId([id.wrapping_add(2); 32]),
                },
                role: None,
            },
        ],
        evidence: vec![MemoryRef {
            owner_id: "  external-memory  ".into(),
            memory_id: MemoryId([id.wrapping_add(3); 32]),
        }],
        summary: format!("  retained relationship {id} revision {revision}  "),
        mutation_id: mutation.into(),
        created_at_ns: 10,
        updated_at_ns: 20 + revision as i64,
        global_version: global,
        relationship_version: local,
    }
}

fn append(
    container: &mut Container,
    value: &RelationshipPublication,
    time: Option<i64>,
) -> Vec<u8> {
    let body = publication::encode(value).unwrap();
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

fn add_unpublished_compact_carriers(path: &Path) {
    let mut container = Container::open(path).unwrap();
    let orphan = record(0x71, 1, 71, 71, "unpublished-create");
    container
        .append(&encode_creation(&RelationshipCreation::new(orphan.clone()).unwrap()).unwrap())
        .unwrap();
    let mut revised = orphan.clone();
    revised.revision = 2;
    revised.global_version = 72;
    revised.relationship_version = 72;
    revised.mutation_id = "unpublished-revision".into();
    revised.summary = "never published Relationship summary".into();
    container
        .append(&encode_delta(&RelationshipDelta::between(&orphan, &revised).unwrap()).unwrap())
        .unwrap();
    container.sync().unwrap();
}

fn file_hash(path: &Path) -> [u8; 32] {
    Sha256::digest(fs::read(path).unwrap()).into()
}

fn history_digests(path: &Path) -> Vec<[u8; 32]> {
    let mut container = Container::open(path).unwrap();
    let graph = crate::graph_store::GraphStore::empty();
    (0..=container.latest_version())
        .map(|watermark| {
            crate::history::owner_history_digest(
                &crate::owner_lifecycle::HistorySource::new(&container).unwrap(),
                watermark,
                &graph,
            )
            .unwrap()
        })
        .collect()
}

fn write_envelope_history(
    path: &Path,
    kind: FileKind,
) -> (Vec<RelationshipRecord>, Vec<Option<i64>>, Vec<Vec<u8>>) {
    match kind {
        FileKind::Reliquary => drop(Cva::create_rel_with_uuid(path, [0x89; 16], None).unwrap()),
        FileKind::Phylactery => drop(Phylactery::create_with_uuid(path, [0x89; 16]).unwrap()),
    }
    let mut container = Container::open(path).unwrap();
    let first = record(0x21, 1, 1, 1, "create-21");
    let first_bytes = append(
        &mut container,
        &RelationshipPublication::Creation(RelationshipCreation::new(first.clone()).unwrap()),
        Some(1_001),
    );
    let mut second = first.clone();
    second.revision = 2;
    second.global_version = 2;
    second.relationship_version = 2;
    second.mutation_id = "revise-21".into();
    second.participants[0].role = Some("  revised exact role  ".into());
    second.summary = "  updated retained relationship  ".into();
    second.updated_at_ns = 15;
    let second_bytes = append(
        &mut container,
        &RelationshipPublication::Delta(RelationshipDelta::between(&first, &second).unwrap()),
        None,
    );
    let terminal = append(
        &mut container,
        &RelationshipPublication::Tombstone {
            id: first.id,
            revision: 3,
            relationship_version: 3,
        },
        Some(1_003),
    );
    let unrelated = record(0x22, 1, 4, 4, "create-22");
    let unrelated_bytes = append(
        &mut container,
        &RelationshipPublication::Creation(RelationshipCreation::new(unrelated.clone()).unwrap()),
        Some(1_004),
    );
    container.sync().unwrap();
    (
        vec![first, second, unrelated],
        vec![Some(1_001), None, Some(1_003), Some(1_004)],
        vec![first_bytes, second_bytes, terminal, unrelated_bytes],
    )
}

fn assert_reopened(
    expected: &[RelationshipRecord],
    times: &[Option<i64>],
    relationship_version: impl Fn() -> u64,
    latest_global: impl Fn() -> u64,
    relationship: impl Fn(RelationshipId, u64) -> crate::Relationship,
    lifecycle: impl Fn(RelationshipId) -> LifecycleState,
    transaction_time: impl Fn(u64) -> Option<i64>,
) {
    assert_eq!(relationship_version(), 4);
    assert_eq!(latest_global(), 4);
    for (index, expected_record) in expected.iter().enumerate() {
        let actual = relationship(expected_record.id, expected_record.revision);
        assert_eq!(actual.id, expected_record.id);
        assert_eq!(actual.revision, expected_record.revision);
        assert_eq!(actual.kind, expected_record.kind);
        assert_eq!(actual.participants, expected_record.participants);
        assert_eq!(actual.evidence, expected_record.evidence);
        assert_eq!(actual.summary, expected_record.summary);
        assert_eq!(actual.mutation_id, expected_record.mutation_id);
        assert_eq!(actual.created_at_ns, expected_record.created_at_ns);
        assert_eq!(actual.updated_at_ns, expected_record.updated_at_ns);
        assert_eq!(actual.global_version, expected_record.global_version);
        assert_eq!(
            actual.relationship_version,
            expected_record.relationship_version
        );
        assert_eq!(transaction_time(index as u64 + 1), times[index]);
    }
    assert_eq!(
        lifecycle(expected[0].id),
        LifecycleState::Tombstoned(TombstoneCoordinates {
            global_version: Some(3),
            domain_version: Some(3),
        })
    );
}

#[test]
fn rel_and_phy_reclamation_preserves_relationship_envelopes_and_discards_orphans() {
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
        let (expected, times, expected_envelopes) = write_envelope_history(&source, kind);
        add_unpublished_compact_carriers(&source);
        let source_hash = file_hash(&source);
        let before = envelope_payloads(&source);
        assert_eq!(before, expected_envelopes);
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
        assert_eq!(report.unpublished_backing_chunks, 2);
        assert_eq!(envelope_payloads(&output), before);
        assert_eq!(history_digests(&output), history_digests(&source));

        let payloads = {
            let mut container = Container::open(&output).unwrap();
            let mut values = Vec::new();
            {
                let mut ingest_frame =
                    |frame: crate::container::PhysicalFrame| -> Result<(), ContainerError> {
                        let payload = frame.payload.as_slice();
                        {
                            values.push(payload.to_vec());
                            Ok(())
                        }
                    };
                let mut cursor = container.physical_cursor().unwrap();
                (|| {
                    while let Some(frame) = cursor.next_frame()? {
                        ingest_frame(frame)?;
                    }
                    Ok::<(), ContainerError>(())
                })()
            }
            .unwrap();
            values
        };
        assert!(!payloads.iter().any(|payload| {
            payload.starts_with(b"CVARNC01")
                && payload
                    .get(8..40)
                    .is_some_and(|id| id.iter().all(|byte| *byte == 0x71))
        }));
        assert!(!payloads.iter().any(|payload| {
            payload.starts_with(b"CVARND01")
                && payload
                    .get(8..40)
                    .is_some_and(|id| id.iter().all(|byte| *byte == 0x71))
        }));
        match kind {
            FileKind::Reliquary => {
                let owner = Cva::open(&output).unwrap();
                assert_reopened(
                    &expected,
                    &times,
                    || owner.relationship_version(),
                    || owner.latest_global_version(),
                    |id, revision| owner.relationship_revision(id, revision).unwrap(),
                    |id| owner.relationship_lifecycle_state(id).unwrap(),
                    |global| owner.transaction_time_ns(global),
                );
            }
            FileKind::Phylactery => {
                let owner = Phylactery::open(&output).unwrap();
                assert_reopened(
                    &expected,
                    &times,
                    || owner.relationship_version(),
                    || owner.container.latest_version(),
                    |id, revision| owner.relationship_revision(id, revision).unwrap(),
                    |id| owner.relationship_lifecycle_state(id).unwrap(),
                    |global| owner.transaction_time_ns(global),
                );
            }
        }
    }
}

#[test]
fn reclamation_scan_and_relocation_validate_relationship_publication_envelopes() {
    let publication = RelationshipPublication::Tombstone {
        id: RelationshipId([0x73; 32]),
        revision: 2,
        relationship_version: 1,
    };
    let body = publication::encode(&publication).unwrap();
    let envelope = publication_envelope::encode(PublicationEnvelope {
        global_version: 1,
        clock: PublicationClock::Allocate {
            transaction_time_ns: Some(7),
        },
        body: &body,
    })
    .unwrap();
    assert_eq!(
        super::relocate_payload(&envelope, &Default::default()).unwrap(),
        envelope
    );
    let mut scan = super::ReclamationScan::default();
    let object = crate::ObjectRef::from_legacy_bytes([9; 16]);
    scan.ingest(object, &envelope).unwrap();

    let malformed = publication_envelope::encode(PublicationEnvelope {
        global_version: 1,
        clock: PublicationClock::Allocate {
            transaction_time_ns: Some(7),
        },
        body: b"CVARTD02truncated",
    })
    .unwrap();
    assert!(scan.ingest(object, &malformed).is_err());
    assert!(super::relocate_payload(&malformed, &Default::default()).is_err());

    let foreign = publication_envelope::encode(PublicationEnvelope {
        global_version: 2,
        clock: PublicationClock::Allocate {
            transaction_time_ns: None,
        },
        body: b"FOREIGN1opaque",
    })
    .unwrap();
    assert!(scan.ingest(object, &foreign).is_err());
    assert!(super::relocate_payload(&foreign, &Default::default()).is_err());
}
