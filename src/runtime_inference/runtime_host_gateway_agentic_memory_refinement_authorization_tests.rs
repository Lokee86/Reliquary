use super::*;
use crate::{AgenticMemoryController, DecisionEndpoint, MemoryRef};
use serde_json::{Value, json};
use std::sync::{Arc, Mutex, Weak};

#[derive(Clone, Copy)]
enum MutationTiming {
    DuringRewrite,
    DuringSecondSelectionRevoke,
    DuringSecondSelectionDetach,
}

struct MutateDuringRefinement {
    gateway: Mutex<Weak<GatewayRuntime>>,
    instance: Mutex<Option<Arc<InstanceRuntime>>>,
    timing: MutationTiming,
    selection_calls: Mutex<usize>,
    calls: Mutex<Vec<Value>>,
}

impl MutateDuringRefinement {
    fn mutate(&self) {
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
        match self.timing {
            MutationTiming::DuringRewrite | MutationTiming::DuringSecondSelectionRevoke => {
                gateway
                    .replace_instance_grants(&instance, BTreeMap::new())
                    .expect("revocation during model evaluation should complete");
            }
            MutationTiming::DuringSecondSelectionDetach => {
                assert!(
                    gateway
                        .detach_instance(&instance)
                        .expect("detachment during model evaluation should complete")
                );
            }
        }
    }

    fn approvals(state: &Value) -> Value {
        json!({
            "version": 1,
            "approved": state["candidates"]
                .as_array()
                .expect("selection batch is supplied")
                .iter()
                .map(|candidate| candidate["memory"].clone())
                .collect::<Vec<_>>()
        })
    }
}

impl DecisionEndpoint for MutateDuringRefinement {
    fn model(&self) -> &str {
        "agentic-refinement-authorization-test"
    }

    fn evaluate(&self, state: &Value, _: &Value) -> Result<Value, DecisionEndpointError> {
        self.calls.lock().unwrap().push(state.clone());
        if state["operation"] == "refine_query_rewrite" {
            if matches!(self.timing, MutationTiming::DuringRewrite) {
                self.mutate();
            }
            return Ok(json!({
                "version": 1,
                "query": "corrected complete query"
            }));
        }
        assert_eq!(state["operation"], "refine_query");
        assert_eq!(state["query"], "corrected complete query");
        let selection_call = {
            let mut calls = self.selection_calls.lock().unwrap();
            *calls += 1;
            *calls
        };
        if selection_call == 2
            && matches!(
                self.timing,
                MutationTiming::DuringSecondSelectionRevoke
                    | MutationTiming::DuringSecondSelectionDetach
            )
        {
            self.mutate();
        }
        Ok(Self::approvals(state))
    }
}

struct RefinementFixture {
    gateway: Arc<GatewayRuntime>,
    instance: Arc<InstanceRuntime>,
    owner: String,
    memory_ids: Vec<crate::MemoryId>,
    decision: Arc<MutateDuringRefinement>,
}

