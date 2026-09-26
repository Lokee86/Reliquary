use crate::dream_candidate_test_support::{memory_with_source_time, test_path};
use crate::dream_cooldown::dream_epoch;
use crate::{Cva, DEFAULT_DREAM_REPROCESS_COOLDOWN_NS, MemoryDraft, Phylactery};
use std::fs;

const DAY_NS: i64 = 24 * 60 * 60 * 1_000_000_000;

#[test]
fn provenance_age_selects_one_current_epoch_without_backlog() {
    assert_eq!(DEFAULT_DREAM_REPROCESS_COOLDOWN_NS, 30 * DAY_NS);
    assert_eq!(dream_epoch(10 * DAY_NS, 9 * DAY_NS), 0);
    assert_eq!(dream_epoch(0, 45 * DAY_NS), 1);
    assert_eq!(dream_epoch(0, 60 * DAY_NS), 2);
    assert_eq!(dream_epoch(0, 95 * DAY_NS), 3);
}

#[test]
fn reliquary_cooldown_uses_authoritative_archive_source_time() {
    let path = test_path("dream-cooldown-authoritative-source.cva");
    let mut cva = Cva::create(path).unwrap();
    let id = memory_with_source_time(
        &mut cva,
        "authoritative-source",
        "Authoritative source",
        "Historical knowledge",
        0,
        40 * DAY_NS,
        45 * DAY_NS,
    );

    assert_eq!(cva.dream_eligible_epoch(id, 59 * DAY_NS).unwrap(), None);
    assert_eq!(cva.dream_eligible_epoch(id, 60 * DAY_NS).unwrap(), Some(2));
}

#[test]
fn initial_processing_satisfies_the_current_provenance_epoch() {
    let path = test_path("dream-cooldown-initial.cva");
    let mut cva = Cva::create(&path).unwrap();
    let id = memory_with_persisted_source_time(
        &mut cva,
        "old-import",
        "Imported memory",
        "Historical knowledge",
        0,
        45 * DAY_NS,
    );
    make_extracted(&mut cva, id);

    let epoch = cva.dream_eligible_epoch(id, 45 * DAY_NS).unwrap().unwrap();
    assert_eq!(epoch, 1);
    cva.reconcile_dream_lifecycle(id).unwrap();
    assert!(cva.mark_dream_processed(id, epoch, 45 * DAY_NS).unwrap());

    assert_eq!(cva.dream_eligible_epoch(id, 45 * DAY_NS).unwrap(), None);
    assert_eq!(cva.dream_eligible_epoch(id, 59 * DAY_NS).unwrap(), None);
    assert_eq!(cva.dream_eligible_epoch(id, 60 * DAY_NS).unwrap(), Some(2));
}

#[test]
fn cooldown_marker_survives_reopen_and_skips_missed_epochs() {
    let path = test_path("dream-cooldown-persistence.cva");
    let mut cva = Cva::create(&path).unwrap();
    let id = memory_with_persisted_source_time(
        &mut cva,
        "persistent",
        "Persistent memory",
        "Historical knowledge",
        0,
        45 * DAY_NS,
    );
    let memory_version = cva.memory_version();
    let memory_revision = cva.memory(id).unwrap().revision;
    assert!(cva.mark_dream_processed(id, 1, 45 * DAY_NS).unwrap());
    assert_eq!(cva.memory_version(), memory_version);
    assert_eq!(cva.memory(id).unwrap().revision, memory_revision);
    cva.sync().unwrap();
    drop(cva);

    let mut cva = Cva::open(&path).unwrap();
    assert_eq!(cva.dream_eligible_epoch(id, 45 * DAY_NS).unwrap(), None);
    assert_eq!(cva.dream_eligible_epoch(id, 95 * DAY_NS).unwrap(), Some(3));
    assert!(cva.mark_dream_processed(id, 3, 95 * DAY_NS).unwrap());
    assert_eq!(cva.memory(id).unwrap().revision, memory_revision);
    assert_eq!(cva.dream_eligible_epoch(id, 95 * DAY_NS).unwrap(), None);
}

