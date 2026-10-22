//! Memory publication envelopes are live physical publications during reclamation.
use crate::container::publication_envelope::{self, PublicationClock, PublicationEnvelope};
use crate::memory_codec::encode_body;
fn encode_creation(c: &MemoryCreation) -> Result<Vec<u8>, crate::MemoryError> {
    publication::encode(&MemoryPublication::Creation(c.clone()))
}
use crate::memory_compact_codec::publication::{self, MemoryPublication};
use crate::memory_compact_contract::{MemoryCreation, MemoryDelta};
use crate::memory_model::{MemoryRecord, memory_body_bytes, memory_body_id};
use crate::{Container, Cva, MemoryBodyId, MemoryId, Phylactery};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!("reclaim-memory-envelope-{}", uuid::Uuid::new_v4()));
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
    id: MemoryId,
    body_id: MemoryBodyId,
    revision: u64,
    global: u64,
    local: u64,
    mutation: &str,
) -> MemoryRecord {
    MemoryRecord {
        id,
        revision,
        body_id,
        category: "fact".into(),
        memory_type: "project".into(),
        authority_kind: "direct".into(),
        temporal_status: "current".into(),
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
        source_ref: None,
        temporal_inference: None,
        mutation_id: mutation.into(),
        created_at_ns: 10,
        updated_at_ns: 20 + revision as i64,
        global_version: global,
        memory_version: local,
    }
}

fn append_memory_publication(container: &mut Container, publication: MemoryPublication) -> Vec<u8> {
    let body = publication::encode(&publication).unwrap();
    let global = container.latest_version() + 1;
    container.set_next_transaction_time_override(Some(1_000 + global as i64));
    let (actual_global, object) = container.append_publication_envelope(&body).unwrap();
    assert_eq!(actual_global, global);
    container.read(object).unwrap()
}

fn setup(
    path: &Path,
    phylactery: bool,
) -> (Vec<MemoryRecord>, Vec<Vec<u8>>, Vec<(u64, Option<i64>)>) {
    if phylactery {
        drop(Phylactery::create(path).unwrap());
    } else {
        drop(Cva::create(path).unwrap());
    }
    let mut container = Container::open(path).unwrap();
    let title = "Envelope retained title";
    let content = "Envelope retained body payload";
    let body_id = memory_body_id(title, content);
    let body = encode_body(body_id, &memory_body_bytes(title, content)).unwrap();
    container.append(&body).unwrap();
    let id = MemoryId([0x51; 32]);
    let first = record(id, body_id, 1, 1, 1, "envelope-create");
    let first_envelope = append_memory_publication(
        &mut container,
        MemoryPublication::Creation(MemoryCreation::new(first.clone()).unwrap()),
    );
    let mut second = first.clone();
    second.revision = 2;
    second.global_version = 2;
    second.memory_version = 2;
    second.mutation_id = "envelope-revise".into();
    second.scope = "personal".into();
    second.updated_at_ns = 22;
    let delta = MemoryDelta::between(&first, &second).unwrap();
    let second_envelope =
        append_memory_publication(&mut container, MemoryPublication::Delta(delta));
    let tombstone = append_memory_publication(
        &mut container,
        MemoryPublication::Tombstone {
            id,
            revision: 3,
            memory_version: 3,
        },
    );

    let orphan_title = "orphan body title";
    let orphan_content = "ORPHAN MEMORY BODY SECRET";
    let orphan_body_id = memory_body_id(orphan_title, orphan_content);
    let orphan_body = encode_body(
        orphan_body_id,
        &memory_body_bytes(orphan_title, orphan_content),
    )
    .unwrap();
    container.append(&orphan_body).unwrap();
    let orphan = record(
        MemoryId([0x52; 32]),
        orphan_body_id,
        1,
        90,
        90,
        "unpublished-memory",
    );
    container
        .append(&encode_creation(&MemoryCreation::new(orphan).unwrap()).unwrap())
        .unwrap();
    container.sync().unwrap();
    let times = (1..=3)
        .map(|global| (global, container.transaction_time_ns(global)))
        .collect();
    drop(container);
    (
        vec![first, second],
        vec![first_envelope, second_envelope, tombstone, body],
        times,
    )
}

fn hash(path: &Path) -> [u8; 32] {
    Sha256::digest(fs::read(path).unwrap()).into()
}

fn envelope_payloads(path: &Path) -> Vec<Vec<u8>> {
    let mut container = Container::open(path).unwrap();
    container
        .chunks()
        .unwrap()
        .into_iter()
        .filter_map(|object| {
            let payload = container.read(object).unwrap();
            payload.starts_with(b"CVAPUB01").then_some(payload)
        })
        .collect()
}

