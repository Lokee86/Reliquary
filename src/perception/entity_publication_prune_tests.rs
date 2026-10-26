//! Authenticated Entity pruning for envelope publications.
use crate::entity_codec::{
    DirectEntityTombstone, EntityTombstone, EntityVersion, encode_direct_tombstone,
    encode_tombstone, encode_version,
};
use crate::entity_compact_codec::publication::{self, EntityPublication};
use crate::entity_compact_codec::{encode_creation, encode_delta};
use crate::entity_compact_contract::{EntityCreation, EntityDelta};
use crate::entity_model::EntityRecord;
use crate::owner_lifecycle::{HistoricalPayload, LifecycleTarget};
use crate::{Cva, EntityGlobalId, EntityId, HistoryPruneRequest, Phylactery};
use reliquary_container::publication_envelope::{PublicationClock, PublicationEnvelope};
use reliquary_container::{Container, FileKind};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!("entity-publication-prune-{}", uuid::Uuid::new_v4()));
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

#[derive(Clone, Copy)]
enum WireFormat {
    Legacy,
    Envelope,
}

fn record(id: u8, revision: u64, mutation: &str, global: u64, local: u64) -> EntityRecord {
    EntityRecord {
        id: EntityId([id; 32]),
        global_id: if revision == 1 {
            None
        } else {
            Some(EntityGlobalId([id.wrapping_add(80); 16]))
        },
        revision,
        canonical_name: format!(
            "PRIVATE ENTITY NAME {id} REVISION {revision} {}",
            "n".repeat(200)
        ),
        aliases: vec![format!("PRIVATE ENTITY ALIAS {id} {}", "a".repeat(80))],
        kind: "project".into(),
        summary: format!(
            "PRIVATE ENTITY SUMMARY {id} REVISION {revision} {}",
            "s".repeat(1400)
        ),
        mutation_id: mutation.into(),
        created_at_ns: 10,
        updated_at_ns: 20 + revision as i64,
        global_version: global,
        entity_version: local,
    }
}

fn reserve(container: &mut Container, time: Option<i64>) -> u64 {
    container.set_next_transaction_time_override(time);
    container.allocate_version().unwrap()
}

fn append_bound_publication(container: &mut Container, global: u64, value: &EntityPublication) {
    let body = publication::encode(value).unwrap();
    let raw = reliquary_container::publication_envelope::encode(PublicationEnvelope {
        global_version: global,
        clock: PublicationClock::BindReserved,
        body: &body,
    })
    .unwrap();
    container.append(&raw).unwrap();
}

fn append_publication(
    container: &mut Container,
    format: WireFormat,
    global: u64,
    time: Option<i64>,
    value: EntityPublication,
) {
    match format {
        WireFormat::Envelope => {
            let body = publication::encode(&value).unwrap();
            container.set_next_transaction_time_override(time);
            assert_eq!(
                container.append_publication_envelope(&body).unwrap().0,
                global
            );
        }
        WireFormat::Legacy => {
            let (carrier, local) = match &value {
                EntityPublication::Creation(creation) => (
                    encode_creation(creation).unwrap(),
                    creation.record.entity_version,
                ),
                EntityPublication::Delta(delta) => {
                    (encode_delta(delta).unwrap(), delta.entity_version)
                }
                EntityPublication::MergeTombstone {
                    id,
                    revision,
                    entity_version,
                    replacement_id,
                } => (
                    encode_tombstone(EntityTombstone {
                        id: *id,
                        revision: *revision,
                        replacement_id: *replacement_id,
                    }),
                    *entity_version,
                ),
                EntityPublication::DirectTombstone {
                    id,
                    revision,
                    entity_version,
                } => (
                    encode_direct_tombstone(DirectEntityTombstone {
                        id: *id,
                        revision: *revision,
                    }),
                    *entity_version,
                ),
            };
            let object = container.append(&carrier).unwrap();
            container.set_next_transaction_time_override(time);
            assert_eq!(container.allocate_version().unwrap(), global);
            container
                .append(&encode_version(EntityVersion {
                    global_version: global,
                    entity_version: local,
                    record: object,
                }))
                .unwrap();
        }
    }
}

