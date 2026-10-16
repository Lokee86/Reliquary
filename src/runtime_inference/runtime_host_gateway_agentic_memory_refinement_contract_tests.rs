use super::*;
use crate::{AgenticMemoryOutcome, DecisionEndpointError, MemoryId, MemoryRef};
use serde_json::{Value, json};
use std::collections::{BTreeSet, VecDeque};

enum Action {
    Reply(Value),
    ApproveAll,
    Failure,
}

struct ScriptedDecision {
    actions: Mutex<VecDeque<Action>>,
    calls: Arc<Mutex<Vec<Value>>>,
}

impl DecisionEndpoint for ScriptedDecision {
    fn model(&self) -> &str {
        "refinement-contract-test"
    }

    fn evaluate(&self, state: &Value, _: &Value) -> Result<Value, DecisionEndpointError> {
        self.calls.lock().unwrap().push(state.clone());
        match self.actions.lock().unwrap().pop_front() {
            Some(Action::Reply(value)) => Ok(value),
            Some(Action::ApproveAll) => Ok(json!({
                "version": 1,
                "approved": state["candidates"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|candidate| candidate["memory"].clone())
                    .collect::<Vec<_>>(),
            })),
            Some(Action::Failure) | None => Err(DecisionEndpointError::Failure(
                "scripted refinement provider failure".into(),
            )),
        }
    }
}

struct Fixture {
    gateway: GatewayRuntime,
    instance: Arc<InstanceRuntime>,
    owner: String,
    ids: Vec<MemoryId>,
    calls: Arc<Mutex<Vec<Value>>>,
}

fn fixture(memory_count: usize, actions: Vec<Action>) -> Fixture {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target/test-tmp")
        .join(format!("agentic-refine-contract-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let mut rel = crate::Cva::create_project(root.join("owner.rel")).unwrap();
    let episode = crate::community_test_support::rel_episode(&mut rel);
    let mut ids = Vec::new();
    for index in 0..memory_count {
        ids.push(crate::community_test_support::rel_memory(
            &mut rel,
            &episode,
            &format!("refine-contract-{index}"),
        ));
    }
    let calls = Arc::new(Mutex::new(Vec::new()));
    let endpoint = Arc::new(ScriptedDecision {
        actions: Mutex::new(actions.into()),
        calls: Arc::clone(&calls),
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
            "refinement-contract-reader".into(),
            None,
            BTreeMap::from([(owner.clone(), BTreeSet::from([OwnerGrant::ViewRel]))]),
        )
        .unwrap();
    gateway.select_rel(&instance, &owner).unwrap();
    Fixture {
        gateway,
        instance,
        owner,
        ids,
        calls,
    }
}

fn rewrite(query: &str) -> Action {
    Action::Reply(json!({"version": 1, "query": query}))
}

fn hex(id: MemoryId) -> String {
    id.0.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn controller_with_evaluated(fixture: &Fixture, count: usize) -> AgenticMemoryController {
    let mut controller = AgenticMemoryController::new();
    let mut staged = controller.stage_query("complete old query").unwrap();
    staged
        .record_evaluated(fixture.ids[..count].iter().map(|memory_id| MemoryRef {
            owner_id: fixture.owner.clone(),
            memory_id: *memory_id,
        }))
        .unwrap();
    staged.commit(&mut controller).unwrap();
    controller
}

#[test]
fn empty_declined_set_rewrites_once_and_commits_query_without_changing_id_sets() {
    let fixture = fixture(0, vec![rewrite("full corrected query")]);
    let mut controller = AgenticMemoryController::new();
    controller
        .stage_query("complete old query")
        .unwrap()
        .commit(&mut controller)
        .unwrap();
    let retained = MemoryRef {
        owner_id: fixture.owner.clone(),
        memory_id: MemoryId([93; 32]),
    };
    controller.selected_ids.insert(retained.clone());
    let searched = controller.searched_ids().clone();
    let selected = controller.selected_ids().clone();

    let result = fixture
        .gateway
        .refine_query(
            &fixture.instance,
            &mut controller,
            "correct the interpretation",
            "empty-declined",
        )
        .unwrap();
    assert_eq!(result.payload().outcome, AgenticMemoryOutcome::NoMatches);
    assert_eq!(controller.current_query(), Some("full corrected query"));
    assert_eq!(controller.searched_ids(), &searched);
    assert_eq!(controller.selected_ids(), &selected);
    let calls = fixture.calls.lock().unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0]["operation"], "refine_query_rewrite");
}

#[test]
fn refinement_without_active_query_fails_before_provider_io() {
    let fixture = fixture(0, vec![]);
    let mut controller = AgenticMemoryController::new();
    let before = controller.clone();
    assert!(matches!(
        fixture.gateway.refine_query(
            &fixture.instance,
            &mut controller,
            "a refinement",
            "no-query",
        ),
        Err(crate::AgenticMemoryError::NoActiveQuery)
    ));
    assert_eq!(controller, before);
    assert!(fixture.calls.lock().unwrap().is_empty());
}

#[test]
fn refinement_is_a_complete_correction_and_selection_receives_that_rewrite() {
    let fixture = fixture(
        2,
        vec![rewrite("basketball statistics in 2025"), Action::ApproveAll],
    );
    let mut controller = controller_with_evaluated(&fixture, fixture.ids.len());
    let result = fixture
        .gateway
        .refine_query(
            &fixture.instance,
            &mut controller,
            "correct baseball to basketball",
            "complete-correction",
        )
        .unwrap();
    assert_eq!(
        controller.current_query(),
        Some("basketball statistics in 2025")
    );
    assert!(matches!(
        result.payload().outcome,
        AgenticMemoryOutcome::Memories(_)
    ));
    let calls = fixture.calls.lock().unwrap();
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0]["current_query"], "complete old query");
    assert_eq!(calls[0]["refinement"], "correct baseball to basketball");
    assert_eq!(calls[1]["query"], "basketball statistics in 2025");
    assert_ne!(
        calls[1]["query"],
        "complete old query correct baseball to basketball"
    );
}

