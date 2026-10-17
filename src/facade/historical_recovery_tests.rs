//! Native Container framing, owner reopen, retry and subsequent live-use recovery.
use super::*;
use crate::{AcceptedMemoryUseReceipt, FreshnessDue, FreshnessState, FreshnessTransitionNotice};

fn reconstruct(cva: &mut Cva) -> HistoricalReconstructionReport {
    cva.reconstruct_historical_freshness(HistoricalReconstructionOptions::default())
        .unwrap()
}
fn projection(cva: &Cva) -> BTreeMap<MemoryId, FreshnessRecord> {
    cva.freshness.records().clone()
}
fn clocks(cva: &Cva) -> (u64, u64, u64, u64, u64) {
    (
        cva.latest_global_version(),
        cva.archive_version(),
        cva.memory_version(),
        cva.graph_version(),
        cva.memory_graph_version(),
    )
}
fn assert_one_frame(before: &[u8], after: &[u8], magic: &[u8; 8]) -> usize {
    assert!(after.starts_with(before));
    let append = &after[before.len()..];
    let length = u64::from_le_bytes(append[..8].try_into().unwrap()) as usize;
    assert_eq!(append.len(), 8 + length);
    assert_eq!(&append[8..16], magic);
    append.len()
}

#[test]
fn historical_native_checkpoint_every_container_byte_prefix_reopens_and_retries() {
    for with_vectors in [false, true] {
        let (path, mut cva, ids) =
            historical_import_fixture_count(&format!("h6-checkpoint-{with_vectors}.rel"), 2);
        if with_vectors {
            // Include both a genuine shared semantic source event and a
            // credit-excluded archived alias in the complete checkpoint.
            super::historical_events::linked_source(&mut cva, ids[0], true);
            super::historical_events::vectors(&mut cva);
        }
        cva.sync().unwrap();
        let before = fs::read(&path).unwrap();
        let old = projection(&cva);
        let original_clocks = clocks(&cva);
        let report = reconstruct(&mut cva);
        assert!(report.committed);
        let accepted = projection(&cva);
        assert_ne!(old, accepted);
        let accepted_due = cva.freshness_due.clone();
        let complete = fs::read(&path).unwrap();
        let append_len = assert_one_frame(&before, &complete, b"CVAFRS04");
        drop(cva);
        let crashed_path = path.with_file_name("checkpoint-prefix.rel");

        // Zero bytes, all 1..7-byte frame headers, the complete header,
        // every payload/checksum prefix, and the complete synchronized frame.
        for prefix in 0..=append_len {
            fs::write(&crashed_path, &complete[..before.len() + prefix]).unwrap();
            let mut reopened = Cva::open(&crashed_path)
                .unwrap_or_else(|e| panic!("vectors={with_vectors}, prefix={prefix}: {e}"));
            let full = prefix == append_len;
            assert_eq!(
                projection(&reopened),
                if full { accepted.clone() } else { old.clone() },
                "vectors={with_vectors}, prefix={prefix}"
            );
            assert_eq!(!reopened.freshness.import_settlements().is_empty(), full);
            assert_eq!(clocks(&reopened), original_clocks);
            assert_eq!(
                fs::read(&crashed_path).unwrap(),
                if full { &complete } else { &before }.clone()
            );
            if full {
                assert_eq!(reopened.freshness_due, accepted_due);
            }

            let retry = reconstruct(&mut reopened);
            assert_eq!(retry.committed, !full);
            assert_eq!(retry.import_id, report.import_id);
            assert_eq!(retry.manifest_digest, report.manifest_digest);
            assert_eq!(retry.activity_cut, report.activity_cut);
            assert_eq!(projection(&reopened), accepted);
            assert_eq!(fs::read(&crashed_path).unwrap(), complete);
            drop(reopened);
            let mut twice = Cva::open(&crashed_path).unwrap();
            assert_eq!(twice.freshness_due, accepted_due);
            assert!(!reconstruct(&mut twice).committed);
            assert_eq!(projection(&twice), accepted);
            assert_eq!(fs::read(&crashed_path).unwrap(), complete);
        }
        eprintln!(
            "C6 checkpoint vectors={with_vectors}: {} native byte prefixes passed",
            append_len + 1
        );
    }
}

#[test]
fn historical_native_complete_corrupt_checkpoint_rejects_without_tail_repair() {
    let (path, mut cva, _) = historical_import_fixture_count("h6-corrupt.rel", 2);
    reconstruct(&mut cva);
    drop(cva);
    let mut bytes = fs::read(&path).unwrap();
    *bytes.last_mut().unwrap() ^= 1; // Complete framing, invalid checkpoint checksum.
    fs::write(&path, &bytes).unwrap();
    assert!(
        Cva::open(&path)
            .err()
            .unwrap()
            .to_string()
            .contains("checksum")
    );
    assert_eq!(fs::read(&path).unwrap(), bytes);
}

