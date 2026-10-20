//! Authenticated native pruning of compact Entity histories.
use crate::entity_codec::decode_version;
use crate::entity_compact_codec::{decode_creation, decode_delta, encode_creation, encode_delta};
use crate::entity_compact_contract::{EntityCreation, EntityDelta, EntityDescriptionProof};
use crate::entity_model::{EntityRecord, EntityRetentionDescriptor};
use crate::entity_store::{EntityReplayPublication, EntityStore};
use crate::owner_lifecycle::{HistoricalPayload, LifecycleTarget, PayloadUnavailable};
use crate::{
    Container, ContainerError, Cva, EntityGlobalId, EntityId, HistoryPruneRequest, Phylactery,
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
            .join(format!("entity-compact-prune-{}", uuid::Uuid::new_v4()));
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
    id: EntityId,
    global_id: Option<EntityGlobalId>,
    revision: u64,
    mutation_id: &str,
    canonical_name: &str,
    aliases: &[&str],
    summary: &str,
    global_version: u64,
    entity_version: u64,
) -> EntityRecord {
    EntityRecord {
        id,
        global_id,
        revision,
        canonical_name: canonical_name.into(),
        aliases: aliases.iter().map(|value| (*value).into()).collect(),
        kind: "project".into(),
        // Keep the native REL net-shrink policy meaningful despite proof overhead.
        summary: format!("{summary}{}", "x".repeat(1024)),
        mutation_id: mutation_id.into(),
        created_at_ns: 10,
        updated_at_ns: 20 + revision as i64,
        global_version,
        entity_version,
    }
}

struct Seed {
    selected_id: EntityId,
    unrelated_id: EntityId,
    selected_records: Vec<EntityRecord>,
    unrelated_record: EntityRecord,
    retained: EntityRetentionDescriptor,
    transaction_times: Vec<(u64, Option<i64>)>,
    orphan_secrets: Vec<String>,
}

