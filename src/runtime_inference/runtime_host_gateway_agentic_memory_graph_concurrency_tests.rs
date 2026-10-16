use super::*;
use crate::{
    AgenticMemoryController, AgenticMemoryOutcome, DecisionEndpoint, DecisionEndpointError,
    GraphRelationKind, MemoryRef,
};
use serde_json::{Value, json};
use std::collections::{BTreeSet, HashMap};
use std::sync::{Arc, Condvar, Mutex, Weak};
use std::time::{Duration, Instant};

struct BoundedBarrier {
    parties: usize,
    arrivals: Mutex<(usize, bool)>,
    changed: Condvar,
}

impl BoundedBarrier {
    fn new(parties: usize) -> Self {
        Self {
            parties,
            arrivals: Mutex::new((0, false)),
            changed: Condvar::new(),
        }
    }

    fn wait(&self) -> Result<(), DecisionEndpointError> {
        let deadline = Instant::now() + Duration::from_secs(3);
        let mut state = self
            .arrivals
            .lock()
            .map_err(|_| DecisionEndpointError::Failure("barrier lock poisoned".into()))?;
        state.0 += 1;
        if state.0 >= self.parties {
            state.1 = true;
            self.changed.notify_all();
            return Ok(());
        }
        while !state.1 {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(DecisionEndpointError::Failure(
                    "timed out waiting for concurrent anchor decision".into(),
                ));
            }
            let (next, timeout) = self
                .changed
                .wait_timeout(state, remaining)
                .map_err(|_| DecisionEndpointError::Failure("barrier lock poisoned".into()))?;
            state = next;
            if timeout.timed_out() && !state.1 {
                return Err(DecisionEndpointError::Failure(
                    "timed out waiting for concurrent anchor decision".into(),
                ));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy)]
enum Fault {
    Provider,
    Panic,
    InvalidApproval,
}

#[derive(Clone, Copy)]
enum AuthorizationChange {
    Revoke,
    Detach,
}

enum DecisionPlan {
    InvertedCompletion {
        barrier: Arc<BoundedBarrier>,
        slower_anchor: String,
        approve_by_anchor: HashMap<String, BTreeSet<String>>,
        completed: Arc<Mutex<Vec<String>>>,
    },
    SuccessThenFault {
        barrier: Arc<BoundedBarrier>,
        failing_anchor: String,
        fault: Fault,
        successful_returned: Arc<(Mutex<bool>, Condvar)>,
    },
    ChangeAuthorization(AuthorizationChange),
}

struct GraphDecisionEndpoint {
    gateway: Mutex<Weak<GatewayRuntime>>,
    instance: Mutex<Option<Arc<InstanceRuntime>>>,
    plan: DecisionPlan,
    calls: Mutex<Vec<String>>,
}

impl GraphDecisionEndpoint {
    fn anchor_id(state: &Value) -> Result<String, DecisionEndpointError> {
        state["anchor"]["memory_id"]
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| DecisionEndpointError::InvalidResponse("anchor id missing"))
    }

    fn approved(state: &Value, ids: Option<&BTreeSet<String>>) -> Value {
        json!({
            "version": 1,
            "approved": state["candidates"]
                .as_array()
                .expect("candidate list is supplied")
                .iter()
                .filter(|candidate| {
                    ids.is_none_or(|allowed| {
                        candidate["memory"]["memory_id"]
                            .as_str()
                            .is_some_and(|id| allowed.contains(id))
                    })
                })
                .map(|candidate| candidate["memory"].clone())
                .collect::<Vec<_>>()
        })
    }

    fn mutate_authority(&self, action: AuthorizationChange) {
        let gateway = self
            .gateway
            .lock()
            .unwrap()
            .upgrade()
            .expect("gateway remains alive");
        let instance = self
            .instance
            .lock()
            .unwrap()
            .as_ref()
            .cloned()
            .expect("fixture instance is installed");
        match action {
            AuthorizationChange::Revoke => gateway
                .replace_instance_grants(&instance, BTreeMap::new())
                .expect("revocation during decision should complete"),
            AuthorizationChange::Detach => assert!(
                gateway
                    .detach_instance(&instance)
                    .expect("detachment during decision should complete")
            ),
        }
    }
}

impl DecisionEndpoint for GraphDecisionEndpoint {
    fn model(&self) -> &str {
        "agentic-graph-concurrency-test"
    }

