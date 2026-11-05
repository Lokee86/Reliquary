use super::*;
use reliquary_owner_session::model::{NodeRef, RelationKind, RelationOrigin};
use reliquary_retrieval::agentic::{MAX_AGENTIC_CANDIDATES, RetrievalTurn};
use reliquary_switchboard::{DecisionEndpoint, DecisionEndpointError};
use serde_json::{Value, json};
use std::sync::{Arc, Barrier, Mutex};

#[derive(Default)]
struct Endpoint {
    replies: Mutex<Vec<Value>>,
    calls: Mutex<Vec<(Value, Value)>>,
    barrier: Option<Arc<Barrier>>,
}

impl Endpoint {
    fn with_replies(replies: impl IntoIterator<Item = Value>) -> Self {
        Self {
            replies: Mutex::new(replies.into_iter().collect()),
            calls: Mutex::new(Vec::new()),
            barrier: None,
        }
    }

    fn calls(&self) -> Vec<(Value, Value)> {
        self.calls.lock().unwrap().clone()
    }
}

impl DecisionEndpoint for Endpoint {
    fn model(&self) -> &str {
        "test"
    }

    fn evaluate(&self, state: &Value, questions: &Value) -> Result<Value, DecisionEndpointError> {
        self.calls
            .lock()
            .unwrap()
            .push((state.clone(), questions.clone()));
        if let Some(barrier) = &self.barrier {
            barrier.wait();
        }
        if self.replies.lock().unwrap().is_empty() {
            let approved = state["candidates"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|candidate| candidate["memory"].clone())
                .collect::<Vec<_>>();
            Ok(json!({"version":1,"approved":approved}))
        } else {
            Ok(self.replies.lock().unwrap().remove(0))
        }
    }
}

fn memory(index: u8, freshness: i16, content: &str) -> AgenticMemory {
    AgenticMemory {
        id: MemoryId([index; 32]),
        revision: 1,
        body_id: MemoryBodyId([index.wrapping_add(100); 32]),
        body: MemoryBody {
            title: format!("memory {index}"),
            content: content.into(),
        },
        freshness,
        vector: None,
    }
}

fn approved_ids(endpoint: &Endpoint, call: usize) -> Vec<String> {
    endpoint.calls()[call].0["candidates"]
        .as_array()
        .unwrap()
        .iter()
        .map(|candidate| {
            candidate["memory"]["memory_id"]
                .as_str()
                .unwrap()
                .to_owned()
        })
        .collect()
}

#[test]
fn query_freshness_ranks_complete_union_before_ten_candidate_cut() {
    let endpoint = Endpoint::default();
    let memories = (0..14)
        .map(|index| memory(index, i16::from(index), "needle evidence"))
        .collect::<Vec<_>>();

    let result = evaluate_retrieval(
        &endpoint,
        [7; 16],
        &RetrievalTurn::default(),
        &memories,
        &[],
        RetrievalAction::Query("needle".into()),
        None,
    )
    .unwrap();

    let offered = approved_ids(&endpoint, 0);
    assert_eq!(offered.len(), MAX_AGENTIC_CANDIDATES);
    assert_eq!(offered[0], hex(&[13; 32]));
    assert_eq!(offered[9], hex(&[4; 32]));
    assert!(!offered.contains(&hex(&[3; 32])));
    assert_eq!(result.next.query(), Some("needle"));
    assert_eq!(result.next.searched().len(), MAX_AGENTIC_CANDIDATES);
    assert!(result.next.selected().is_empty());
    assert_eq!(result.memories.len(), MAX_AGENTIC_CANDIDATES);
}

#[test]
fn selected_raw_match_is_reported_only_when_no_new_candidate_is_approved() {
    let selected = memory(1, 0, "needle already delivered");
    let mut turn = RetrievalTurn::default();
    turn.begin_query("needle").unwrap();
    turn.record_evaluated([selected.id]);
    turn.confirm_selected([selected.id]).unwrap();
    let endpoint = Endpoint::with_replies([json!({"version":1,"approved":[]})]);

    let result = evaluate_retrieval(
        &endpoint,
        [7; 16],
        &turn,
        &[selected.clone()],
        &[],
        RetrievalAction::Query("needle".into()),
        None,
    )
    .unwrap();
    assert!(result.memories.is_empty());
    assert_eq!(result.already_provided, [selected.id].into());
    assert!(endpoint.calls().is_empty());

    let new_memory = memory(2, 1, "needle new result");
    let mixed = Endpoint::with_replies([json!({
        "version":1,
        "approved":[{"owner_id":hex(&[7;16]),"memory_id":hex(&new_memory.id.0)}]
    })]);
    let result = evaluate_retrieval(
        &mixed,
        [7; 16],
        &turn,
        &[selected, new_memory.clone()],
        &[],
        RetrievalAction::Query("needle".into()),
        None,
    )
    .unwrap();
    assert_eq!(
        result.memories.iter().map(|m| m.id).collect::<Vec<_>>(),
        [new_memory.id]
    );
    assert!(result.already_provided.is_empty());
}

