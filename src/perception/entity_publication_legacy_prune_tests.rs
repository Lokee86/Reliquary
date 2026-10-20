//! Pruning keeps source proofs valid for imported raw Entity carriers followed
//! by compact publication envelopes.
use crate::entity_compact_codec::publication::{self, EntityPublication};
use crate::entity_compact_contract::EntityDelta;
use crate::entity_model::EntityDraft;
use crate::owner_lifecycle::{HistoricalPayload, LifecycleTarget};
use crate::{Cva, EntityGlobalId, FileKind, HistoryPruneRequest, Phylactery};
use std::fs;
use std::path::{Path, PathBuf};

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!(
                "entity-legacy-envelope-prune-{}",
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

fn draft(mutation_id: &str, summary: &str, updated_at_ns: i64) -> EntityDraft {
    EntityDraft {
        canonical_name: "Imported legacy project entity".into(),
        aliases: vec!["Imported legacy alias".into()],
        kind: "project".into(),
        summary: summary.into(),
        mutation_id: mutation_id.into(),
        created_at_ns: 10,
        updated_at_ns,
    }
}

fn history_digest(container: &mut crate::Container) -> [u8; 32] {
    let watermark = container.latest_version();
    crate::owner_maintenance::owner_history_digest(
        container,
        watermark,
        &crate::graph_store::GraphStore::empty(),
    )
    .unwrap()
}

fn seed_entity(
    container: &mut crate::Container,
    entities: &mut crate::entity_store::EntityStore,
) -> crate::EntityId {
    container.set_owner_write_scope(true);
    container.set_next_transaction_time_override(Some(-31));
    let (legacy, changed) = entities
        .publish_legacy_fixture(
            container,
            None,
            0,
            draft(
                "imported-legacy-create",
                &"Original imported description ".repeat(64),
                11,
            ),
            Some(EntityGlobalId([0xA4; 16])),
        )
        .unwrap();
    assert!(changed);

    let previous = entities
        .records()
        .iter()
        .find(|record| record.id == legacy.id)
        .unwrap()
        .clone();
    let mut revised = previous.clone();
    revised.revision += 1;
    revised.global_version = container.next_version_candidate();
    revised.entity_version = entities.entity_version() + 1;
    revised.mutation_id = "envelope-revision-after-import".into();
    revised.summary = "Envelope revision description ".repeat(64);
    revised.updated_at_ns = 12;
    let delta = EntityDelta::between(&previous, &revised).unwrap();
    let body = publication::encode(&EntityPublication::Delta(delta.clone())).unwrap();
    container.set_next_transaction_time_override(Some(0));
    let (global, _) = container.append_publication_envelope(&body).unwrap();
    assert_eq!(global, revised.global_version);
    let applied = crate::entity_compact_apply::apply_delta(&previous, &delta).unwrap();
    assert_eq!(applied, revised);
    entities.insert_rebuilt(applied).unwrap();

    let terminal_global = container.next_version_candidate();
    let terminal_local = entities.entity_version() + 1;
    let body = publication::encode(&EntityPublication::DirectTombstone {
        id: legacy.id,
        revision: revised.revision + 1,
        entity_version: terminal_local,
    })
    .unwrap();
    container.set_next_transaction_time_override(None);
    let (global, _) = container.append_publication_envelope(&body).unwrap();
    assert_eq!(global, terminal_global);
    entities
        .insert_direct_tombstone_rebuilt(
            crate::entity_codec::DirectEntityTombstone {
                id: legacy.id,
                revision: revised.revision + 1,
            },
            global,
            terminal_local,
        )
        .unwrap();
    container.set_owner_write_scope(false);
    legacy.id
}

fn prune_rel(source: &Path, output: &Path, id: crate::EntityId) -> [u8; 32] {
    let mut owner = Cva::open(source).unwrap();
    owner
        .prune_tombstoned_history(
            output,
            HistoryPruneRequest {
                targets: vec![LifecycleTarget::Entity(id)],
            },
        )
        .unwrap()
        .manifest_digest
}

fn prune_phy(source: &Path, output: &Path, id: crate::EntityId) -> [u8; 32] {
    let mut owner = Phylactery::open(source).unwrap();
    owner
        .prune_tombstoned_history(
            output,
            HistoryPruneRequest {
                targets: vec![LifecycleTarget::Entity(id)],
            },
        )
        .unwrap()
        .manifest_digest
}

fn assert_pruned(
    path: &Path,
    id: crate::EntityId,
    source_digest: [u8; 32],
    manifest_digest: [u8; 32],
    times: &[Option<i64>],
) {
    let mut owner = crate::Container::open(path).unwrap();
    assert_eq!(history_digest(&mut owner), source_digest);
    for (index, time) in times.iter().enumerate() {
        assert_eq!(owner.transaction_time_ns(index as u64 + 1), *time);
    }
    drop(owner);

    let retained = match crate::Container::open(path)
        .unwrap()
        .identity()
        .unwrap()
        .file_kind
    {
        FileKind::Reliquary => {
            let owner = Cva::open(path).unwrap();
            assert_eq!(owner.entity_version(), 3);
            let descriptor = owner.entity_retention_descriptor(id).unwrap();
            assert_eq!(descriptor.tombstone_revision, 3);
            assert_eq!(descriptor.revisions.len(), 2);
            for revision in 1..=2 {
                assert!(matches!(
                    owner.historical_entity_revision(id, revision).unwrap(),
                    HistoricalPayload::Pruned { .. }
                ));
            }
            descriptor
        }
        FileKind::Phylactery => {
            let owner = Phylactery::open(path).unwrap();
            assert_eq!(owner.entity_version(), 3);
            let descriptor = owner.entity_retention_descriptor(id).unwrap();
            assert_eq!(descriptor.tombstone_revision, 3);
            assert_eq!(descriptor.revisions.len(), 2);
            for revision in 1..=2 {
                assert!(matches!(
                    owner.historical_entity_revision(id, revision).unwrap(),
                    HistoricalPayload::Pruned { .. }
                ));
            }
            descriptor
        }
    };

    let mut container = crate::Container::open(path).unwrap();
    let mut saw_pruned_envelope = false;
    for object in container.chunks().unwrap() {
        let payload = container.read(object).unwrap();
        let Some(envelope) = crate::container::publication_envelope::decode(&payload).unwrap()
        else {
            continue;
        };
        if let Some((entity_id, revision, proof)) =
            crate::entity_description_proof::compact_description_proof(&payload).unwrap()
        {
            if entity_id == id {
                assert_eq!(proof.manifest_digest, manifest_digest);
                assert_ne!(proof.original_carrier_digest, [0; 32]);
                assert!(matches!(
                    publication::decode(envelope.body, envelope.global_version).unwrap(),
                    Some(EntityPublication::Creation(_)) | Some(EntityPublication::Delta(_))
                ));
                saw_pruned_envelope = true;
                assert!(revision <= 2);
            }
        }
    }
    assert!(
        saw_pruned_envelope,
        "the revised accepted envelope is pruned"
    );
}

fn exercise(kind: FileKind) {
    let scratch = Scratch::new();
    let source = scratch.file("source.owner");
    let output = scratch.file("pruned.owner");
    let id = match kind {
        FileKind::Reliquary => {
            let mut owner = Cva::create_rel_with_uuid(&source, [0xB3; 16], None).unwrap();
            let id = seed_entity(&mut owner.container, &mut owner.entities);
            owner.sync().unwrap();
            id
        }
        FileKind::Phylactery => {
            let mut owner = Phylactery::create_with_uuid(&source, [0xB3; 16]).unwrap();
            let id = seed_entity(&mut owner.container, &mut owner.entities);
            owner.sync().unwrap();
            id
        }
    };
    let source_bytes = fs::read(&source).unwrap();
    let mut container = crate::Container::open(&source).unwrap();
    let source_digest = history_digest(&mut container);
    let times = (1..=container.latest_version())
        .map(|global| container.transaction_time_ns(global))
        .collect::<Vec<_>>();
    drop(container);

    let manifest_digest = match kind {
        FileKind::Reliquary => prune_rel(&source, &output, id),
        FileKind::Phylactery => prune_phy(&source, &output, id),
    };
    assert_eq!(fs::read(&source).unwrap(), source_bytes);
    assert_pruned(&output, id, source_digest, manifest_digest, &times);
}

#[test]
fn raw_imported_entity_carrier_followed_by_envelope_prunes_with_source_proof_parity() {
    exercise(FileKind::Reliquary);
    exercise(FileKind::Phylactery);
}
