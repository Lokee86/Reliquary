use super::publication::{self, Publication};
use super::{ExternalChunkRef, ExternalMemoryEvidence, ExternalMessageRef};
use crate::insomnia::completion::InsomniaCompletionBody;
use crate::memory_compact_codec::{decode_creation, encode_delta};
use crate::memory_compact_contract::MemoryDelta;
use crate::memory_model::{MemoryRecord, memory_body_bytes, memory_body_id};
use crate::{Cva, MemoryId};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

struct ScratchDirectory(PathBuf);

impl ScratchDirectory {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!(
                "external-compact-publication-{}",
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

fn sample_record() -> (MemoryRecord, Vec<u8>) {
    let title = "External compact title";
    let content = "Derived semantic memory text.";
    let body = memory_body_bytes(title, content);
    let body_id = memory_body_id(title, content);
    (
        MemoryRecord {
            id: MemoryId([11; 32]),
            revision: 1,
            body_id,
            category: "knowledge".into(),
            memory_type: "fact".into(),
            authority_kind: "direct".into(),
            temporal_status: "current".into(),
            scope: "project".into(),
            lifecycle_state: "extracted".into(),
            archived: false,
            superseded_by: None,
            parent_id: None,
            source_node_id: None,
            content_source_conversation_id: None,
            content_source_node_id: None,
            grounding_source_conversation_id: None,
            grounding_source_node_id: None,
            source_episode_id: None,
            source_time_ns: Some(42),
            source_ref: None,
            temporal_inference: None,
            mutation_id: "external-compact-test".into(),
            created_at_ns: 42,
            updated_at_ns: 43,
            global_version: 1,
            memory_version: 1,
        },
        body,
    )
}

fn evidence(window: [u8; 32]) -> ExternalMemoryEvidence {
    let message = ExternalMessageRef {
        source_namespace: "host:compact-test".into(),
        conversation_id: "conversation".into(),
        message_id: "message".into(),
        content_hash: [19; 32],
    };
    ExternalMemoryEvidence {
        source_owner_id: "rel-source".into(),
        principal_id: "alice".into(),
        processing_window: window,
        authority: ExternalChunkRef {
            message,
            start: 0,
            end: 1,
        },
        content: None,
        grounding: None,
    }
}

fn sample_publication() -> Publication {
    let window = [17; 32];
    let (record, body_bytes) = sample_record();
    Publication {
        window,
        global_start: record.global_version,
        transaction_ns: 900,
        birth_turn: Some(7),
        records: vec![record.clone()],
        bodies: vec![InsomniaCompletionBody {
            id: record.body_id,
            bytes: body_bytes,
        }],
        bindings: vec![publication::Binding {
            memory: record.id.0,
            body: record.body_id.0,
            evidence: evidence(window),
        }],
        routing: Vec::new(),
    }
}

fn json_payload(frame: &[u8]) -> Value {
    let end = frame.len() - 32;
    serde_json::from_slice(&frame[36..end]).unwrap()
}

fn publication_frame(
    magic: &[u8; 8],
    start: u64,
    count: u32,
    transaction_ns: i64,
    json: &[u8],
) -> Vec<u8> {
    let mut frame = magic.to_vec();
    frame.extend_from_slice(&(json.len() as u64).to_le_bytes());
    frame.extend_from_slice(&start.to_le_bytes());
    frame.extend_from_slice(&count.to_le_bytes());
    frame.extend_from_slice(&transaction_ns.to_le_bytes());
    frame.extend_from_slice(json);
    let digest = Sha256::digest(&frame);
    frame.extend_from_slice(&digest);
    frame
}

fn legacy_publication_frame(publication: &Publication) -> Vec<u8> {
    let records: Vec<Value> = publication
        .records
        .iter()
        .map(|record| {
            json!({
                "global": record.global_version,
                "memory": record.memory_version,
                "bytes": crate::memory_codec::encode_record(record).unwrap(),
            })
        })
        .collect();
    let bodies: Vec<Value> = publication
        .bodies
        .iter()
        .map(|body| json!({ "id": body.id.0, "bytes": body.bytes }))
        .collect();
    let routing: Vec<Vec<u8>> = publication
        .routing
        .iter()
        .map(|metadata| crate::memory_routing_codec::encode(metadata).unwrap())
        .collect();
    let wire = json!({
        "window": publication.window,
        "global_start": publication.global_start,
        "transaction_ns": publication.transaction_ns,
        "birth_turn": publication.birth_turn,
        "records": records,
        "bodies": bodies,
        "bindings": publication.bindings,
        "routing": routing,
    });
    let json_bytes = serde_json::to_vec(&wire).unwrap();
    publication_frame(
        b"CVAXMEM1",
        publication.global_start,
        publication.records.len() as u32,
        publication.transaction_ns,
        &json_bytes,
    )
}

fn reframe_compact(frame: &[u8], mut json: Value) -> Vec<u8> {
    let record_bytes = json["records"][0]["bytes"]
        .as_array_mut()
        .expect("compact wire record bytes array");
    let bytes: Vec<u8> = record_bytes
        .iter()
        .map(|byte| byte.as_u64().unwrap() as u8)
        .collect();
    assert_eq!(record_bytes.len(), bytes.len());
    let json_bytes = serde_json::to_vec(&json).unwrap();
    publication_frame(
        b"CVAXMEM2",
        u64::from_le_bytes(frame[16..24].try_into().unwrap()),
        u32::from_le_bytes(frame[24..28].try_into().unwrap()),
        i64::from_le_bytes(frame[28..36].try_into().unwrap()),
        &json_bytes,
    )
}

#[test]
fn cvaxmem2_uses_compact_creation_bytes_without_redundant_record_coordinates() {
    let publication = sample_publication();
    let bytes = publication::encode(&publication).unwrap();
    assert_eq!(&bytes[..8], b"CVAXMEM2");
    assert_eq!(
        publication::decode_embedded_version_range(&bytes).unwrap(),
        Some((1, 1, 900))
    );

    let wire = json_payload(&bytes);
    let record = wire["records"][0].as_object().unwrap();
    assert_eq!(record.len(), 1);
    assert!(record.contains_key("bytes"));
    let encoded: Vec<u8> = record["bytes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|byte| byte.as_u64().unwrap() as u8)
        .collect();
    assert!(encoded.starts_with(b"CVAMC001"));
    assert!(decode_creation(&encoded).is_ok());

    let decoded = publication::decode(&bytes).unwrap().unwrap();
    assert_eq!(decoded.records, publication.records);
    assert_eq!(decoded.records[0].global_version, 1);
    assert_eq!(decoded.records[0].memory_version, 1);
    assert_eq!(
        serde_json::to_value(&decoded.bindings).unwrap(),
        serde_json::to_value(&publication.bindings).unwrap()
    );
    assert_eq!(decoded.bodies[0].id, publication.bodies[0].id);
}

#[test]
fn cvaxmem2_fails_closed_on_malformed_creation_or_delta_carrier() {
    let publication = sample_publication();
    let bytes = publication::encode(&publication).unwrap();
    let wire = json_payload(&bytes);

    let mut malformed_creation = wire.clone();
    let creation_bytes = malformed_creation["records"][0]["bytes"]
        .as_array_mut()
        .unwrap();
    creation_bytes[0] = json!(b'X');
    let malformed_creation = reframe_compact(&bytes, malformed_creation);
    assert!(publication::decode(&malformed_creation).is_err());

    let mut next = publication.records[0].clone();
    next.revision = 2;
    next.global_version += 1;
    next.memory_version += 1;
    next.temporal_status = "historical".into();
    next.mutation_id = "external-compact-delta-test".into();
    next.updated_at_ns += 1;
    let delta =
        encode_delta(&MemoryDelta::between(&publication.records[0], &next).unwrap()).unwrap();
    let mut delta_carrier = wire;
    delta_carrier["records"][0]["bytes"] = json!(delta);
    let delta_carrier = reframe_compact(&bytes, delta_carrier);
    assert!(publication::decode(&delta_carrier).is_err());
}

#[test]
fn cvaxmem1_adapter_reads_populated_legacy_record_coordinates() {
    let publication = sample_publication();
    let record = publication.records[0].clone();
    let bytes = legacy_publication_frame(&publication);

    let decoded = publication::decode(&bytes).unwrap().unwrap();
    assert_eq!(decoded.records, vec![record]);
    assert_eq!(decoded.records[0].global_version, 1);
    assert_eq!(decoded.records[0].memory_version, 1);
    assert_eq!(
        serde_json::to_value(&decoded.bindings).unwrap(),
        serde_json::to_value(&publication.bindings).unwrap()
    );
    assert_eq!(
        publication::decode_embedded_version_range(&bytes).unwrap(),
        Some((1, 1, 900))
    );
}

#[test]
fn v1_owner_replays_populated_legacy_external_publication_without_rewriting_bytes() {
    let scratch = ScratchDirectory::new();
    let path = scratch.file("populated-legacy-external-v1.rel");
    let mut publication = sample_publication();
    // This owner has no archive activity from which to derive a birth turn.
    publication.birth_turn = Some(0);
    let expected_record = publication.records[0].clone();
    let expected_body = publication.bodies[0].clone();
    let expected_evidence = publication.bindings[0].evidence.clone();

    let mut rel = Cva::create(&path).unwrap();
    rel.container
        .append(&legacy_publication_frame(&publication))
        .unwrap();
    rel.container
        .commit_embedded_version_range_at(1, 1, Some(publication.transaction_ns))
        .unwrap();
    rel.sync().unwrap();
    drop(rel);

    let mut file_bytes = fs::read(&path).unwrap();
    file_bytes[8..10].copy_from_slice(&1_u16.to_le_bytes());
    file_bytes[10..12].copy_from_slice(&0_u16.to_le_bytes());
    fs::write(&path, &file_bytes).unwrap();
    let before = fs::read(&path).unwrap();

    let mut reopened = Cva::open_v1_read_only(&path).unwrap();
    assert_eq!(
        reopened.latest_global_version(),
        expected_record.global_version
    );
    assert_eq!(
        reopened.transaction_time_ns(expected_record.global_version),
        Some(publication.transaction_ns)
    );
    assert_eq!(
        reopened.memory(expected_record.id).unwrap().content,
        "Derived semantic memory text."
    );
    assert_eq!(
        reopened.memory_body_id(expected_record.id).unwrap(),
        expected_body.id
    );
    assert_eq!(
        reopened
            .external_memory_evidence(expected_record.id)
            .unwrap(),
        Some(expected_evidence)
    );
    assert_eq!(reopened.memory_version(), expected_record.memory_version);
    drop(reopened);

    assert_eq!(fs::read(&path).unwrap(), before);
}

#[test]
fn v1_external_publication_memory_migration_preserves_semantics_and_source_bytes() {
    let scratch = ScratchDirectory::new();
    let source_path = scratch.file("populated-legacy-external-source.rel");
    let destination = scratch.file("populated-legacy-external-output.rel");
    let mut publication = sample_publication();
    publication.birth_turn = Some(0);
    let record = publication.records[0].clone();
    let body = publication.bodies[0].clone();
    let routing = crate::MemoryRoutingMetadata {
        memory_id: record.id,
        body_id: body.id,
        entity_mentions: vec![crate::MemoryEntityMention {
            field: crate::MemoryTextField::Content,
            start_byte: 0,
            end_byte: "Derived semantic memory text.".len() as u32,
            text: "Derived semantic memory text.".into(),
        }],
    };
    publication.routing.push(routing.clone());

    let mut rel = Cva::create(&source_path).unwrap();
    rel.container
        .append(&legacy_publication_frame(&publication))
        .unwrap();
    rel.container
        .commit_embedded_version_range_at(1, 1, Some(publication.transaction_ns))
        .unwrap();
    rel.sync().unwrap();
    drop(rel);

    let mut file_bytes = fs::read(&source_path).unwrap();
    file_bytes[8..10].copy_from_slice(&1_u16.to_le_bytes());
    file_bytes[10..12].copy_from_slice(&0_u16.to_le_bytes());
    fs::write(&source_path, &file_bytes).unwrap();
    let source_before = fs::read(&source_path).unwrap();
    let request = crate::storage_upgrade::StorageUpgradeRequest {
        source: source_path.clone(),
        destination,
    };
    let plan = crate::storage_upgrade::plan_storage_upgrade(request).unwrap();
    let staged = crate::storage_upgrade::stage_memory_storage_upgrade(&plan).unwrap();

    let mut source = Cva::open_v1_read_only(&source_path).unwrap();
    let mut migrated = Cva::open(staged.test_path()).unwrap();
    let source_memory = source.memory(record.id).unwrap();
    let migrated_memory = migrated.memory(record.id).unwrap();
    assert_eq!(migrated_memory, source_memory);
    assert_eq!(migrated.memory_body_id(record.id).unwrap(), body.id);
    assert_eq!(source.memory_body_id(record.id).unwrap(), body.id);
    assert_eq!(migrated.memory_routing_metadata(record.id), Some(&routing));
    assert_eq!(source.memory_routing_metadata(record.id), Some(&routing));
    assert_eq!(
        migrated.external_memory_evidence(record.id).unwrap(),
        source.external_memory_evidence(record.id).unwrap()
    );
    assert_eq!(migrated.memories.records(), source.memories.records());
    assert_eq!(
        migrated.latest_global_version(),
        source.latest_global_version()
    );
    assert_eq!(migrated.memory_version(), source.memory_version());
    for global in 0..=source.latest_global_version() {
        assert_eq!(
            migrated.transaction_time_ns(global),
            source.transaction_time_ns(global),
            "transaction time at global version {global}"
        );
    }
    let migrated_bytes = fs::read(staged.test_path()).unwrap();
    assert!(
        migrated_bytes
            .windows(8)
            .any(|window| window == b"CVAXMEM2")
    );
    assert_eq!(fs::read(&source_path).unwrap(), source_before);

    drop(migrated);
    drop(source);
    drop(staged);
}

#[test]
fn v1_owner_rejects_compact_external_publication_without_rewriting_bytes() {
    let scratch = ScratchDirectory::new();
    let path = scratch.file("compact-external-v1.rel");
    let mut rel = Cva::create(&path).unwrap();
    let bytes = publication::encode(&sample_publication()).unwrap();
    rel.container.append(&bytes).unwrap();
    rel.container
        .commit_embedded_version_range_at(1, 1, Some(900))
        .unwrap();
    rel.sync().unwrap();
    drop(rel);

    let mut file_bytes = fs::read(&path).unwrap();
    file_bytes[8..10].copy_from_slice(&1_u16.to_le_bytes());
    file_bytes[10..12].copy_from_slice(&0_u16.to_le_bytes());
    fs::write(&path, &file_bytes).unwrap();
    let before = fs::read(&path).unwrap();
    assert!(matches!(
        Cva::open_v1_read_only(&path),
        Err(crate::CvaError::Memories(
            crate::MemoryError::ConflictingFormat
        ))
    ));
    assert_eq!(fs::read(&path).unwrap(), before);
}