#[test]
fn refinement_rewrites_full_query_and_reconsiders_only_declined_ids_in_freshness_order() {
    let mut turn = RetrievalTurn::default();
    turn.begin_query("old complete query").unwrap();
    turn.record_evaluated([MemoryId([1; 32]), MemoryId([2; 32]), MemoryId([3; 32])]);
    turn.confirm_selected([MemoryId([2; 32])]).unwrap();
    let endpoint = Endpoint::with_replies([
        json!({"version":1,"query":"revised complete query"}),
        json!({"version":1,"approved":[]}),
    ]);
    let memories = vec![
        memory(1, 8, "first declined"),
        memory(2, 99, "selected is not reconsidered"),
        memory(3, 12, "second declined"),
        memory(4, 100, "unsearched is not reconsidered"),
    ];

    let result = evaluate_retrieval(
        &endpoint,
        [7; 16],
        &turn,
        &memories,
        &[],
        RetrievalAction::Refine("this week".into()),
        None,
    )
    .unwrap();
    let calls = endpoint.calls();
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0].0["operation"], "refine_query_rewrite");
    assert_eq!(calls[0].0["current_query"], "old complete query");
    assert_eq!(calls[0].0["refinement"], "this week");
    assert_eq!(calls[1].0["operation"], "refine_query");
    assert_eq!(calls[1].0["query"], "revised complete query");
    assert_eq!(approved_ids(&endpoint, 1), [hex(&[3; 32]), hex(&[1; 32])]);
    assert_eq!(result.next.query(), Some("revised complete query"));
    assert_eq!(result.next.selected(), turn.selected());
    assert_eq!(result.next.searched().len(), 3);
    assert!(result.memories.is_empty());
}

#[test]
fn invalid_refinement_reply_and_selection_outside_offer_fail_closed() {
    let mut turn = RetrievalTurn::default();
    turn.begin_query("before").unwrap();
    turn.record_evaluated([MemoryId([1; 32])]);
    let original = turn.clone();
    let invalid_rewrite = Endpoint::with_replies([json!({"version":2,"query":"after"})]);
    assert!(
        evaluate_retrieval(
            &invalid_rewrite,
            [7; 16],
            &turn,
            &[],
            &[],
            RetrievalAction::Refine("change".into()),
            None,
        )
        .is_err()
    );
    assert_eq!(turn, original);
    let extra_rewrite = Endpoint::with_replies([json!({
        "version":1,"query":"after","comment":"extra"
    })]);
    assert!(
        evaluate_retrieval(
            &extra_rewrite,
            [7; 16],
            &turn,
            &[],
            &[],
            RetrievalAction::Refine("change".into()),
            None,
        )
        .is_err()
    );
    assert_eq!(turn, original);

    let forged = Endpoint::with_replies([json!({
        "version":1,
        "approved":[{"owner_id":hex(&[7;16]),"memory_id":hex(&[9;32])}]
    })]);
    assert!(
        evaluate_retrieval(
            &forged,
            [7; 16],
            &RetrievalTurn::default(),
            &[memory(1, 1, "needle")],
            &[],
            RetrievalAction::Query("needle".into()),
            None,
        )
        .is_err()
    );

    for response in [
        json!({"version":1,"approved":[],"extra":true}),
        json!({"version":1,"approved":[
            {"owner_id":hex(&[8;16]),"memory_id":hex(&[1;32])}
        ]}),
        json!({"version":1,"approved":[
            {"owner_id":hex(&[7;16]),"memory_id":hex(&[1;32])},
            {"owner_id":hex(&[7;16]),"memory_id":hex(&[1;32])}
        ]}),
    ] {
        let strict = Endpoint::with_replies([response]);
        assert!(
            evaluate_retrieval(
                &strict,
                [7; 16],
                &RetrievalTurn::default(),
                &[memory(1, 1, "needle")],
                &[],
                RetrievalAction::Query("needle".into()),
                None,
            )
            .is_err()
        );
    }
}