    fn evaluate(&self, state: &Value, _: &Value) -> Result<Value, DecisionEndpointError> {
        let anchor = Self::anchor_id(state)?;
        self.calls.lock().unwrap().push(anchor.clone());
        match &self.plan {
            DecisionPlan::InvertedCompletion {
                barrier,
                slower_anchor,
                approve_by_anchor,
                completed,
            } => {
                barrier.wait()?;
                if &anchor == slower_anchor {
                    std::thread::sleep(Duration::from_millis(60));
                }
                completed.lock().unwrap().push(anchor.clone());
                Ok(Self::approved(state, approve_by_anchor.get(&anchor)))
            }
            DecisionPlan::SuccessThenFault {
                barrier,
                failing_anchor,
                fault,
                successful_returned,
            } => {
                barrier.wait()?;
                if &anchor == failing_anchor {
                    let deadline = Instant::now() + Duration::from_secs(3);
                    let (lock, changed) = &**successful_returned;
                    let mut success = lock.lock().unwrap();
                    while !*success {
                        let remaining = deadline.saturating_duration_since(Instant::now());
                        if remaining.is_zero() {
                            return Err(DecisionEndpointError::Failure(
                                "successful anchor did not finish before injected failure".into(),
                            ));
                        }
                        let (next, timeout) = changed.wait_timeout(success, remaining).unwrap();
                        success = next;
                        if timeout.timed_out() && !*success {
                            return Err(DecisionEndpointError::Failure(
                                "successful anchor did not finish before injected failure".into(),
                            ));
                        }
                    }
                    match fault {
                        Fault::Provider => Err(DecisionEndpointError::Failure(
                            "injected provider failure".into(),
                        )),
                        Fault::Panic => panic!("injected anchor worker panic"),
                        Fault::InvalidApproval => Ok(json!({
                            "version": 1,
                            "approved": [{"owner_id":"invented", "memory_id":"00"}]
                        })),
                    }
                } else {
                    {
                        let (lock, changed) = &**successful_returned;
                        *lock.lock().unwrap() = true;
                        changed.notify_all();
                    }
                    Ok(Self::approved(state, None))
                }
            }
            DecisionPlan::ChangeAuthorization(action) => {
                self.mutate_authority(*action);
                Ok(Self::approved(state, None))
            }
        }
    }
}

struct GraphFixture {
    gateway: Arc<GatewayRuntime>,
    instance: Arc<InstanceRuntime>,
    owner: String,
    anchors: [MemoryRef; 2],
    neighbors: [MemoryRef; 3],
    endpoint: Arc<GraphDecisionEndpoint>,
}

