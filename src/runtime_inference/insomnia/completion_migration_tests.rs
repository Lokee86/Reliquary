use super::completion::{
    InsomniaCompletion, InsomniaCompletionBody, decode_completion, decode_embedded_version_range,
    encode_legacy_compact,
};
use crate::memory_codec::encode_record;
use crate::memory_model::MemoryRecord;
use crate::{
    CHRONOS_INFERENCE_CONTRACT_VERSION, EpisodeId, MemoryBodyId, MemoryEntityMention, MemoryId,
    MemoryRef, MemoryRoutingMetadata, MemorySourceRef, MemoryTemporalInference, MemoryTextField,
    TemporalIndicationKind, TemporalInference, TemporalInferenceResolution,
};
use crate::{Container, Cva};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

const GLOBAL_START: u64 = 5;
const GLOBAL_COUNT: u32 = 2;
const TRANSACTION_TIME_NS: i64 = 900;

fn push_string(out: &mut Vec<u8>, value: &str) {
    out.extend_from_slice(&(value.len() as u32).to_le_bytes());
    out.extend_from_slice(value.as_bytes());
}

fn push_memory_ids(out: &mut Vec<u8>, ids: &[MemoryId]) {
    out.extend_from_slice(&(ids.len() as u32).to_le_bytes());
    for id in ids {
        out.extend_from_slice(&id.0);
    }
}

fn fixture_record(index: u8, global_version: u64, memory_version: u64) -> MemoryRecord {
    let id = MemoryId([index; 32]);
    let body_id = MemoryBodyId([index + 20; 32]);
    MemoryRecord {
        id,
        revision: 1,
        body_id,
        category: "decision".into(),
        memory_type: "project".into(),
        authority_kind: "direct".into(),
        temporal_status: "current".into(),
        scope: "private".into(),
        lifecycle_state: "extracted".into(),
        archived: index % 2 == 0,
        superseded_by: None,
        parent_id: Some(MemoryId([99; 32])),
        source_node_id: Some(format!("assistant-{index}")),
        content_source_conversation_id: Some("migration-conversation".into()),
        content_source_node_id: Some(format!("user-{index}")),
        grounding_source_conversation_id: Some("grounding-conversation".into()),
        grounding_source_node_id: Some(format!("grounding-{index}")),
        source_episode_id: Some(EpisodeId([index + 40; 32])),
        source_time_ns: Some(700 + index as i64),
        source_ref: Some(MemorySourceRef {
            owner_id: "rel-migration-fixture".into(),
            principal_id: Some("phy-migration-principal".into()),
            source_episode_id: EpisodeId([index + 40; 32]),
            source_node_id: format!("source-{index}"),
            content_source_conversation_id: Some("content-conversation".into()),
            content_source_node_id: Some(format!("content-{index}")),
            grounding_source_conversation_id: Some("grounding-conversation".into()),
            grounding_source_node_id: Some(format!("grounding-{index}")),
        }),
        temporal_inference: Some(MemoryTemporalInference {
            body_id,
            source_time_ns: Some(700 + index as i64),
            inference: TemporalInference {
                model: "chronos-migration-test".into(),
                contract_version: CHRONOS_INFERENCE_CONTRACT_VERSION.into(),
                resolutions: vec![TemporalInferenceResolution {
                    start_byte: 0,
                    end_byte: 8,
                    kind: TemporalIndicationKind::Recurrence,
                    evidence: "biweekly".into(),
                    canonical_expression: "every two weeks".into(),
                }],
            },
        }),
        mutation_id: format!("migration-record-{index}"),
        created_at_ns: 100 + index as i64,
        updated_at_ns: 200 + index as i64,
        global_version,
        memory_version,
    }
}