#[test]
fn selection_rejects_duplicate_foreign_and_extended_responses() {
    let identity = json!({"owner_id":hex(&[7;16]),"memory_id":hex(&[1;32])});
    for response in [
        json!({"version":1,"approved":[identity.clone(),identity.clone()]}),
        json!({"version":1,"approved":[{"owner_id":hex(&[8;16]),"memory_id":hex(&[1;32])}]}),
        json!({"version":1,"approved":[identity.clone()],"extra":true}),
        json!({"version":1,"approved":[{"owner_id":hex(&[7;16]),"memory_id":hex(&[1;32]),"extra":true}]}),
    ] {
        let endpoint = Endpoint::with_replies([response]);
        let turn = RetrievalTurn::default();
        assert!(
            evaluate_retrieval(
                &endpoint,
                [7; 16],
                &turn,
                &[memory(1, 0, "needle"), memory(2, 0, "needle")],
                &[],
                RetrievalAction::Query("needle".into()),
                None
            )
            .is_err()
        );
        assert_eq!(turn, RetrievalTurn::default());
    }
}

#[test]
fn traversal_is_one_hop_and_excludes_previously_searched_candidates() {
    let anchor = memory(1, 0, "anchor");
    let unseen = memory(2, 0, "neighbor");
    let excluded = memory(3, 0, "already presented");
    let mut turn = RetrievalTurn::default();
    turn.begin_query("need").unwrap();
    turn.record_evaluated([anchor.id, excluded.id]);
    turn.confirm_selected([anchor.id]).unwrap();
    let relation = |source: MemoryId, target: MemoryId| Relation {
        source: NodeRef::Memory(source.0),
        target: NodeRef::Memory(target.0),
        kind: RelationKind::Topical,
        active: true,
        origin: RelationOrigin::Dream,
        graph_version: 1,
    };
    let endpoint = Endpoint::default();

    let result = evaluate_retrieval(
        &endpoint,
        [7; 16],
        &turn,
        &[anchor.clone(), unseen.clone(), excluded.clone()],
        &[
            relation(anchor.id, unseen.id),
            relation(anchor.id, excluded.id),
            relation(unseen.id, MemoryId([4; 32])),
        ],
        RetrievalAction::Traverse(vec![anchor.id]),
        None,
    )
    .unwrap();
    assert_eq!(
        result.memories.iter().map(|m| m.id).collect::<Vec<_>>(),
        [unseen.id]
    );
    assert_eq!(approved_ids(&endpoint, 0), [hex(&unseen.id.0)]);
    assert!(result.next.searched().contains(&unseen.id));
    assert!(!result.next.searched().contains(&MemoryId([4; 32])));
    assert_eq!(result.anchors.len(), 1);
    assert_eq!(result.anchors[0].unseen_neighbors, 1);
    assert_eq!(result.anchors[0].declined_neighbors, 1);
    assert_eq!(result.anchors[0].approved, [unseen.id].into());
    assert!(result.anchors[0].relations.iter().any(|edge| {
        edge.neighbor == unseen.id && edge.direction == RetrievalDirection::Outgoing
    }));
}

#[test]
fn traversal_allows_readable_unselected_anchors_and_keeps_independent_ten_candidate_pools() {
    let first_anchor = MemoryId([1; 32]);
    let second_anchor = MemoryId([2; 32]);
    let shared = MemoryId([100; 32]);
    let mut memories = vec![memory(1, 0, "first anchor"), memory(2, 0, "second anchor")];
    let mut relations = Vec::new();
    let mut turn = RetrievalTurn::default();
    turn.begin_query("need").unwrap();
    for index in 0..11u8 {
        let first_neighbor = if index == 0 {
            shared
        } else {
            MemoryId([index + 10; 32])
        };
        let second_neighbor = if index == 0 {
            shared
        } else {
            MemoryId([index + 30; 32])
        };
        for (id, freshness) in [
            (
                first_neighbor,
                if index == 0 { 100 } else { i16::from(index) },
            ),
            (
                second_neighbor,
                if index == 0 { 100 } else { i16::from(index) },
            ),
        ] {
            if !memories.iter().any(|item| item.id == id) {
                memories.push(memory(id.0[0], freshness, "relevant neighbor"));
            }
        }
        relations.push(Relation {
            source: NodeRef::Memory(first_anchor.0),
            target: NodeRef::Memory(first_neighbor.0),
            kind: RelationKind::Topical,
            active: true,
            origin: RelationOrigin::Dream,
            graph_version: 1,
        });
        relations.push(Relation {
            source: NodeRef::Memory(second_neighbor.0),
            target: NodeRef::Memory(second_anchor.0),
            kind: RelationKind::Causal,
            active: true,
            origin: RelationOrigin::Dream,
            graph_version: 1,
        });
    }
    let endpoint = Endpoint::default();
    let result = evaluate_retrieval(
        &endpoint,
        [7; 16],
        &turn,
        &memories,
        &relations,
        RetrievalAction::Traverse(vec![second_anchor, first_anchor]),
        None,
    )
    .unwrap();

    assert_eq!(result.anchors.len(), 2);
    assert_eq!(result.anchors[0].anchor, second_anchor);
    assert_eq!(result.anchors[1].anchor, first_anchor);
    assert_eq!(result.anchors[0].unseen_neighbors, 11);
    assert_eq!(result.anchors[1].unseen_neighbors, 11);
    assert_eq!(endpoint.calls().len(), 2);
    assert_eq!(approved_ids(&endpoint, 0).len(), MAX_AGENTIC_CANDIDATES);
    assert_eq!(approved_ids(&endpoint, 1).len(), MAX_AGENTIC_CANDIDATES);
    assert_eq!(
        result
            .memories
            .iter()
            .filter(|item| item.id == shared)
            .count(),
        1
    );
    assert_eq!(result.memories.len(), 19);
    assert!(
        result.anchors[0].relations.iter().any(|edge| {
            edge.neighbor == shared && edge.direction == RetrievalDirection::Incoming
        })
    );
    assert!(
        result.anchors[1].relations.iter().any(|edge| {
            edge.neighbor == shared && edge.direction == RetrievalDirection::Outgoing
        })
    );
    assert!(result.next.selected().is_empty());
    assert_eq!(result.next.searched().len(), 19);
}

