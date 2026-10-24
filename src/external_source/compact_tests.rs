use super::test_support::{external_index_state, publish_window};
use super::*;
use crate::external_index::codec::Record;
use crate::external_index::{codec, store};
use sha2::{Digest, Sha256};
use std::{fs, path::PathBuf};

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!("external-compact-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn empty() -> store::SourceState {
    store::SourceState {
        namespace: "x".into(),
        contract: 1,
        profile: [1; 32],
        dimensions: 2,
        generation: 9,
        revision: 17,
        cursor: 4,
        messages: Vec::new(),
        last_base: 3,
        last_digest: [2; 32],
        rebuild: false,
    }
}
fn checksum(bytes: &mut [u8]) {
    let end = bytes.len() - 32;
    let hash = Sha256::digest(&bytes[..end]);
    bytes[end..].copy_from_slice(&hash);
}

#[test]
fn compact_external_index_roundtrip_bounds_and_independent_layout() {
    let state = empty();
    let bytes = store::encode(&state).unwrap();
    // Independently specified binary offsets: no JSON keys or numeric hash arrays.
    assert_eq!(&bytes[..9], b"CVAXIDX2\0");
    assert_eq!(bytes.len(), 163);
    assert_eq!(u64::from_le_bytes(bytes[9..17].try_into().unwrap()), 114);
    assert_eq!(&bytes[17..22], b"\x01\0\0\0x");
    assert_eq!(&bytes[26..58], &[1; 32]);
    assert_eq!(u64::from_le_bytes(bytes[62..70].try_into().unwrap()), 9);
    assert_eq!(u64::from_le_bytes(bytes[70..78].try_into().unwrap()), 17);
    match codec::decode(&bytes).unwrap() {
        Record::Snapshot(decoded) => assert_eq!(decoded, state),
        _ => panic!("snapshot"),
    }
    for end in 0..bytes.len() {
        assert!(codec::decode(&bytes[..end]).is_err());
    }
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert!(codec::decode(&trailing).is_err());
    let mut invalid = bytes.clone();
    invalid[126] = 2;
    checksum(&mut invalid);
    assert!(codec::decode(&invalid).is_err());
    let mut count = bytes.clone();
    count[127..131].copy_from_slice(&u32::MAX.to_le_bytes());
    checksum(&mut count);
    assert!(codec::decode(&count).is_err());
    let mut unknown = bytes.clone();
    unknown[8] = 3;
    checksum(&mut unknown);
    assert!(codec::decode(&unknown).is_err());
    let discard = store::discard_payload();
    assert_eq!(discard.len(), 49);
    assert!(matches!(codec::decode(&discard), Ok(Record::Discard)));
    let mut store = store::ExternalIndexStore::default();
    store.ingest(&bytes);
    assert_eq!(store.sources["x"], state);
    store.ingest(&discard);
    assert!(store.damaged && store.sources.is_empty());
    store.ingest(&bytes);
    assert_eq!(store.sources["x"], state);
    assert!(store.damaged);
}