fn fixture_completion(version: u8) -> InsomniaCompletion {
    let globals = if version == 1 {
        [GLOBAL_START, GLOBAL_START + 6]
    } else {
        [GLOBAL_START, GLOBAL_START + 1]
    };
    let records = vec![
        fixture_record(1, globals[0], 8),
        fixture_record(2, globals[1], 13),
    ];
    let memory_ids = records.iter().map(|record| record.id).collect::<Vec<_>>();
    let bodies = if version >= 2 {
        records
            .iter()
            .map(|record| InsomniaCompletionBody {
                id: record.body_id,
                bytes: format!("body-for-{}", record.id.0[0]).into_bytes(),
            })
            .collect()
    } else {
        Vec::new()
    };
    let external_memory_refs = if version >= 3 {
        vec![MemoryRef {
            owner_id: "phy-external-migration-fixture".into(),
            memory_id: MemoryId([71; 32]),
        }]
    } else {
        Vec::new()
    };
    let routing_metadata = if version >= 5 {
        records
            .iter()
            .map(|record| MemoryRoutingMetadata {
                memory_id: record.id,
                body_id: record.body_id,
                entity_mentions: vec![MemoryEntityMention {
                    field: MemoryTextField::Title,
                    start_byte: 0,
                    end_byte: 8,
                    text: "Decision".into(),
                }],
            })
            .collect()
    } else {
        Vec::new()
    };
    let has_birth_proof = version == 6;
    InsomniaCompletion {
        episode_id: EpisodeId([17; 32]),
        attempt: 3,
        started_at_ns: 100,
        completed_at_ns: 110,
        transaction_time_ns: if version >= 4 { TRANSACTION_TIME_NS } else { 0 },
        extractor_model: "migration-model".into(),
        extractor_version: "migration-version".into(),
        rejected_count: 2,
        memory_ids: memory_ids.clone(),
        external_memory_refs,
        global_version_start: GLOBAL_START,
        bodies,
        records,
        routing_metadata,
        freshness_birth_turn: has_birth_proof.then_some(77),
        freshness_policy_version: has_birth_proof
            .then_some(crate::freshness::FRESHNESS_POLICY_VERSION as u64),
        freshness_birth_memory_ids: if has_birth_proof {
            memory_ids
        } else {
            Vec::new()
        },
    }
}

fn legacy_payload(version: u8, completion: &InsomniaCompletion) -> Vec<u8> {
    let mut out = format!("CVAINSC{version}").into_bytes();
    out.extend_from_slice(&completion.episode_id.0);
    out.extend_from_slice(&completion.attempt.to_le_bytes());
    out.extend_from_slice(&completion.started_at_ns.to_le_bytes());
    out.extend_from_slice(&completion.completed_at_ns.to_le_bytes());
    out.extend_from_slice(&completion.rejected_count.to_le_bytes());
    if version >= 2 {
        out.extend_from_slice(&completion.global_version_start.to_le_bytes());
        out.extend_from_slice(&(completion.records.len() as u32).to_le_bytes());
    }
    if version >= 4 {
        out.extend_from_slice(&completion.transaction_time_ns.to_le_bytes());
    }
    push_string(&mut out, &completion.extractor_model);
    push_string(&mut out, &completion.extractor_version);
    push_memory_ids(&mut out, &completion.memory_ids);
    if version >= 3 {
        out.extend_from_slice(&(completion.external_memory_refs.len() as u32).to_le_bytes());
        for memory_ref in &completion.external_memory_refs {
            push_string(&mut out, &memory_ref.owner_id);
            out.extend_from_slice(&memory_ref.memory_id.0);
        }
    }
    if version >= 2 {
        out.extend_from_slice(&(completion.bodies.len() as u32).to_le_bytes());
        for body in &completion.bodies {
            out.extend_from_slice(&body.id.0);
            out.extend_from_slice(&(body.bytes.len() as u32).to_le_bytes());
            out.extend_from_slice(&body.bytes);
        }
    }
    out.extend_from_slice(&(completion.records.len() as u32).to_le_bytes());
    for record in &completion.records {
        out.extend_from_slice(&record.global_version.to_le_bytes());
        out.extend_from_slice(&record.memory_version.to_le_bytes());
        let encoded = encode_record(record).unwrap();
        out.extend_from_slice(&(encoded.len() as u32).to_le_bytes());
        out.extend_from_slice(&encoded);
    }
    if version >= 5 {
        out.extend_from_slice(&(completion.routing_metadata.len() as u32).to_le_bytes());
        for routing in &completion.routing_metadata {
            let encoded = crate::memory_routing_codec::encode(routing).unwrap();
            out.extend_from_slice(&(encoded.len() as u32).to_le_bytes());
            out.extend_from_slice(&encoded);
        }
    }
    if version == 6 {
        out.extend_from_slice(&completion.freshness_birth_turn.unwrap().to_le_bytes());
        out.extend_from_slice(&completion.freshness_policy_version.unwrap().to_le_bytes());
        push_memory_ids(&mut out, &completion.freshness_birth_memory_ids);
    }
    out
}

