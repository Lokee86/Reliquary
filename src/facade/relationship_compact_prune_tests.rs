//! Native compact Relationship description pruning for REL and PHY owners.
use crate::owner_lifecycle::{HistoricalPayload, LifecycleTarget, PayloadUnavailable};
use crate::owner_maintenance::{HistoryPruneManifest, PrunedPayload, RetentionDomain};
use crate::relationship_codec::encode_record;
use crate::relationship_compact_codec::{decode_creation, decode_delta, encode_creation};
use crate::relationship_compact_contract::{RelationshipCreation, RelationshipDescriptionProof};
use crate::relationship_model::RelationshipRecord;
use crate::{
    Container, Cva, EntityId, EntityRef, HistoryPruneRequest, MemoryId, MemoryRef, Phylactery,
    RelationshipDraft, RelationshipError, RelationshipId, RelationshipParticipant,
};
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
                "relationship-compact-prune-{}",
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

fn draft(mutation: &str, summary: &str, updated: i64) -> RelationshipDraft {
    RelationshipDraft {
        kind: "observes".into(),
        participants: vec![RelationshipParticipant {
            entity: EntityRef {
                owner_id: "remote-owner".into(),
                entity_id: EntityId([0x41; 32]),
            },
            role: Some("witness".into()),
        }],
        evidence: vec![MemoryRef {
            owner_id: "memory-owner".into(),
            memory_id: MemoryId([0x42; 32]),
        }],
        summary: format!("{summary} {}", "x".repeat(1024)),
        mutation_id: mutation.into(),
        created_at_ns: 10,
        updated_at_ns: updated,
    }
}

fn hash(path: &Path) -> [u8; 32] {
    Sha256::digest(fs::read(path).unwrap()).into()
}

fn contains(haystack: &[u8], needle: &str) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle.as_bytes())
}

fn selected_proofs(path: &Path, id: RelationshipId) -> Vec<(u64, RelationshipDescriptionProof)> {
    let mut container = Container::open(path).unwrap();
    let mut proofs = Vec::new();
    for object in container.chunks().unwrap() {
        let raw = container.read(object).unwrap();
        if raw.starts_with(b"CVARNC") {
            let creation = decode_creation(&raw).unwrap();
            if creation.record.id == id {
                proofs.push((1, creation.description_proof.unwrap()));
            }
        } else if raw.starts_with(b"CVARND") {
            let delta = decode_delta(&raw).unwrap();
            if delta.id == id {
                proofs.push((delta.revision, delta.description_proof.unwrap()));
            }
        }
    }
    proofs.sort_by_key(|(revision, _)| *revision);
    proofs
}

fn source_digests(path: &Path, id: RelationshipId) -> BTreeMap<u64, [u8; 32]> {
    let mut container = Container::open(path).unwrap();
    let mut digests = BTreeMap::new();
    for object in container.chunks().unwrap() {
        let marker = container.read(object).unwrap();
        let Some(version) = crate::relationship_codec::decode_version(&marker).unwrap() else {
            continue;
        };
        let raw = container.read(version.record).unwrap();
        if raw.starts_with(b"CVARNC") {
            let creation = decode_creation(&raw).unwrap();
            if creation.record.id == id {
                digests.insert(1, Sha256::digest(&raw).into());
            }
        } else if raw.starts_with(b"CVARND") {
            let delta = decode_delta(&raw).unwrap();
            if delta.id == id {
                digests.insert(delta.revision, Sha256::digest(&raw).into());
            }
        }
    }
    digests
}

#[derive(Clone, Copy)]
enum ProofTamper {
    WrongManifestDigest,
    WrongCarrierDigest,
    MissingDescriptor,
    WrongDescriptor,
    CrossTargetDescriptor,
    DuplicateProofCarrier,
    IncompleteChain,
}

