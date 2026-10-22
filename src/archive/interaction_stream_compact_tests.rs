use crate::interaction_stream_codec::{self as codec, CompactRecord};
use crate::interaction_stream_store::InteractionStreamStore;
use crate::{Container, Cva, InteractionRole, InteractionStreamRecord, InteractionStreamStatus};
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
            .join(".test-tmp")
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
fn compact_stream_checkpoint_writer_reopen_and_rejection_are_clock_neutral() {
    let scratch = Scratch::new();
    let path = scratch.0.join("stream.rel");
    let mut rel = Some(Cva::create(&path).unwrap());
    for (text, status) in [
        ("", InteractionStreamStatus::Streaming),
        ("é", InteractionStreamStatus::Streaming),
        ("é🙂", InteractionStreamStatus::Interrupted),
        ("é🙂!", InteractionStreamStatus::Interrupted),
    ] {
        let record = checkpoint(text, status);
        let owner = rel.as_mut().unwrap();
        owner.put_interaction_stream(record.clone()).unwrap();
        owner.sync().unwrap();
        drop(rel.take().unwrap());
        let reopened = Cva::open(&path).unwrap();
        assert_eq!(reopened.interaction_stream_record("m"), Some(&record));
        rel = Some(reopened);
    }
    let mut rel = rel.unwrap();
    let before = fs::read(&path).unwrap();
    rel.put_interaction_stream(checkpoint("é🙂!", InteractionStreamStatus::Interrupted))
        .unwrap();
    for bad in [
        checkpoint("é🙂!", InteractionStreamStatus::Streaming),
        checkpoint("éother", InteractionStreamStatus::Interrupted),
    ] {
        assert!(rel.put_interaction_stream(bad).is_err());
    }
    rel.sync().unwrap();
    assert_eq!(fs::read(&path).unwrap(), before);
    let mut payloads = Vec::new();
    {
        let mut ingest_frame =
            |frame: crate::container::PhysicalFrame| -> Result<(), crate::CvaError> {
                let payload = frame.payload.as_slice();
                {
                    payloads.push(payload.to_vec());
                    Ok(())
                }
            };
        let mut cursor = rel.container.physical_cursor().unwrap();
        (|| {
            while let Some(frame) = cursor.next_frame()? {
                ingest_frame(frame)?;
            }
            Ok::<(), crate::CvaError>(())
        })()
    }
    .unwrap();
    assert!(!payloads.iter().any(|p| p.starts_with(b"CVAISTR1")));
    assert!(!payloads.iter().any(|p| p.starts_with(b"CVAVERS")));
    assert_eq!(payloads.iter().filter(|p| codec::is_compact(p)).count(), 5);
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
    {
        let mut ingest_frame =
            |frame: crate::container::PhysicalFrame| -> Result<(), crate::CvaError> {
                let payload = frame.payload.as_slice();
                {
                    if codec::is_compact(payload) {
                        current_bytes += payload.len() + 8;
                        count += 1;
                    }
                    Ok(())
                }
            };
        let mut cursor = container.physical_cursor().unwrap();
        (|| {
            while let Some(frame) = cursor.next_frame()? {
                ingest_frame(frame)?;
            }
            Ok::<(), crate::CvaError>(())
        })()
    }
    .unwrap();
    assert_eq!(
        (frozen_full_checkpoint_model_bytes, current_bytes, count),
        (79925, 9809, 258)
    );
}