#[test]
fn rel_and_phy_reclamation_keeps_memory_envelopes_live_and_reclaims_orphans() {
    for phylactery in [false, true] {
        let scratch = Scratch::new();
        let source = scratch.file(if phylactery {
            "source.phy"
        } else {
            "source.rel"
        });
        let output = scratch.file(if phylactery {
            "reclaimed.phy"
        } else {
            "reclaimed.rel"
        });
        let (expected, original_payloads, expected_times) = setup(&source, phylactery);
        let source_hash = hash(&source);
        let before_envelopes = envelope_payloads(&source);
        let report = if phylactery {
            Phylactery::open(&source)
                .unwrap()
                .reclaim_storage(&output)
                .unwrap()
        } else {
            Cva::open(&source)
                .unwrap()
                .reclaim_storage(&output)
                .unwrap()
        };
        assert_eq!(hash(&source), source_hash);
        assert_eq!(report.unpublished_backing_chunks, 1);
        assert_eq!(report.orphan_memory_body_chunks, 1);
        assert_eq!(envelope_payloads(&output), before_envelopes);
        assert_eq!(before_envelopes.len(), 3);
        if phylactery {
            let mut reopened = Phylactery::open(&output).unwrap();
            let history = (1..=2)
                .map(|revision| {
                    reopened
                        .memory_revision(MemoryId([0x51; 32]), revision)
                        .unwrap()
                })
                .collect::<Vec<_>>();
            assert_eq!(history.len(), expected.len());
            assert_eq!(reopened.memories.records(), expected.as_slice());
            assert_eq!(history[0].title, "Envelope retained title");
            assert_eq!(history[0].content, "Envelope retained body payload");
            assert_eq!(history[1].content, "Envelope retained body payload");
            assert_eq!(history[0].scope, "project");
            assert_eq!(history[1].scope, "personal");
            for (global, time) in &expected_times {
                assert_eq!(reopened.transaction_time_ns(*global), *time);
            }
        } else {
            let mut reopened = Cva::open(&output).unwrap();
            let history = (1..=2)
                .map(|revision| {
                    reopened
                        .memory_revision(MemoryId([0x51; 32]), revision)
                        .unwrap()
                })
                .collect::<Vec<_>>();
            assert_eq!(history.len(), expected.len());
            assert_eq!(reopened.memories.records(), expected.as_slice());
            assert_eq!(history[0].title, "Envelope retained title");
            assert_eq!(history[0].content, "Envelope retained body payload");
            assert_eq!(history[1].content, "Envelope retained body payload");
            assert_eq!(history[0].scope, "project");
            assert_eq!(history[1].scope, "personal");
            for (global, time) in &expected_times {
                assert_eq!(reopened.transaction_time_ns(*global), *time);
            }
        }
        assert_eq!(original_payloads.len(), 4);
        let output_bytes = fs::read(&output).unwrap();
        assert!(
            !output_bytes
                .windows(b"ORPHAN MEMORY BODY SECRET".len())
                .any(|window| window == b"ORPHAN MEMORY BODY SECRET")
        );
    }
}

#[test]
fn reclamation_scan_and_relocation_reject_malformed_memory_envelopes() {
    let scratch = Scratch::new();
    let path = scratch.file("malformed.rel");
    drop(Cva::create(&path).unwrap());
    let mut container = Container::open(&path).unwrap();
    let malformed = publication_envelope::encode(PublicationEnvelope {
        global_version: 1,
        clock: PublicationClock::Allocate {
            transaction_time_ns: Some(1),
        },
        body: b"CVAMC002truncated",
    })
    .unwrap();
    let object = container.append(&malformed).unwrap();
    let mut scan = crate::maintenance::reclamation_scan::ReclamationScan::default();
    assert!(scan.ingest(object, &malformed).is_err());
    assert!(super::relocate_payload(&malformed, &Default::default()).is_err());

    let foreign = publication_envelope::encode(PublicationEnvelope {
        global_version: 2,
        clock: PublicationClock::Allocate {
            transaction_time_ns: Some(2),
        },
        body: b"FOREIGN1opaque",
    })
    .unwrap();
    let foreign_object = container.append(&foreign).unwrap();
    assert!(scan.ingest(foreign_object, &foreign).is_err());
    assert!(super::relocate_payload(&foreign, &Default::default()).is_err());

    let title = "valid";
    let content = "supported body";
    let body_id = memory_body_id(title, content);
    let mut record = record(MemoryId([0x61; 32]), body_id, 1, 1, 1, "valid-envelope");
    let body = publication::encode(&MemoryPublication::Creation(
        MemoryCreation::new(record.clone()).unwrap(),
    ))
    .unwrap();
    let envelope = publication_envelope::encode(PublicationEnvelope {
        global_version: 1,
        clock: PublicationClock::Allocate {
            transaction_time_ns: Some(1),
        },
        body: &body,
    })
    .unwrap();
    assert_eq!(
        super::relocate_payload(&envelope, &Default::default()).unwrap(),
        envelope
    );
    let _ = encode_body(body_id, &memory_body_bytes(title, content)).unwrap();
}