#[test]
fn phylactery_cooldown_state_survives_reopen() {
    let path = test_path("dream-cooldown.phy");
    let mut phy = Phylactery::create(&path).unwrap();
    let draft = MemoryDraft {
        category: "preference".into(),
        memory_type: "user".into(),
        authority_kind: "direct".into(),
        temporal_status: "unknown".into(),
        title: "Preference".into(),
        content: "Use concise output.".into(),
        scope: "private".into(),
        lifecycle_state: "knowledge".into(),
        archived: false,
        superseded_by: None,
        parent_id: None,
        source_node_id: None,
        content_source_conversation_id: None,
        content_source_node_id: None,
        grounding_source_conversation_id: None,
        grounding_source_node_id: None,
        source_episode_id: None,
        source_time_ns: Some(0),
        mutation_id: "phy-cooldown".into(),
        created_at_ns: 45 * DAY_NS,
        updated_at_ns: 45 * DAY_NS,
    };
    let id = phy.publish_memory(None, 0, draft).unwrap().0.id;
    assert!(phy.mark_dream_processed(id, 1, 45 * DAY_NS).unwrap());
    phy.sync().unwrap();
    drop(phy);

    let mut phy = Phylactery::open(path).unwrap();
    assert_eq!(phy.dream_eligible_epoch(id, 59 * DAY_NS).unwrap(), None);
    assert_eq!(phy.dream_eligible_epoch(id, 60 * DAY_NS).unwrap(), Some(2));
}

#[test]
fn divergent_reconcile_keeps_highest_epoch_and_latest_processing_time() {
    let left = test_path("dream-cooldown-left.cva");
    let right = left.with_file_name("dream-cooldown-right.cva");
    let output = left.with_file_name("dream-cooldown-merged.cva");
    let mut base = Cva::create_project(&left).unwrap();
    let id = memory_with_persisted_source_time(
        &mut base,
        "reconcile",
        "Reconciled memory",
        "Shared knowledge",
        0,
        45 * DAY_NS,
    );
    base.sync().unwrap();
    drop(base);
    fs::copy(&left, &right).unwrap();

    for (path, epoch, processed_at_ns, node) in [
        (&left, 3, 45 * DAY_NS, "left-node"),
        (&right, 1, 95 * DAY_NS, "right-node"),
    ] {
        let mut cva = Cva::open(path).unwrap();
        cva.mark_dream_processed(id, epoch, processed_at_ns)
            .unwrap();
        cva.append_node(
            node.into(),
            format!("{node}-conversation"),
            None,
            "user".into(),
            46 * DAY_NS,
            node,
        )
        .unwrap();
        cva.sync().unwrap();
    }

    Cva::reconcile(&left, &right, &output).unwrap();
    let mut merged = Cva::open(output).unwrap();
    assert_eq!(
        merged
            .dream_cooldown_records()
            .into_iter()
            .find(|(memory_id, _)| *memory_id == id)
            .map(|(_, state)| (state.epoch, state.processed_at_ns)),
        Some((3, Some(95 * DAY_NS)))
    );
    assert_eq!(merged.dream_eligible_epoch(id, 95 * DAY_NS).unwrap(), None);
    assert_eq!(
        merged.dream_eligible_epoch(id, 120 * DAY_NS).unwrap(),
        Some(4)
    );
}