fn seed_entities(store: &mut EntityStore, container: &mut Container) -> Seed {
    let selected_id = EntityId([0xA1; 32]);
    let unrelated_id = EntityId([0xB2; 32]);
    let global_a = EntityGlobalId([0xC3; 16]);
    let first = record(
        selected_id,
        None,
        1,
        "selected-create",
        "PRIVATE NAME REVISION ONE",
        &["PRIVATE ALIAS ONE"],
        "PRIVATE SUMMARY REVISION ONE",
        1,
        1,
    );
    assert!(
        store
            .replay_publication(container, EntityReplayPublication::Record(first))
            .unwrap()
    );
    let first = store.records().last().unwrap().clone();

    let unrelated = record(
        unrelated_id,
        Some(EntityGlobalId([0xD4; 16])),
        1,
        "unrelated-create",
        "UNRELATED AVAILABLE NAME",
        &["UNRELATED AVAILABLE ALIAS"],
        "UNRELATED AVAILABLE SUMMARY",
        2,
        2,
    );
    assert!(
        store
            .replay_publication(container, EntityReplayPublication::Record(unrelated))
            .unwrap()
    );
    let unrelated = store.records().last().unwrap().clone();

    let second = record(
        selected_id,
        Some(global_a),
        2,
        "selected-global-id-set",
        "PRIVATE NAME REVISION TWO",
        &["PRIVATE ALIAS TWO"],
        "PRIVATE SUMMARY REVISION TWO",
        3,
        3,
    );
    assert!(
        store
            .replay_publication(container, EntityReplayPublication::Record(second))
            .unwrap()
    );
    let second = store.records().last().unwrap().clone();

    let third = record(
        selected_id,
        None,
        3,
        "selected-global-id-clear",
        "PRIVATE NAME REVISION THREE",
        &["PRIVATE ALIAS THREE"],
        "PRIVATE SUMMARY REVISION THREE",
        4,
        4,
    );
    assert!(
        store
            .replay_publication(container, EntityReplayPublication::Record(third))
            .unwrap()
    );
    let third = store.records().last().unwrap().clone();
    assert_eq!(
        [first.global_id, second.global_id, third.global_id],
        [None, Some(global_a), None]
    );

    assert!(
        store
            .replay_publication(
                container,
                EntityReplayPublication::Tombstone {
                    id: selected_id,
                    revision: 4,
                    replacement_id: None,
                },
            )
            .unwrap()
    );

    let retained = store.retention_descriptor(selected_id).unwrap();
    let mut orphan_creation = record(
        selected_id,
        None,
        1,
        "unpublished-private-creation",
        "UNPUBLISHED PRIVATE CREATION SECRET",
        &["UNPUBLISHED PRIVATE ALIAS SECRET"],
        "UNPUBLISHED PRIVATE CREATION SUMMARY SECRET",
        100,
        100,
    );
    let creation_bytes =
        encode_creation(&EntityCreation::new(orphan_creation.clone()).unwrap()).unwrap();
    container.append(&creation_bytes).unwrap();
    let mut orphan_secrets = vec![
        orphan_creation.canonical_name.clone(),
        orphan_creation.aliases[0].clone(),
        orphan_creation.summary.clone(),
    ];
    orphan_creation.revision = 2;
    orphan_creation.global_version = 101;
    orphan_creation.entity_version = 101;
    orphan_creation.mutation_id = "unpublished-private-delta".into();
    orphan_creation.canonical_name = "UNPUBLISHED PRIVATE DELTA NAME SECRET".into();
    orphan_creation.aliases = vec!["UNPUBLISHED PRIVATE DELTA ALIAS SECRET".into()];
    orphan_creation.summary = "UNPUBLISHED PRIVATE DELTA SUMMARY SECRET".into();
    orphan_secrets.extend([
        orphan_creation.canonical_name.clone(),
        orphan_creation.aliases[0].clone(),
        orphan_creation.summary.clone(),
    ]);
    let orphan_previous = record(
        selected_id,
        None,
        1,
        "unpublished-private-creation",
        "UNPUBLISHED PRIVATE CREATION SECRET",
        &["UNPUBLISHED PRIVATE ALIAS SECRET"],
        "UNPUBLISHED PRIVATE CREATION SUMMARY SECRET",
        100,
        100,
    );
    container
        .append(
            &encode_delta(&EntityDelta::between(&orphan_previous, &orphan_creation).unwrap())
                .unwrap(),
        )
        .unwrap();
    container.sync().unwrap();

    let mut transaction_times = store
        .records()
        .iter()
        .map(|record| {
            (
                record.global_version,
                container.transaction_time_ns(record.global_version),
            )
        })
        .collect::<Vec<_>>();
    let tombstone_global = retained.tombstone_global_version;
    transaction_times.push((
        tombstone_global,
        container.transaction_time_ns(tombstone_global),
    ));
    Seed {
        selected_id,
        unrelated_id,
        selected_records: vec![first, second, third],
        unrelated_record: unrelated,
        retained,
        transaction_times,
        orphan_secrets,
    }
}

fn hash_file(path: &Path) -> [u8; 32] {
    Sha256::digest(fs::read(path).unwrap()).into()
}

fn contains_bytes(haystack: &[u8], needle: &str) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle.as_bytes())
}

fn payloads(path: &Path) -> Vec<(crate::ObjectRef, Vec<u8>)> {
    let mut container = Container::open(path).unwrap();
    let mut values = Vec::new();
    container
        .visit_payloads::<ContainerError>(|object, payload| {
            values.push((object, payload.to_vec()));
            Ok(())
        })
        .unwrap();
    values
}

fn selected_proofs(path: &Path, selected_id: EntityId) -> Vec<(u64, EntityDescriptionProof)> {
    let mut proofs = Vec::new();
    for (_, payload) in payloads(path) {
        if payload.starts_with(b"CVAENC") {
            let creation = decode_creation(&payload).unwrap();
            if creation.record.id == selected_id {
                proofs.push((1, creation.description_proof.unwrap()));
            }
        } else if payload.starts_with(b"CVAEND") {
            let delta = decode_delta(&payload).unwrap();
            if delta.id == selected_id {
                proofs.push((delta.revision, delta.description_proof.unwrap()));
            }
        }
    }
    proofs.sort_by_key(|(revision, _)| *revision);
    proofs
}

