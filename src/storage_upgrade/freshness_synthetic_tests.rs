use super::{StorageUpgradeRequest, plan_storage_upgrade, stage_graph_storage_upgrade};
use crate::freshness_storage::FreshnessReplayEntry;
use crate::historical_freshness_fingerprint::{freshness_at_event, freshness_at_turn};
use crate::{Cva, FreshnessEventCut, FreshnessEventProof, MemoryDraft, MemoryId};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!("freshness-synthetic-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn draft(title: &str, mutation_id: &str) -> MemoryDraft {
    MemoryDraft {
        category: "fact".into(),
        memory_type: "project".into(),
        authority_kind: "direct".into(),
        temporal_status: "current".into(),
        title: title.into(),
        content: format!("A real legacy Memory record anchors {title} usage."),
        scope: "project".into(),
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
        source_time_ns: Some(17),
        mutation_id: mutation_id.into(),
        created_at_ns: 17,
        updated_at_ns: 17,
    }
}

fn downgrade_header_to_v1(path: &Path) {
    let mut bytes = fs::read(path).unwrap();
    bytes[8..10].copy_from_slice(&1_u16.to_le_bytes());
    bytes[10..12].copy_from_slice(&0_u16.to_le_bytes());
    fs::write(path, bytes).unwrap();
}

fn replay_entries(path: &Path, v1: bool) -> (Vec<FreshnessReplayEntry>, u64) {
    let owner = if v1 {
        Cva::open_v1_read_only(path).unwrap()
    } else {
        Cva::open(path).unwrap()
    };
    (
        owner.freshness.retained_history_entries().unwrap(),
        owner.latest_global_version(),
    )
}

fn event_rows(
    entries: &[FreshnessReplayEntry],
) -> Vec<(String, u64, BTreeMap<MemoryId, i16>, FreshnessEventProof)> {
    entries
        .iter()
        .filter_map(|entry| match entry {
            FreshnessReplayEntry::Event {
                event_id,
                source_turn,
                effects,
                proof,
                ..
            } => Some((event_id.clone(), *source_turn, effects.clone(), *proof)),
            _ => None,
        })
        .collect()
}

fn append_accepted_use_batch(owner: &mut Cva, use_id: &str, memory: MemoryId) {
    let payload = owner
        .freshness
        .prepare_accepted_use_batch(
            owner.owner_uuid().unwrap(),
            use_id.to_owned(),
            owner.rel_turn_count(),
            vec![memory],
            FreshnessEventProof {
                graph_version: owner.memory_graph_version(),
                community_generation: None,
                community_graph_version: None,
            },
            vec![BTreeMap::from([(
                memory,
                owner.freshness_policy().access_principal,
            )])],
        )
        .unwrap();
    owner.container.append(&payload).unwrap();
    owner.container.sync().unwrap();
    owner.freshness.ingest_appended(&payload).unwrap();
}

fn append_legacy_activity_turn(owner: &mut Cva, turn: u64) {
    let content = "Accepted activity advances the legacy baseline to a measurable score.";
    let content_id = crate::archive_store::hash_content(content.as_bytes());
    let node = crate::Node {
        id: format!("synthetic-freshness-turn-{turn}"),
        conversation_id: "synthetic-freshness".into(),
        parent_id: None,
        role: if turn % 2 == 0 { "user" } else { "assistant" }.into(),
        principal_id: None,
        timestamp_ns: turn as i64,
        content_id,
    };
    let prepared = owner
        .archive
        .activity
        .prepare_node(&mut owner.archive.nodes, &node)
        .unwrap()
        .unwrap();
    owner
        .archive
        .put_content(&mut owner.container, content_id, content)
        .unwrap();
    let object = owner
        .container
        .append(&crate::archive_codec::encode_node(&node).unwrap())
        .unwrap();
    let version = owner
        .archive
        .publish_record(&mut owner.container, object)
        .unwrap();
    owner.archive.nodes.insert(node).unwrap();
    owner
        .archive
        .activity
        .commit_new(prepared, version.archive_version);
}