#[test]
fn provenance_less_memory_uses_updated_time_then_last_dream_time_as_fallback() {
    let path = test_path("dream-cooldown-no-provenance.cva");
    let mut cva = Cva::create(path).unwrap();
    let draft = MemoryDraft {
        category: "project".into(),
        memory_type: "fact".into(),
        authority_kind: "direct".into(),
        temporal_status: "unknown".into(),
        title: "No provenance".into(),
        content: "This Memory has no recoverable source provenance.".into(),
        scope: "private".into(),
        lifecycle_state: "knowledge".into(),
        archived: false,
        superseded_by: None,
        parent_id: None,
        source_node_id: None,
        content_source_conversation_id: None,
        content_source_node_id: None,
        grounding_source_conversation_id: None,
        grounding_source_node_id: None,
        source_episode_id: None,
        source_time_ns: None,
        mutation_id: "no-provenance".into(),
        created_at_ns: 10 * DAY_NS,
        updated_at_ns: 10 * DAY_NS,
    };
    let id = cva.publish_memory(None, 0, draft).unwrap().0.id;

    assert_eq!(cva.dream_eligible_epoch(id, 39 * DAY_NS).unwrap(), None);
    assert_eq!(cva.dream_eligible_epoch(id, 40 * DAY_NS).unwrap(), Some(0));
    assert!(cva.mark_dream_processed(id, 0, 50 * DAY_NS).unwrap());
    assert_eq!(cva.dream_eligible_epoch(id, 79 * DAY_NS).unwrap(), None);
    assert_eq!(cva.dream_eligible_epoch(id, 80 * DAY_NS).unwrap(), Some(0));
    assert!(cva.mark_dream_processed(id, 0, 80 * DAY_NS).unwrap());
    assert_eq!(cva.dream_eligible_epoch(id, 109 * DAY_NS).unwrap(), None);
    assert_eq!(cva.dream_eligible_epoch(id, 110 * DAY_NS).unwrap(), Some(0));
}

#[test]
fn dream_pair_history_survives_reopen() {
    let path = test_path("dream-pair-history.cva");
    let mut cva = Cva::create(&path).unwrap();
    let left = memory_with_source_time(&mut cva, "pair-left", "Left", "Left knowledge", 10, 10, 10);
    let right = memory_with_source_time(
        &mut cva,
        "pair-right",
        "Right",
        "Right knowledge",
        20,
        20,
        20,
    );
    assert!(cva.mark_dream_pair_evaluated(left, right).unwrap());
    cva.sync().unwrap();
    drop(cva);

    let cva = Cva::open(path).unwrap();
    assert!(
        cva.dream_pair_records()
            .iter()
            .any(|(a, b)| { (*a == left && *b == right) || (*a == right && *b == left) })
    );
}

#[test]
fn divergent_reconcile_unions_dream_pair_history() {
    let left_path = test_path("dream-pair-left.cva");
    let right_path = left_path.with_file_name("dream-pair-right.cva");
    let output = left_path.with_file_name("dream-pair-merged.cva");
    let mut base = Cva::create_project(&left_path).unwrap();
    let a = memory_with_source_time(&mut base, "pair-a", "A", "A", 10, 10, 10);
    let b = memory_with_source_time(&mut base, "pair-b", "B", "B", 20, 20, 20);
    let c = memory_with_source_time(&mut base, "pair-c", "C", "C", 30, 30, 30);
    base.sync().unwrap();
    drop(base);
    fs::copy(&left_path, &right_path).unwrap();

    let mut left = Cva::open(&left_path).unwrap();
    left.mark_dream_pair_evaluated(a, b).unwrap();
    left.sync().unwrap();
    drop(left);

    let mut right = Cva::open(&right_path).unwrap();
    right.mark_dream_pair_evaluated(a, c).unwrap();
    right.sync().unwrap();
    drop(right);

    Cva::reconcile(&left_path, &right_path, &output).unwrap();
    let merged = Cva::open(output).unwrap();
    assert_eq!(merged.dream_pair_records().len(), 2);
}

