//! Native pruning of Relationship envelope publications in REL and PHY owners.
use crate::owner_lifecycle::{HistoricalPayload, LifecycleTarget, PayloadUnavailable};
use crate::owner_maintenance::owner_history_digest;
use crate::relationship_compact_codec::publication::{self, RelationshipPublication};
use crate::relationship_compact_contract::{
    RelationshipCreation, RelationshipDelta, RelationshipDescriptionProof,
};
use crate::relationship_model::{RelationshipParticipant, RelationshipRecord};
use crate::{
    Cva, EntityId, EntityRef, HistoryPruneRequest, MemoryId, MemoryRef, Phylactery, RelationshipId,
};
use reliquary_container::publication_envelope;
use reliquary_container::{Container, FileKind};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!(
                "relationship-publication-prune-{}",
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
    revision: u64,
    global: u64,
    local: u64,
    mutation: &str,
    summary: &str,
) -> RelationshipRecord {
    RelationshipRecord {
        id: RelationshipId([id; 32]),
        revision,
        kind: "supports".into(),
        participants: vec![RelationshipParticipant {
            entity: EntityRef {
                owner_id: "entity-owner".into(),
                entity_id: EntityId([id.wrapping_add(20); 32]),
            },
            role: Some("source".into()),
        }],
        evidence: vec![MemoryRef {
            owner_id: "memory-owner".into(),
            memory_id: MemoryId([id.wrapping_add(40); 32]),
        }],
        summary: format!("{summary} {}", "p".repeat(900)),
        mutation_id: mutation.into(),
        created_at_ns: -17,
        updated_at_ns: 30 + revision as i64,
        global_version: global,
        relationship_version: local,
    }
}

fn append_publication(
    container: &mut Container,
    value: &RelationshipPublication,
    time: Option<i64>,
) {
    let body = publication::encode(value).unwrap();
    container.set_next_transaction_time_override(time);
    container.append_publication_envelope(&body).unwrap();
}

fn write_history(
    container: &mut Container,
) -> (
    RelationshipId,
    RelationshipId,
    Vec<Option<i64>>,
    BTreeMap<u64, [u8; 32]>,
) {
    let first = record(
        31,
        1,
        1,
        1,
        "selected-create",
        "PRIVATE RELATIONSHIP SUMMARY ONE",
    );
    let mut second = record(
        31,
        2,
        2,
        2,
        "selected-revise",
        "PRIVATE RELATIONSHIP SUMMARY TWO",
    );
    second.participants.push(RelationshipParticipant {
        entity: EntityRef {
            owner_id: "entity-owner-2".into(),
            entity_id: EntityId([52; 32]),
        },
        role: Some("target".into()),
    });
    let create =
        RelationshipPublication::Creation(RelationshipCreation::new(first.clone()).unwrap());
    let delta =
        RelationshipPublication::Delta(RelationshipDelta::between(&first, &second).unwrap());
    let expected_digests = BTreeMap::from([
        (
            1,
            Sha256::digest(publication::comparison_carrier(&create).unwrap()).into(),
        ),
        (
            2,
            Sha256::digest(publication::comparison_carrier(&delta).unwrap()).into(),
        ),
    ]);
    append_publication(container, &create, Some(-111));
    append_publication(container, &delta, None);
    append_publication(
        container,
        &RelationshipPublication::Tombstone {
            id: first.id,
            revision: 3,
            relationship_version: 3,
        },
        Some(0),
    );
    let unrelated = record(
        44,
        1,
        4,
        4,
        "unrelated-create",
        "AVAILABLE UNRELATED SUMMARY",
    );
    append_publication(
        container,
        &RelationshipPublication::Creation(RelationshipCreation::new(unrelated.clone()).unwrap()),
        Some(-444),
    );
    (
        first.id,
        unrelated.id,
        vec![Some(-111), None, Some(0), Some(-444)],
        expected_digests,
    )
}