fn repack_with_tampering(source: &Path, output: &Path, id: RelationshipId, tamper: ProofTamper) {
    let mut input = Container::open(source).unwrap();
    let objects = input.chunks().unwrap();
    let mut manifest_digest = None;
    let mut manifest_payload = None;
    if matches!(
        tamper,
        ProofTamper::MissingDescriptor
            | ProofTamper::WrongDescriptor
            | ProofTamper::CrossTargetDescriptor
    ) {
        let manifest_object = objects
            .iter()
            .find_map(|object| {
                let raw = input.read(*object).unwrap();
                raw.starts_with(b"CVAHPR01").then_some((*object, raw))
            })
            .unwrap();
        let mut manifest = HistoryPruneManifest::decode(&manifest_object.1).unwrap();
        let descriptor = manifest
            .descriptors
            .iter_mut()
            .find(|descriptor| {
                descriptor.target.domain == RetentionDomain::Relationship
                    && descriptor.target.identity.as_slice() == id.0.as_slice()
            })
            .unwrap();
        match tamper {
            ProofTamper::MissingDescriptor => {
                descriptor.target.identity = vec![0xE3; 32];
                descriptor.payloads = descriptor
                    .payloads
                    .iter()
                    .map(|payload| match payload {
                        PrunedPayload::RelationshipDescription(_, revision) => {
                            PrunedPayload::RelationshipDescription([0xE3; 32], *revision)
                        }
                        other => other.clone(),
                    })
                    .collect();
            }
            ProofTamper::WrongDescriptor => descriptor.retained_structure_digest = [0xE4; 32],
            ProofTamper::CrossTargetDescriptor => descriptor.target.identity = vec![0xE5; 32],
            _ => unreachable!(),
        }
        let encoded = manifest.encode().unwrap();
        manifest_digest = Some(manifest.digest().unwrap());
        manifest_payload = Some((manifest_object.0, encoded));
    }
    let mut destination = input.create_empty_like(output).unwrap();
    let mut relocated = BTreeMap::new();
    let mut duplicate_creation = None;
    for object in objects {
        let raw = input.read(object).unwrap();
        let payload = if let Some((manifest_object, replacement)) = &manifest_payload {
            if object == *manifest_object {
                replacement.clone()
            } else {
                tampered_relationship_carrier(
                    &raw,
                    id,
                    tamper,
                    manifest_digest,
                    &mut duplicate_creation,
                )
            }
        } else {
            tampered_relationship_carrier(
                &raw,
                id,
                tamper,
                manifest_digest,
                &mut duplicate_creation,
            )
        };
        let relocated_payload =
            crate::storage_reclamation::relocate_payload(&payload, &relocated).unwrap();
        relocated.insert(object, destination.append(&relocated_payload).unwrap());
    }
    if matches!(tamper, ProofTamper::DuplicateProofCarrier) {
        destination
            .append(&duplicate_creation.expect("selected creation carrier"))
            .unwrap();
    }
    destination.sync().unwrap();
}

fn tampered_relationship_carrier(
    raw: &[u8],
    id: RelationshipId,
    tamper: ProofTamper,
    manifest_digest: Option<[u8; 32]>,
    duplicate_creation: &mut Option<Vec<u8>>,
) -> Vec<u8> {
    if raw.starts_with(b"CVARNC") {
        let Ok(mut creation) = decode_creation(raw) else {
            return raw.to_vec();
        };
        if creation.record.id != id {
            return raw.to_vec();
        }
        if matches!(tamper, ProofTamper::DuplicateProofCarrier) {
            *duplicate_creation = Some(raw.to_vec());
            return raw.to_vec();
        }
        if matches!(tamper, ProofTamper::IncompleteChain) {
            return raw.to_vec();
        }
        let Some(proof) = creation.description_proof.as_mut() else {
            return raw.to_vec();
        };
        match tamper {
            ProofTamper::WrongManifestDigest => proof.manifest_digest = [0xE1; 32],
            ProofTamper::WrongCarrierDigest => proof.original_carrier_digest = [0xE2; 32],
            ProofTamper::MissingDescriptor
            | ProofTamper::WrongDescriptor
            | ProofTamper::CrossTargetDescriptor => {
                proof.manifest_digest = manifest_digest.unwrap()
            }
            _ => {}
        }
        return encode_creation(&creation).unwrap();
    }
    if raw.starts_with(b"CVARND") {
        let Ok(mut delta) = decode_delta(raw) else {
            return raw.to_vec();
        };
        if delta.id != id {
            return raw.to_vec();
        }
        match tamper {
            ProofTamper::IncompleteChain => delta.description_proof = None,
            ProofTamper::WrongManifestDigest => {
                delta.description_proof.as_mut().unwrap().manifest_digest = [0xE1; 32]
            }
            ProofTamper::WrongCarrierDigest => {
                delta
                    .description_proof
                    .as_mut()
                    .unwrap()
                    .original_carrier_digest = [0xE2; 32]
            }
            ProofTamper::MissingDescriptor
            | ProofTamper::WrongDescriptor
            | ProofTamper::CrossTargetDescriptor => {
                delta.description_proof.as_mut().unwrap().manifest_digest = manifest_digest.unwrap()
            }
            _ => return raw.to_vec(),
        }
        return crate::relationship_compact_codec::encode_delta(&delta).unwrap();
    }
    raw.to_vec()
}

