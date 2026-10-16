use super::*;
use crate::{
    DecisionEndpointError, EmbeddingEndpoint, EmbeddingEndpointError, EmbeddingMode,
    SimulatedEmbeddingEndpoint, VectorNormalization,
};
use serde_json::{Value, json};
use std::sync::Weak;

#[derive(Clone, Copy)]
enum DuringDecision {
    Revoke,
    Detach,
}

struct MutateAuthorizationDuringDecision {
    gateway: Mutex<Weak<GatewayRuntime>>,
    instance: Mutex<Option<Arc<InstanceRuntime>>>,
    action: DuringDecision,
}

impl DecisionEndpoint for MutateAuthorizationDuringDecision {
    fn model(&self) -> &str {
        "agentic-query-authorization-test"
    }

    fn evaluate(&self, state: &Value, _: &Value) -> Result<Value, DecisionEndpointError> {
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
        match self.action {
            DuringDecision::Revoke => gateway
                .replace_instance_grants(&instance, BTreeMap::new())
                .expect("revocation during model evaluation should complete"),
            DuringDecision::Detach => {
                assert!(
                    gateway
                        .detach_instance(&instance)
                        .expect("detachment during model evaluation should complete")
                );
            }
        }
        Ok(json!({
            "version": 1,
            "approved": state["candidates"]
                .as_array()
                .expect("decision candidates are supplied")
                .iter()
                .map(|candidate| candidate["memory"].clone())
                .collect::<Vec<_>>()
        }))
    }
}

struct QueryOnlyEmbedding(SimulatedEmbeddingEndpoint);

impl EmbeddingEndpoint for QueryOnlyEmbedding {
    fn dimensions(&self) -> u32 {
        self.0.dimensions()
    }

    fn normalization(&self) -> VectorNormalization {
        self.0.normalization()
    }

    fn embed(
        &self,
        mode: EmbeddingMode,
        inputs: &[String],
    ) -> Result<Vec<Vec<f32>>, EmbeddingEndpointError> {
        if inputs
            .iter()
            .any(|input| input.starts_with("continuity compatibility "))
        {
            return Err(EmbeddingEndpointError::Failure(
                "compatibility profile probes are unavailable in this fixture".into(),
            ));
        }
        self.0.embed(mode, inputs)
    }
}

struct QueryFixture {
    gateway: Arc<GatewayRuntime>,
    instance: Arc<InstanceRuntime>,
    owner: String,
}

