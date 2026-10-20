//! Scratch compact Relationship import evidence and replay-interval tests.
use crate::relationship_codec::{
    RelationshipTombstone, RelationshipVersion, encode_record, encode_tombstone, encode_version,
};
use crate::relationship_compact_codec::{encode_creation, encode_delta};
use crate::relationship_compact_contract::{RelationshipCreation, RelationshipDelta};
use crate::relationship_model::RelationshipRecord;
use crate::relationship_store::RelationshipReplayPublication;
use crate::{
    Cva, EntityId, EntityRef, MemoryId, MemoryRef, RelationshipId, RelationshipParticipant,
};
use std::fs;
use std::path::{Path, PathBuf};

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!(
                "reconciliation-relationship-compact-{}",
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

fn record(revision: u64, global: u64, local: u64, mutation: &str) -> RelationshipRecord {
    RelationshipRecord {
        id: RelationshipId([0x5C; 32]),
        revision,
        kind: "  observes  ".into(),
        participants: vec![
            RelationshipParticipant {
                entity: EntityRef {
                    owner_id: "  source-owner  ".into(),
                    entity_id: EntityId([0x61; 32]),
                },
                role: Some("  original role  ".into()),
            },
            RelationshipParticipant {
                entity: EntityRef {
                    owner_id: "second-owner".into(),
                    entity_id: EntityId([0x62; 32]),
                },
                role: None,
            },
        ],
        evidence: vec![MemoryRef {
            owner_id: "  source-memory-owner  ".into(),
            memory_id: MemoryId([0x63; 32]),
        }],
        summary: "  authored summary  ".into(),
        mutation_id: mutation.into(),
        created_at_ns: 100,
        updated_at_ns: 101,
        global_version: global,
        relationship_version: local,
    }
}

fn append_version(cva: &mut Cva, carrier: crate::ObjectRef, global: u64, local: u64) {
    cva.container
        .append(&encode_version(RelationshipVersion {
            global_version: global,
            relationship_version: local,
            record: carrier,
        }))
        .unwrap();
}

fn populated_owner(path: &Path, compact: bool) -> Cva {
    let mut cva = Cva::create(path).unwrap();
    for (expected, time) in [(1, 10_101), (2, 10_202)] {
        assert_eq!(
            cva.container
                .allocate_version_with_transaction_time(Some(time))
                .unwrap(),
            expected
        );
    }

    let first = record(1, 1, 1, "shared-create");
    let first_bytes = if compact {
        encode_creation(&RelationshipCreation::new(first.clone()).unwrap()).unwrap()
    } else {
        encode_record(&first).unwrap()
    };
    let first_carrier = cva.container.append(&first_bytes).unwrap();
    append_version(&mut cva, first_carrier, 1, 1);

    let mut second = record(2, 2, 2, "shared-revise");
    second.participants[0].role = Some("  revised role  ".into());
    second.evidence.push(MemoryRef {
        owner_id: "second-memory-owner".into(),
        memory_id: MemoryId([0x64; 32]),
    });
    second.summary = "  revised summary  ".into();
    second.updated_at_ns = 105;
    let second_bytes = if compact {
        encode_delta(&RelationshipDelta::between(&first, &second).unwrap()).unwrap()
    } else {
        encode_record(&second).unwrap()
    };
    let second_carrier = cva.container.append(&second_bytes).unwrap();
    append_version(&mut cva, second_carrier, 2, 2);
    drop(cva);
    Cva::open(path).unwrap()
}

#[test]
fn import_evidence_matches_for_legacy_and_compact_semantically_equal_histories() {
    let scratch = Scratch::new();
    let legacy = populated_owner(&scratch.file("legacy.rel"), false);
    let compact = populated_owner(&scratch.file("compact.rel"), true);

    for (global, local) in [(1, 1), (2, 2)] {
        let legacy_publication = legacy
            .relationships
            .retained_publication_at(global, local)
            .unwrap()
            .unwrap();
        let compact_publication = compact
            .relationships
            .retained_publication_at(global, local)
            .unwrap()
            .unwrap();
        assert!(matches!(
            legacy_publication,
            RelationshipReplayPublication::Record(_)
        ));
        assert!(matches!(
            compact_publication,
            RelationshipReplayPublication::Record(_)
        ));
        assert_eq!(
            legacy_publication.normalized_import_evidence().unwrap(),
            compact_publication.normalized_import_evidence().unwrap(),
            "normalized source evidence at ({global}, {local})"
        );
    }

    let legacy_second = legacy.relationships.records().last().unwrap();
    let mut relocated = legacy_second.clone();
    relocated.global_version = 20;
    relocated.relationship_version = 7;
    assert_eq!(
        RelationshipReplayPublication::Record(legacy_second.clone())
            .normalized_import_evidence()
            .unwrap(),
        RelationshipReplayPublication::Record(relocated)
            .normalized_import_evidence()
            .unwrap()
    );
}

fn append_interval_and_check(cva: &mut Cva, payloads: &[&[u8]], accepted: bool) {
    let boundary = cva.container.chunks().unwrap().len();
    for payload in payloads {
        cva.container.append(payload).unwrap();
    }
    let result = super::validate_replay_interval(cva, boundary);
    assert_eq!(result.is_ok(), accepted, "interval result: {result:?}");
}

#[test]
fn interval_classifier_accepts_valid_compact_relationship_creation_and_delta() {
    let scratch = Scratch::new();
    let mut cva = Cva::create(scratch.file("valid-interval.rel")).unwrap();
    let first = record(1, 1, 1, "interval-create");
    let creation = encode_creation(&RelationshipCreation::new(first.clone()).unwrap()).unwrap();
    let mut second = record(2, 2, 2, "interval-update");
    second.summary = "  updated  ".into();
    second.updated_at_ns = 102;
    let delta = encode_delta(&RelationshipDelta::between(&first, &second).unwrap()).unwrap();
    append_interval_and_check(&mut cva, &[&creation, &delta], true);
}

#[test]
fn interval_classifier_rejects_unknown_version_malformed_mask_and_terminal() {
    let first = record(1, 1, 1, "interval-create");
    let mut second = record(2, 2, 2, "interval-update");
    second.summary = "  updated  ".into();
    second.updated_at_ns = 102;
    let valid_delta = encode_delta(&RelationshipDelta::between(&first, &second).unwrap()).unwrap();
    let mutation_len = u32::from_le_bytes(valid_delta[72..76].try_into().unwrap()) as usize;
    let mask_offset = 76 + mutation_len;
    let mut invalid_mask = valid_delta.clone();
    let mask = u64::from_le_bytes(
        invalid_mask[mask_offset..mask_offset + 8]
            .try_into()
            .unwrap(),
    );
    invalid_mask[mask_offset..mask_offset + 8]
        .copy_from_slice(&(mask | (1_u64 << 63)).to_le_bytes());
    let truncated = &valid_delta[..8];
    let unknown_version = b"CVARNC99".as_slice();
    for (name, payload) in [
        ("unknown-version.rel", unknown_version),
        ("truncated.rel", truncated),
        ("invalid-mask.rel", invalid_mask.as_slice()),
    ] {
        let scratch = Scratch::new();
        let mut cva = Cva::create(scratch.file(name)).unwrap();
        append_interval_and_check(&mut cva, &[payload], false);
    }
}

#[test]
fn interval_classifier_accepts_relationship_terminal_publication() {
    let scratch = Scratch::new();
    let mut cva = Cva::create(scratch.file("terminal-interval.rel")).unwrap();
    let terminal = encode_tombstone(RelationshipTombstone {
        id: RelationshipId([0x5C; 32]),
        revision: 2,
    });
    append_interval_and_check(&mut cva, &[&terminal], true);
}