fn setup_rel(
    path: &Path,
) -> (
    Cva,
    Vec<RelationshipRecord>,
    RelationshipId,
    RelationshipId,
    Vec<(u64, Option<i64>)>,
) {
    let mut owner = Cva::create(path).unwrap();
    let first = owner
        .publish_relationship(
            None,
            0,
            draft("selected-create", "PRIVATE RELATIONSHIP SUMMARY ONE", 11),
        )
        .unwrap()
        .0;
    let unrelated = owner
        .publish_relationship(
            None,
            0,
            draft("unrelated-create", "AVAILABLE UNRELATED SUMMARY", 12),
        )
        .unwrap()
        .0;
    let second = owner
        .publish_relationship(
            Some(first.id),
            first.revision,
            draft("selected-revise", "PRIVATE RELATIONSHIP SUMMARY TWO", 13),
        )
        .unwrap()
        .0;
    owner
        .tombstone_relationship(first.id, second.revision)
        .unwrap();
    let records = owner
        .relationships
        .records()
        .iter()
        .filter(|record| record.id == first.id)
        .cloned()
        .collect::<Vec<_>>();
    append_selected_orphans(&mut owner.container, &records[0]);
    let times = (1..=owner.latest_global_version())
        .map(|global| (global, owner.transaction_time_ns(global)))
        .collect();
    (owner, records, first.id, unrelated.id, times)
}

fn setup_phy(
    path: &Path,
) -> (
    Phylactery,
    Vec<RelationshipRecord>,
    RelationshipId,
    RelationshipId,
    Vec<(u64, Option<i64>)>,
) {
    let mut owner = Phylactery::create(path).unwrap();
    let first = owner
        .publish_relationship(
            None,
            0,
            draft("selected-create", "PRIVATE RELATIONSHIP SUMMARY ONE", 11),
        )
        .unwrap()
        .0;
    let unrelated = owner
        .publish_relationship(
            None,
            0,
            draft("unrelated-create", "AVAILABLE UNRELATED SUMMARY", 12),
        )
        .unwrap()
        .0;
    let second = owner
        .publish_relationship(
            Some(first.id),
            first.revision,
            draft("selected-revise", "PRIVATE RELATIONSHIP SUMMARY TWO", 13),
        )
        .unwrap()
        .0;
    owner
        .tombstone_relationship(first.id, second.revision)
        .unwrap();
    let records = owner
        .relationships
        .records()
        .iter()
        .filter(|record| record.id == first.id)
        .cloned()
        .collect::<Vec<_>>();
    append_selected_orphans(&mut owner.container, &records[0]);
    let times = (1..=owner.latest_global_version())
        .map(|global| (global, owner.transaction_time_ns(global)))
        .collect();
    (owner, records, first.id, unrelated.id, times)
}

fn append_selected_orphans(container: &mut Container, record: &RelationshipRecord) {
    let mut orphan = record.clone();
    orphan.summary = "UNPUBLISHED SELECTED RELATIONSHIP SECRET".into();
    orphan.mutation_id = "unpublished-selected-carrier".into();
    container.append(&encode_record(&orphan).unwrap()).unwrap();
    container
        .append(&encode_creation(&RelationshipCreation::new(orphan).unwrap()).unwrap())
        .unwrap();
    container.sync().unwrap();
}