fn query_fixture(
    embedding_route: bool,
    decision_route: bool,
    establish_profile: bool,
    action: DuringDecision,
) -> QueryFixture {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target/test-tmp")
        .join(format!("agentic-query-auth-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();

    let mut rel = crate::Cva::create_project(root.join("owner.rel")).unwrap();
    let episode = crate::community_test_support::rel_episode(&mut rel);
    crate::community_test_support::rel_memory(&mut rel, &episode, "authorization needle");
    let embedding = Arc::new(SimulatedEmbeddingEndpoint::new(
        8,
        VectorNormalization::L2,
        421,
    ));
    let profile = rel
        .establish_compatibility_profile(embedding.as_ref())
        .unwrap();
    rel.build_missing_memory_vectors(profile.id, embedding.as_ref())
        .unwrap();

    let decision_holder = Arc::new(MutateAuthorizationDuringDecision {
        gateway: Mutex::new(Weak::new()),
        instance: Mutex::new(None),
        action,
    });
    let decision_endpoint: Arc<dyn DecisionEndpoint> = decision_holder.clone();
    let embedding_endpoint: Arc<dyn EmbeddingEndpoint + Send + Sync> = if establish_profile {
        embedding.clone()
    } else {
        Arc::new(QueryOnlyEmbedding((*embedding).clone()))
    };
    let routes = ReliquaryRuntimeRoutes::new(
        None,
        None,
        None,
        None,
        embedding_route.then_some(embedding_endpoint),
    )
    .with_memory_retrieval_decision(decision_route.then_some(decision_endpoint));

    let gateway = Arc::new_cyclic(|weak| {
        *decision_holder.gateway.lock().unwrap() = weak.clone();
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
    let grants = BTreeMap::from([(owner.clone(), BTreeSet::from([OwnerGrant::ViewRel]))]);
    let instance = gateway
        .attach_instance("authorization-reader".into(), None, grants)
        .unwrap();
    gateway.select_rel(&instance, &owner).unwrap();
    *decision_holder.instance.lock().unwrap() = Some(Arc::clone(&instance));

    if establish_profile && embedding_route {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        loop {
            if gateway.capture_agentic_read(&instance).unwrap().owners[0]
                .profile
                .is_some()
            {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "vector worker did not publish its profile"
            );
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }

    QueryFixture {
        gateway,
        instance,
        owner,
    }
}

fn assert_query_error_preserves_controller(fixture: &QueryFixture, expected_error: &str) {
    let mut controller = AgenticMemoryController::new();
    controller
        .stage_query("previous query")
        .unwrap()
        .commit(&mut controller)
        .unwrap();
    controller.searched_ids.insert(MemoryRef {
        owner_id: fixture.owner.clone(),
        memory_id: crate::MemoryId([17; 32]),
    });
    let before = controller.clone();
    let error = fixture
        .gateway
        .memory_query(
            &fixture.instance,
            &mut controller,
            "authorization needle",
            "auth-test",
        )
        .unwrap_err();
    assert!(format!("{error}").contains(expected_error));
    assert_eq!(controller, before);
}

#[test]
fn missing_dedicated_decision_route_preserves_controller() {
    let fixture = query_fixture(true, false, true, DuringDecision::Revoke);
    assert_query_error_preserves_controller(&fixture, "dedicated retrieval decision route");
}

#[test]
fn missing_query_embedding_route_preserves_controller() {
    let fixture = query_fixture(false, true, true, DuringDecision::Revoke);
    assert_query_error_preserves_controller(&fixture, "embedding route");
}

#[test]
fn missing_memory_compatibility_profile_preserves_controller() {
    let fixture = query_fixture(true, true, false, DuringDecision::Revoke);
    let mut controller = AgenticMemoryController::new();
    controller
        .stage_query("previous query")
        .unwrap()
        .commit(&mut controller)
        .unwrap();
    let before = controller.clone();
    let error = fixture
        .gateway
        .memory_query(
            &fixture.instance,
            &mut controller,
            "authorization needle",
            "missing-profile",
        )
        .unwrap_err();
    assert!(format!("{error}").contains("no established Memory compatibility profile"));
    assert_eq!(controller, before);
}

fn revocation_or_detachment_during_decision_fails_final_authorization(
    action: DuringDecision,
    label: &str,
) {
    let fixture = query_fixture(true, true, true, action);
    let envelope = fixture
        .gateway
        .capture_agentic_read(&fixture.instance)
        .unwrap();
    let before = fixture
        .gateway
        .with_rel(&envelope.owners[0].token, |runtime| {
            Ok((
                runtime.cva.rel_turn_count(),
                runtime.cva.freshness.records().clone(),
            ))
        })
        .unwrap();
    let mut controller = AgenticMemoryController::new();
    controller
        .stage_query("previous query")
        .unwrap()
        .commit(&mut controller)
        .unwrap();
    controller.searched_ids.insert(MemoryRef {
        owner_id: fixture.owner.clone(),
        memory_id: crate::MemoryId([18; 32]),
    });
    let controller_before = controller.clone();

    assert!(
        fixture
            .gateway
            .memory_query(
                &fixture.instance,
                &mut controller,
                "authorization needle",
                label,
            )
            .is_err()
    );
    assert_eq!(controller, controller_before);
    assert!(
        controller.selected_ids().is_empty(),
        "query preparation must not record accepted use"
    );

    // A fresh attachment verifies the durable owner state even when the original
    // attachment was revoked or detached while the decision endpoint ran.
    let observer = fixture
        .gateway
        .attach_instance(
            "authorization-post-check".into(),
            None,
            BTreeMap::from([(fixture.owner.clone(), BTreeSet::from([OwnerGrant::ViewRel]))]),
        )
        .unwrap();
    fixture
        .gateway
        .select_rel(&observer, &fixture.owner)
        .unwrap();
    let post_envelope = fixture.gateway.capture_agentic_read(&observer).unwrap();
    let after = fixture
        .gateway
        .with_rel(&post_envelope.owners[0].token, |runtime| {
            Ok((
                runtime.cva.rel_turn_count(),
                runtime.cva.freshness.records().clone(),
            ))
        })
        .unwrap();
    assert_eq!(
        before, after,
        "query preparation must not record accepted use"
    );
}

#[test]
fn revocation_during_decision_prevents_prepared_content_and_controller_commit() {
    revocation_or_detachment_during_decision_fails_final_authorization(
        DuringDecision::Revoke,
        "revoke-during-decision",
    );
}

#[test]
fn detachment_during_decision_prevents_prepared_content_and_controller_commit() {
    revocation_or_detachment_during_decision_fails_final_authorization(
        DuringDecision::Detach,
        "detach-during-decision",
    );
}
