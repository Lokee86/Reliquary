//! Historical Entity parity between legacy carrier/link pairs and envelopes.
use crate::container::publication_envelope::{PublicationClock, PublicationEnvelope};
use crate::entity_codec::{
    DirectEntityTombstone, EntityTombstone, EntityVersion, encode_direct_tombstone,
    encode_tombstone, encode_version,
};
use crate::entity_compact_codec::publication::{self, EntityPublication};
use crate::entity_compact_codec::{encode_creation, encode_delta};
use crate::entity_compact_contract::{EntityCreation, EntityDelta};
use crate::entity_model::EntityRecord;
use crate::entity_store::EntityStore;
use crate::{Container, Cva, EntityGlobalId, EntityId, MemoryDraft, Phylactery};
use std::fs;
use std::path::PathBuf;

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!(
                "entity-publication-history-{}",
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

#[derive(Clone, Copy)]
enum WireFormat {
    LegacyCarrierLink,
    Envelope,
}

fn record(
    id: u8,
    name: &str,
    mutation_id: &str,
    revision: u64,
    global: u64,
    local: u64,
) -> EntityRecord {
    EntityRecord {
        id: EntityId([id; 32]),
        global_id: Some(EntityGlobalId([id.wrapping_add(90); 16])),
        revision,
        canonical_name: name.into(),
        aliases: vec![format!("{name} alias")],
        kind: "project".into(),
        summary: format!("summary {name} r{revision}"),
        mutation_id: mutation_id.into(),
        created_at_ns: 5,
        updated_at_ns: 5 + revision as i64,
        global_version: global,
        entity_version: local,
    }
}

fn reserve(container: &mut Container, expected: u64, time_ns: i64) {
    container.set_next_transaction_time_override(Some(time_ns));
    assert_eq!(container.allocate_version().unwrap(), expected);
}

fn append_entity_publication(
    container: &mut Container,
    format: WireFormat,
    global: u64,
    time_ns: i64,
    value: EntityPublication,
) {
    match format {
        WireFormat::Envelope => {
            let body = publication::encode(&value).unwrap();
            container.set_next_transaction_time_override(Some(time_ns));
            assert_eq!(
                container.append_publication_envelope(&body).unwrap().0,
                global
            );
        }
        WireFormat::LegacyCarrierLink => {
            let (body, local) = match &value {
                EntityPublication::Creation(creation) => (
                    encode_creation(creation).unwrap(),
                    creation.record.entity_version,
                ),
                EntityPublication::Delta(delta) => {
                    (encode_delta(delta).unwrap(), delta.entity_version)
                }
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
            };
            let carrier = container.append(&body).unwrap();
            container.set_next_transaction_time_override(Some(time_ns));
            assert_eq!(container.allocate_version().unwrap(), global);
            container
                .append(&encode_version(EntityVersion {
                    global_version: global,
                    entity_version: local,
                    record: carrier,
                }))
                .unwrap();
        }
    }
}

fn memory_draft() -> MemoryDraft {
    MemoryDraft {
        category: "fact".into(),
        memory_type: "project".into(),
        authority_kind: "direct".into(),
        temporal_status: "current".into(),
        title: "Interleaved Memory".into(),
        content: "This is the same accepted Memory between Entity publications.".into(),
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
        mutation_id: "interleaved-memory".into(),
        created_at_ns: 5,
        updated_at_ns: 5,
    }
}

trait MemoryPublisher {
    fn container_mut(&mut self) -> &mut Container;
    fn publish_test_memory(&mut self) -> Result<(), String>;
}

impl MemoryPublisher for Cva {
    fn container_mut(&mut self) -> &mut Container {
        &mut self.container
    }
    fn publish_test_memory(&mut self) -> Result<(), String> {
        self.container.set_next_transaction_time_override(Some(300));
        self.publish_memory(None, 0, memory_draft())
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
}

impl MemoryPublisher for Phylactery {
    fn container_mut(&mut self) -> &mut Container {
        &mut self.container
    }
    fn publish_test_memory(&mut self) -> Result<(), String> {
        self.container.set_next_transaction_time_override(Some(300));
        self.publish_memory(None, 0, memory_draft())
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
}

fn write_history(owner: &mut impl MemoryPublisher, format: WireFormat) -> (EntityId, EntityId) {
    let container = owner.container_mut();
    reserve(container, 1, 100);
    reserve(container, 2, 200);

    // Both histories bind local sequence 1 to reserved global 2, then local
    // sequence 2 to reserved global 1. Physical marker order stays semantic.
    let first = record(11, "First", "first-create", 1, 2, 1);
    let mut revised = first.clone();
    revised.revision = 2;
    revised.global_version = 1;
    revised.entity_version = 2;
    revised.mutation_id = "first-revision".into();
    revised.summary = "first revised".into();
    revised.updated_at_ns = 8;

    match format {
        WireFormat::Envelope => {
            for (global, value) in [
                (
                    2,
                    EntityPublication::Creation(EntityCreation::new(first.clone()).unwrap()),
                ),
                (
                    1,
                    EntityPublication::Delta(EntityDelta::between(&first, &revised).unwrap()),
                ),
            ] {
                let body = publication::encode(&value).unwrap();
                let envelope =
                    crate::container::publication_envelope::encode(PublicationEnvelope {
                        global_version: global,
                        clock: PublicationClock::BindReserved,
                        body: &body,
                    })
                    .unwrap();
                container.append(&envelope).unwrap();
            }
        }
        WireFormat::LegacyCarrierLink => {
            let first_ref = container
                .append(&encode_creation(&EntityCreation::new(first.clone()).unwrap()).unwrap())
                .unwrap();
            container
                .append(&encode_version(EntityVersion {
                    global_version: 2,
                    entity_version: 1,
                    record: first_ref,
                }))
                .unwrap();
            let second_ref = container
                .append(&encode_delta(&EntityDelta::between(&first, &revised).unwrap()).unwrap())
                .unwrap();
            container
                .append(&encode_version(EntityVersion {
                    global_version: 1,
                    entity_version: 2,
                    record: second_ref,
                }))
                .unwrap();
        }
    }

    // Exercise an independent ordinary Memory write between owner-local Entity
    // events, with identical global coordinate and transaction time.
    owner.publish_test_memory().unwrap();
    assert_eq!(owner.container_mut().next_version - 1, 3);

    let third = record(22, "Merge target", "target-create", 1, 4, 3);
    append_entity_publication(
        owner.container_mut(),
        format,
        4,
        400,
        EntityPublication::Creation(EntityCreation::new(third.clone()).unwrap()),
    );
    append_entity_publication(
        owner.container_mut(),
        format,
        5,
        500,
        EntityPublication::MergeTombstone {
            id: first.id,
            revision: 3,
            entity_version: 4,
            replacement_id: third.id,
        },
    );
    append_entity_publication(
        owner.container_mut(),
        format,
        6,
        600,
        EntityPublication::DirectTombstone {
            id: third.id,
            revision: 2,
            entity_version: 5,
        },
    );
    (first.id, third.id)
}

fn assert_entity_cuts_equal(
    left_container: &Container,
    left: &EntityStore,
    right_container: &Container,
    right: &EntityStore,
) {
    assert_eq!(left.entity_version(), right.entity_version());
    for cut in 0..=left.entity_version() {
        let left_cut =
            crate::historical_entity_fingerprint::entity_cut(left_container, left, cut).unwrap();
        let right_cut =
            crate::historical_entity_fingerprint::entity_cut(right_container, right, cut).unwrap();
        assert_eq!(
            left_cut.fingerprint, right_cut.fingerprint,
            "Entity cut {cut}"
        );
        assert_eq!(
            left_cut.current, right_cut.current,
            "current set at Entity cut {cut}"
        );
        assert_eq!(
            left_cut.replacements, right_cut.replacements,
            "replacement map at Entity cut {cut}"
        );
    }
}

fn assert_global_times_equal(left: &Container, right: &Container) {
    assert_eq!(left.latest_version(), right.latest_version());
    for global in 1..=left.latest_version() {
        assert_eq!(
            left.transaction_time_ns(global),
            right.transaction_time_ns(global),
            "global {global}"
        );
    }
}

#[test]
fn rel_legacy_and_envelope_histories_match_every_entity_and_global_cut() {
    let scratch = Scratch::new();
    let owner_uuid = [0xA1; 16];
    let mut legacy = Cva::create_rel_with_uuid(
        scratch.file("legacy.rel"),
        owner_uuid,
        Some("Project".into()),
    )
    .unwrap();
    let mut envelope = Cva::create_rel_with_uuid(
        scratch.file("envelope.rel"),
        owner_uuid,
        Some("Project".into()),
    )
    .unwrap();
    let legacy_ids = write_history(&mut legacy, WireFormat::LegacyCarrierLink);
    let envelope_ids = write_history(&mut envelope, WireFormat::Envelope);
    assert_eq!(legacy_ids, envelope_ids);
    legacy.sync().unwrap();
    envelope.sync().unwrap();
    drop(legacy);
    drop(envelope);

    let legacy = Cva::open(scratch.file("legacy.rel")).unwrap();
    let envelope = Cva::open(scratch.file("envelope.rel")).unwrap();
    assert_entity_cuts_equal(
        &legacy.container,
        &legacy.entities,
        &envelope.container,
        &envelope.entities,
    );
    assert_global_times_equal(&legacy.container, &envelope.container);
    let (merged_id, direct_id) = legacy_ids;
    assert_eq!(
        legacy.entity_retention_descriptor(merged_id).unwrap(),
        envelope.entity_retention_descriptor(merged_id).unwrap()
    );
    assert_eq!(
        legacy.entity_retention_descriptor(direct_id).unwrap(),
        envelope.entity_retention_descriptor(direct_id).unwrap()
    );
    for global in 0..=legacy.latest_global_version() {
        assert_eq!(
            crate::historical_global_cuts::reliquary_global_cuts(&legacy, global).unwrap(),
            crate::historical_global_cuts::reliquary_global_cuts(&envelope, global).unwrap(),
            "REL global cut {global}",
        );
    }
}

#[test]
fn phy_legacy_and_envelope_histories_match_every_entity_and_global_cut() {
    let scratch = Scratch::new();
    let owner_uuid = [0xB2; 16];
    let mut legacy = Phylactery::create_with_uuid(scratch.file("legacy.phy"), owner_uuid).unwrap();
    let mut envelope =
        Phylactery::create_with_uuid(scratch.file("envelope.phy"), owner_uuid).unwrap();
    let legacy_ids = write_history(&mut legacy, WireFormat::LegacyCarrierLink);
    let envelope_ids = write_history(&mut envelope, WireFormat::Envelope);
    assert_eq!(legacy_ids, envelope_ids);
    legacy.sync().unwrap();
    envelope.sync().unwrap();
    drop(legacy);
    drop(envelope);

    let legacy = Phylactery::open(scratch.file("legacy.phy")).unwrap();
    let envelope = Phylactery::open(scratch.file("envelope.phy")).unwrap();
    assert_entity_cuts_equal(
        &legacy.container,
        &legacy.entities,
        &envelope.container,
        &envelope.entities,
    );
    assert_global_times_equal(&legacy.container, &envelope.container);
    let (merged_id, direct_id) = legacy_ids;
    assert_eq!(
        legacy.entity_retention_descriptor(merged_id).unwrap(),
        envelope.entity_retention_descriptor(merged_id).unwrap()
    );
    assert_eq!(
        legacy.entity_retention_descriptor(direct_id).unwrap(),
        envelope.entity_retention_descriptor(direct_id).unwrap()
    );
    for global in 0..=legacy.latest_global_version() {
        assert_eq!(
            crate::historical_global_cuts::phylactery_global_cuts(&legacy, global).unwrap(),
            crate::historical_global_cuts::phylactery_global_cuts(&envelope, global).unwrap(),
            "PHY global cut {global}",
        );
    }
}