#[test]
fn compact_external_index_reclaim_keeps_latest_lookup_and_memory_proof() {
    let scratch = Scratch::new();
    let source = scratch.0.join("source.rel");
    let output = scratch.0.join("reclaimed.rel");
    let host = Host::new("secret canonical evidence");
    let mut rel = Cva::create(&source).unwrap();
    rebuild(&mut rel, &host);
    rebuild(&mut rel, &host);
    let expected = rebuild(&mut rel, &host);
    let window = [31; 32];
    let mut prepared = prepared_memory(window, "An independent semantic decision.");
    prepared.evidence.authority.message = host.refs()[0].clone();
    let memory = publish_window(&mut rel, window, &[prepared]).unwrap().remove(0);
    let evidence = rel.external_memory_evidence(memory.id).unwrap();
    let expected_hits = hits(&mut rel);
    let latest = rel.external_index.sources["host:test"].clone();
    let legacy = serde_json::to_vec(&latest).unwrap().len() + 48;
    let current = store::encode(&latest).unwrap().len();
    assert!(current < legacy);
    eprintln!("external-index payload bytes legacy={legacy} current={current}");
    rel.sync().unwrap();
    let before = fs::read(&source).unwrap();
    let head = rel.latest_global_version();
    let report = rel.reclaim_storage(&output).unwrap();
    assert_eq!(report.superseded_external_index_chunks, 2);
    let mut reopened = Cva::open(&output).unwrap();
    assert_eq!(reopened.latest_global_version(), head);
    assert_eq!(
        external_index_state(&reopened, "host:test")
            .unwrap()
            .unwrap()
            .status(),
        expected
    );
    assert_eq!(hits(&mut reopened), expected_hits);
    assert_eq!(
        reopened.external_memory_evidence(memory.id).unwrap(),
        evidence
    );
    assert_eq!(reopened.memory(memory.id).unwrap().content, memory.content);
    assert_eq!(fs::read(&source).unwrap(), before);
    let mut rows = 0;
    {
        let mut ingest_frame =
            |frame: crate::container::PhysicalFrame| -> Result<(), crate::CvaError> {
                let payload = frame.payload.as_slice();
                {
                    if payload.starts_with(b"CVAXIDX2") {
                        rows += 1;
                    }
                    Ok(())
                }
            };
        let mut cursor = reopened.container.physical_cursor().unwrap();
        (|| {
            while let Some(frame) = cursor.next_frame()? {
                ingest_frame(frame)?;
            }
            Ok::<(), crate::CvaError>(())
        })()
    }
    .unwrap();
    assert_eq!(rows, 1);
}

#[test]
fn compact_external_index_reclaim_preserves_global_damage_and_multiple_namespaces() {
    let scratch = Scratch::new();
    let source = scratch.0.join("source.rel");
    let output = scratch.0.join("reclaimed.rel");
    let mut rel = Cva::create(&source).unwrap();
    let mut state = empty();
    for namespace in ["x", "y"] {
        state.namespace = namespace.into();
        rel.container
            .append(&store::encode(&state).unwrap())
            .unwrap();
    }
    let mut bad = store::encode(&state).unwrap();
    *bad.last_mut().unwrap() ^= 1;
    rel.container.append(&bad).unwrap();
    state.namespace = "x".into();
    state.revision += 1;
    rel.container
        .append(&store::encode(&state).unwrap())
        .unwrap();
    rel.sync().unwrap();
    drop(rel);
    // These codec-only rows lack profiles, so backing validation discards them;
    // damage and absent namespaces must survive physical cleanup either way.
    let mut rel = Cva::open(&source).unwrap();
    assert!(rel.external_index.damaged);
    rel.reclaim_storage(&output).unwrap();
    let reopened = Cva::open(&output).unwrap();
    assert!(reopened.external_index.damaged);
    assert_eq!(
        external_index_state(&reopened, "y"),
        Err(ExternalSourceError::RebuildRequired)
    );
    let mut owner = store::ExternalIndexStore::default();
    for namespace in ["x", "y"] {
        state.namespace = namespace.into();
        owner.ingest(&store::encode(&state).unwrap());
    }
    assert_eq!(owner.sources.len(), 2);
    state.namespace = "x".into();
    state.revision += 1;
    owner.ingest(&store::encode(&state).unwrap());
    assert_eq!(owner.sources["y"].revision, state.revision - 1);
}

