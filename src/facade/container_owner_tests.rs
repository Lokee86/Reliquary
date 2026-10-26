use crate::Cva;
use reliquary_container::{Container, ContainerError};
use std::fs;
use std::path::{Path, PathBuf};

fn test_dir() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".test-tmp")
        .join(format!("container-owner-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// Applies owner-authorized repair in recovery tests after they identify the
/// accepted physical prefix. Ordinary owner open remains read-only on failure.
pub(crate) fn repair_incomplete_physical_tail(path: &Path, expected_valid_end: u64) -> u64 {
    let before = fs::read(path).unwrap();
    let mut repair = Container::open_for_repair(path).unwrap();
    let plan = repair.plan_tail_repair().unwrap().expect("incomplete tail");
    assert_eq!(plan.valid_end(), expected_valid_end);
    assert_eq!(plan.observed_len(), before.len() as u64);

    let mut cursor = repair
        .physical_cursor_through(plan.valid_end())
        .expect("bounded accepted prefix");
    while cursor.next_frame().unwrap().is_some() {}
    drop(cursor);

    repair.repair_incomplete_tail(plan).unwrap();
    drop(repair);
    assert_eq!(fs::metadata(path).unwrap().len(), expected_valid_end);
    expected_valid_end
}

pub(crate) fn assert_normal_open_requires_repair(
    path: &Path,
    open: impl FnOnce(&Path) -> Result<(), String>,
    expected_valid_end: u64,
) {
    let before = fs::read(path).unwrap();
    let error = open(path).expect_err("ordinary owner open must not repair");
    assert!(
        error.contains("incomplete") || error.contains("repair"),
        "{error}"
    );
    assert!(matches!(
        Container::open(path),
        Err(ContainerError::RepairRequired { valid_end }) if valid_end == expected_valid_end
    ));
    assert_eq!(fs::read(path).unwrap(), before);
}

#[test]
fn grouped_completion_range_keeps_owner_transaction_time_and_frontier() {
    let (_path, cva, _) =
        crate::insomnia::completion_tests::setup("container-owner-grouped-range.cva");
    let mut cva = cva;
    crate::insomnia::completion_tests::process(&mut cva, 2, 110);
    let end = std::fs::metadata(cva.container.path()).unwrap().len();
    let before_head = cva.latest_global_version();
    let before_times = (1..=before_head)
        .map(|global| cva.container.transaction_time_ns(global))
        .collect::<Vec<_>>();

    let mut cursor = cva.container.physical_cursor_through(end).unwrap();
    let mut completion_range = None;
    let mut final_frontier = 0;
    while let Some(frame) = cursor.next_frame().unwrap() {
        final_frontier = frame.latest_global;
        if let Some((start, count, time)) =
            crate::insomnia::completion::decode_embedded_version_range(&frame.payload).unwrap()
        {
            completion_range = Some((start, count, time, frame.latest_global));
        }
    }
    let (start, count, time, frontier) = completion_range.expect("grouped range");
    assert!(count > 0);
    assert_eq!(frontier, start + u64::from(count) - 1);
    assert_eq!(time, before_times[(start - 1) as usize]);
    for global in start..start + u64::from(count) {
        assert_eq!(before_times[(global - 1) as usize], time);
    }
    assert_eq!(final_frontier, before_head);
    assert_eq!(cva.latest_global_version(), before_head);
    assert_eq!(
        (1..=before_head)
            .map(|global| cva.container.transaction_time_ns(global))
            .collect::<Vec<_>>(),
        before_times
    );
}

#[test]
fn divergent_reconciliation_preserves_known_and_unknown_transaction_times() {
    let dir = test_dir();
    let left = dir.join("left.rel");
    let right = dir.join("right.rel");
    let output = dir.join("merged.rel");

    let mut base = Cva::create_project(&left).unwrap();
    base.container.set_next_transaction_time_override(Some(100));
    base.append_node(
        "base".into(),
        "base-c".into(),
        None,
        "user".into(),
        1,
        "base",
    )
    .unwrap();
    base.sync().unwrap();
    drop(base);
    fs::copy(&left, &right).unwrap();

    let mut left_cva = Cva::open(&left).unwrap();
    left_cva.container.set_next_transaction_time_override(None);
    left_cva
        .append_node(
            "left".into(),
            "left-c".into(),
            None,
            "user".into(),
            2,
            "left",
        )
        .unwrap();
    left_cva.sync().unwrap();
    drop(left_cva);

    let mut right_cva = Cva::open(&right).unwrap();
    right_cva
        .container
        .set_next_transaction_time_override(Some(300));
    right_cva
        .append_node(
            "right".into(),
            "right-c".into(),
            None,
            "user".into(),
            3,
            "right",
        )
        .unwrap();
    right_cva.sync().unwrap();
    drop(right_cva);

    Cva::reconcile(&left, &right, &output).unwrap();
    let merged = Cva::open(output).unwrap();
    let versions = merged.record_versions();
    assert_eq!(versions.len(), 3);
    assert_eq!(
        merged.transaction_time_ns(versions[0].global_version),
        Some(100)
    );
    assert_eq!(merged.transaction_time_ns(versions[1].global_version), None);
    assert_eq!(
        merged.transaction_time_ns(versions[2].global_version),
        Some(300)
    );
    assert_eq!(merged.version_at_or_before(99), None);
    assert_eq!(
        merged.version_at_or_before(100),
        Some(versions[0].global_version)
    );
    assert_eq!(
        merged.version_at_or_before(300),
        Some(versions[0].global_version)
    );
}

#[test]
fn semantic_replay_reports_earlier_owner_error_before_later_clock_corruption() {
    let path = test_dir().join("ordered-errors.rel");
    drop(Cva::create(&path).unwrap());
    let mut writer = Container::open(&path).unwrap();
    writer.append(b"CVACONT1").unwrap(); // complete malformed Archive record
    writer.append(b"CVAVERS2").unwrap(); // later complete malformed clock record
    writer.sync().unwrap();
    drop(writer);
    let before = fs::read(&path).unwrap();
    assert!(matches!(
        Container::open(&path),
        Err(ContainerError::InvalidVersionRecord)
    ));
    assert!(matches!(Cva::open(&path), Err(crate::CvaError::Archive(_))));
    assert_eq!(fs::read(&path).unwrap(), before);
}

#[test]
fn ordinary_rel_open_projects_superseded_compactions_without_reclaiming_bytes() {
    let path = test_dir().join("compaction-projection.rel");
    let mut rel = Cva::create_project(&path).unwrap();
    rel.put_conversation_compaction(
        "conversation".into(),
        "message-1".into(),
        "old summary".into(),
        None,
    )
    .unwrap();
    drop(rel);

    // A completed superseding record without its later cleanup is a valid crash prefix.
    let record = crate::ConversationCompaction {
        conversation_id: "conversation".into(),
        through_message_id: "message-2".into(),
        summary: "new summary".into(),
        generation: 2,
    };
    let payload =
        crate::conversation_compaction_codec::encode_live(&record, Some("message-1")).unwrap();
    let mut writer = Container::open(&path).unwrap();
    writer.append(&payload).unwrap();
    writer.sync().unwrap();
    drop(writer);

    let bytes = fs::read(&path).unwrap();
    let rel = Cva::open(&path).unwrap();
    assert_eq!(rel.conversation_compactions("conversation"), vec![record]);
    assert_eq!(rel.latest_global_version(), 0);
    assert_eq!(fs::read(&path).unwrap(), bytes);
    drop(rel);
    assert_eq!(fs::read(&path).unwrap(), bytes);
}

fn pending_retired_entity(retired: crate::EntityId) -> crate::MemoryEntityResolutionStatus {
    crate::MemoryEntityResolutionStatus::Pending(crate::EntityResolutionPending {
        candidate_entity_ids: vec![retired],
        reason: crate::EntityResolutionReason::Ambiguous,
        candidate_set_fingerprint: [1; 32],
        context_fingerprint: [2; 32],
        first_seen_at_ns: 10,
        last_attempt_at_ns: 10,
        attempt_count: 1,
    })
}

#[test]
fn ordinary_rel_open_does_not_rewrite_retired_entity_resolution() {
    use crate::entity_candidate_test_support::{
        entity_draft, install_rel_mention, publish_rel_memory,
    };
    use crate::entity_resolution_codec::DecodedEntityResolution;

    let path = test_dir().join("entity-resolution-read-only.rel");
    let mut rel = Cva::create_project(&path).unwrap();
    let survivor = rel
        .publish_entity(None, 0, entity_draft("Survivor", &[], "survivor", 1))
        .unwrap()
        .0;
    let retired = rel
        .publish_entity(None, 0, entity_draft("Retired", &[], "retired", 1))
        .unwrap()
        .0;
    let memory = publish_rel_memory(&mut rel, "retired-mention", "Subject", "Retired is here");
    let key = install_rel_mention(&mut rel, memory, "Retired");
    rel.merge_entities(survivor.id, retired.id, 20).unwrap();

    rel.entity_resolutions
        .persist_change(
            &mut rel.container,
            DecodedEntityResolution {
                key,
                revision: 1,
                updated_at_ns: 30,
                status: Some(pending_retired_entity(retired.id)),
            },
        )
        .unwrap();
    rel.sync().unwrap();
    drop(rel);

    let before = fs::read(&path).unwrap();
    let mut reopened = Cva::open(&path).unwrap();
    assert_eq!(fs::read(&path).unwrap(), before);
    assert_eq!(reopened.entity_resolution(key).unwrap().revision, 1);
    assert_eq!(reopened.repair_retired_entity_references().unwrap(), 1);
    assert_ne!(fs::read(&path).unwrap(), before);
    assert_eq!(reopened.entity_resolution(key).unwrap().revision, 2);
    let repaired_bytes = fs::read(&path).unwrap();
    drop(reopened);

    let reopened = Cva::open(&path).unwrap();
    assert_eq!(fs::read(&path).unwrap(), repaired_bytes);
    assert_eq!(reopened.entity_resolution(key).unwrap().revision, 2);
}

#[test]
fn ordinary_phy_open_does_not_rewrite_retired_entity_resolution() {
    use crate::Phylactery;
    use crate::entity_candidate_test_support::{
        entity_draft, install_phy_mention, publish_phy_memory,
    };
    use crate::entity_resolution_codec::DecodedEntityResolution;

    let path = test_dir().join("entity-resolution-read-only.phy");
    let mut phy = Phylactery::create(&path).unwrap();
    let survivor = phy
        .publish_entity(None, 0, entity_draft("Survivor", &[], "survivor", 1))
        .unwrap()
        .0;
    let retired = phy
        .publish_entity(None, 0, entity_draft("Retired", &[], "retired", 1))
        .unwrap()
        .0;
    let memory = publish_phy_memory(&mut phy, "Retired is here");
    let key = install_phy_mention(&mut phy, memory, "Retired");
    phy.merge_entities(survivor.id, retired.id, 20).unwrap();

    phy.entity_resolutions
        .persist_change(
            &mut phy.container,
            DecodedEntityResolution {
                key,
                revision: 1,
                updated_at_ns: 30,
                status: Some(pending_retired_entity(retired.id)),
            },
        )
        .unwrap();
    phy.sync().unwrap();
    drop(phy);

    let before = fs::read(&path).unwrap();
    let mut reopened = Phylactery::open(&path).unwrap();
    assert_eq!(fs::read(&path).unwrap(), before);
    assert_eq!(reopened.entity_resolution(key).unwrap().revision, 1);
    assert_eq!(reopened.repair_retired_entity_references().unwrap(), 1);
    assert_ne!(fs::read(&path).unwrap(), before);
    let repaired_bytes = fs::read(&path).unwrap();
    drop(reopened);

    let reopened = Phylactery::open(&path).unwrap();
    assert_eq!(fs::read(&path).unwrap(), repaired_bytes);
    assert_eq!(reopened.entity_resolution(key).unwrap().revision, 2);
}
