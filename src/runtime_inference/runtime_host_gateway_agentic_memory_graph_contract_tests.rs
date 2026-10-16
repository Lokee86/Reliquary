use super::*;
use crate::{
    AgenticMemoryController, AgenticMemoryOutcome, DecisionEndpoint, DecisionEndpointError,
    GraphDirection, GraphRelationKind, MemoryId, MemoryRef,
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

struct GraphFixture {
    gateway: GatewayRuntime,
    instance: Arc<InstanceRuntime>,
    owner: String,
    ids: Vec<MemoryId>,
    calls: Arc<Mutex<Vec<Value>>>,
    approvals_by_anchor: Arc<Mutex<BTreeMap<String, BTreeSet<String>>>>,
}

struct AnchorDecision {
    calls: Arc<Mutex<Vec<Value>>>,
    approvals_by_anchor: Arc<Mutex<BTreeMap<String, BTreeSet<String>>>>,
}

impl DecisionEndpoint for AnchorDecision {
    fn model(&self) -> &str {
        "agentic-graph-contract-test"
    }

    fn evaluate(&self, state: &Value, _: &Value) -> Result<Value, DecisionEndpointError> {
        self.calls.lock().unwrap().push(state.clone());
        let anchor_id = state["anchor"]["memory_id"].as_str().unwrap();
        let approvals_by_anchor = self.approvals_by_anchor.lock().unwrap();
        let selected = approvals_by_anchor.get(anchor_id);
        let approved = state["candidates"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|candidate| {
                selected.is_none_or(|set| {
                    set.contains(candidate["memory"]["memory_id"].as_str().unwrap())
                })
            })
            .map(|candidate| candidate["memory"].clone())
            .collect::<Vec<_>>();
        Ok(json!({"version": 1, "approved": approved}))
    }
}

fn fixture(
    memory_count: usize,
    edges: &[(usize, usize, GraphRelationKind)],
    approvals_by_anchor: BTreeMap<String, BTreeSet<String>>,
) -> GraphFixture {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target/test-tmp")
        .join(format!("agentic-graph-contract-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let mut rel = crate::Cva::create_project(root.join("owner.rel")).unwrap();
    let episode = crate::community_test_support::rel_episode(&mut rel);
    let ids: Vec<_> = (0..memory_count)
        .map(|index| {
            crate::community_test_support::rel_memory(
                &mut rel,
                &episode,
                &format!("graph-contract-{index}"),
            )
        })
        .collect();
    for (source, target, kind) in edges {
        rel.set_memory_relation(ids[*source], ids[*target], *kind, true, rel.graph_version())
            .unwrap();
    }
    let calls = Arc::new(Mutex::new(Vec::new()));
    let approvals_by_anchor = Arc::new(Mutex::new(approvals_by_anchor));
    let endpoint = Arc::new(AnchorDecision {
        calls: Arc::clone(&calls),
        approvals_by_anchor: Arc::clone(&approvals_by_anchor),
    });
    let gateway = GatewayRuntime::new(
        root.join("state"),
        ReliquaryRuntimeRoutes::default().with_memory_retrieval_decision(Some(endpoint)),
        InsomniaWorkerConfig {
            workers: 0,
            ..Default::default()
        },
        EpisodePolicy::default(),
    );
    let owner = gateway.mount_rel(InteractionRuntime::new(rel)).unwrap();
    let instance = gateway
        .attach_instance(
            "graph-contract-reader".into(),
            None,
            BTreeMap::from([(owner.clone(), BTreeSet::from([OwnerGrant::ViewRel]))]),
        )
        .unwrap();
    gateway.select_rel(&instance, &owner).unwrap();
    GraphFixture {
        gateway,
        instance,
        owner,
        ids,
        calls,
        approvals_by_anchor,
    }
}

fn memory_ref(owner: &str, id: MemoryId) -> MemoryRef {
    MemoryRef {
        owner_id: owner.into(),
        memory_id: id,
    }
}

fn hex(id: MemoryId) -> String {
    id.0.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn controller(query: &str) -> AgenticMemoryController {
    let mut controller = AgenticMemoryController::new();
    controller
        .stage_query(query)
        .unwrap()
        .commit(&mut controller)
        .unwrap();
    controller
}

#[test]
fn more_than_ten_neighbors_are_freshness_ordered_and_repeated_traversal_reveals_the_rest() {
    let mut rel = crate::Cva::create_project(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target/test-tmp")
            .join(format!("agentic-graph-freshness-{}.rel", Uuid::new_v4())),
    )
    .unwrap();
    let episode = crate::community_test_support::rel_episode(&mut rel);
    let anchor = crate::community_test_support::rel_memory(&mut rel, &episode, "fresh-anchor");
    let mut old_neighbors = Vec::new();
    for index in 0..11 {
        old_neighbors.push(crate::community_test_support::rel_memory(
            &mut rel,
            &episode,
            &format!("old-neighbor-{index}"),
        ));
    }
    let mut previous = None;
    for index in 0..1_000 {
        let id = format!("freshness-aging-{index}");
        rel.append_node(
            id.clone(),
            "graph-freshness-aging".into(),
            previous,
            if index % 2 == 0 { "user" } else { "assistant" }.into(),
            index,
            "advance accepted REL activity",
        )
        .unwrap();
        previous = Some(id);
    }
    let late_neighbor =
        crate::community_test_support::rel_memory(&mut rel, &episode, "late-fresh-neighbor");
    for neighbor in old_neighbors.iter().chain(std::iter::once(&late_neighbor)) {
        rel.set_memory_relation(
            anchor,
            *neighbor,
            GraphRelationKind::Topical,
            true,
            rel.graph_version(),
        )
        .unwrap();
    }

    let calls = Arc::new(Mutex::new(Vec::new()));
    let approvals_by_anchor = Arc::new(Mutex::new(BTreeMap::new()));
    let endpoint = Arc::new(AnchorDecision {
        calls: Arc::clone(&calls),
        approvals_by_anchor,
    });
    let gateway = GatewayRuntime::new(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target/test-tmp")
            .join(format!("agentic-graph-freshness-state-{}", Uuid::new_v4())),
        ReliquaryRuntimeRoutes::default().with_memory_retrieval_decision(Some(endpoint)),
        InsomniaWorkerConfig {
            workers: 0,
            ..Default::default()
        },
        EpisodePolicy::default(),
    );
    let owner = gateway.mount_rel(InteractionRuntime::new(rel)).unwrap();
    let instance = gateway
        .attach_instance(
            "graph-freshness-reader".into(),
            None,
            BTreeMap::from([(owner.clone(), BTreeSet::from([OwnerGrant::ViewRel]))]),
        )
        .unwrap();
    gateway.select_rel(&instance, &owner).unwrap();
    let anchor_ref = memory_ref(&owner, anchor);
    let late_ref = memory_ref(&owner, late_neighbor);
    let mut controller = controller("complete graph query");
    let first = gateway
        .traverse_graph(
            &instance,
            &mut controller,
            &[anchor_ref.clone()],
            "graph-page-1",
        )
        .unwrap();
    let AgenticMemoryOutcome::Memories(first_content) = &first.payload().outcome else {
        panic!("first traversal should prepare ten approved neighbors");
    };
    assert_eq!(first_content.len(), 10);
    assert!(first_content.iter().any(|item| item.memory == late_ref));
    assert_eq!(controller.searched_ids().len(), 10);
    assert!(controller.selected_ids().is_empty());
    assert_eq!(first.payload().anchors[0].unseen_neighbors, 12);

    let second = gateway
        .traverse_graph(&instance, &mut controller, &[anchor_ref], "graph-page-2")
        .unwrap();
    let AgenticMemoryOutcome::Memories(second_content) = &second.payload().outcome else {
        panic!("second traversal should reveal the remaining neighbors");
    };
    assert_eq!(second_content.len(), 2);
    assert_eq!(second.payload().anchors[0].unseen_neighbors, 2);
    assert_eq!(controller.searched_ids().len(), 12);
    assert_eq!(calls.lock().unwrap().len(), 2);
}

#[test]
fn traversal_reports_incoming_and_outgoing_relations_without_recursing() {
    let fixture = fixture(
        6,
        &[
            (0, 1, GraphRelationKind::Topical),
            (1, 0, GraphRelationKind::Causal),
            (0, 2, GraphRelationKind::Factual),
            (3, 0, GraphRelationKind::References),
            (1, 4, GraphRelationKind::Topical),
        ],
        BTreeMap::new(),
    );
    let mut controller = controller("complete graph query");
    let anchor = memory_ref(&fixture.owner, fixture.ids[0]);
    let result = fixture
        .gateway
        .traverse_graph(&fixture.instance, &mut controller, &[anchor], "directions")
        .unwrap();
    let AgenticMemoryOutcome::Memories(content) = &result.payload().outcome else {
        panic!("direct neighbors should be prepared");
    };
    let content_ids = content
        .iter()
        .map(|item| item.memory.memory_id)
        .collect::<BTreeSet<_>>();
    assert_eq!(
        content_ids,
        BTreeSet::from([fixture.ids[1], fixture.ids[2], fixture.ids[3]])
    );
    assert!(
        !content_ids.contains(&fixture.ids[4]),
        "neighbor-of-neighbor is excluded"
    );
    let relations = &result.payload().anchors[0].relations;
    assert_eq!(relations.len(), 4);
    assert!(relations.iter().any(|edge| {
        edge.neighbor.memory_id == fixture.ids[1]
            && edge.kind == GraphRelationKind::Topical
            && edge.direction == GraphDirection::Outgoing
    }));
    assert!(relations.iter().any(|edge| {
        edge.neighbor.memory_id == fixture.ids[1]
            && edge.kind == GraphRelationKind::Causal
            && edge.direction == GraphDirection::Incoming
    }));
    assert!(relations.iter().any(|edge| {
        edge.neighbor.memory_id == fixture.ids[3]
            && edge.kind == GraphRelationKind::References
            && edge.direction == GraphDirection::Incoming
    }));
}

#[test]
fn selected_and_declined_anchors_remain_legal_and_report_pre_call_counts() {
    let fixture = fixture(
        5,
        &[
            (0, 1, GraphRelationKind::Topical),
            (0, 2, GraphRelationKind::Causal),
            (3, 2, GraphRelationKind::Factual),
            (3, 4, GraphRelationKind::References),
        ],
        BTreeMap::new(),
    );
    let selected_anchor = memory_ref(&fixture.owner, fixture.ids[0]);
    let declined_anchor = memory_ref(&fixture.owner, fixture.ids[3]);
    let mut controller = controller("complete graph query");
    let selected_neighbor = memory_ref(&fixture.owner, fixture.ids[1]);
    let declined_neighbor = memory_ref(&fixture.owner, fixture.ids[2]);
    controller.selected_ids.insert(selected_anchor.clone());
    controller.selected_ids.insert(selected_neighbor.clone());
    controller.searched_ids.insert(selected_anchor.clone());
    controller.searched_ids.insert(declined_anchor.clone());
    controller.searched_ids.insert(declined_neighbor.clone());

    let result = fixture
        .gateway
        .traverse_graph(
            &fixture.instance,
            &mut controller,
            &[declined_anchor.clone(), selected_anchor.clone()],
            "selected-and-declined-anchors",
        )
        .unwrap();
    assert_eq!(result.payload().anchors.len(), 2);
    assert!(result.payload().anchors[0].anchor < result.payload().anchors[1].anchor);
    let selected_report = result
        .payload()
        .anchors
        .iter()
        .find(|report| report.anchor == selected_anchor)
        .unwrap();
    assert_eq!(selected_report.unseen_neighbors, 0);
    assert_eq!(selected_report.declined_neighbors, 1);
    let declined_report = result
        .payload()
        .anchors
        .iter()
        .find(|report| report.anchor == declined_anchor)
        .unwrap();
    assert_eq!(declined_report.unseen_neighbors, 1);
    assert_eq!(declined_report.declined_neighbors, 1);
    assert!(controller.searched_ids().contains(&declined_neighbor));
    assert!(controller.selected_ids().contains(&selected_neighbor));
}

#[test]
fn overlapping_neighbors_are_reported_per_anchor_and_content_is_union_deduplicated() {
    let fixture = fixture(
        5,
        &[
            (0, 2, GraphRelationKind::Topical),
            (0, 3, GraphRelationKind::Factual),
            (1, 2, GraphRelationKind::Causal),
            (1, 4, GraphRelationKind::References),
        ],
        BTreeMap::new(),
    );
    let first_anchor = memory_ref(&fixture.owner, fixture.ids[0]);
    let second_anchor = memory_ref(&fixture.owner, fixture.ids[1]);
    let shared = memory_ref(&fixture.owner, fixture.ids[2]);
    let first_only = memory_ref(&fixture.owner, fixture.ids[3]);
    let second_only = memory_ref(&fixture.owner, fixture.ids[4]);
    *fixture.approvals_by_anchor.lock().unwrap() = BTreeMap::from([
        (hex(fixture.ids[0]), BTreeSet::from([hex(fixture.ids[3])])),
        (
            hex(fixture.ids[1]),
            BTreeSet::from([hex(fixture.ids[2]), hex(fixture.ids[4])]),
        ),
    ]);
    let mut controller = controller("complete graph query");
    let result = fixture
        .gateway
        .traverse_graph(
            &fixture.instance,
            &mut controller,
            &[second_anchor.clone(), first_anchor.clone()],
            "overlap-union",
        )
        .unwrap();
    let AgenticMemoryOutcome::Memories(content) = &result.payload().outcome else {
        panic!("overlapping approved neighbors should be prepared");
    };
    assert_eq!(
        content
            .iter()
            .map(|item| item.memory.clone())
            .collect::<Vec<_>>(),
        vec![shared.clone(), first_only.clone(), second_only.clone()]
            .into_iter()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>()
    );
    assert_eq!(
        content.iter().filter(|item| item.memory == shared).count(),
        1
    );
    let reports = &result.payload().anchors;
    assert_eq!(reports.len(), 2);
    assert!(reports[0].anchor < reports[1].anchor);
    let first_report = reports
        .iter()
        .find(|report| report.anchor == first_anchor)
        .unwrap();
    let second_report = reports
        .iter()
        .find(|report| report.anchor == second_anchor)
        .unwrap();
    assert!(!first_report.approved.contains(&shared));
    assert!(second_report.approved.contains(&shared));
    assert_eq!(fixture.calls.lock().unwrap().len(), 2);
}