fn write_history(container: &mut Container, format: WireFormat) -> (EntityId, EntityId) {
    // Preserve the accepted reverse-global history and a missing reservation time.
    assert_eq!(reserve(container, Some(-111)), 1);
    assert_eq!(reserve(container, None), 2);
    let first = record(11, 1, "private-create-a", 2, 1);
    let mut revised = first.clone();
    revised.revision = 2;
    revised.global_version = 1;
    revised.entity_version = 2;
    revised.mutation_id = "private-revise-a".into();
    revised.canonical_name = format!("PRIVATE ENTITY NAME A REVISION TWO {}", "q".repeat(200));
    revised.summary = format!("PRIVATE ENTITY SUMMARY A REVISION TWO {}", "r".repeat(1400));
    match format {
        WireFormat::Envelope => {
            append_bound_publication(
                container,
                2,
                &EntityPublication::Creation(EntityCreation::new(first.clone()).unwrap()),
            );
            append_bound_publication(
                container,
                1,
                &EntityPublication::Delta(EntityDelta::between(&first, &revised).unwrap()),
            );
        }
        WireFormat::Legacy => {
            let carrier = container
                .append(&encode_creation(&EntityCreation::new(first.clone()).unwrap()).unwrap())
                .unwrap();
            container
                .append(&encode_version(EntityVersion {
                    global_version: 2,
                    entity_version: 1,
                    record: carrier,
                }))
                .unwrap();
            let carrier = container
                .append(&encode_delta(&EntityDelta::between(&first, &revised).unwrap()).unwrap())
                .unwrap();
            container
                .append(&encode_version(EntityVersion {
                    global_version: 1,
                    entity_version: 2,
                    record: carrier,
                }))
                .unwrap();
        }
    }

    let second = record(22, 1, "private-create-b", 3, 3);
    append_publication(
        container,
        format,
        3,
        Some(0),
        EntityPublication::Creation(EntityCreation::new(second.clone()).unwrap()),
    );
    append_publication(
        container,
        format,
        4,
        Some(-444),
        EntityPublication::MergeTombstone {
            id: first.id,
            revision: 3,
            entity_version: 4,
            replacement_id: second.id,
        },
    );
    append_publication(
        container,
        format,
        5,
        None,
        EntityPublication::DirectTombstone {
            id: second.id,
            revision: 2,
            entity_version: 5,
        },
    );
    (first.id, second.id)
}

fn owner_history_digest(container: &mut Container, watermark: u64) -> [u8; 32] {
    crate::owner_maintenance::owner_history_digest(
        container,
        watermark,
        &crate::graph_owner::GraphOwner::empty(),
    )
    .unwrap()
}

fn file_hash(path: &Path) -> [u8; 32] {
    Sha256::digest(fs::read(path).unwrap()).into()
}

fn contains_bytes(bytes: &[u8], text: &str) -> bool {
    bytes
        .windows(text.len())
        .any(|window| window == text.as_bytes())
}

fn assert_pruned_rel(path: &Path, ids: (EntityId, EntityId), times: &[Option<i64>]) {
    let owner = Cva::open(path).unwrap();
    assert_eq!(owner.entity_version(), 5);
    let merged = owner.entity_retention_descriptor(ids.0).unwrap();
    let direct = owner.entity_retention_descriptor(ids.1).unwrap();
    assert_eq!(merged.tombstone_revision, 3);
    assert_eq!(merged.replacement_id, Some(ids.1));
    assert_eq!(direct.tombstone_revision, 2);
    assert_eq!(direct.replacement_id, None);
    for revision in 1..=2 {
        assert!(matches!(
            owner.historical_entity_revision(ids.0, revision).unwrap(),
            HistoricalPayload::Pruned { .. }
        ));
    }
    assert!(matches!(
        owner.historical_entity_revision(ids.1, 1).unwrap(),
        HistoricalPayload::Pruned { .. }
    ));
    for (index, time) in times.iter().enumerate() {
        assert_eq!(owner.transaction_time_ns(index as u64 + 1), *time);
    }
}

fn make_envelope_prune_source(
    path: &Path,
    kind: FileKind,
) -> (EntityId, EntityId, Vec<Option<i64>>) {
    let owner_uuid = [0xA3; 16];
    let ids = match kind {
        FileKind::Reliquary => {
            let mut owner = Cva::create_rel_with_uuid(path, owner_uuid, None).unwrap();
            let ids = write_history(&mut owner.container, WireFormat::Envelope);
            owner.sync().unwrap();
            ids
        }
        FileKind::Phylactery => {
            let mut owner = Phylactery::create_with_uuid(path, owner_uuid).unwrap();
            let ids = write_history(&mut owner.container, WireFormat::Envelope);
            owner.sync().unwrap();
            ids
        }
    };
    let times = [Some(-111), None, Some(0), Some(-444), None].to_vec();
    (ids.0, ids.1, times)
}