fn source_carrier_digests(path: &Path, selected_id: EntityId) -> BTreeMap<u64, [u8; 32]> {
    let mut container = Container::open(path).unwrap();
    let mut versions = BTreeMap::new();
    for object in container.chunks().unwrap() {
        let payload = container.read(object).unwrap();
        if let Some(version) = decode_version(&payload).unwrap() {
            let raw = container.read(version.record).unwrap();
            let id_revision = if raw.starts_with(b"CVAENC") {
                let creation = decode_creation(&raw).unwrap();
                (creation.record.id, 1)
            } else if raw.starts_with(b"CVAEND") {
                let delta = decode_delta(&raw).unwrap();
                (delta.id, delta.revision)
            } else {
                continue;
            };
            if id_revision.0 == selected_id {
                versions.insert(id_revision.1, Sha256::digest(raw).into());
            }
        }
    }
    versions
}

fn assert_pruned_entity(owner_path: &Path, seed: &Seed, descriptor: &EntityRetentionDescriptor) {
    let mut owner = Cva::open(owner_path).unwrap();
    assert_eq!(
        owner
            .entities
            .retention_descriptor(seed.selected_id)
            .unwrap(),
        *descriptor
    );
    assert_eq!(
        owner.entities.entity_version(),
        descriptor.tombstone_entity_version
    );
    for revision in 1..=3 {
        assert!(matches!(
            owner
                .historical_entity_revision(seed.selected_id, revision)
                .unwrap(),
            HistoricalPayload::Pruned { .. }
        ));
        assert!(matches!(
            owner.entity_revision(seed.selected_id, revision),
            Err(crate::EntityError::PayloadUnavailable(
                PayloadUnavailable::Pruned
            ))
        ));
    }
    let unrelated = owner.entity_revision(seed.unrelated_id, 1).unwrap();
    assert_eq!(
        unrelated.canonical_name,
        seed.unrelated_record.canonical_name
    );
    assert_eq!(unrelated.global_id, seed.unrelated_record.global_id);
    for (global, transaction_time) in &seed.transaction_times {
        assert_eq!(owner.transaction_time_ns(*global), *transaction_time);
    }
}

#[test]
fn rel_prune_redacts_all_selected_history_preserves_clocks_and_reclaims_authenticated_proofs() {
    let scratch = Scratch::new();
    let source = scratch.file("source.rel");
    let output = scratch.file("pruned.rel");
    let reclaimed = scratch.file("reclaimed.rel");
    let mut rel = Cva::create(&source).unwrap();
    let seed = seed_entities(&mut rel.entities, &mut rel.container);
    let source_hash = hash_file(&source);
    let expected_source_records = rel.entities.records().to_vec();
    let original_digests = source_carrier_digests(&source, seed.selected_id);
    assert_eq!(original_digests.len(), 3);

    let report = rel
        .prune_tombstoned_history(
            &output,
            HistoryPruneRequest {
                targets: vec![LifecycleTarget::Entity(seed.selected_id)],
            },
        )
        .unwrap();
    assert_eq!(hash_file(&source), source_hash);
    assert_eq!(rel.entities.records(), expected_source_records.as_slice());

    let mut pruned = Cva::open(&output).unwrap();
    let descriptor = pruned
        .entities
        .retention_descriptor(seed.selected_id)
        .unwrap();
    assert_eq!(descriptor, seed.retained);
    assert_pruned_entity(&output, &seed, &descriptor);
    assert_eq!(
        descriptor
            .revisions
            .iter()
            .map(|revision| revision.global_id)
            .collect::<Vec<_>>(),
        vec![None, Some(EntityGlobalId([0xC3; 16])), None]
    );
    assert_eq!(
        descriptor
            .revisions
            .iter()
            .map(|revision| (revision.global_version, revision.entity_version))
            .collect::<Vec<_>>(),
        seed.selected_records
            .iter()
            .map(|record| (record.global_version, record.entity_version))
            .collect::<Vec<_>>()
    );

    let output_bytes = fs::read(&output).unwrap();
    for secret in seed
        .selected_records
        .iter()
        .flat_map(|record| [record.canonical_name.as_str(), record.summary.as_str()])
        .chain(
            seed.selected_records
                .iter()
                .flat_map(|record| record.aliases.iter().map(String::as_str)),
        )
        .chain(seed.orphan_secrets.iter().map(String::as_str))
    {
        assert!(
            !contains_bytes(&output_bytes, secret),
            "secret survived: {secret}"
        );
    }
    assert!(contains_bytes(
        &output_bytes,
        &seed.unrelated_record.summary
    ));

    let proofs = selected_proofs(&output, seed.selected_id);
    assert_eq!(proofs.len(), 3);
    for (revision, proof) in &proofs {
        assert_eq!(proof.manifest_digest, report.manifest_digest);
        assert_eq!(proof.original_carrier_digest, original_digests[revision]);
    }

    pruned.reclaim_storage(&reclaimed).unwrap();
    let mut after_reclaim = Cva::open(&reclaimed).unwrap();
    assert_eq!(
        after_reclaim
            .entities
            .retention_descriptor(seed.selected_id)
            .unwrap(),
        descriptor
    );
    assert_eq!(selected_proofs(&reclaimed, seed.selected_id), proofs);
    assert_pruned_entity(&reclaimed, &seed, &descriptor);
}

