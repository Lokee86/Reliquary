use super::{AgenticMemoryController, StagedAgenticMemoryOperation};
use crate::agentic_memory_retrieval_model::{
    AgenticMemoryOperation, AgenticMemoryOutcome, AgenticMemoryPayload, PreparedAgenticMemoryResult,
};
use crate::{MemoryId, MemoryRef};
use std::collections::{BTreeMap, BTreeSet};

fn memory(owner: &str, byte: u8) -> MemoryRef {
    MemoryRef {
        owner_id: owner.to_owned(),
        memory_id: MemoryId([byte; 32]),
    }
}

fn staged_query(controller: &AgenticMemoryController, query: &str) -> StagedAgenticMemoryOperation {
    controller.stage_query(query).expect("valid query")
}

#[test]
fn owner_qualified_refs_prevent_memory_id_collisions_and_sort_deterministically() {
    let same_id_a = memory("owner-a", 7);
    let same_id_b = memory("owner-b", 7);
    let mut ids = BTreeSet::from([same_id_b.clone(), same_id_a.clone()]);
    assert_eq!(ids.iter().collect::<Vec<_>>(), vec![&same_id_a, &same_id_b]);

    let mut controller = AgenticMemoryController::new();
    let mut operation = staged_query(&controller, "query");
    operation
        .record_evaluated([same_id_b.clone(), same_id_a.clone()])
        .unwrap();
    operation.commit(&mut controller).expect("commit");
    assert_eq!(
        controller.searched_ids().iter().collect::<Vec<_>>(),
        vec![&same_id_a, &same_id_b]
    );
    ids.clear();
}

#[test]
fn new_query_resets_searched_but_preserves_turn_selected_ids() {
    let searched = memory("owner", 1);
    let selected = memory("owner", 2);
    let mut controller = AgenticMemoryController {
        current_query: Some("old".into()),
        searched_ids: BTreeSet::from([searched]),
        selected_ids: BTreeSet::from([selected.clone()]),
    };
    let operation = staged_query(&controller, " new ");
    operation.commit(&mut controller).expect("commit");
    assert_eq!(controller.current_query(), Some("new"));
    assert!(controller.searched_ids().is_empty());
    assert_eq!(controller.selected_ids(), &BTreeSet::from([selected]));
}

#[test]
fn refinement_requires_active_query_and_preserves_both_sets() {
    let selected = memory("owner", 3);
    let searched = memory("owner", 4);
    let mut controller = AgenticMemoryController {
        current_query: Some("first".into()),
        searched_ids: BTreeSet::from([searched.clone()]),
        selected_ids: BTreeSet::from([selected.clone()]),
    };
    let mut refinement = controller
        .stage_refinement(" second ")
        .expect("active query");
    assert!(refinement.record_evaluated([memory("owner", 5)]).is_err());
    refinement.record_evaluated([searched.clone()]).unwrap();
    refinement.commit(&mut controller).expect("commit");
    assert_eq!(controller.current_query(), Some("second"));
    assert!(controller.searched_ids().contains(&searched));
    assert_eq!(controller.searched_ids(), &BTreeSet::from([searched]));
    assert_eq!(controller.selected_ids(), &BTreeSet::from([selected]));
    assert!(matches!(
        AgenticMemoryController::new().stage_refinement("x"),
        Err(crate::agentic_memory_retrieval_model::AgenticMemoryError::NoActiveQuery)
    ));
}

#[test]
fn traversal_commits_atomic_union_only_after_all_evaluation() {
    let mut controller = AgenticMemoryController::new();
    staged_query(&controller, "query")
        .commit(&mut controller)
        .unwrap();
    let first = memory("a", 1);
    let second = memory("b", 1);
    let mut traversal = controller.stage_traversal().unwrap();
    traversal.record_evaluated([first.clone()]).unwrap();
    traversal.record_evaluated([second.clone()]).unwrap();
    assert!(controller.searched_ids().is_empty());
    traversal.commit(&mut controller).unwrap();
    assert_eq!(controller.searched_ids(), &BTreeSet::from([first, second]));
}

#[test]
fn prepared_result_does_not_mark_selected() {
    let approved = memory("owner", 8);
    let mut controller = AgenticMemoryController::new();
    let mut operation = controller.stage_query("query").unwrap();
    operation.record_evaluated([approved.clone()]).unwrap();
    operation.commit(&mut controller).unwrap();
    let prepared = PreparedAgenticMemoryResult {
        delivery_id: "delivery".into(),
        payload: AgenticMemoryPayload {
            operation: AgenticMemoryOperation::Query,
            outcome: AgenticMemoryOutcome::Memories(vec![crate::AgenticMemoryContent {
                memory: approved.clone(),
                title: "title".into(),
                content: "body".into(),
            }]),
            anchors: Vec::new(),
        },
        delivered: BTreeSet::from([approved.clone()]),
        owner_cuts: BTreeMap::from([("owner".into(), 0)]),
    };
    assert_eq!(prepared.delivered(), &BTreeSet::from([approved.clone()]));
    assert_eq!(controller.searched_ids(), &BTreeSet::from([approved]));
    assert!(controller.selected_ids().is_empty());
}