#[test]
fn graph_upgrade_preserves_nonempty_legacy_accepted_use_events_and_source() {
    let fixture = Fixture::new();
    let source_path = fixture.0.join("legacy-with-events.rel");
    let destination = fixture.0.join("staged-not-published.rel");

    let memory_ids = {
        let mut owner = Cva::create(&source_path).unwrap();
        let (first, first_changed) = owner
            .memories
            .publish_legacy_fixture(
                &mut owner.container,
                None,
                0,
                draft("First synthetic legacy source", "synthetic-legacy-memory-a"),
            )
            .unwrap();
        assert!(first_changed);
        let (second, second_changed) = owner
            .memories
            .publish_legacy_fixture(
                &mut owner.container,
                None,
                0,
                draft(
                    "Second synthetic legacy source",
                    "synthetic-legacy-memory-b",
                ),
            )
            .unwrap();
        assert!(second_changed);

        // Enrollment is explicit for legacy Memories. Both accepted-use
        // batches then write an event effect and receipt at the same turn and
        // empty Graph proof cut, without using compact Memory or Graph carriers.
        assert!(owner.enroll_legacy_freshness_baseline(first.id).unwrap());
        assert!(owner.enroll_legacy_freshness_baseline(second.id).unwrap());
        // One default decay interval takes a baseline from 100 to 99, leaving
        // a measurable accepted-use postimage without a large activity fixture.
        for turn in 0..10 {
            append_legacy_activity_turn(&mut owner, turn);
        }
        append_accepted_use_batch(&mut owner, "synthetic-use-a", first.id);
        append_accepted_use_batch(&mut owner, "synthetic-use-b", second.id);
        owner.sync().unwrap();
        [first.id, second.id]
    };
    downgrade_header_to_v1(&source_path);

    let source_bytes = fs::read(&source_path).unwrap();
    let source_digest = Sha256::digest(&source_bytes);
    let source_mtime = fs::metadata(&source_path).unwrap().modified().unwrap();
    let (before_entries, before_global_head) = replay_entries(&source_path, true);
    assert!(before_global_head > 0);
    let before_events = event_rows(&before_entries);
    assert_eq!(
        before_events.len(),
        2,
        "both accepted-use events are durable"
    );
    assert!(before_events.iter().all(|(_, turn, effects, proof)| {
        *turn == 10
            && effects.len() == 1
            && effects.values().all(|effect| *effect > 0)
            && proof.graph_version == 0
            && proof.community_generation.is_none()
            && proof.community_graph_version.is_none()
    }));
    let mut effected_memories = before_events
        .iter()
        .flat_map(|event| event.2.keys().copied())
        .collect::<Vec<_>>();
    let mut expected_memories = memory_ids.to_vec();
    effected_memories.sort();
    expected_memories.sort();
    assert_eq!(effected_memories, expected_memories);
    assert_ne!(before_events[0].0, before_events[1].0);
    for use_id in ["synthetic-use-a", "synthetic-use-b"] {
        assert!(before_entries.iter().any(|entry| matches!(entry,
            FreshnessReplayEntry::AcceptedUseReceipt { use_id: found, source_turn: 10, .. }
                if found == use_id
        )));
    }

    let plan = plan_storage_upgrade(StorageUpgradeRequest {
        source: source_path.clone(),
        destination: destination.clone(),
    })
    .unwrap();
    let staged = stage_graph_storage_upgrade(&plan).unwrap();
    let staged_path = staged.test_path();
    let (after_entries, after_global_head) = replay_entries(staged_path, false);
    assert_eq!(
        after_entries, before_entries,
        "accepted-use event identities/order"
    );
    assert_eq!(
        event_rows(&after_entries),
        before_events,
        "event effects and proofs"
    );
    assert_eq!(after_global_head, before_global_head, "global clock head");

    let source = Cva::open_v1_read_only(&source_path).unwrap();
    let staged_owner = Cva::open(staged_path).unwrap();
    let activity_source = freshness_at_turn(&source, 10).unwrap();
    let activity_staged = freshness_at_turn(&staged_owner, 10).unwrap();
    assert_eq!(activity_source.fingerprint, activity_staged.fingerprint);
    assert_eq!(activity_source.records, activity_staged.records);
    let baseline = freshness_at_turn(&source, 9).unwrap();
    let mut sorted_event_ids = before_events
        .iter()
        .map(|event| event.0.clone())
        .collect::<Vec<_>>();
    sorted_event_ids.sort();
    let first_event = before_events
        .iter()
        .find(|event| event.0 == sorted_event_ids[0])
        .unwrap();
    let first_effected_memory = *first_event.2.keys().next().unwrap();
    for (event_id_inclusive, expected_count) in [(Some(sorted_event_ids[0].clone()), 1), (None, 2)]
    {
        let cut = FreshnessEventCut {
            rel_turn: 10,
            event_id_inclusive,
            graph_version: 0,
        };
        let source_cut = freshness_at_event(&source, &cut).unwrap();
        let staged_cut = freshness_at_event(&staged_owner, &cut).unwrap();
        assert_eq!(source_cut.fingerprint, staged_cut.fingerprint);
        assert_eq!(source_cut.records, staged_cut.records);
        assert_eq!(source_cut.records.len(), memory_ids.len());
        for memory_id in memory_ids {
            let base_score = baseline.records[&memory_id].score_at(10).unwrap();
            let cut_score = source_cut.records[&memory_id].score_at(10).unwrap();
            if expected_count == 1 && memory_id != first_effected_memory {
                assert_eq!(
                    cut_score, base_score,
                    "unselected same-turn event is excluded"
                );
            } else {
                assert!(
                    cut_score > base_score,
                    "accepted event effect has a postimage"
                );
            }
        }
    }

    assert_eq!(fs::read(&source_path).unwrap(), source_bytes);
    assert_eq!(
        Sha256::digest(fs::read(&source_path).unwrap()),
        source_digest
    );
    assert_eq!(
        fs::metadata(&source_path).unwrap().modified().unwrap(),
        source_mtime
    );
    assert!(!destination.exists());
    drop(staged);
    assert!(!destination.exists());
}