fn skip_string(bytes: &[u8], cursor: &mut usize) {
    let len = u32::from_le_bytes(bytes[*cursor..*cursor + 4].try_into().unwrap()) as usize;
    *cursor += 4 + len;
}

fn compact_record_section(bytes: &[u8], variant: u8) -> (usize, usize) {
    // CVAINSC8 inserts the original variant byte after the common magic.
    let mut cursor = 9 + match variant {
        1 => 64 - 8,
        2 | 3 => 76 - 8,
        _ => 84 - 8,
    };
    skip_string(bytes, &mut cursor);
    skip_string(bytes, &mut cursor);
    let memory_count = u32::from_le_bytes(bytes[cursor..cursor + 4].try_into().unwrap()) as usize;
    cursor += 4 + memory_count * 32;
    if variant >= 3 {
        let ref_count = u32::from_le_bytes(bytes[cursor..cursor + 4].try_into().unwrap()) as usize;
        cursor += 4;
        for _ in 0..ref_count {
            skip_string(bytes, &mut cursor);
            cursor += 32;
        }
    }
    if variant >= 2 {
        let body_count = u32::from_le_bytes(bytes[cursor..cursor + 4].try_into().unwrap()) as usize;
        cursor += 4;
        for _ in 0..body_count {
            cursor += 32;
            let len = u32::from_le_bytes(bytes[cursor..cursor + 4].try_into().unwrap()) as usize;
            cursor += 4 + len;
        }
    }
    let count = u32::from_le_bytes(bytes[cursor..cursor + 4].try_into().unwrap()) as usize;
    (cursor + 4, count)
}

fn assert_completion_fields_equal(left: &InsomniaCompletion, right: &InsomniaCompletion) {
    assert_eq!(right.episode_id, left.episode_id);
    assert_eq!(right.attempt, left.attempt);
    assert_eq!(right.started_at_ns, left.started_at_ns);
    assert_eq!(right.completed_at_ns, left.completed_at_ns);
    assert_eq!(right.transaction_time_ns, left.transaction_time_ns);
    assert_eq!(right.extractor_model, left.extractor_model);
    assert_eq!(right.extractor_version, left.extractor_version);
    assert_eq!(right.rejected_count, left.rejected_count);
    assert_eq!(right.memory_ids, left.memory_ids);
    assert_eq!(right.external_memory_refs, left.external_memory_refs);
    assert_eq!(right.global_version_start, left.global_version_start);
    assert_eq!(right.bodies.len(), left.bodies.len());
    for (actual, expected) in right.bodies.iter().zip(&left.bodies) {
        assert_eq!(actual.id, expected.id);
        assert_eq!(actual.bytes, expected.bytes);
    }
    assert_eq!(right.records, left.records);
    assert_eq!(right.routing_metadata, left.routing_metadata);
    assert_eq!(right.freshness_birth_turn, left.freshness_birth_turn);
    assert_eq!(
        right.freshness_policy_version,
        left.freshness_policy_version
    );
    assert_eq!(
        right.freshness_birth_memory_ids,
        left.freshness_birth_memory_ids
    );
}