#[test]
fn malformed_rewrite_preserves_controller_and_stops_before_reconsideration() {
    let fixture = fixture(
        2,
        vec![Action::Reply(json!({
            "version": 1,
            "query": "corrected",
            "alternatives": ["extra unsupported answer"],
        }))],
    );
    let mut controller = controller_with_evaluated(&fixture, fixture.ids.len());
    let before = controller.clone();
    assert!(
        fixture
            .gateway
            .refine_query(
                &fixture.instance,
                &mut controller,
                "correct the old query",
                "malformed-rewrite",
            )
            .is_err()
    );
    assert_eq!(controller, before);
    let calls = fixture.calls.lock().unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0]["operation"], "refine_query_rewrite");
}

#[test]
fn malformed_later_batch_discards_earlier_approvals_and_does_not_search_new_memory() {
    let fixture = fixture(
        15,
        vec![
            rewrite("complete revised information need"),
            Action::ApproveAll,
            Action::Reply(json!({
                "version": 1,
                "approved": [{"owner_id": "unknown-owner", "memory_id": "unknown-id"}],
            })),
        ],
    );
    let mut controller = controller_with_evaluated(&fixture, 14);
    controller.selected_ids.insert(MemoryRef {
        owner_id: fixture.owner.clone(),
        memory_id: fixture.ids[0],
    });
    let before = controller.clone();
    assert!(
        fixture
            .gateway
            .refine_query(
                &fixture.instance,
                &mut controller,
                "apply corrected premise",
                "bad-second-batch",
            )
            .is_err()
    );
    assert_eq!(controller, before);
    let calls = fixture.calls.lock().unwrap();
    assert_eq!(calls.len(), 3);
    assert_eq!(calls[1]["query"], "complete revised information need");
    assert_eq!(calls[2]["query"], "complete revised information need");
    assert_eq!(calls[1]["candidates"].as_array().unwrap().len(), 10);
    assert_eq!(calls[2]["candidates"].as_array().unwrap().len(), 3);
    let supplied = calls[1..]
        .iter()
        .flat_map(|call| {
            call["candidates"]
                .as_array()
                .unwrap()
                .iter()
                .map(|candidate| {
                    candidate["memory"]["memory_id"]
                        .as_str()
                        .unwrap()
                        .to_owned()
                })
        })
        .collect::<BTreeSet<_>>();
    let expected = fixture.ids[1..14]
        .iter()
        .copied()
        .map(hex)
        .collect::<BTreeSet<_>>();
    assert_eq!(supplied, expected);
    assert!(
        !supplied.contains(&hex(fixture.ids[0])),
        "selected ID is excluded"
    );
    assert!(
        !supplied.contains(&hex(fixture.ids[14])),
        "unsearched Memory is never retrieved"
    );
}

#[test]
fn provider_failure_after_an_approved_batch_preserves_the_entire_controller() {
    let fixture = fixture(
        15,
        vec![
            rewrite("complete revised information need"),
            Action::ApproveAll,
            Action::Failure,
        ],
    );
    let mut controller = controller_with_evaluated(&fixture, 14);
    controller.selected_ids.insert(MemoryRef {
        owner_id: fixture.owner.clone(),
        memory_id: fixture.ids[0],
    });
    let before = controller.clone();
    assert!(
        fixture
            .gateway
            .refine_query(
                &fixture.instance,
                &mut controller,
                "apply corrected premise",
                "provider-failure-second-batch",
            )
            .is_err()
    );
    assert_eq!(controller, before);
    let calls = fixture.calls.lock().unwrap();
    assert_eq!(calls.len(), 3);
    assert_eq!(calls[1]["candidates"].as_array().unwrap().len(), 10);
    assert_eq!(calls[2]["candidates"].as_array().unwrap().len(), 3);
}