#[test]
fn phy_prune_preserves_option_identity_history_and_unrelated_description() {
    let scratch = Scratch::new();
    let source = scratch.file("source.phy");
    let output = scratch.file("pruned.phy");
    let mut phy = Phylactery::create(&source).unwrap();
    let seed = seed_entities(&mut phy.entities, &mut phy.container);
    let source_hash = hash_file(&source);
    let expected_records = phy.entities.records().to_vec();
    let original_digests = source_carrier_digests(&source, seed.selected_id);
    let report = phy
        .prune_tombstoned_history(
            &output,
            HistoryPruneRequest {
                targets: vec![LifecycleTarget::Entity(seed.selected_id)],
            },
        )
        .unwrap();
    assert_eq!(hash_file(&source), source_hash);
    assert_eq!(phy.entities.records(), expected_records.as_slice());

    let mut pruned = Phylactery::open(&output).unwrap();
    let descriptor = pruned
        .entities
        .retention_descriptor(seed.selected_id)
        .unwrap();
    assert_eq!(descriptor, seed.retained);
    assert_eq!(
        descriptor
            .revisions
            .iter()
            .map(|revision| revision.global_id)
            .collect::<Vec<_>>(),
        vec![None, Some(EntityGlobalId([0xC3; 16])), None]
    );
    for revision in 1..=3 {
        assert!(matches!(
            pruned
                .historical_entity_revision(seed.selected_id, revision)
                .unwrap(),
            HistoricalPayload::Pruned { .. }
        ));
        assert!(matches!(
            pruned.entity_revision(seed.selected_id, revision),
            Err(crate::EntityError::PayloadUnavailable(
                PayloadUnavailable::Pruned
            ))
        ));
    }
    for (global, transaction_time) in &seed.transaction_times {
        assert_eq!(pruned.transaction_time_ns(*global), *transaction_time);
    }
    let proofs = selected_proofs(&output, seed.selected_id);
    assert_eq!(proofs.len(), 3);
    for (revision, proof) in &proofs {
        assert_eq!(proof.manifest_digest, report.manifest_digest);
        assert_eq!(proof.original_carrier_digest, original_digests[revision]);
    }
    assert_eq!(
        report.output_file_bytes,
        fs::metadata(&output).unwrap().len()
    );
    let unrelated = pruned.entity_revision(seed.unrelated_id, 1).unwrap();
    assert_eq!(
        unrelated.canonical_name,
        seed.unrelated_record.canonical_name
    );
    let output_bytes = fs::read(&output).unwrap();
    for secret in seed
        .selected_records
        .iter()
        .flat_map(|record| [record.canonical_name.as_str(), record.summary.as_str()])
        .chain(
            seed.selected_records
                .iter()
                .flat_map(|record| record.aliases.iter().map(String::as_str)),
        )
        .chain(seed.orphan_secrets.iter().map(String::as_str))
    {
        assert!(
            !contains_bytes(&output_bytes, secret),
            "secret survived: {secret}"
        );
    }
}

#[derive(Clone, Copy)]
enum ProofTamper {
    ManifestDigest,
    OriginalCarrierDigest,
    RemoveProof,
    DuplicateProofCarrier,
    DetachedProofCarrier,
}