#[test]
fn traversal_anchor_decisions_run_concurrently_and_failure_does_not_commit_turn_state() {
    let first = memory(1, 0, "anchor one");
    let second = memory(2, 0, "anchor two");
    let first_neighbor = memory(3, 0, "neighbor one");
    let second_neighbor = memory(4, 0, "neighbor two");
    let relation = |source: MemoryId, target: MemoryId| Relation {
        source: NodeRef::Memory(source.0),
        target: NodeRef::Memory(target.0),
        kind: RelationKind::Topical,
        active: true,
        origin: RelationOrigin::Dream,
        graph_version: 1,
    };
    let mut turn = RetrievalTurn::default();
    turn.begin_query("need").unwrap();
    turn.record_evaluated([MemoryId([9; 32])]);
    let before = turn.clone();
    let endpoint = Endpoint {
        barrier: Some(Arc::new(Barrier::new(2))),
        ..Endpoint::default()
    };
    let result = evaluate_retrieval(
        &endpoint,
        [7; 16],
        &turn,
        &[
            first.clone(),
            second.clone(),
            first_neighbor.clone(),
            second_neighbor.clone(),
        ],
        &[
            relation(first.id, first_neighbor.id),
            relation(second.id, second_neighbor.id),
        ],
        RetrievalAction::Traverse(vec![first.id, second.id]),
        None,
    )
    .unwrap();
    assert_eq!(endpoint.calls().len(), 2);
    assert_eq!(turn, before);
    assert_eq!(result.next.searched().len(), before.searched().len() + 2);

    let invalid_second_anchor = Endpoint::with_replies([
        json!({"version":1,"approved":[]}),
        json!({"version":1,"approved":[{
            "owner_id":hex(&[8;16]),"memory_id":hex(&second_neighbor.id.0)
        }]}),
    ]);
    assert!(
        evaluate_retrieval(
            &invalid_second_anchor,
            [7; 16],
            &turn,
            &[first, second, first_neighbor, second_neighbor],
            &[
                relation(MemoryId([1; 32]), MemoryId([3; 32])),
                relation(MemoryId([2; 32]), MemoryId([4; 32]))
            ],
            RetrievalAction::Traverse(vec![MemoryId([1; 32]), MemoryId([2; 32])]),
            None,
        )
        .is_err()
    );
    assert_eq!(turn, before);
}

#[test]
fn traversal_accepts_uncapped_anchors_and_deduplicates_in_input_order() {
    let anchors = (0..11)
        .map(|index| memory(index, 0, "anchor"))
        .collect::<Vec<_>>();
    let mut turn = RetrievalTurn::default();
    turn.begin_query("query").unwrap();
    let supplied = vec![anchors[7].id, anchors[2].id, anchors[7].id]
        .into_iter()
        .chain(anchors.iter().map(|anchor| anchor.id))
        .collect::<Vec<_>>();

    let result = evaluate_retrieval(
        &Endpoint::default(),
        [7; 16],
        &turn,
        &anchors,
        &[],
        RetrievalAction::Traverse(supplied),
        None,
    )
    .unwrap();

    assert_eq!(result.anchors.len(), anchors.len());
    assert_eq!(result.anchors[0].anchor, anchors[7].id);
    assert_eq!(result.anchors[1].anchor, anchors[2].id);
    assert_eq!(
        result
            .anchors
            .iter()
            .map(|report| report.anchor)
            .collect::<BTreeSet<_>>()
            .len(),
        anchors.len()
    );
    assert!(result.next.searched().is_empty());
}
