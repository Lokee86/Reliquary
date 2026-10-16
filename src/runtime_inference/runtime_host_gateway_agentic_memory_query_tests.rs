use super::*;
use crate::{DecisionEndpointError, SimulatedEmbeddingEndpoint, VectorNormalization};
use serde_json::{Value, json};

struct ApproveBatch {
    calls: Arc<Mutex<Vec<Value>>>,
    invalid: bool,
}
impl DecisionEndpoint for ApproveBatch {
    fn model(&self) -> &str {
        "agentic-query-test"
    }
    fn evaluate(&self, state: &Value, _: &Value) -> Result<Value, DecisionEndpointError> {
        self.calls.lock().unwrap().push(state.clone());
        if self.invalid {
            return Ok(
                json!({"version": 1, "approved": [{"owner_id":"unknown", "memory_id":"bad"}]}),
            );
        }
        Ok(
            json!({"version": 1, "approved": state["candidates"].as_array().unwrap().iter()
            .map(|candidate| candidate["memory"].clone()).collect::<Vec<_>>()}),
        )
    }
}

fn fixture(invalid: bool) -> (GatewayRuntime, Arc<InstanceRuntime>, Arc<Mutex<Vec<Value>>>) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target/test-tmp")
        .join(format!("agentic-query-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let mut rel = crate::Cva::create_project(root.join("owner.rel")).unwrap();
    let episode = crate::community_test_support::rel_episode(&mut rel);
    for n in 0..14 {
        crate::community_test_support::rel_memory(&mut rel, &episode, &format!("needle-{n}"));
    }
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
    let calls = Arc::new(Mutex::new(Vec::new()));
    let decision = Arc::new(ApproveBatch {
        calls: Arc::clone(&calls),
        invalid,
    });
    let routes = ReliquaryRuntimeRoutes::new(None, None, None, None, Some(embedding))
        .with_memory_retrieval_decision(Some(decision));
    let gateway = GatewayRuntime::new(
        root.join("state"),
        routes,
        InsomniaWorkerConfig {
            workers: 0,
            ..Default::default()
        },
        EpisodePolicy::default(),
    );
    let owner = gateway.mount_rel(InteractionRuntime::new(rel)).unwrap();
    let instance = gateway
        .attach_instance(
            "reader".into(),
            None,
            BTreeMap::from([(owner, BTreeSet::from([OwnerGrant::ViewRel]))]),
        )
        .unwrap();
    let owner = gateway
        .rel_executions
        .read()
        .unwrap()
        .keys()
        .next()
        .unwrap()
        .clone();
    gateway.select_rel(&instance, &owner).unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        let read = gateway.capture_agentic_read(&instance).unwrap();
        if read.owners[0].profile.is_some() {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "vector worker did not publish its profile"
        );
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    (gateway, instance, calls)
}

#[test]
fn memory_query_prepares_at_most_ten_and_never_marks_selected_or_reinforces() {
    let (gateway, instance, calls) = fixture(false);
    let envelope = gateway.capture_agentic_read(&instance).unwrap();
    let before = gateway
        .with_rel(&envelope.owners[0].token, |runtime| {
            Ok((
                runtime.cva.rel_turn_count(),
                runtime.cva.freshness.records().clone(),
            ))
        })
        .unwrap();
    let mut controller = AgenticMemoryController::new();
    let prepared = gateway
        .memory_query(&instance, &mut controller, "needle", "query-1")
        .unwrap();
    let AgenticMemoryOutcome::Memories(content) = &prepared.payload().outcome else {
        panic!("content expected");
    };
    assert_eq!(content.len(), 10);
    assert_eq!(prepared.delivered().len(), 10);
    assert_eq!(controller.current_query(), Some("needle"));
    assert_eq!(controller.searched_ids().len(), 10);
    assert!(controller.selected_ids().is_empty());
    let calls = calls.lock().unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0]["query"], "needle");
    assert_eq!(calls[0]["candidates"].as_array().unwrap().len(), 10);
    let after = gateway
        .with_rel(&envelope.owners[0].token, |runtime| {
            Ok((
                runtime.cva.rel_turn_count(),
                runtime.cva.freshness.records().clone(),
            ))
        })
        .unwrap();
    assert_eq!(before, after);
}

