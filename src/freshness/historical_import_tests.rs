use super::*;
use crate::freshness::historical_dream::HistoricalDreamPlan;
use crate::freshness::historical_planner::{MemoryPlan, SourceKey, TimelineEntry};
use crate::freshness::historical_semantic::HistoricalSemanticPlan;

fn id(n: u8) -> MemoryId {
    MemoryId([n; 32])
}
fn input() -> HistoricalImportInput {
    let mut record = FreshnessRecord::created();
    record.admit(1).unwrap();
    let timeline = HistoricalTimeline {
        entries: vec![TimelineEntry {
            key: SourceKey {
                conversation_id: "c".into(),
                node_id: "u".into(),
            },
            parent_id: None,
            timestamp_ns: 1,
            activity: true,
            position: Some(1),
        }],
        activity_order: vec![SourceKey {
            conversation_id: "c".into(),
            node_id: "u".into(),
        }],
    };
    HistoricalImportInput {
        import_id: [1; 32],
        manifest_digest: [2; 32],
        as_of_position: 1,
        timeline,
        provenance: HistoricalProvenancePlan {
            memories: vec![MemoryPlan {
                memory_id: "m".into(),
                availability: MemoryAvailability::Supported { position: 1 },
                cohort_id: "m".into(),
            }],
            coverage: BTreeMap::new(),
        },
        semantic_plan: HistoricalSemanticPlan {
            events: Vec::new(),
            coverage: BTreeMap::new(),
        },
        dream_plan: HistoricalDreamPlan {
            events: Vec::new(),
            coverage: BTreeMap::new(),
        },
        initial_records: [(id(1), record)].into_iter().collect(),
        adjacency: MemoryAdjacency::new(),
        memberships: CommunityMembership::new(),
        authorized: [id(1)].into_iter().collect(),
        availability: [(id(1), MemoryAvailability::Supported { position: 1 })]
            .into_iter()
            .collect(),
        workers: 1,
    }
}

#[test]
fn one_shot_import_publishes_complete_projection_without_event_queue() {
    let mut store = HistoricalCheckpointStore::default();
    let result =
        execute_historical_import(input(), &mut store, &FreshnessPolicy::default_v1()).unwrap();
    assert_eq!(result.commit, HistoricalCheckpointCommit::Published);
    assert_eq!(result.propagated_events, 0);
    assert_eq!(result.checkpoint.position, 1);
    assert!(result.checkpoint.applied_event_ids.is_empty());
}

#[test]
fn repeated_one_shot_import_reuses_checkpoint_identity() {
    let mut store = HistoricalCheckpointStore::default();
    let first =
        execute_historical_import(input(), &mut store, &FreshnessPolicy::default_v1()).unwrap();
    let second =
        execute_historical_import(input(), &mut store, &FreshnessPolicy::default_v1()).unwrap();
    assert_eq!(first.checkpoint, second.checkpoint);
    assert_eq!(second.commit, HistoricalCheckpointCommit::AlreadyCommitted);
}
