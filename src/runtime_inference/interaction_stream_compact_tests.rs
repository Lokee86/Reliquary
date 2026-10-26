use crate::{Cva, InteractionRole, InteractionStreamRecord, InteractionStreamStatus};
use reliquary_container::Container;
use std::{fs, path::PathBuf};

fn fixture_begin_frame() -> Vec<u8> {
    (0.."435641495354523200010000006d01000000730001ffffffffffffffff".len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(
                &"435641495354523200010000006d01000000730001ffffffffffffffff"[i..i + 2],
                16,
            )
            .unwrap()
        })
        .collect()
}

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
fn compact_stream_checkpoint_writer_reopen_and_rejection_are_clock_neutral() {
    let scratch = Scratch::new();
    let path = scratch.0.join("stream.rel");
    let mut rel = Cva::create(&path).unwrap();
    for (text, status) in [
        ("", InteractionStreamStatus::Streaming),
        ("é", InteractionStreamStatus::Streaming),
        ("é🙂", InteractionStreamStatus::Interrupted),
        ("é🙂!", InteractionStreamStatus::Interrupted),
    ] {
        let record = checkpoint(text, status);
        rel.put_interaction_stream(record.clone()).unwrap();
        rel.sync().unwrap();
        drop(rel);
        rel = Cva::open(&path).unwrap();
        assert_eq!(rel.interaction_stream_record("m"), Some(&record));
    }
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
    let source = rel.container.read_source().unwrap();
    let mut cursor = source.physical_cursor().unwrap();
    while let Some(frame) = cursor.next_frame().unwrap() {
        payloads.push(frame.payload);
    }
    assert!(!payloads.iter().any(|p| p.starts_with(b"CVAISTR1")));
    assert!(!payloads.iter().any(|p| p.starts_with(b"CVAVERS")));
    assert_eq!(
        payloads
            .iter()
            .filter(|p| p.starts_with(b"CVAISTR2"))
            .count(),
        5
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
    let boundaries = {
        let physical = Container::open_read_only(&source).unwrap();
        let mut cursor = physical.physical_cursor().unwrap();
        let mut ends = vec![acknowledged];
        while let Some(frame) = cursor.next_frame().unwrap() {
            if frame.end as usize > acknowledged {
                ends.push(frame.end as usize);
            }
        }
        ends
    };
    for cut in acknowledged..=bytes.len() {
        let path = scratch.0.join(format!("torn{cut}.rel"));
        fs::write(&path, &bytes[..cut]).unwrap();
        let accepted_end = boundaries
            .iter()
            .copied()
            .filter(|end| *end <= cut)
            .max()
            .unwrap();
        if cut != accepted_end {
            crate::container_owner_tests::assert_normal_open_requires_repair(
                &path,
                |p| Cva::open(p).map(|_| ()).map_err(|e| e.to_string()),
                accepted_end as u64,
            );
            crate::container_owner_tests::repair_incomplete_physical_tail(
                &path,
                accepted_end as u64,
            );
        }
        let reopened = Cva::open(&path).unwrap();
        let record = reopened.interaction_stream_record("m").unwrap();
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
    let source = merged.container.read_source().unwrap();
    let mut cursor = source.physical_cursor().unwrap();
    while let Some(frame) = cursor.next_frame().unwrap() {
        assert!(!frame.payload.starts_with(b"CVAISTR1"));
    }
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
    phy.container.append(&fixture_begin_frame()).unwrap();
    phy.sync().unwrap();
    drop(phy);
    assert!(crate::Phylactery::open(&path).is_err());
}