#[test]
fn legacy_completion_variants_migrate_to_compact_records_without_losing_envelope_data() {
    for version in 1..=6 {
        let fixture = fixture_completion(version);
        let legacy = legacy_payload(version, &fixture);
        let decoded_legacy = decode_completion(&legacy)
            .unwrap_or_else(|error| panic!("legacy CVAINSC{version} failed: {error}"))
            .unwrap();
        let old_range = decode_embedded_version_range(&legacy).unwrap();
        let migrated = encode_legacy_compact(&legacy, &decoded_legacy)
            .unwrap_or_else(|error| panic!("CVAINSC{version} migration failed: {error}"));
        assert_eq!(&migrated[..8], b"CVAINSC8");
        assert_eq!(migrated[8], version);

        let decoded_migrated = decode_completion(&migrated).unwrap().unwrap();
        assert_completion_fields_equal(&decoded_legacy, &decoded_migrated);
        assert_eq!(
            decode_embedded_version_range(&migrated).unwrap(),
            old_range,
            "CVAINSC{version} embedded range changed during migration"
        );
        let expected_range = match version {
            1 => None,
            2 | 3 => Some((GLOBAL_START, GLOBAL_COUNT, None)),
            _ => Some((GLOBAL_START, GLOBAL_COUNT, Some(TRANSACTION_TIME_NS))),
        };
        assert_eq!(old_range, expected_range, "CVAINSC{version} source range");

        if version == 1 {
            assert_eq!(decoded_migrated.global_version_start, GLOBAL_START);
            assert_eq!(decoded_migrated.records[0].global_version, GLOBAL_START);
            assert_eq!(decoded_migrated.records[1].global_version, GLOBAL_START + 6);
            assert_eq!(decode_embedded_version_range(&migrated).unwrap(), None);
        }

        let (mut cursor, count) = compact_record_section(&migrated, version);
        assert_eq!(count, decoded_legacy.records.len());
        for expected in &decoded_legacy.records {
            let len = u32::from_le_bytes(migrated[cursor..cursor + 4].try_into().unwrap()) as usize;
            cursor += 4;
            let encoded = &migrated[cursor..cursor + len];
            cursor += len;
            assert_eq!(&encoded[..8], b"CVAMC001");
            let decoded = crate::memory_compact_codec::decode_creation(encoded).unwrap();
            assert_eq!(decoded.record, *expected);
        }
        assert!(!migrated.windows(8).any(|window| window == b"CVAMEMR8"));
    }
}

#[test]
fn compact_legacy_migration_rejects_unknown_original_variants_and_truncated_records() {
    let fixture = fixture_completion(6);
    let valid = legacy_payload(6, &fixture);
    let decoded = decode_completion(&valid).unwrap().unwrap();

    let mut unknown_variant = valid.clone();
    unknown_variant[7] = b'9';
    assert!(encode_legacy_compact(&unknown_variant, &decoded).is_err());

    let truncated = &valid[..valid.len() - 1];
    assert!(decode_completion(truncated).is_err());
    assert!(encode_legacy_compact(truncated, &decoded).is_err());
}

struct ScratchDirectory(PathBuf);