fn assert_pruned(
    owner: &mut Cva,
    id: RelationshipId,
    unrelated_id: RelationshipId,
    expected: &[RelationshipRecord],
    times: &[(u64, Option<i64>)],
) {
    let descriptor = owner.relationship_retention_descriptor(id).unwrap();
    assert_eq!(descriptor.revisions.len(), expected.len());
    for (retained, record) in descriptor.revisions.iter().zip(expected) {
        assert_eq!(retained.revision, record.revision);
        assert_eq!(retained.global_version, record.global_version);
        assert_eq!(retained.relationship_version, record.relationship_version);
        assert_eq!(retained.kind, record.kind);
        assert_eq!(retained.participants, record.participants);
        assert_eq!(retained.evidence, record.evidence);
        assert_eq!(retained.created_at_ns, record.created_at_ns);
        assert_eq!(retained.updated_at_ns, record.updated_at_ns);
        assert_eq!(retained.mutation_id, record.mutation_id);
        assert!(matches!(
            owner
                .historical_relationship_revision(id, record.revision)
                .unwrap(),
            HistoricalPayload::Pruned { .. }
        ));
        assert!(matches!(
            owner.relationship_revision(id, record.revision),
            Err(RelationshipError::PayloadUnavailable(
                PayloadUnavailable::Pruned
            ))
        ));
    }
    let unrelated = owner.relationship_revision(unrelated_id, 1).unwrap();
    assert!(unrelated.summary.starts_with("AVAILABLE UNRELATED SUMMARY"));
    for (global, time) in times {
        assert_eq!(owner.transaction_time_ns(*global), *time);
    }
}

fn assert_pruned_phy(
    owner: &mut Phylactery,
    id: RelationshipId,
    unrelated_id: RelationshipId,
    expected: &[RelationshipRecord],
    times: &[(u64, Option<i64>)],
) {
    let descriptor = owner.relationship_retention_descriptor(id).unwrap();
    assert_eq!(descriptor.revisions.len(), expected.len());
    for (retained, record) in descriptor.revisions.iter().zip(expected) {
        assert_eq!(retained.revision, record.revision);
        assert_eq!(retained.global_version, record.global_version);
        assert_eq!(retained.relationship_version, record.relationship_version);
        assert_eq!(retained.kind, record.kind);
        assert_eq!(retained.participants, record.participants);
        assert_eq!(retained.evidence, record.evidence);
        assert_eq!(retained.created_at_ns, record.created_at_ns);
        assert_eq!(retained.updated_at_ns, record.updated_at_ns);
        assert_eq!(retained.mutation_id, record.mutation_id);
        assert!(matches!(
            owner
                .historical_relationship_revision(id, record.revision)
                .unwrap(),
            HistoricalPayload::Pruned { .. }
        ));
        assert!(matches!(
            owner.relationship_revision(id, record.revision),
            Err(RelationshipError::PayloadUnavailable(
                PayloadUnavailable::Pruned
            ))
        ));
    }
    assert!(
        owner
            .relationship_revision(unrelated_id, 1)
            .unwrap()
            .summary
            .starts_with("AVAILABLE UNRELATED SUMMARY")
    );
    for (global, time) in times {
        assert_eq!(owner.transaction_time_ns(*global), *time);
    }
}