#[test]
fn compact_external_index_reclaim_preserves_two_live_namespaces() {
    let scratch = Scratch::new();
    let source = scratch.0.join("source.rel");
    let output = scratch.0.join("output.rel");
    let host = Host::new("secret canonical evidence");
    let mut rel = Cva::create(&source).unwrap();
    rebuild(&mut rel, &host);
    let mut other = rel.external_index.sources["host:test"].clone();
    other.namespace = "host:other".into();
    for message in &mut other.messages {
        message.descriptor.reference.source_namespace = other.namespace.clone();
        for chunk in &mut message.chunks {
            chunk.reference.message = message.descriptor.reference.clone();
        }
    }
    rel.container
        .append(&store::encode(&other).unwrap())
        .unwrap();
    let expected = rebuild(&mut rel, &host);
    rel.sync().unwrap();
    drop(rel);
    let mut rel = Cva::open(&source).unwrap();
    assert_eq!(rel.external_index.sources.len(), 2);
    let report = rel.reclaim_storage(&output).unwrap();
    assert_eq!(report.superseded_external_index_chunks, 1);
    let mut reopened = Cva::open(&output).unwrap();
    assert_eq!(reopened.external_index.sources.len(), 2);
    assert_eq!(reopened.external_index.sources["host:other"], other);
    assert_eq!(
        reopened.external_index.sources["host:test"].status(),
        expected
    );
    assert_eq!(hits(&mut reopened).len(), 1);
}

#[test]
fn compact_external_index_message_fields_and_invalid_logical_shapes() {
    let reference = ExternalMessageRef {
        source_namespace: "x".into(),
        conversation_id: "c".into(),
        message_id: "m".into(),
        content_hash: [3; 32],
    };
    let descriptor = ExternalMessageDescriptor {
        reference: reference.clone(),
        role: "user".into(),
        principal_id: Some("p".into()),
        timestamp_ns: -13,
        active: true,
    };
    let chunk = store::IndexedChunk {
        reference: ExternalChunkRef {
            message: reference,
            start: 0,
            end: 2,
        },
        matrix: [4; 32],
        row: 9,
        terms: vec![[1; 32], [2; 32]],
    };
    let mut state = empty();
    state.messages.push(store::IndexedMessage {
        descriptor,
        chunks: vec![chunk],
    });
    for role in ["user", "assistant", "system", "tool"] {
        state.messages[0].descriptor.role = role.into();
        let bytes = store::encode(&state).unwrap();
        match codec::decode(&bytes).unwrap() {
            Record::Snapshot(decoded) => assert_eq!(decoded, state),
            _ => panic!("snapshot"),
        }
    }
    let bytes = store::encode(&state).unwrap();
    assert_eq!(bytes.len(), 333);
    for offset in [173, 174, 188] {
        let mut bad = bytes.clone();
        bad[offset] = 9;
        checksum(&mut bad);
        assert!(codec::decode(&bad).is_err());
    }
    let mut invalid_utf8 = bytes.clone();
    invalid_utf8[140] = 0xff;
    checksum(&mut invalid_utf8);
    assert!(codec::decode(&invalid_utf8).is_err());
    let mut end = bytes.clone();
    end[193..197].copy_from_slice(&0u32.to_le_bytes());
    checksum(&mut end);
    assert!(codec::decode(&end).is_err());
    let mut duplicate = bytes.clone();
    duplicate[269..301].copy_from_slice(&[1; 32]);
    checksum(&mut duplicate);
    assert!(codec::decode(&duplicate).is_err());
}

#[test]
fn compact_external_index_current_records_reject_in_v1_and_phy() {
    let scratch = Scratch::new();
    let path = scratch.0.join("source.rel");
    let mut rel = Cva::create(&path).unwrap();
    rel.container
        .append(&store::encode(&empty()).unwrap())
        .unwrap();
    rel.sync().unwrap();
    drop(rel);
    let mut bytes = fs::read(&path).unwrap();
    bytes[8..10].copy_from_slice(&1u16.to_le_bytes());
    fs::write(&path, &bytes).unwrap();
    assert!(Cva::open(&path).is_err());
    assert_eq!(fs::read(&path).unwrap(), bytes);
    let path = scratch.0.join("wrong.phy");
    let mut phy = crate::Phylactery::create(&path).unwrap();
    phy.container
        .append(&store::encode(&empty()).unwrap())
        .unwrap();
    phy.sync().unwrap();
    drop(phy);
    assert!(crate::Phylactery::open(&path).is_err());
}
