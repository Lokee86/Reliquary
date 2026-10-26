use crate::interaction_stream_codec::{self as codec, CompactRecord};
use crate::interaction_stream_store::InteractionStreamStore;
use crate::{InteractionRole, InteractionStreamRecord, InteractionStreamStatus};
use reliquary_container::Container;
use std::{fs, path::PathBuf};

fn checkpoint(text: &str, status: InteractionStreamStatus) -> InteractionStreamRecord {
    InteractionStreamRecord {
        message_id: "m".into(),
        session_id: "s".into(),
        parent_message_id: None,
        role: InteractionRole::Agent,
        timestamp_ns: -1,
        content: text.into(),
        status,
    }
}

fn begin() -> CompactRecord {
    CompactRecord::Begin(checkpoint("", InteractionStreamStatus::Streaming))
}

fn append(base_len: u32, suffix: &str) -> CompactRecord {
    CompactRecord::Append {
        message_id: "m".into(),
        base_len,
        suffix: suffix.into(),
    }
}

fn interrupted(content_len: u32) -> CompactRecord {
    CompactRecord::Interrupted {
        message_id: "m".into(),
        content_len,
    }
}

fn ingest(store: &mut InteractionStreamStore, frame: &CompactRecord) {
    store
        .ingest(&codec::encode_compact(frame).unwrap())
        .unwrap();
}

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".test-output")
            .join(format!("stream-compact-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn compact_stream_independent_wire_vectors_and_malformed_records() {
    let vectors = [
        (
            begin(),
            "435641495354523200010000006d01000000730001ffffffffffffffff",
        ),
        (
            append(0, "é"),
            "435641495354523201010000006d0000000002000000c3a9",
        ),
        (interrupted(2), "435641495354523202010000006d02000000"),
    ];
    for (record, hex) in vectors {
        let bytes: Vec<_> = (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
            .collect();
        assert_eq!(codec::encode_compact(&record).unwrap(), bytes);
        assert_eq!(codec::decode_compact(&bytes).unwrap(), Some(record));
        for end in 7..bytes.len() {
            assert!(
                codec::decode_compact(&bytes[..end]).is_err(),
                "prefix {end}"
            );
        }
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert!(codec::decode_compact(&trailing).is_err());
    }
    for payload in [b"CVAISTR3".as_slice(), b"CVAISTR2\x03".as_slice()] {
        assert!(codec::decode_compact(payload).is_err());
    }
    let mut invalid = codec::encode_compact(&append(0, "é")).unwrap();
    *invalid.last_mut().unwrap() = 0xff;
    assert!(codec::decode_compact(&invalid).is_err());
    assert!(codec::encode_compact(&append(0, "")).is_err());
    assert!(codec::encode_compact(&append(u32::MAX, "x")).is_err());
    let mut parent = codec::encode_compact(&begin()).unwrap();
    parent[19] = 2;
    assert!(codec::decode_compact(&parent).is_err());
    let mut role = codec::encode_compact(&begin()).unwrap();
    role[20] = 2;
    assert!(codec::decode_compact(&role).is_err());
}

#[test]
fn compact_stream_replay_preserves_retry_interruption_and_conflict_state() {
    let mut store = InteractionStreamStore::default();
    assert!(
        store
            .ingest(&codec::encode_compact(&append(0, "x")).unwrap())
            .is_err()
    );
    assert!(
        store
            .ingest(&codec::encode_compact(&interrupted(0)).unwrap())
            .is_err()
    );
    ingest(&mut store, &begin());
    ingest(&mut store, &append(0, "é"));
    ingest(&mut store, &append(0, "é"));
    ingest(&mut store, &interrupted(2));
    ingest(&mut store, &begin());
    ingest(&mut store, &append(2, "🙂"));
    assert_eq!(
        store.record("m").unwrap(),
        &checkpoint("é🙂", InteractionStreamStatus::Interrupted)
    );
    for bad in [
        append(0, "é"),
        append(1, "hello"),
        append(2, "oops"),
        interrupted(2),
    ] {
        let before = store.record("m").unwrap().clone();
        assert!(store.ingest(&codec::encode_compact(&bad).unwrap()).is_err());
        assert_eq!(store.record("m"), Some(&before));
    }
    let mut changed = checkpoint("", InteractionStreamStatus::Streaming);
    changed.timestamp_ns = 0;
    assert!(
        store
            .ingest(&codec::encode_compact(&CompactRecord::Begin(changed)).unwrap())
            .is_err()
    );
    ingest(&mut store, &interrupted(6));
}

#[test]
fn compact_stream_every_complete_frame_prefix_is_replayable() {
    let frames = [begin(), append(0, "é"), interrupted(2), append(2, "🙂")];
    let expected = [
        ("", InteractionStreamStatus::Streaming),
        ("é", InteractionStreamStatus::Streaming),
        ("é", InteractionStreamStatus::Interrupted),
        ("é🙂", InteractionStreamStatus::Interrupted),
    ];
    let mut store = InteractionStreamStore::default();
    for (frame, (text, status)) in frames.iter().zip(expected) {
        ingest(&mut store, frame);
        assert_eq!(store.record("m"), Some(&checkpoint(text, status)));
    }
}

#[test]
fn compact_stream_long_stream_has_linear_raw_growth() {
    let scratch = Scratch::new();
    let mut container = Container::create(scratch.0.join("raw.rel")).unwrap();
    let mut store = InteractionStreamStore::default();
    let mut frozen_full_checkpoint_model_bytes = 0;
    let mut record = checkpoint("", InteractionStreamStatus::Streaming);
    record.message_id = "message".into();
    record.session_id = "session".into();
    for _ in 0..256 {
        record.content.push('é');
        frozen_full_checkpoint_model_bytes += 31
            + record.message_id.len()
            + record.session_id.len()
            + record.parent_message_id.as_ref().map_or(0, |p| 4 + p.len())
            + record.content.len()
            + 8;
        store.put(&mut container, record.clone()).unwrap();
    }
    record.status = InteractionStreamStatus::Interrupted;
    frozen_full_checkpoint_model_bytes += 31
        + record.message_id.len()
        + record.session_id.len()
        + record.parent_message_id.as_ref().map_or(0, |p| 4 + p.len())
        + record.content.len()
        + 8;
    store.put(&mut container, record).unwrap();
    let mut current_bytes = 0;
    let mut count = 0;
    let source = container.read_source().unwrap();
    let mut cursor = source.physical_cursor().unwrap();
    while let Some(frame) = cursor.next_frame().unwrap() {
        if frame.payload.starts_with(b"CVAISTR2") {
            current_bytes += frame.payload.len() + 8;
            count += 1;
        }
    }
    assert_eq!(
        (frozen_full_checkpoint_model_bytes, current_bytes, count),
        (79925, 9809, 258)
    );
}