fn hex_id(id: crate::MemoryId) -> String {
    id.0.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn graph_fixture(make_plan: impl FnOnce(&[crate::MemoryId; 5]) -> DecisionPlan) -> GraphFixture {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target/test-tmp")
        .join(format!("agentic-graph-concurrency-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let mut rel = crate::Cva::create_project(root.join("owner.rel")).unwrap();
    let episode = crate::community_test_support::rel_episode(&mut rel);
    let ids: [crate::MemoryId; 5] = std::array::from_fn(|n| {
        crate::community_test_support::rel_memory(&mut rel, &episode, &format!("graph-{n}"))
    });
    for (source, target, kind) in [
        (0, 2, GraphRelationKind::Factual),
        (0, 4, GraphRelationKind::Topical),
        (1, 3, GraphRelationKind::Causal),
        (1, 4, GraphRelationKind::References),
    ] {
        rel.set_memory_relation(ids[source], ids[target], kind, true, rel.graph_version())
            .unwrap();
    }

    let endpoint = Arc::new(GraphDecisionEndpoint {
        gateway: Mutex::new(Weak::new()),
        instance: Mutex::new(None),
        plan: make_plan(&ids),
        calls: Mutex::new(Vec::new()),
    });
    let decision: Arc<dyn DecisionEndpoint> = endpoint.clone();
    let routes = ReliquaryRuntimeRoutes::default().with_memory_retrieval_decision(Some(decision));
    let gateway = Arc::new_cyclic(|weak| {
        *endpoint.gateway.lock().unwrap() = weak.clone();
        GatewayRuntime::new(
            root.join("state"),
            routes,
            InsomniaWorkerConfig {
                workers: 0,
                ..Default::default()
            },
            EpisodePolicy::default(),
        )
    });
    let owner = gateway.mount_rel(InteractionRuntime::new(rel)).unwrap();
    let instance = gateway
        .attach_instance(
            "graph-concurrency-reader".into(),
            None,
            BTreeMap::from([(owner.clone(), BTreeSet::from([OwnerGrant::ViewRel]))]),
        )
        .unwrap();
    gateway.select_rel(&instance, &owner).unwrap();
    *endpoint.instance.lock().unwrap() = Some(Arc::clone(&instance));
    let refs = ids.map(|memory_id| MemoryRef {
        owner_id: owner.clone(),
        memory_id,
    });
    GraphFixture {
        gateway,
        instance,
        owner,
        anchors: [refs[0].clone(), refs[1].clone()],
        neighbors: [refs[2].clone(), refs[3].clone(), refs[4].clone()],
        endpoint,
    }
}

fn active_controller() -> AgenticMemoryController {
    let mut controller = AgenticMemoryController::new();
    controller
        .stage_query("complete graph query")
        .unwrap()
        .commit(&mut controller)
        .unwrap();
    controller
}

fn owner_state(fixture: &GraphFixture, instance: &InstanceRuntime) -> (u64, String) {
    let envelope = fixture.gateway.capture_agentic_read(instance).unwrap();
    fixture
        .gateway
        .with_rel(&envelope.owners[0].token, |runtime| {
            Ok((
                runtime.cva.rel_turn_count(),
                format!("{:?}", runtime.cva.freshness.records()),
            ))
        })
        .unwrap()
}

fn assert_owner_state_unchanged(fixture: &GraphFixture, before: (u64, String)) {
    let observer = fixture
        .gateway
        .attach_instance(
            "graph-concurrency-post-check".into(),
            None,
            BTreeMap::from([(fixture.owner.clone(), BTreeSet::from([OwnerGrant::ViewRel]))]),
        )
        .unwrap();
    fixture
        .gateway
        .select_rel(&observer, &fixture.owner)
        .unwrap();
    assert_eq!(before, owner_state(fixture, &observer));
}

fn response_ids(memory_refs: &[MemoryRef]) -> BTreeSet<String> {
    memory_refs
        .iter()
        .map(|memory| hex_id(memory.memory_id))
        .collect()
}

#[test]
fn concurrent_anchor_results_are_sorted_and_overlap_approvals_are_unioned() {
    let completed = Arc::new(Mutex::new(Vec::new()));
    let fixture = graph_fixture(|ids| DecisionPlan::InvertedCompletion {
        barrier: Arc::new(BoundedBarrier::new(2)),
        slower_anchor: hex_id(ids[0]),
        approve_by_anchor: HashMap::from([
            (
                hex_id(ids[0]),
                response_ids(&[MemoryRef {
                    owner_id: "".into(),
                    memory_id: ids[2],
                }]),
            ),
            (
                hex_id(ids[1]),
                BTreeSet::from([hex_id(ids[3]), hex_id(ids[4])]),
            ),
        ]),
        completed: Arc::clone(&completed),
    });
    let mut controller = active_controller();
    let result = fixture
        .gateway
        .traverse_graph(
            &fixture.instance,
            &mut controller,
            &fixture.anchors,
            "inverted-completion",
        )
        .unwrap();
    assert_eq!(
        *completed.lock().unwrap(),
        vec![
            hex_id(fixture.anchors[1].memory_id),
            hex_id(fixture.anchors[0].memory_id)
        ],
        "the second anchor should finish first"
    );
    let reports = &result.payload().anchors;
    assert_eq!(reports.len(), 2);
    let mut expected_anchors = fixture.anchors.clone();
    expected_anchors.sort();
    assert_eq!(
        reports
            .iter()
            .map(|report| report.anchor.clone())
            .collect::<Vec<_>>(),
        expected_anchors
    );
    let AgenticMemoryOutcome::Memories(content) = &result.payload().outcome else {
        panic!("approved shared and unique content expected");
    };
    let refs = content
        .iter()
        .map(|item| item.memory.clone())
        .collect::<Vec<_>>();
    assert_eq!(refs, {
        let mut expected = vec![
            fixture.neighbors[0].clone(),
            fixture.neighbors[1].clone(),
            fixture.neighbors[2].clone(),
        ];
        expected.sort();
        expected
    });
    assert_eq!(result.delivered().len(), 3);
    for report in reports {
        if report.anchor == fixture.anchors[0] {
            assert!(report.approved.contains(&fixture.neighbors[0]));
            assert!(!report.approved.contains(&fixture.neighbors[2]));
        } else {
            assert!(report.approved.contains(&fixture.neighbors[1]));
            assert!(report.approved.contains(&fixture.neighbors[2]));
        }
    }
    assert_eq!(controller.searched_ids().len(), 3);
    assert!(controller.selected_ids().is_empty());
}

fn failure_after_parallel_success_rolls_back(fault: Fault, delivery_id: &str) {
    let fixture = graph_fixture(|ids| DecisionPlan::SuccessThenFault {
        barrier: Arc::new(BoundedBarrier::new(2)),
        failing_anchor: hex_id(ids[1]),
        fault,
        successful_returned: Arc::new((Mutex::new(false), Condvar::new())),
    });
    let mut controller = active_controller();
    let before = controller.clone();
    assert!(
        fixture
            .gateway
            .traverse_graph(
                &fixture.instance,
                &mut controller,
                &fixture.anchors,
                delivery_id,
            )
            .is_err()
    );
    assert_eq!(controller, before);
}

#[test]
fn provider_error_after_other_anchor_approves_rolls_back_everything() {
    failure_after_parallel_success_rolls_back(Fault::Provider, "provider-error");
}

#[test]
fn worker_panic_after_other_anchor_approves_rolls_back_everything() {
    failure_after_parallel_success_rolls_back(Fault::Panic, "worker-panic");
}

#[test]
fn invalid_approval_after_other_anchor_approves_rolls_back_everything() {
    failure_after_parallel_success_rolls_back(Fault::InvalidApproval, "invalid-approval");
}

fn authorization_change_during_anchor_decision_rolls_back(
    action: AuthorizationChange,
    delivery_id: &str,
) {
    let fixture = graph_fixture(|_| DecisionPlan::ChangeAuthorization(action));
    let mut controller = active_controller();
    let before_controller = controller.clone();
    let before_owner = owner_state(&fixture, &fixture.instance);
    assert!(
        fixture
            .gateway
            .traverse_graph(
                &fixture.instance,
                &mut controller,
                &fixture.anchors[..1],
                delivery_id,
            )
            .is_err()
    );
    assert_eq!(controller, before_controller);
    assert_owner_state_unchanged(&fixture, before_owner);
}

#[test]
fn revocation_during_anchor_decision_discards_approved_content() {
    authorization_change_during_anchor_decision_rolls_back(
        AuthorizationChange::Revoke,
        "anchor-revoked",
    );
}

#[test]
fn detachment_during_anchor_decision_discards_approved_content() {
    authorization_change_during_anchor_decision_rolls_back(
        AuthorizationChange::Detach,
        "anchor-detached",
    );
}

#[test]
fn traversal_requires_active_query_and_authorized_existing_anchors() {
    let fixture = graph_fixture(|_| DecisionPlan::ChangeAuthorization(AuthorizationChange::Revoke));
    let mut empty = AgenticMemoryController::new();
    assert!(
        fixture
            .gateway
            .traverse_graph(
                &fixture.instance,
                &mut empty,
                &fixture.anchors[..1],
                "no-query",
            )
            .is_err()
    );
    assert!(fixture.endpoint.calls.lock().unwrap().is_empty());

    let mut controller = active_controller();
    let before = controller.clone();
    let mut invalid = fixture.anchors[0].clone();
    invalid.owner_id = "ungranted-owner".into();
    assert!(
        fixture
            .gateway
            .traverse_graph(
                &fixture.instance,
                &mut controller,
                &[invalid],
                "unauthorized-anchor",
            )
            .is_err()
    );
    let mut nonexistent = fixture.anchors[0].clone();
    nonexistent.memory_id = crate::MemoryId([255; 32]);
    assert!(
        fixture
            .gateway
            .traverse_graph(
                &fixture.instance,
                &mut controller,
                &[nonexistent],
                "missing-anchor",
            )
            .is_err()
    );
    assert_eq!(controller, before);
    assert!(fixture.endpoint.calls.lock().unwrap().is_empty());
}