#[test]
fn compact_stream_all_torn_checkpoint_tails_retain_acknowledged_utf8() {
    let scratch = Scratch::new();
    let source = scratch.0.join("source.rel");
    let mut rel = Cva::create(&source).unwrap();
    rel.put_interaction_stream(checkpoint("é", InteractionStreamStatus::Streaming))
        .unwrap();
    rel.sync().unwrap();
    let acknowledged = fs::metadata(&source).unwrap().len() as usize;
    rel.put_interaction_stream(checkpoint("é🙂", InteractionStreamStatus::Interrupted))
        .unwrap();
    rel.sync().unwrap();
    drop(rel);
    let bytes = fs::read(&source).unwrap();
    let append_end = acknowledged
        + 8
        + codec::encode_compact(&CompactRecord::Append {
            message_id: "m".into(),
            base_len: "é".len() as u32,
            suffix: "🙂".into(),
        })
        .unwrap()
        .len();
    for cut in acknowledged..=bytes.len() {
        let path = scratch.0.join(format!("torn{cut}.rel"));
        fs::write(&path, &bytes[..cut]).unwrap();
        let record = match Cva::open_read_only(&path) {
            Ok(opened) => {
                let record = opened.interaction_stream_record("m").cloned().unwrap();
                drop(opened);
                assert_eq!(fs::read(&path).unwrap(), bytes[..cut]);
                record
            }
            Err(crate::CvaError::Container(crate::ContainerError::RepairRequired {
                valid_end,
            })) => {
                let expected_end = if cut < append_end {
                    acknowledged
                } else {
                    append_end
                };
                assert_eq!(valid_end, expected_end as u64, "cut={cut}");
                let before_repair = fs::read(&path).unwrap();
                assert_eq!(before_repair, bytes[..cut]);
                assert_eq!(
                    Cva::repair_incomplete_tail(&path).unwrap(),
                    crate::TailRepairOutcome::Repaired,
                    "cut={cut}"
                );
                assert_eq!(fs::read(&path).unwrap(), bytes[..expected_end]);
                Cva::open(&path)
                    .unwrap()
                    .interaction_stream_record("m")
                    .cloned()
                    .unwrap()
            }
            Err(error) => {
                assert_eq!(fs::read(&path).unwrap(), bytes[..cut]);
                panic!("unexpected complete-record rejection at cut={cut}: {error}");
            }
        };
        if cut == acknowledged {
            assert_eq!(record.content, "é");
            assert_eq!(record.status, InteractionStreamStatus::Streaming);
        }
        if cut >= append_end && cut < bytes.len() {
            assert_eq!(record.content, "é🙂");
            assert_eq!(record.status, InteractionStreamStatus::Streaming);
        }
        assert!(record.content == "é" || record.content == "é🙂");
        assert!(record.content.starts_with("é"));
        if record.status == InteractionStreamStatus::Interrupted {
            assert_eq!(record.content, "é🙂");
        }
    }
    assert_eq!(fs::read(&source).unwrap(), bytes);
}

#[test]
fn compact_stream_destination_reconciliation_uses_suffix_writer_and_interruption_dominance() {
    let scratch = Scratch::new();
    let left = scratch.0.join("left.rel");
    let right = scratch.0.join("right.rel");
    let output = scratch.0.join("merged.rel");
    let mut rel = Cva::create(&left).unwrap();
    rel.put_interaction_stream(checkpoint("é", InteractionStreamStatus::Streaming))
        .unwrap();
    rel.sync().unwrap();
    drop(rel);
    fs::copy(&left, &right).unwrap();
    let mut rel = Cva::open(&left).unwrap();
    rel.put_interaction_stream(checkpoint("é", InteractionStreamStatus::Interrupted))
        .unwrap();
    rel.sync().unwrap();
    drop(rel);
    let mut rel = Cva::open(&right).unwrap();
    rel.put_interaction_stream(checkpoint("é🙂", InteractionStreamStatus::Streaming))
        .unwrap();
    rel.sync().unwrap();
    drop(rel);
    let before = (fs::read(&left).unwrap(), fs::read(&right).unwrap());
    Cva::reconcile(&left, &right, &output).unwrap();
    let mut merged = Cva::open(&output).unwrap();
    assert_eq!(
        merged.interaction_stream_record("m"),
        Some(&checkpoint("é🙂", InteractionStreamStatus::Interrupted))
    );
    {
        let mut ingest_frame =
            |frame: crate::container::PhysicalFrame| -> Result<(), crate::CvaError> {
                let payload = frame.payload.as_slice();
                {
                    assert!(!payload.starts_with(b"CVAISTR1"));
                    Ok(())
                }
            };
        let mut cursor = merged.container.physical_cursor().unwrap();
        (|| {
            while let Some(frame) = cursor.next_frame()? {
                ingest_frame(frame)?;
            }
            Ok::<(), crate::CvaError>(())
        })()
    }
    .unwrap();
    assert_eq!(
        (fs::read(&left).unwrap(), fs::read(&right).unwrap()),
        before
    );
}

#[test]
fn compact_stream_is_rel_only_and_rejected_in_v1_without_mutation() {
    let scratch = Scratch::new();
    let path = scratch.0.join("current.rel");
    let mut rel = Cva::create(&path).unwrap();
    rel.put_interaction_stream(checkpoint("", InteractionStreamStatus::Streaming))
        .unwrap();
    rel.sync().unwrap();
    drop(rel);
    let mut bytes = fs::read(&path).unwrap();
    bytes[8..10].copy_from_slice(&1u16.to_le_bytes());
    fs::write(&path, &bytes).unwrap();
    assert!(Cva::open(&path).is_err());
    assert_eq!(fs::read(&path).unwrap(), bytes);
    let path = scratch.0.join("wrong-owner.phy");
    let mut phy = crate::Phylactery::create(&path).unwrap();
    phy.container
        .append(&codec::encode_compact(&begin()).unwrap())
        .unwrap();
    phy.sync().unwrap();
    drop(phy);
    assert!(crate::Phylactery::open(&path).is_err());
}