fn tampered_copy(
    source: &Path,
    original_source: &Path,
    output: &Path,
    selected_id: EntityId,
    tamper: ProofTamper,
) {
    let mut input = Container::open(source).unwrap();
    let objects = input.chunks().unwrap();
    let target = objects
        .iter()
        .find_map(|object| {
            let payload = input.read(*object).unwrap();
            (payload.starts_with(b"CVAENC")
                && decode_creation(&payload)
                    .is_ok_and(|creation| creation.record.id == selected_id))
            .then_some((*object, payload))
        })
        .unwrap();
    let mut replacement = target.1.clone();
    let mut detached_proof = None;
    if matches!(tamper, ProofTamper::DetachedProofCarrier) {
        let mut original = Container::open(original_source).unwrap();
        let original_carrier = original
            .chunks()
            .unwrap()
            .into_iter()
            .find_map(|object| {
                let payload = original.read(object).unwrap();
                if !payload.starts_with(b"CVAENC") {
                    return None;
                }
                let creation = decode_creation(&payload).ok()?;
                (creation.record.id == selected_id && creation.record.revision == 1)
                    .then_some(payload)
            })
            .expect("original source creation carrier");
        let accepted_pruned = decode_creation(&target.1).unwrap();
        assert!(accepted_pruned.description_proof.is_some());
        assert!(
            decode_creation(&original_carrier)
                .unwrap()
                .description_proof
                .is_none()
        );
        detached_proof = Some(target.1.clone());
        replacement = original_carrier;
    } else if !matches!(tamper, ProofTamper::DuplicateProofCarrier) {
        let mut creation = decode_creation(&replacement).unwrap();
        match tamper {
            ProofTamper::ManifestDigest => {
                creation.description_proof.as_mut().unwrap().manifest_digest = [0xE1; 32];
            }
            ProofTamper::OriginalCarrierDigest => {
                creation
                    .description_proof
                    .as_mut()
                    .unwrap()
                    .original_carrier_digest = [0xE2; 32];
            }
            ProofTamper::RemoveProof => creation.description_proof = None,
            ProofTamper::DuplicateProofCarrier | ProofTamper::DetachedProofCarrier => {
                unreachable!()
            }
        }
        replacement = encode_creation(&creation).unwrap();
    }

    let mut destination = input.create_empty_like(output).unwrap();
    let mut relocated = BTreeMap::new();
    for object in objects {
        let raw = input.read(object).unwrap();
        let payload = if object == target.0 {
            replacement.as_slice().to_vec()
        } else {
            crate::storage_reclamation::relocate_payload(&raw, &relocated).unwrap()
        };
        let new_object = destination.append(&payload).unwrap();
        relocated.insert(object, new_object);
    }
    if matches!(tamper, ProofTamper::DuplicateProofCarrier) {
        destination.append(&target.1).unwrap();
    }
    if let Some(detached) = detached_proof {
        destination.append(&detached).unwrap();
    }
    destination.sync().unwrap();
}

#[test]
fn rel_open_rejects_rechecksummed_missing_wrong_and_duplicate_compact_description_proofs() {
    let scratch = Scratch::new();
    let source = scratch.file("source.rel");
    let pruned = scratch.file("pruned.rel");
    let mut rel = Cva::create(&source).unwrap();
    let seed = seed_entities(&mut rel.entities, &mut rel.container);
    rel.prune_tombstoned_history(
        &pruned,
        HistoryPruneRequest {
            targets: vec![LifecycleTarget::Entity(seed.selected_id)],
        },
    )
    .unwrap();

    for (name, tamper) in [
        ("wrong-manifest.rel", ProofTamper::ManifestDigest),
        ("wrong-original.rel", ProofTamper::OriginalCarrierDigest),
        ("missing-proof.rel", ProofTamper::RemoveProof),
        ("duplicate-proof.rel", ProofTamper::DuplicateProofCarrier),
        ("detached-proof.rel", ProofTamper::DetachedProofCarrier),
    ] {
        let path = scratch.file(name);
        tampered_copy(&pruned, &source, &path, seed.selected_id, tamper);
        assert!(Cva::open(&path).is_err(), "tampering was accepted: {name}");
    }
}