impl ScratchDirectory {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!(
                "insomnia-completion-migration-{}",
                uuid::Uuid::new_v4()
            ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn file(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for ScratchDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn write_v1_header(path: &Path) {
    let mut bytes = fs::read(path).unwrap();
    bytes[8..10].copy_from_slice(&1_u16.to_le_bytes());
    bytes[10..12].copy_from_slice(&0_u16.to_le_bytes());
    fs::write(path, bytes).unwrap();
}

pub(crate) fn copy_with_legacy_v6_completion(cva: &mut Cva, path: &Path) {
    cva.sync().unwrap();
    let identity = cva.container.identity().unwrap();
    let owner_uuid = cva.container.owner_uuid().unwrap();
    let mut copied = Container::create_with_identity_and_uuid(path, identity, owner_uuid).unwrap();
    let mut relocated = BTreeMap::new();
    let records = cva.memories.records().to_vec();
    let mut completion_count = 0;
    cva.container
        .visit_payloads::<crate::ContainerError>(|object, payload| {
            let transformed = if payload.starts_with(b"CVAINSC7") {
                completion_count += 1;
                let decoded = decode_completion(payload).unwrap().unwrap();
                legacy_payload(6, &decoded)
            } else if payload.starts_with(b"CVAMC001") || payload.starts_with(b"CVAMD001") {
                let coordinates = if payload.starts_with(b"CVAMC001") {
                    let record = crate::memory_compact_codec::decode_creation(payload)
                        .unwrap()
                        .record;
                    (record.global_version, record.memory_version)
                } else {
                    let delta = crate::memory_compact_codec::decode_delta(payload).unwrap();
                    (delta.global_version, delta.memory_version)
                };
                let record = records
                    .iter()
                    .find(|record| (record.global_version, record.memory_version) == coordinates)
                    .expect("fixture compact publication has accepted owner record");
                crate::memory_codec::encode_record(record).unwrap()
            } else {
                crate::storage_reclamation::relocate_payload(payload, &relocated).unwrap()
            };
            let new_object = copied.append(&transformed)?;
            relocated.insert(object, new_object);
            Ok(())
        })
        .unwrap();
    assert_eq!(completion_count, 1, "expected one grouped completion");
    copied.sync().unwrap();
    drop(copied);
    write_v1_header(path);
}

fn completion_payload_with_magic(path: &Path, magic: &[u8; 8]) -> Vec<u8> {
    let mut container = Container::open_read_only_for_scan(path).unwrap();
    let mut found = Vec::new();
    container
        .visit_payloads::<crate::ContainerError>(|_, payload| {
            if payload.starts_with(magic) {
                found.push(payload.to_vec());
            }
            Ok(())
        })
        .unwrap();
    assert_eq!(
        found.len(),
        1,
        "expected one {} payload",
        String::from_utf8_lossy(magic)
    );
    found.remove(0)
}

#[test]
fn staged_owner_migration_preserves_grouped_v6_completion_and_owner_history() {
    let scratch = ScratchDirectory::new();
    let current_path = scratch.file("current.rel");
    let source_path = scratch.file("legacy.rel");
    let destination_path = scratch.file("staged-output.rel");
    let (mut cva, _episode) = super::completion_compact_tests::queued_owner(&current_path);
    super::completion_tests::process(&mut cva, 2, 110);
    let current_payload = super::completion_compact_tests::completion_payload(&mut cva);
    let current_completion = decode_completion(&current_payload).unwrap().unwrap();
    assert_eq!(current_completion.records.len(), 2);

    copy_with_legacy_v6_completion(&mut cva, &source_path);
    drop(cva);
    let source_bytes = fs::read(&source_path).unwrap();
    let mut source_container = Container::open_v1_read_only(&source_path).unwrap();
    let source_uuid = source_container.owner_uuid().unwrap();
    let source_identity = source_container.identity().unwrap();
    drop(source_container);
    let legacy_bytes = completion_payload_with_magic(&source_path, b"CVAINSC6");
    let legacy_completion = decode_completion(&legacy_bytes).unwrap().unwrap();
    assert_completion_fields_equal(&current_completion, &legacy_completion);

    let plan = crate::storage_upgrade::plan_storage_upgrade(
        crate::storage_upgrade::StorageUpgradeRequest {
            source: source_path.clone(),
            destination: destination_path.clone(),
        },
    )
    .unwrap();
    let staged = crate::storage_upgrade::stage_memory_storage_upgrade(&plan).unwrap();
    let migrated_bytes = completion_payload_with_magic(staged.test_path(), b"CVAINSC8");
    let migrated_completion = decode_completion(&migrated_bytes).unwrap().unwrap();
    assert_completion_fields_equal(&legacy_completion, &migrated_completion);

    let mut source = Cva::open_v1_read_only(&source_path).unwrap();
    let mut migrated = Cva::open(staged.test_path()).unwrap();
    assert_eq!(source.owner_uuid(), Some(source_uuid));
    assert_eq!(migrated.owner_uuid(), Some(source_uuid));
    assert_eq!(source.container.identity(), Some(source_identity));
    assert_eq!(migrated.container.identity(), Some(source_identity));
    assert_eq!(
        source.latest_global_version(),
        migrated.latest_global_version()
    );
    assert_eq!(source.memory_version(), migrated.memory_version());
    assert_eq!(migrated.memories.records(), source.memories.records());
    assert_eq!(
        migrated.memories.records(),
        migrated_completion.records.as_slice()
    );
    for record in &legacy_completion.records {
        assert_eq!(
            migrated.memory(record.id).unwrap(),
            source.memory(record.id).unwrap()
        );
    }
    for global_version in 0..=source.latest_global_version() {
        assert_eq!(
            migrated.transaction_time_ns(global_version),
            source.transaction_time_ns(global_version),
            "transaction time at global version {global_version}"
        );
    }
    assert_eq!(fs::read(&source_path).unwrap(), source_bytes);
    drop(migrated);
    drop(source);
    drop(staged);
    assert!(!destination_path.exists());
}