fn mutate_creation_mutation_id_and_reseal(path: &Path, id: EntityId) {
    let mut container = Container::open(path).unwrap();
    let mut target = None;
    for object in container.chunks().unwrap() {
        let raw = container.read(object).unwrap();
        let Some(envelope) = reliquary_container::publication_envelope::decode(&raw).unwrap()
        else {
            continue;
        };
        let Some(EntityPublication::Creation(creation)) =
            publication::decode(envelope.body, envelope.global_version).unwrap()
        else {
            continue;
        };
        if creation.record.id != id || creation.description_proof.is_none() {
            continue;
        }
        let envelope_body_offset = raw.len() - envelope.body.len();
        let global_id_tag = envelope.body[40];
        let after_global_id = 41 + if global_id_tag == 1 { 16 } else { 0 };
        let mutation_len_offset = after_global_id + 8;
        let mutation_len = u32::from_le_bytes(
            envelope.body[mutation_len_offset..mutation_len_offset + 4]
                .try_into()
                .unwrap(),
        ) as usize;
        assert!(mutation_len > 0);
        target = Some((
            object.offset() as usize + 8 + envelope_body_offset,
            envelope.global_version,
            envelope.body.len(),
            mutation_len_offset + 4,
        ));
        break;
    }
    drop(container);

    let (body_file_offset, global, body_len, mutation_at) =
        target.expect("pruned Entity creation envelope");
    let mut file = fs::read(path).unwrap();
    let mutation_offset = body_file_offset + mutation_at;
    file[mutation_offset] ^= 1;
    let checksum_at = body_file_offset + body_len - 32;
    let digest = Sha256::digest(&file[body_file_offset..checksum_at]);
    file[checksum_at..checksum_at + 32].copy_from_slice(&digest);
    let body = &file[body_file_offset..body_file_offset + body_len];
    assert!(
        publication::decode(body, global).is_ok(),
        "tamper remains a structurally valid Entity body"
    );
    fs::write(path, file).unwrap();
}

#[test]
fn native_rel_and_phy_pruning_preserves_terminal_descriptors_and_authenticates_envelope_sources() {
    for kind in [FileKind::Reliquary, FileKind::Phylactery] {
        let scratch = Scratch::new();
        let source = scratch.file("source.cva");
        let output = scratch.file("pruned.cva");
        let (merged_id, direct_id, times) = make_envelope_prune_source(&source, kind);
        let source_hash = file_hash(&source);

        if kind == FileKind::Reliquary {
            let mut owner = Cva::open(&source).unwrap();
            let descriptors = [
                owner.entity_retention_descriptor(merged_id).unwrap(),
                owner.entity_retention_descriptor(direct_id).unwrap(),
            ];
            owner
                .prune_tombstoned_history(
                    &output,
                    HistoryPruneRequest {
                        targets: vec![
                            LifecycleTarget::Entity(merged_id),
                            LifecycleTarget::Entity(direct_id),
                        ],
                    },
                )
                .unwrap();
            assert_eq!(file_hash(&source), source_hash);
            drop(owner);
            assert_pruned_rel(&output, (merged_id, direct_id), &times);
            let reopened = Cva::open(&output).unwrap();
            assert_eq!(
                reopened.entity_retention_descriptor(merged_id).unwrap(),
                descriptors[0]
            );
            assert_eq!(
                reopened.entity_retention_descriptor(direct_id).unwrap(),
                descriptors[1]
            );
        } else {
            let mut owner = Phylactery::open(&source).unwrap();
            let descriptors = [
                owner.entity_retention_descriptor(merged_id).unwrap(),
                owner.entity_retention_descriptor(direct_id).unwrap(),
            ];
            owner
                .prune_tombstoned_history(
                    &output,
                    HistoryPruneRequest {
                        targets: vec![
                            LifecycleTarget::Entity(merged_id),
                            LifecycleTarget::Entity(direct_id),
                        ],
                    },
                )
                .unwrap();
            assert_eq!(file_hash(&source), source_hash);
            drop(owner);
            let reopened = Phylactery::open(&output).unwrap();
            assert_eq!(
                reopened.entity_retention_descriptor(merged_id).unwrap(),
                descriptors[0]
            );
            assert_eq!(
                reopened.entity_retention_descriptor(direct_id).unwrap(),
                descriptors[1]
            );
            assert_eq!(reopened.entity_version(), 5);
        }

        let output_bytes = fs::read(&output).unwrap();
        for id in [11_u8, 22_u8] {
            for revision in 1..=2 {
                let secret = format!("PRIVATE ENTITY NAME {id} REVISION {revision}");
                assert!(
                    !contains_bytes(&output_bytes, &secret),
                    "plaintext survived: {secret}"
                );
            }
            let secret = format!("PRIVATE ENTITY SUMMARY {id}");
            assert!(
                !contains_bytes(&output_bytes, &secret),
                "plaintext survived: {secret}"
            );
        }

        mutate_creation_mutation_id_and_reseal(&output, merged_id);
        if kind == FileKind::Reliquary {
            assert!(
                Cva::open(&output).is_err(),
                "REL must reject proof/body structural mismatch"
            );
        } else {
            assert!(
                Phylactery::open(&output).is_err(),
                "PHY must reject proof/body structural mismatch"
            );
        }
    }
}