#[test]
fn ineligible_evaluated_batches_fail_without_partial_union() {
    let seen = memory("owner", 1);
    let delivered = memory("owner", 2);
    let rejected_batch_peer = memory("owner", 3);
    let new = memory("owner", 4);
    let mut controller = AgenticMemoryController {
        current_query: Some("query".into()),
        searched_ids: BTreeSet::from([seen.clone()]),
        selected_ids: BTreeSet::from([delivered.clone()]),
    };
    let mut traversal = controller.stage_traversal().unwrap();
    assert!(
        traversal
            .record_evaluated([rejected_batch_peer.clone(), seen.clone()])
            .is_err()
    );
    traversal.record_evaluated([new.clone()]).unwrap();
    traversal.commit(&mut controller).unwrap();
    assert_eq!(
        controller.searched_ids(),
        &BTreeSet::from([seen.clone(), new])
    );
    let mut query = controller.stage_query("query").unwrap();
    assert!(
        query
            .record_evaluated([rejected_batch_peer, delivered.clone()])
            .is_err()
    );
    query.record_evaluated([seen.clone()]).unwrap();
    query.commit(&mut controller).unwrap();
    assert_eq!(controller.searched_ids(), &BTreeSet::from([seen]));
    assert_eq!(controller.selected_ids(), &BTreeSet::from([delivered]));
}

#[test]
fn dropped_failed_staged_work_leaves_controller_unchanged() {
    let original = AgenticMemoryController {
        current_query: Some("original".into()),
        searched_ids: BTreeSet::from([memory("owner", 9)]),
        selected_ids: BTreeSet::from([memory("owner", 10)]),
    };
    let controller = original.clone();
    let failed = controller.stage_query("   ");
    assert!(failed.is_err());
    let mut staged = controller.stage_query("replacement").unwrap();
    staged.record_evaluated([memory("owner", 11)]).unwrap();
    drop(staged);
    let mut refinement = controller.stage_refinement("replacement").unwrap();
    refinement.record_evaluated([memory("owner", 9)]).unwrap();
    drop(refinement);
    let mut traversal = controller.stage_traversal().unwrap();
    traversal.record_evaluated([memory("owner", 11)]).unwrap();
    drop(traversal);
    assert_eq!(controller, original);
}

#[test]
fn stale_staged_commit_rejects_without_overwrite() {
    let mut controller = AgenticMemoryController::new();
    let mut first = staged_query(&controller, "first");
    let mut second = staged_query(&controller, "second");
    first.record_evaluated([memory("owner", 1)]).unwrap();
    second.record_evaluated([memory("owner", 2)]).unwrap();
    second.commit(&mut controller).unwrap();
    let committed = controller.clone();
    assert_eq!(
        first.commit(&mut controller),
        Err(crate::agentic_memory_retrieval_model::AgenticMemoryError::StaleOperation)
    );
    assert_eq!(controller, committed);
}

#[test]
fn controller_state_has_exactly_the_three_explicit_fields() {
    let AgenticMemoryController {
        current_query,
        searched_ids,
        selected_ids,
    } = AgenticMemoryController::new();
    assert!(current_query.is_none());
    assert!(searched_ids.is_empty());
    assert!(selected_ids.is_empty());
}

#[test]
fn freshness_order_is_complete_passive_neutral_and_owner_qualified() {
    use super::{grade_agentic_memories, order_agentic_memories};
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target/test-tmp")
        .join(format!("agentic-grade-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let mut first = crate::Cva::create_project(root.join("first.rel")).unwrap();
    let second = crate::Cva::create_project(root.join("second.rel")).unwrap();
    let ids = (1..=16)
        .map(|byte| crate::MemoryId([byte; 32]))
        .collect::<Vec<_>>();
    let owner_uuid = first.owner_uuid().unwrap();
    first
        .freshness
        .persist_policy_if_needed(&mut first.container, owner_uuid)
        .unwrap();
    for n in 0..2000 {
        first
            .append_node(
                format!("activity-{n}"),
                "grade-test".into(),
                None,
                "user".into(),
                n,
                "activity",
            )
            .unwrap();
    }
    for (id, birth_turn) in [(ids[0], 0), (ids[1], 800), (ids[15], 2000)] {
        let record = crate::FreshnessRecord {
            score: 100,
            admitted_at_turn: Some(birth_turn),
            accounted_turn: Some(birth_turn),
        };
        let payload = first
            .freshness
            .encode_initialize(owner_uuid, id, birth_turn, record);
        first.freshness.ingest(&payload).unwrap();
    }
    first.sync().unwrap();
    let version = first.latest_global_version();
    let turn = first.rel_turn_count();
    let records = first.freshness.records().clone();
    let bytes = std::fs::read(root.join("first.rel")).unwrap();
    let mut ranked = grade_agentic_memories(&first, &ids, turn).unwrap();
    ranked.extend(grade_agentic_memories(&second, &[ids[2]], 0).unwrap());
    order_agentic_memories(&mut ranked);
    assert_eq!(ranked.len(), 17);
    assert_eq!(ranked[0].memory.memory_id, ids[15]);
    assert_eq!(ranked.last().unwrap().freshness, -100);
    assert_eq!(ranked[ranked.len() - 2].freshness, -20);
    assert!(
        ranked
            .windows(2)
            .all(|pair| pair[0].freshness > pair[1].freshness
                || (pair[0].freshness == pair[1].freshness && pair[0].memory < pair[1].memory))
    );
    assert_eq!(
        ranked
            .iter()
            .filter(|item| item.memory.memory_id == ids[2])
            .count(),
        2
    );
    assert_eq!(first.latest_global_version(), version);
    assert_eq!(first.rel_turn_count(), turn);
    assert_eq!(first.freshness.records(), &records);
    assert_eq!(std::fs::read(root.join("first.rel")).unwrap(), bytes);
    assert!(grade_agentic_memories(&first, &ids, turn + 1).is_err());
}
