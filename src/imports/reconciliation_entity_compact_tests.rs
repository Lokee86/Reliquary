use super::*;
use reliquary_perception_store::entity_codec::{
    DirectEntityTombstone, EntityTombstone, encode_direct_tombstone, encode_tombstone,
};
fn encode_creation(c: &EntityCreation) -> Result<Vec<u8>, crate::EntityError> {
    reliquary_perception_store::entity_compact_codec::publication::encode(
        &reliquary_perception_store::entity_compact_codec::publication::EntityPublication::Creation(c.clone()),
    )
}
fn encode_delta(d: &EntityDelta) -> Result<Vec<u8>, crate::EntityError> {
    reliquary_perception_store::entity_compact_codec::publication::encode(
        &reliquary_perception_store::entity_compact_codec::publication::EntityPublication::Delta(d.clone()),
    )
}
use reliquary_perception_store::entity_compact_contract::{EntityCreation, EntityDelta};
use reliquary_perception_store::entity_model::EntityRecord;
use crate::{Cva, EntityId};
use std::fs;
use std::path::PathBuf;

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!("reconcile-compact-entity-{}", uuid::Uuid::new_v4()));
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

fn record() -> EntityRecord {
    EntityRecord {
        id: EntityId([0x41; 32]),
        global_id: None,
        revision: 1,
        canonical_name: "Compact Entity".into(),
        aliases: vec!["Alias".into()],
        kind: "project".into(),
        summary: "Exact semantic summary".into(),
        mutation_id: "compact-entity-create".into(),
        created_at_ns: 10,
        updated_at_ns: 20,
        global_version: 1,
        entity_version: 1,
    }
}

fn append_and_check(cva: &mut Cva, payloads: &[&[u8]], accepted: bool) {
    let boundary = cva.container.chunks().unwrap().len();
    for payload in payloads {
        cva.container.append(payload).unwrap();
    }
    let result = super::validate_replay_interval(cva, boundary);
    assert_eq!(result.is_ok(), accepted, "classifier result: {result:?}");
}

#[test]
fn replay_interval_accepts_valid_compact_entity_creation_and_delta() {
    let scratch = Scratch::new();
    let path = scratch.file("accepted.rel");
    let mut cva = Cva::create(&path).unwrap();

    let first = record();
    let creation = encode_creation(&EntityCreation::new(first.clone()).unwrap()).unwrap();
    let mut next = first.clone();
    next.revision = 2;
    next.entity_version = 2;
    next.global_version = 2;
    next.mutation_id = "compact-entity-revise".into();
    next.summary = "Updated exact summary".into();
    let delta = encode_delta(&EntityDelta::between(&first, &next).unwrap()).unwrap();

    append_and_check(&mut cva, &[&creation, &delta], true);
}

#[test]
fn replay_interval_rejects_unknown_truncated_and_reserved_mask_compact_entity_frames() {
    let first = record();
    let mut next = first.clone();
    next.revision = 2;
    next.entity_version = 2;
    next.global_version = 2;
    next.mutation_id = "compact-entity-revise".into();
    next.summary = "Updated exact summary".into();
    let valid_delta = encode_delta(&EntityDelta::between(&first, &next).unwrap()).unwrap();

    let mut reserved_mask = valid_delta.clone();
    let mutation_len = u32::from_le_bytes(reserved_mask[64..68].try_into().unwrap()) as usize;
    let mask_offset = 68 + mutation_len;
    let mask = u64::from_le_bytes(
        reserved_mask[mask_offset..mask_offset + 8]
            .try_into()
            .unwrap(),
    );
    reserved_mask[mask_offset..mask_offset + 8]
        .copy_from_slice(&(mask | (1_u64 << 63)).to_le_bytes());

    for (name, payload) in [
        ("unknown-version.rel", b"CVAENC02".as_slice()),
        ("truncated.rel", &valid_delta[..8]),
        ("reserved-mask.rel", reserved_mask.as_slice()),
    ] {
        let scratch = Scratch::new();
        let path = scratch.file(name);
        let mut cva = Cva::create(&path).unwrap();
        append_and_check(&mut cva, &[payload], false);
    }
}

#[test]
fn replay_interval_still_rejects_direct_and_merge_entity_terminal_carriers() {
    for (name, terminal) in [
        (
            "direct-terminal.rel",
            encode_direct_tombstone(DirectEntityTombstone {
                id: EntityId([0x51; 32]),
                revision: 2,
            }),
        ),
        (
            "merge-terminal.rel",
            encode_tombstone(EntityTombstone {
                id: EntityId([0x61; 32]),
                revision: 2,
                replacement_id: EntityId([0x62; 32]),
            }),
        ),
    ] {
        let scratch = Scratch::new();
        let path = scratch.file(name);
        let mut cva = Cva::create(&path).unwrap();
        append_and_check(&mut cva, &[&terminal], false);
    }
}