#[test]
fn rel_and_phy_native_compact_pruning_preserves_retained_structure_and_proofs() {
    for phy_owner in [false, true] {
        let scratch = Scratch::new();
        let source = scratch.file(if phy_owner {
            "source.phy"
        } else {
            "source.rel"
        });
        let output = scratch.file(if phy_owner {
            "pruned.phy"
        } else {
            "pruned.rel"
        });
        let (expected, id, unrelated_id, times, source_hash, report) = if phy_owner {
            let (mut phy, expected, id, unrelated_id, times) = setup_phy(&source);
            let source_hash = hash(&source);
            let report = phy
                .prune_tombstoned_history(
                    &output,
                    HistoryPruneRequest {
                        targets: vec![LifecycleTarget::Relationship(id)],
                    },
                )
                .unwrap();
            (expected, id, unrelated_id, times, source_hash, report)
        } else {
            let (mut rel, expected, id, unrelated_id, times) = setup_rel(&source);
            let source_hash = hash(&source);
            let report = rel
                .prune_tombstoned_history(
                    &output,
                    HistoryPruneRequest {
                        targets: vec![LifecycleTarget::Relationship(id)],
                    },
                )
                .unwrap();
            (expected, id, unrelated_id, times, source_hash, report)
        };
        let original_digests = source_digests(&source, id);
        assert_eq!(original_digests.len(), 2);
        assert_eq!(hash(&source), source_hash);
        if phy_owner {
            let mut output_owner = Phylactery::open(&output).unwrap();
            output_owner.relationship_retention_descriptor(id).unwrap();
            assert_pruned_phy(&mut output_owner, id, unrelated_id, &expected, &times);
        } else {
            let mut output_owner = Cva::open(&output).unwrap();
            output_owner.relationship_retention_descriptor(id).unwrap();
            assert_pruned(&mut output_owner, id, unrelated_id, &expected, &times);
        }
        let bytes = fs::read(&output).unwrap();
        assert!(!contains(&bytes, "PRIVATE RELATIONSHIP SUMMARY ONE"));
        assert!(!contains(&bytes, "PRIVATE RELATIONSHIP SUMMARY TWO"));
        assert!(!contains(
            &bytes,
            "UNPUBLISHED SELECTED RELATIONSHIP SECRET"
        ));
        assert!(contains(&bytes, "AVAILABLE UNRELATED SUMMARY"));
        let proofs = selected_proofs(&output, id);
        assert_eq!(proofs.len(), 2);
        for (revision, proof) in &proofs {
            assert_eq!(proof.manifest_digest, report.manifest_digest);
            assert_eq!(proof.original_carrier_digest, original_digests[revision]);
        }
        assert_eq!(
            report.output_file_bytes,
            fs::metadata(&output).unwrap().len()
        );
    }
}

#[test]
fn rel_open_rejects_rechecksummed_relationship_proof_digest_tampering() {
    let cases = [
        ("wrong-manifest", ProofTamper::WrongManifestDigest),
        ("wrong-carrier", ProofTamper::WrongCarrierDigest),
        ("missing-descriptor", ProofTamper::MissingDescriptor),
        ("wrong-descriptor", ProofTamper::WrongDescriptor),
        ("cross-target", ProofTamper::CrossTargetDescriptor),
        ("duplicate-proof", ProofTamper::DuplicateProofCarrier),
        ("incomplete-chain", ProofTamper::IncompleteChain),
    ];
    for phy_owner in [false, true] {
        for (name, tamper) in cases {
            let scratch = Scratch::new();
            let source = scratch.file(if phy_owner {
                "source.phy"
            } else {
                "source.rel"
            });
            let pruned = scratch.file(if phy_owner {
                "pruned.phy"
            } else {
                "pruned.rel"
            });
            let id = if phy_owner {
                let (mut owner, _, id, _, _) = setup_phy(&source);
                owner
                    .prune_tombstoned_history(
                        &pruned,
                        HistoryPruneRequest {
                            targets: vec![LifecycleTarget::Relationship(id)],
                        },
                    )
                    .unwrap();
                id
            } else {
                let (mut owner, _, id, _, _) = setup_rel(&source);
                owner
                    .prune_tombstoned_history(
                        &pruned,
                        HistoryPruneRequest {
                            targets: vec![LifecycleTarget::Relationship(id)],
                        },
                    )
                    .unwrap();
                id
            };
            let tampered =
                scratch.file(&format!("{name}.{}", if phy_owner { "phy" } else { "rel" }));
            repack_with_tampering(&pruned, &tampered, id, tamper);
            let opened = if phy_owner {
                Phylactery::open(&tampered).is_ok()
            } else {
                Cva::open(&tampered).is_ok()
            };
            assert!(!opened, "tampering was accepted: {name}, phy={phy_owner}");
        }
    }
}
