//! Pruning a raw legacy Relationship creation followed by envelope delta/terminal frames.
use crate::container::publication_envelope;
use crate::owner_lifecycle::{HistoricalPayload, LifecycleTarget, PayloadUnavailable};
use crate::owner_maintenance::owner_history_digest;
use crate::relationship_codec::{self, RelationshipVersion};
use crate::relationship_compact_codec::publication::{self, RelationshipPublication};
use crate::relationship_compact_contract::RelationshipDelta;
use crate::relationship_model::{RelationshipParticipant, RelationshipRecord};
use crate::{
    Container, Cva, EntityId, EntityRef, FileKind, HistoryPruneRequest, MemoryId, MemoryRef,
    Phylactery, RelationshipId,
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
                "relationship-legacy-envelope-prune-{}",
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

fn legacy_record(id: RelationshipId) -> RelationshipRecord {
    RelationshipRecord {
        id,
        revision: 1,
        kind: "supports".into(),
        participants: vec![RelationshipParticipant {
            entity: EntityRef {
                owner_id: "remote-entity-owner".into(),
                entity_id: EntityId([0x51; 32]),
            },
            role: Some("source".into()),
        }],
        evidence: vec![MemoryRef {
            owner_id: "remote-memory-owner".into(),
            memory_id: MemoryId([0x52; 32]),
        }],
        summary: format!("LEGACY PRIVATE RELATIONSHIP SUMMARY {}", "l".repeat(3_000)),
        mutation_id: "raw-legacy-creation".into(),
        created_at_ns: -900,
        updated_at_ns: -899,
        global_version: 0,
        relationship_version: 0,
    }
}

fn append_envelope(container: &mut Container, value: &RelationshipPublication, time: Option<i64>) {
    let body = publication::encode(value).unwrap();
    container.set_next_transaction_time_override(time);
    container.append_publication_envelope(&body).unwrap();
}

fn seed(container: &mut Container) -> (RelationshipId, [u8; 32], [u8; 32]) {
    let id = RelationshipId([0x53; 32]);
    let mut legacy = legacy_record(id);
    let legacy_carrier = relationship_codec::encode_record(&legacy).unwrap();
    let legacy_digest = Sha256::digest(&legacy_carrier).into();
    container.set_owner_write_scope(true);
    container.set_next_transaction_time_override(Some(-77));
    let carrier = container.append(&legacy_carrier).unwrap();
    let global = container.allocate_version().unwrap();
    assert_eq!(global, 1);
    container
        .append(&relationship_codec::encode_version(RelationshipVersion {
            global_version: global,
            relationship_version: 1,
            record: carrier,
        }))
        .unwrap();
    legacy.global_version = global;
    legacy.relationship_version = 1;

    let mut revised = legacy.clone();
    revised.revision = 2;
    revised.global_version = 2;
    revised.relationship_version = 2;
    revised.summary = format!(
        "ENVELOPE PRIVATE RELATIONSHIP SUMMARY {}",
        "e".repeat(3_000)
    );
    revised.mutation_id = "envelope-delta-after-legacy-import".into();
    revised.updated_at_ns = -898;
    let delta = RelationshipDelta::between(&legacy, &revised).unwrap();
    let delta_carrier =
        publication::comparison_carrier(&RelationshipPublication::Delta(delta.clone())).unwrap();
    let delta_digest = Sha256::digest(&delta_carrier).into();
    append_envelope(container, &RelationshipPublication::Delta(delta), Some(0));
    assert_eq!(container.latest_version(), 2);

    append_envelope(
        container,
        &RelationshipPublication::Tombstone {
            id,
            revision: 3,
            relationship_version: 3,
        },
        None,
    );
    assert_eq!(container.latest_version(), 3);
    container.set_owner_write_scope(false);
    (id, legacy_digest, delta_digest)
}

fn history_digests(path: &Path) -> Vec<[u8; 32]> {
    let mut container = Container::open(path).unwrap();
    (0..=container.latest_version())
        .map(|watermark| {
            owner_history_digest(
                &mut container,
                watermark,
                &crate::graph_store::GraphStore::empty(),
            )
            .unwrap()
        })
        .collect()
}

fn hash(path: &Path) -> [u8; 32] {
    Sha256::digest(fs::read(path).unwrap()).into()
}

fn prune(kind: FileKind, source: &Path, output: &Path, id: RelationshipId) -> [u8; 32] {
    match kind {
        FileKind::Reliquary => {
            let mut owner = Cva::open(source).unwrap();
            owner
                .prune_tombstoned_history(
                    output,
                    HistoryPruneRequest {
                        targets: vec![LifecycleTarget::Relationship(id)],
                    },
                )
                .unwrap()
                .manifest_digest
        }
        FileKind::Phylactery => {
            let mut owner = Phylactery::open(source).unwrap();
            owner
                .prune_tombstoned_history(
                    output,
                    HistoryPruneRequest {
                        targets: vec![LifecycleTarget::Relationship(id)],
                    },
                )
                .unwrap()
                .manifest_digest
        }
    }
}

fn assert_pruned(
    kind: FileKind,
    output: &Path,
    id: RelationshipId,
    source_digests: &[[u8; 32]],
    manifest_digest: [u8; 32],
    legacy_digest: [u8; 32],
    delta_digest: [u8; 32],
    source_size: u64,
    source_descriptor: &crate::RelationshipRetentionDescriptor,
) {
    assert_eq!(history_digests(output), source_digests);
    let mut container = Container::open(output).unwrap();
    let mut seen_proofs = std::collections::BTreeMap::new();
    for object in container.chunks().unwrap() {
        let raw = container.read(object).unwrap();
        let proof = if let Some(envelope) = publication_envelope::decode(&raw).unwrap() {
            match publication::decode(envelope.body, envelope.global_version).unwrap() {
                Some(RelationshipPublication::Creation(creation)) if creation.record.id == id => {
                    creation.description_proof.map(|proof| (1, proof))
                }
                Some(RelationshipPublication::Delta(delta)) if delta.id == id => {
                    delta.description_proof.map(|proof| (delta.revision, proof))
                }
                _ => None,
            }
        } else if raw.starts_with(b"CVARNC") {
            let creation = crate::relationship_compact_codec::decode_creation(&raw).unwrap();
            (creation.record.id == id)
                .then_some(creation.description_proof)
                .flatten()
                .map(|proof| (creation.record.revision, proof))
        } else if raw.starts_with(b"CVARND") {
            let delta = crate::relationship_compact_codec::decode_delta(&raw).unwrap();
            (delta.id == id)
                .then_some(delta.description_proof)
                .flatten()
                .map(|proof| (delta.revision, proof))
        } else {
            None
        };
        if let Some((revision, proof)) = proof {
            seen_proofs.insert(revision, proof);
        }
    }
    assert_eq!(seen_proofs.len(), 2);
    assert_eq!(seen_proofs[&1].manifest_digest, manifest_digest);
    assert_eq!(seen_proofs[&1].original_carrier_digest, legacy_digest);
    assert_eq!(seen_proofs[&2].manifest_digest, manifest_digest);
    assert_eq!(seen_proofs[&2].original_carrier_digest, delta_digest);
    drop(container);

    match kind {
        FileKind::Reliquary => {
            let owner = Cva::open(output).unwrap();
            let descriptor = owner.relationship_retention_descriptor(id).unwrap();
            assert_eq!(descriptor, *source_descriptor);
            assert_eq!(descriptor.tombstone_revision, 3);
            assert_eq!(descriptor.revisions.len(), 2);
            for revision in 1..=2 {
                assert!(matches!(
                    owner
                        .historical_relationship_revision(id, revision)
                        .unwrap(),
                    HistoricalPayload::Pruned { .. }
                ));
                assert!(matches!(
                    owner.relationship_revision(id, revision),
                    Err(crate::RelationshipError::PayloadUnavailable(
                        PayloadUnavailable::Pruned
                    ))
                ));
            }
        }
        FileKind::Phylactery => {
            let owner = Phylactery::open(output).unwrap();
            let descriptor = owner.relationship_retention_descriptor(id).unwrap();
            assert_eq!(descriptor, *source_descriptor);
            assert_eq!(descriptor.tombstone_revision, 3);
            assert_eq!(descriptor.revisions.len(), 2);
            for revision in 1..=2 {
                assert!(matches!(
                    owner
                        .historical_relationship_revision(id, revision)
                        .unwrap(),
                    HistoricalPayload::Pruned { .. }
                ));
                assert!(matches!(
                    owner.relationship_revision(id, revision),
                    Err(crate::RelationshipError::PayloadUnavailable(
                        PayloadUnavailable::Pruned
                    ))
                ));
            }
        }
    }
    let output_bytes = fs::read(output).unwrap();
    assert!(
        output_bytes.len() < source_size as usize,
        "pruning must shrink the large selected summaries"
    );
    assert!(
        !output_bytes
            .windows(b"LEGACY PRIVATE RELATIONSHIP SUMMARY".len())
            .any(|window| window == b"LEGACY PRIVATE RELATIONSHIP SUMMARY")
    );
    assert!(
        !output_bytes
            .windows(b"ENVELOPE PRIVATE RELATIONSHIP SUMMARY".len())
            .any(|window| window == b"ENVELOPE PRIVATE RELATIONSHIP SUMMARY")
    );
}

fn exercise(kind: FileKind) {
    let scratch = Scratch::new();
    let source = scratch.file(if kind == FileKind::Reliquary {
        "source.rel"
    } else {
        "source.phy"
    });
    let output = scratch.file(if kind == FileKind::Reliquary {
        "pruned.rel"
    } else {
        "pruned.phy"
    });
    let (id, legacy_digest, delta_digest) = match kind {
        FileKind::Reliquary => {
            let mut owner = Cva::create_rel_with_uuid(&source, [0x61; 16], None).unwrap();
            let evidence = seed(&mut owner.container);
            owner.sync().unwrap();
            evidence
        }
        FileKind::Phylactery => {
            let mut owner = Phylactery::create_with_uuid(&source, [0x61; 16]).unwrap();
            let evidence = seed(&mut owner.container);
            owner.sync().unwrap();
            evidence
        }
    };
    let source_bytes = fs::read(&source).unwrap();
    let source_size = source_bytes.len() as u64;
    let source_hash = hash(&source);
    let source_digests = history_digests(&source);
    // Real owner reopen validates the mixed raw-creation/envelope-delta/terminal chain.
    let source_descriptor = match kind {
        FileKind::Reliquary => {
            let owner = Cva::open(&source).unwrap();
            assert_eq!(owner.relationship_version(), 3);
            let descriptor = owner.relationship_retention_descriptor(id).unwrap();
            assert_eq!(descriptor.tombstone_revision, 3);
            descriptor
        }
        FileKind::Phylactery => {
            let owner = Phylactery::open(&source).unwrap();
            assert_eq!(owner.relationship_version(), 3);
            let descriptor = owner.relationship_retention_descriptor(id).unwrap();
            assert_eq!(descriptor.tombstone_revision, 3);
            descriptor
        }
    };

    let manifest_digest = prune(kind, &source, &output, id);
    assert_eq!(fs::read(&source).unwrap(), source_bytes);
    assert_eq!(hash(&source), source_hash);
    assert_pruned(
        kind,
        &output,
        id,
        &source_digests,
        manifest_digest,
        legacy_digest,
        delta_digest,
        source_size,
        &source_descriptor,
    );
}

#[test]
fn raw_legacy_creation_followed_by_envelope_delta_prunes_with_source_carrier_proof_parity() {
    exercise(FileKind::Reliquary);
    exercise(FileKind::Phylactery);
}