fn refinement_fixture(timing: MutationTiming) -> RefinementFixture {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target/test-tmp")
        .join(format!("agentic-refine-auth-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let mut rel = crate::Cva::create_project(root.join("owner.rel")).unwrap();
    let episode = crate::community_test_support::rel_episode(&mut rel);
    let memory_ids = (0..14)
        .map(|n| {
            crate::community_test_support::rel_memory(&mut rel, &episode, &format!("declined-{n}"))
        })
        .collect::<Vec<_>>();

    let decision = Arc::new(MutateDuringRefinement {
        gateway: Mutex::new(Weak::new()),
        instance: Mutex::new(None),
        timing,
        selection_calls: Mutex::new(0),
        calls: Mutex::new(Vec::new()),
    });
    let endpoint: Arc<dyn DecisionEndpoint> = decision.clone();
    let routes = ReliquaryRuntimeRoutes::default().with_memory_retrieval_decision(Some(endpoint));
    let gateway = Arc::new_cyclic(|weak| {
        *decision.gateway.lock().unwrap() = weak.clone();
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
            "refinement-reader".into(),
            None,
            BTreeMap::from([(owner.clone(), BTreeSet::from([OwnerGrant::ViewRel]))]),
        )
        .unwrap();
    gateway.select_rel(&instance, &owner).unwrap();
    *decision.instance.lock().unwrap() = Some(Arc::clone(&instance));

    RefinementFixture {
        gateway,
        instance,
        owner,
        memory_ids,
        decision,
    }
}

fn controller_with_fourteen_declined(fixture: &RefinementFixture) -> AgenticMemoryController {
    let mut controller = AgenticMemoryController::new();
    let mut staged = controller.stage_query("old full query").unwrap();
    staged
        .record_evaluated(fixture.memory_ids.iter().map(|memory_id| MemoryRef {
            owner_id: fixture.owner.clone(),
            memory_id: *memory_id,
        }))
        .unwrap();
    staged.commit(&mut controller).unwrap();
    assert_eq!(controller.declined_ids().len(), 14);
    controller
}

fn assert_owner_state_unchanged(fixture: &RefinementFixture, before: (u64, String)) {
    let observer = fixture
        .gateway
        .attach_instance(
            "refinement-post-check".into(),
            None,
            BTreeMap::from([(fixture.owner.clone(), BTreeSet::from([OwnerGrant::ViewRel]))]),
        )
        .unwrap();
    fixture
        .gateway
        .select_rel(&observer, &fixture.owner)
        .unwrap();
    let envelope = fixture.gateway.capture_agentic_read(&observer).unwrap();
    let after = fixture
        .gateway
        .with_rel(&envelope.owners[0].token, |runtime| {
            Ok((
                runtime.cva.rel_turn_count(),
                format!("{:?}", runtime.cva.freshness.records()),
            ))
        })
        .unwrap();
    assert_eq!(before, after, "refinement must not record accepted use");
}

fn owner_state(fixture: &RefinementFixture) -> (u64, String) {
    let envelope = fixture
        .gateway
        .capture_agentic_read(&fixture.instance)
        .unwrap();
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

fn assert_selection_completed_in_two_batches(fixture: &RefinementFixture) {
    let calls = fixture.decision.calls.lock().unwrap();
    assert_eq!(calls.len(), 3, "rewrite and both selection batches ran");
    assert_eq!(calls[0]["operation"], "refine_query_rewrite");
    assert_eq!(calls[0]["current_query"], "old full query");
    assert_eq!(calls[1]["operation"], "refine_query");
    assert_eq!(calls[1]["candidates"].as_array().unwrap().len(), 10);
    assert_eq!(calls[2]["operation"], "refine_query");
    assert_eq!(calls[2]["candidates"].as_array().unwrap().len(), 4);
}

#[test]
fn revocation_during_rewrite_prevents_reconsideration_and_preserves_state() {
    let fixture = refinement_fixture(MutationTiming::DuringRewrite);
    let mut controller = controller_with_fourteen_declined(&fixture);
    let before_controller = controller.clone();
    let before_owner = owner_state(&fixture);

    assert!(
        fixture
            .gateway
            .refine_query(
                &fixture.instance,
                &mut controller,
                "correct the interpretation",
                "rewrite-revoked",
            )
            .is_err()
    );
    assert_eq!(controller, before_controller);
    assert_eq!(fixture.decision.calls.lock().unwrap().len(), 1);
    assert_owner_state_unchanged(&fixture, before_owner);
}

fn second_batch_authorization_change_preserves_state(timing: MutationTiming, id: &str) {
    let fixture = refinement_fixture(timing);
    let mut controller = controller_with_fourteen_declined(&fixture);
    let before_controller = controller.clone();
    let before_owner = owner_state(&fixture);

    assert!(
        fixture
            .gateway
            .refine_query(
                &fixture.instance,
                &mut controller,
                "correct the interpretation",
                id,
            )
            .is_err()
    );
    assert_eq!(controller, before_controller);
    assert_selection_completed_in_two_batches(&fixture);
    assert_owner_state_unchanged(&fixture, before_owner);
}

#[test]
fn revocation_during_second_selection_batch_discards_prepared_content() {
    second_batch_authorization_change_preserves_state(
        MutationTiming::DuringSecondSelectionRevoke,
        "second-batch-revoked",
    );
}

#[test]
fn detachment_during_second_selection_batch_discards_prepared_content() {
    second_batch_authorization_change_preserves_state(
        MutationTiming::DuringSecondSelectionDetach,
        "second-batch-detached",
    );
}
