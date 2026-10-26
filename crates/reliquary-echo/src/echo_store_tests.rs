use crate::echo_codec::{decode, encode};
use crate::{EchoError, EchoEvent, EchoEventKind, EchoStore};
use reliquary_container::Container;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_DIR: AtomicU64 = AtomicU64::new(0);

fn temp_rel() -> (PathBuf, PathBuf) {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".test-tmp")
        .join(format!(
            "{}-{}",
            std::process::id(),
            NEXT_DIR.fetch_add(1, Ordering::Relaxed)
        ));
    fs::create_dir_all(&dir).unwrap();
    (dir.clone(), dir.join("owner.rel"))
}

fn event(sequence: u64, kind: EchoEventKind, content: &str) -> EchoEvent {
    EchoEvent {
        conversation_id: "conversation".into(),
        message_id: "turn".into(),
        sequence,
        timestamp_ns: 42,
        model_round: Some(3),
        kind,
        correlation_id: None,
        name: None,
        content: content.into(),
    }
}

#[test]
fn codec_preserves_current_magic_fields_and_ack_tag_eight() {
    let mut ack = event(7, EchoEventKind::ToolContextAccepted, "receiver proof");
    ack.correlation_id = Some("delivery-1".into());
    let bytes = encode(&ack).unwrap();
    let mut expected = b"CVAECHO1\x0c\x00\x00\x00conversation\x04\x00\x00\x00turn".to_vec();
    expected.extend_from_slice(&7_u64.to_le_bytes());
    expected.extend_from_slice(&42_i64.to_le_bytes());
    expected.extend_from_slice(
        b"\x01\x03\x00\x00\x00\x08\x01\x0a\x00\x00\x00delivery-1\x00\x0e\x00\x00\x00receiver proof",
    );
    assert_eq!(bytes, expected);
    assert_eq!(decode(&bytes).unwrap(), Some(ack));
    // Kind tag follows magic, two source strings, sequence, timestamp and round.
    assert_eq!(bytes[8 + 4 + 12 + 4 + 4 + 8 + 8 + 1 + 4], 8);
}

#[test]
fn codec_ignores_other_records_and_rejects_malformed_known_records() {
    assert_eq!(decode(b"CVAPUB01unrelated").unwrap(), None);
    let valid = encode(&event(0, EchoEventKind::ReasoningSummary, "summary")).unwrap();
    let mut trailing = valid.clone();
    trailing.push(0);
    assert!(matches!(
        decode(&trailing),
        Err(EchoError::InvalidRecord("trailing bytes"))
    ));
    for prefix in 8..valid.len() {
        assert!(
            decode(&valid[..prefix]).is_err(),
            "accepted prefix length {prefix}"
        );
    }
}

#[test]
fn replay_orders_numeric_sequences_and_enforces_duplicate_identity() {
    let first = event(2, EchoEventKind::ReasoningTrace, "trace");
    let mut store = EchoStore::default();
    store.ingest(&encode(&first).unwrap()).unwrap();
    store
        .ingest(&encode(&event(1, EchoEventKind::Commentary, "comment")).unwrap())
        .unwrap();
    store.ingest(b"other owner").unwrap();
    let before = store.all_records();
    let mut malformed = encode(&first).unwrap();
    malformed.push(0);
    assert!(store.ingest(&malformed).is_err());
    assert_eq!(store.all_records(), before);
    assert_eq!(
        store
            .for_turn("conversation", "turn")
            .iter()
            .map(|e| e.sequence)
            .collect::<Vec<_>>(),
        vec![1, 2]
    );
    store.ingest(&encode(&first).unwrap()).unwrap();
    assert!(matches!(
        store.ingest(&encode(&event(2, EchoEventKind::Commentary, "conflict")).unwrap()),
        Err(EchoError::ConflictingSequence)
    ));
}

#[test]
fn put_is_idempotent_and_appends_before_mutating_store() {
    let (dir, path) = temp_rel();
    let mut container = Container::create(&path).unwrap();
    let mut store = EchoStore::default();
    let record = event(0, EchoEventKind::ReasoningSummary, "summary");
    assert!(store.put(&mut container, record.clone()).unwrap());
    assert!(!store.put(&mut container, record).unwrap());
    assert_eq!(store.all_records().len(), 1);
    drop(container);
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn failed_physical_append_does_not_change_accepted_state() {
    let (dir, path) = temp_rel();
    drop(Container::create(&path).unwrap());
    let mut read_only = Container::open_read_only(&path).unwrap();
    let mut store = EchoStore::default();
    let error = store
        .put(
            &mut read_only,
            event(0, EchoEventKind::ReasoningSummary, "summary"),
        )
        .unwrap_err();
    assert!(matches!(error, EchoError::Container(_)));
    assert!(store.all_records().is_empty());
    drop(read_only);
    fs::remove_dir_all(dir).unwrap();
}