#[test]
fn identical_maintenance_state_does_not_defeat_reconciliation_noop() {
    let left = test_path("dream-maintenance-noop-left.cva");
    let right = left.with_file_name("dream-maintenance-noop-right.cva");
    let output = left.with_file_name("dream-maintenance-noop-merged.cva");
    let mut base = Cva::create_project(&left).unwrap();
    let a = memory_with_persisted_source_time(&mut base, "noop-a", "A", "A", 0, DAY_NS);
    let b = memory_with_persisted_source_time(&mut base, "noop-b", "B", "B", 0, DAY_NS);
    base.sync().unwrap();
    drop(base);
    fs::copy(&left, &right).unwrap();

    let mut left_cva = Cva::open(&left).unwrap();
    left_cva.mark_dream_processed(a, 1, 45 * DAY_NS).unwrap();
    left_cva.mark_dream_pair_evaluated(a, b).unwrap();
    left_cva
        .append_node(
            "noop-node".into(),
            "noop-conversation".into(),
            None,
            "user".into(),
            50 * DAY_NS,
            "same semantic tail",
        )
        .unwrap();
    left_cva.sync().unwrap();
    drop(left_cva);

    let mut right_cva = Cva::open(&right).unwrap();
    right_cva
        .append_node(
            "noop-node".into(),
            "noop-conversation".into(),
            None,
            "user".into(),
            50 * DAY_NS,
            "same semantic tail",
        )
        .unwrap();
    right_cva.mark_dream_pair_evaluated(a, b).unwrap();
    right_cva.mark_dream_processed(a, 1, 45 * DAY_NS).unwrap();
    right_cva.sync().unwrap();
    drop(right_cva);

    assert_eq!(
        Cva::compare(&left, &right).unwrap().relation,
        crate::CvaRelation::Diverged
    );
    let result = Cva::reconcile(&left, &right, &output).unwrap();
    assert!(!result.canonical_change_required);
    assert_eq!(fs::read(&output).unwrap(), fs::read(&left).unwrap());
}

#[test]
fn legacy_processed_memory_uses_updated_time_as_non_flooding_baseline() {
    let path = test_path("dream-cooldown-legacy.cva");
    let mut cva = Cva::create(path).unwrap();
    let id = memory_with_persisted_source_time(
        &mut cva,
        "legacy",
        "Legacy memory",
        "Already processed before cooldown markers existed",
        0,
        45 * DAY_NS,
    );

    assert_eq!(cva.dream_eligible_epoch(id, 45 * DAY_NS).unwrap(), None);
    assert_eq!(cva.dream_eligible_epoch(id, 59 * DAY_NS).unwrap(), None);
    assert_eq!(cva.dream_eligible_epoch(id, 60 * DAY_NS).unwrap(), Some(2));
}

fn memory_with_persisted_source_time(
    cva: &mut Cva,
    mutation_id: &str,
    title: &str,
    content: &str,
    source_time_ns: i64,
    updated_at_ns: i64,
) -> crate::MemoryId {
    let draft = MemoryDraft {
        category: "project".into(),
        memory_type: "fact".into(),
        authority_kind: "direct".into(),
        temporal_status: "unknown".into(),
        title: title.into(),
        content: content.into(),
        scope: "private".into(),
        lifecycle_state: "knowledge".into(),
        archived: false,
        superseded_by: None,
        parent_id: None,
        source_node_id: None,
        content_source_conversation_id: None,
        content_source_node_id: None,
        grounding_source_conversation_id: None,
        grounding_source_node_id: None,
        source_episode_id: None,
        source_time_ns: Some(source_time_ns),
        mutation_id: mutation_id.into(),
        created_at_ns: updated_at_ns,
        updated_at_ns,
    };
    cva.publish_memory(None, 0, draft).unwrap().0.id
}

fn make_extracted(cva: &mut Cva, id: crate::MemoryId) {
    let memory = cva.memory(id).unwrap();
    let draft = MemoryDraft {
        category: memory.category.clone(),
        memory_type: memory.memory_type.clone(),
        authority_kind: memory.authority_kind.clone(),
        temporal_status: memory.temporal_status.clone(),
        title: memory.title.clone(),
        content: memory.content.clone(),
        scope: memory.scope.clone(),
        lifecycle_state: "extracted".into(),
        archived: false,
        superseded_by: memory.superseded_by,
        parent_id: memory.parent_id,
        source_node_id: memory.source_node_id.clone(),
        content_source_conversation_id: memory.content_source_conversation_id.clone(),
        content_source_node_id: memory.content_source_node_id.clone(),
        grounding_source_conversation_id: memory.grounding_source_conversation_id.clone(),
        grounding_source_node_id: memory.grounding_source_node_id.clone(),
        source_episode_id: memory.source_episode_id,
        source_time_ns: memory.source_time_ns,
        mutation_id: "dream-cooldown-extracted".into(),
        created_at_ns: memory.created_at_ns,
        updated_at_ns: memory.updated_at_ns.saturating_add(1),
    };
    cva.publish_memory(Some(id), memory.revision, draft)
        .unwrap();
}