#[test]
fn memory_query_rejects_unknown_approvals_without_mutating_turn_state() {
    let (gateway, instance, _) = fixture(true);
    let mut controller = AgenticMemoryController::new();
    controller
        .stage_query("old query")
        .unwrap()
        .commit(&mut controller)
        .unwrap();
    let before = controller.clone();
    assert!(
        gateway
            .memory_query(&instance, &mut controller, "needle", "query-bad")
            .is_err()
    );
    assert_eq!(controller, before);
}

#[test]
fn memory_query_selected_only_fallback_has_ids_no_content_or_decision_call() {
    let (gateway, instance, calls) = fixture(false);
    let envelope = gateway.capture_agentic_read(&instance).unwrap();
    let selected = gateway
        .with_rel(&envelope.owners[0].token, |runtime| {
            Ok(runtime
                .cva
                .memory_ids()
                .into_iter()
                .map(|memory_id| MemoryRef {
                    owner_id: envelope.owners[0].token.owner_id.clone(),
                    memory_id,
                })
                .collect::<BTreeSet<_>>())
        })
        .unwrap();
    let mut controller = AgenticMemoryController::new();
    controller.current_query = Some("old".into());
    controller.searched_ids = selected.clone();
    controller.selected_ids = selected.clone();
    let result = gateway
        .memory_query(&instance, &mut controller, "needle", "provided")
        .unwrap();
    assert_eq!(
        result.payload().outcome,
        AgenticMemoryOutcome::AlreadyProvided(selected.clone())
    );
    assert!(result.delivered().is_empty());
    assert!(calls.lock().unwrap().is_empty());
    assert_eq!(controller.current_query(), Some("needle"));
    assert!(controller.searched_ids().is_empty());
    assert_eq!(controller.selected_ids(), &selected);
}

#[test]
fn memory_query_mixed_new_results_suppress_previously_provided_report() {
    let (gateway, instance, calls) = fixture(false);
    let envelope = gateway.capture_agentic_read(&instance).unwrap();
    let old = gateway
        .with_rel(&envelope.owners[0].token, |runtime| {
            Ok(runtime
                .cva
                .memory_ids()
                .into_iter()
                .take(4)
                .map(|memory_id| MemoryRef {
                    owner_id: envelope.owners[0].token.owner_id.clone(),
                    memory_id,
                })
                .collect::<BTreeSet<_>>())
        })
        .unwrap();
    let mut controller = AgenticMemoryController::new();
    controller.selected_ids = old.clone();
    let result = gateway
        .memory_query(&instance, &mut controller, "needle", "mixed")
        .unwrap();
    let AgenticMemoryOutcome::Memories(content) = &result.payload().outcome else {
        panic!("new content expected");
    };
    assert_eq!(content.len(), 10);
    assert!(content.iter().all(|item| !old.contains(&item.memory)));
    assert_eq!(calls.lock().unwrap().len(), 1);
    assert_eq!(controller.selected_ids(), &old);
}

#[test]
fn memory_query_empty_scope_returns_no_matches_without_a_decision_route() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target/test-tmp")
        .join(format!("agentic-empty-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let embedding = Arc::new(SimulatedEmbeddingEndpoint::new(
        8,
        VectorNormalization::L2,
        421,
    ));
    let gateway = GatewayRuntime::new(
        root,
        ReliquaryRuntimeRoutes::new(None, None, None, None, Some(embedding)),
        InsomniaWorkerConfig {
            workers: 0,
            ..Default::default()
        },
        EpisodePolicy::default(),
    );
    let instance = gateway
        .attach_instance("empty-reader".into(), None, BTreeMap::new())
        .unwrap();
    let mut controller = AgenticMemoryController::new();
    controller.current_query = Some("old".into());
    controller.searched_ids.insert(MemoryRef {
        owner_id: "old-owner".into(),
        memory_id: crate::MemoryId([3; 32]),
    });
    let result = gateway
        .memory_query(&instance, &mut controller, "new", "empty")
        .unwrap();
    assert_eq!(result.payload().outcome, AgenticMemoryOutcome::NoMatches);
    assert!(result.delivered().is_empty());
    assert_eq!(controller.current_query(), Some("new"));
    assert!(controller.searched_ids().is_empty());
}