fn history_digest(container: &mut Container) -> [u8; 32] {
    let watermark = container.latest_version();
    owner_history_digest(
        container,
        watermark,
        &crate::graph_store::GraphStore::empty(),
    )
    .unwrap()
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

fn file_hash(path: &Path) -> [u8; 32] {
    Sha256::digest(fs::read(path).unwrap()).into()
}

fn contains(bytes: &[u8], value: &str) -> bool {
    bytes
        .windows(value.len())
        .any(|window| window == value.as_bytes())
}

fn proofs(path: &Path, id: RelationshipId) -> BTreeMap<u64, RelationshipDescriptionProof> {
    let mut container = Container::open(path).unwrap();
    let mut result = BTreeMap::new();
    for object in container.chunks().unwrap() {
        let raw = container.read(object).unwrap();
        let Some(envelope) = publication_envelope::decode(&raw).unwrap() else {
            continue;
        };
        match publication::decode(envelope.body, envelope.global_version).unwrap() {
            Some(RelationshipPublication::Creation(creation)) if creation.record.id == id => {
                result.insert(1, creation.description_proof.unwrap());
            }
            Some(RelationshipPublication::Delta(delta)) if delta.id == id => {
                result.insert(delta.revision, delta.description_proof.unwrap());
            }
            _ => {}
        }
    }
    result
}

fn mutate_pruned_manifest_digest(path: &Path, id: RelationshipId) {
    let mut container = Container::open(path).unwrap();
    let target = container
        .chunks()
        .unwrap()
        .into_iter()
        .find_map(|object| {
            let raw = container.read(object).unwrap();
            let envelope = publication_envelope::decode(&raw).ok().flatten()?;
            let Some(RelationshipPublication::Creation(creation)) =
                publication::decode(envelope.body, envelope.global_version)
                    .ok()
                    .flatten()
            else {
                return None;
            };
            (creation.record.id == id && creation.description_proof.is_some()).then_some((
                object.offset() as usize + 8,
                envelope.body.len(),
                raw.len() - envelope.body.len(),
            ))
        })
        .expect("pruned Relationship creation envelope");
    drop(container);
    let (chunk_file_offset, body_len, body_at) = target;
    let body_offset = chunk_file_offset + body_at;
    let mut bytes = fs::read(path).unwrap();
    let manifest_at = body_offset + body_len - 64;
    bytes[manifest_at] ^= 1;
    let checksum_at = body_offset + body_len - 32;
    let digest = Sha256::digest(&bytes[body_offset..checksum_at]);
    bytes[checksum_at..checksum_at + 32].copy_from_slice(&digest);
    assert!(publication::decode(&bytes[body_offset..body_offset + body_len], 1).is_ok());
    fs::write(path, bytes).unwrap();
}

#[test]
fn rel_and_phy_envelope_pruning_preserves_history_proofs_and_removes_selected_plaintext() {
    for kind in [FileKind::Reliquary, FileKind::Phylactery] {
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
        let (
            selected,
            unrelated,
            times,
            original_digests,
            descriptor,
            before_digest,
            source_hash,
            report,
        ) = if kind == FileKind::Reliquary {
            let mut owner = Cva::create_rel_with_uuid(&source, [0x91; 16], None).unwrap();
            let (selected, unrelated, times, digests) = write_history(&mut owner.container);
            owner.sync().unwrap();
            drop(owner);
            let mut owner = Cva::open(&source).unwrap();
            let descriptor = owner.relationship_retention_descriptor(selected).unwrap();
            let before_digest = history_digest(&mut owner.container);
            let source_hash = file_hash(&source);
            let report = owner
                .prune_tombstoned_history(
                    &output,
                    HistoryPruneRequest {
                        targets: vec![LifecycleTarget::Relationship(selected)],
                    },
                )
                .unwrap();
            (
                selected,
                unrelated,
                times,
                digests,
                descriptor,
                before_digest,
                source_hash,
                report,
            )
        } else {
            let mut owner = Phylactery::create_with_uuid(&source, [0x91; 16]).unwrap();
            let (selected, unrelated, times, digests) = write_history(&mut owner.container);
            owner.sync().unwrap();
            drop(owner);
            let mut owner = Phylactery::open(&source).unwrap();
            let descriptor = owner.relationship_retention_descriptor(selected).unwrap();
            let before_digest = history_digest(&mut owner.container);
            let source_hash = file_hash(&source);
            let report = owner
                .prune_tombstoned_history(
                    &output,
                    HistoryPruneRequest {
                        targets: vec![LifecycleTarget::Relationship(selected)],
                    },
                )
                .unwrap();
            (
                selected,
                unrelated,
                times,
                digests,
                descriptor,
                before_digest,
                source_hash,
                report,
            )
        };
        assert_eq!(descriptor.tombstone_revision, 3);
        assert_eq!(descriptor.tombstone_global_version, 3);
        assert_eq!(descriptor.tombstone_relationship_version, 3);
        assert_eq!(descriptor.revisions.len(), 2);
        assert_eq!(file_hash(&source), source_hash);
        assert_eq!(history_digests(&output), history_digests(&source));
        let actual_proofs = proofs(&output, selected);
        assert_eq!(actual_proofs.len(), 2);
        for (revision, proof) in actual_proofs {
            assert_eq!(proof.manifest_digest, report.manifest_digest);
            assert_eq!(proof.original_carrier_digest, original_digests[&revision]);
        }

        let bytes = fs::read(&output).unwrap();
        assert!(!contains(&bytes, "PRIVATE RELATIONSHIP SUMMARY ONE"));
        assert!(!contains(&bytes, "PRIVATE RELATIONSHIP SUMMARY TWO"));
        assert!(contains(&bytes, "AVAILABLE UNRELATED SUMMARY"));
        if kind == FileKind::Reliquary {
            let mut owner = Cva::open(&output).unwrap();
            assert_eq!(
                owner.relationship_retention_descriptor(selected).unwrap(),
                descriptor
            );
            assert_eq!(
                owner
                    .relationship_revision(unrelated, 1)
                    .unwrap()
                    .summary
                    .starts_with("AVAILABLE UNRELATED SUMMARY"),
                true
            );
            for revision in 1..=2 {
                assert!(matches!(
                    owner
                        .historical_relationship_revision(selected, revision)
                        .unwrap(),
                    HistoricalPayload::Pruned { .. }
                ));
                assert!(matches!(
                    owner.relationship_revision(selected, revision),
                    Err(crate::RelationshipError::PayloadUnavailable(
                        PayloadUnavailable::Pruned
                    ))
                ));
            }
            for (global, time) in times.iter().enumerate() {
                assert_eq!(owner.transaction_time_ns(global as u64 + 1), *time);
            }
            assert_eq!(history_digest(&mut owner.container), before_digest);
        } else {
            let mut owner = Phylactery::open(&output).unwrap();
            assert_eq!(
                owner.relationship_retention_descriptor(selected).unwrap(),
                descriptor
            );
            assert!(
                owner
                    .relationship_revision(unrelated, 1)
                    .unwrap()
                    .summary
                    .starts_with("AVAILABLE UNRELATED SUMMARY")
            );
            for revision in 1..=2 {
                assert!(matches!(
                    owner
                        .historical_relationship_revision(selected, revision)
                        .unwrap(),
                    HistoricalPayload::Pruned { .. }
                ));
                assert!(matches!(
                    owner.relationship_revision(selected, revision),
                    Err(crate::RelationshipError::PayloadUnavailable(
                        PayloadUnavailable::Pruned
                    ))
                ));
            }
            for (global, time) in times.iter().enumerate() {
                assert_eq!(owner.transaction_time_ns(global as u64 + 1), *time);
            }
            assert_eq!(history_digest(&mut owner.container), before_digest);
        }

        mutate_pruned_manifest_digest(&output, selected);
        let reopened = if kind == FileKind::Reliquary {
            Cva::open(&output).is_ok()
        } else {
            Phylactery::open(&output).is_ok()
        };
        assert!(
            !reopened,
            "rechecksummed Relationship proof tampering accepted for {kind:?}"
        );
    }
}