#[test]
fn historical_native_due_reopen_and_every_live_use_prefix_preserve_all_recipients() {
    let (path, mut cva, mut ids) = historical_import_fixture_count("h6-live-use.rel", 3);
    ids.sort();
    let report = reconstruct(&mut cva);
    let settlement = cva.freshness.latest_import_settlement().unwrap().clone();
    let accepted_due = cva.freshness_due.clone();
    assert!(
        ids.iter()
            .all(|id| cva.freshness_record(*id).unwrap().score == 99)
    );
    let before = fs::read(&path).unwrap();
    drop(cva);
    let mut cva = Cva::open(&path).unwrap();
    assert_eq!(fs::read(&path).unwrap(), before);
    assert_eq!(cva.freshness_due, accepted_due);
    let mut due = cva.freshness_due.clone();
    assert!(due.drain_due(1015, 10).is_empty());
    assert_eq!(
        due.drain_due(1016, 10),
        ids.iter()
            .map(|&memory| FreshnessDue {
                memory,
                boundary_turn: 1016,
                from_state: FreshnessState::Fresh,
                to_state: FreshnessState::Stale,
            })
            .collect::<Vec<_>>()
    );

    // Real accepted Archive activity makes the +25 award visible without
    // saturation. It must not be reconstructed, discarded or rewritten.
    let mut parent = None;
    for turn in 33..=1030 {
        let id = format!("live-{turn}");
        cva.append_node(
            id.clone(),
            "live".into(),
            parent,
            "user".into(),
            1000 + turn as i64,
            "Legitimate post-checkpoint activity.",
        )
        .unwrap();
        parent = Some(id);
    }
    cva.sync().unwrap();
    let grown_due = cva.freshness_due.clone();
    let grown = fs::read(&path).unwrap();
    let old_projection = projection(&cva);
    let old_visible = cva.freshness_records_at(1030).unwrap();
    assert!(ids.iter().all(|id| old_visible[id].score == -1));
    drop(cva);
    let mut cva = Cva::open(&path).unwrap();
    assert_eq!(fs::read(&path).unwrap(), grown);
    assert_eq!(cva.freshness_records_at(1030).unwrap(), old_visible);
    assert!(!reconstruct(&mut cva).committed);
    let mut notices = cva.drain_freshness_notifications(2);
    notices.extend(cva.drain_freshness_notifications(2));
    assert_eq!(
        notices,
        ids.iter()
            .map(|&memory| FreshnessTransitionNotice::Crossing(FreshnessDue {
                memory,
                boundary_turn: 1016,
                from_state: FreshnessState::Fresh,
                to_state: FreshnessState::Stale,
            }))
            .collect::<Vec<_>>()
    );
    assert!(cva.drain_freshness_notifications(10).is_empty());
    assert_eq!(cva.freshness_due, grown_due);
    assert_eq!(projection(&cva), old_projection);
    assert_eq!(fs::read(&path).unwrap(), grown);

    let mut delivered = ids.clone();
    delivered.reverse();
    delivered.push(ids[0]); // One award per unique Memory in the same receipt.
    let receipt = AcceptedMemoryUseReceipt {
        owner_uuid: cva.owner_uuid().unwrap(),
        accepted_turn: 1030,
        use_id: "real-post-checkpoint-delivery".into(),
        memories: delivered,
    };
    assert_eq!(
        cva.record_accepted_memory_use(receipt.clone(), 3).unwrap(),
        3
    );
    let live_projection = projection(&cva);
    let live_visible = cva.freshness_records_at(1030).unwrap();
    assert!(ids.iter().all(|id| live_visible[id].score == 24));
    assert_eq!(cva.freshness.latest_import_settlement(), Some(&settlement));
    let complete = fs::read(&path).unwrap();
    let append_len = assert_one_frame(&grown, &complete, b"CVAFRS02");
    assert!(!reconstruct(&mut cva).committed);
    assert_eq!(
        cva.record_accepted_memory_use(receipt.clone(), 1).unwrap(),
        0
    );
    assert_eq!(fs::read(&path).unwrap(), complete);
    drop(cva);
    let crashed_path = path.with_file_name("live-use-prefix.rel");

    // The real multi-recipient receipt is one native frame. A torn receipt
    // cannot award only some recipients or become a global suppression key.
    for prefix in 0..=append_len {
        fs::write(&crashed_path, &complete[..grown.len() + prefix]).unwrap();
        let mut reopened = Cva::open(&crashed_path).unwrap();
        let full = prefix == append_len;
        assert_eq!(
            projection(&reopened),
            if full {
                &live_projection
            } else {
                &old_projection
            }
            .clone()
        );
        assert_eq!(
            reopened.freshness_records_at(1030).unwrap(),
            if full { &live_visible } else { &old_visible }.clone()
        );
        assert_eq!(
            reopened.freshness.latest_import_settlement(),
            Some(&settlement)
        );
        let recovered = fs::read(&crashed_path).unwrap();
        assert_eq!(recovered, if full { &complete } else { &grown }.clone());
        let retry = reconstruct(&mut reopened);
        assert!(!retry.committed);
        assert_eq!(retry.manifest_digest, report.manifest_digest);
        assert_eq!(fs::read(&crashed_path).unwrap(), recovered);
        assert_eq!(
            reopened
                .record_accepted_memory_use(receipt.clone(), 4)
                .unwrap(),
            if full { 0 } else { 3 }
        );
        assert_eq!(projection(&reopened), live_projection);
        assert_eq!(reopened.freshness_records_at(1030).unwrap(), live_visible);
        assert_eq!(fs::read(&crashed_path).unwrap(), complete);
        drop(reopened);
        let mut twice = Cva::open(&crashed_path).unwrap();
        assert!(!reconstruct(&mut twice).committed);
        assert_eq!(
            twice
                .record_accepted_memory_use(receipt.clone(), 2)
                .unwrap(),
            0
        );
        assert_eq!(twice.freshness_records_at(1030).unwrap(), live_visible);
        assert_eq!(fs::read(&crashed_path).unwrap(), complete);
    }
    eprintln!(
        "C6 post-checkpoint multi-recipient use: {} native byte prefixes passed",
        append_len + 1
    );
}
