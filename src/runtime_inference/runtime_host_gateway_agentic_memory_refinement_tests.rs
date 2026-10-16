use super::*;
use crate::{AgenticMemoryOutcome, DecisionEndpointError, MemoryRef};
use serde_json::{Value, json};

struct RefineAndApprove(Arc<Mutex<Vec<Value>>>);
impl DecisionEndpoint for RefineAndApprove {
    fn model(&self) -> &str {
        "refine-and-approve"
    }
    fn evaluate(&self, state: &Value, _: &Value) -> Result<Value, DecisionEndpointError> {
        self.0.lock().unwrap().push(state.clone());
        if state["operation"] == "refine_query_rewrite" {
            return Ok(json!({"version":1, "query":"corrected complete query"}));
        }
        assert_eq!(state["operation"], "refine_query");
        assert_eq!(state["query"], "corrected complete query");
        Ok(
            json!({"version":1, "approved":state["candidates"].as_array().unwrap().iter()
            .map(|candidate| candidate["memory"].clone()).collect::<Vec<_>>()}),
        )
    }
}

#[test]
fn refinement_reconsiders_more_than_ten_declined_without_an_embedding_route() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target/test-tmp")
        .join(format!("agentic-refine-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let mut rel = crate::Cva::create_project(root.join("owner.rel")).unwrap();
    let episode = crate::community_test_support::rel_episode(&mut rel);
    let mut ids = Vec::new();
    for n in 0..14 {
        ids.push(crate::community_test_support::rel_memory(
            &mut rel,
            &episode,
            &format!("declined-{n}"),
        ));
    }
    let calls = Arc::new(Mutex::new(Vec::new()));
    let routes = ReliquaryRuntimeRoutes::default()
        .with_memory_retrieval_decision(Some(Arc::new(RefineAndApprove(Arc::clone(&calls)))));
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
            BTreeMap::from([(owner.clone(), BTreeSet::from([OwnerGrant::ViewRel]))]),
        )
        .unwrap();
    gateway.select_rel(&instance, &owner).unwrap();
    let mut controller = AgenticMemoryController::new();
    let mut old = controller.stage_query("old full query").unwrap();
    old.record_evaluated(ids.iter().map(|memory_id| MemoryRef {
        owner_id: owner.clone(),
        memory_id: *memory_id,
    }))
    .unwrap();
    old.commit(&mut controller).unwrap();
    controller.selected_ids.insert(MemoryRef {
        owner_id: owner,
        memory_id: ids[0],
    });
    let searched = controller.searched_ids().clone();
    let selected = controller.selected_ids().clone();
    let envelope = gateway.capture_agentic_read(&instance).unwrap();
    let before = gateway
        .with_rel(&envelope.owners[0].token, |runtime| {
            Ok((
                runtime.cva.rel_turn_count(),
                runtime.cva.freshness.records().clone(),
            ))
        })
        .unwrap();
    let result = gateway
        .refine_query(
            &instance,
            &mut controller,
            "correct the interpretation",
            "refine-1",
        )
        .unwrap();
    let AgenticMemoryOutcome::Memories(content) = &result.payload().outcome else {
        panic!("content expected");
    };
    assert_eq!(content.len(), 13);
    assert_eq!(result.delivered().len(), 13);
    assert!(result.delivered().is_disjoint(&selected));
    assert_eq!(controller.current_query(), Some("corrected complete query"));
    assert_eq!(controller.searched_ids(), &searched);
    assert_eq!(controller.selected_ids(), &selected);
    let calls = calls.lock().unwrap();
    assert_eq!(calls.len(), 3);
    assert_eq!(calls[0]["current_query"], "old full query");
    assert_eq!(calls[1]["candidates"].as_array().unwrap().len(), 10);
    assert_eq!(calls[2]["candidates"].as_array().unwrap().len(), 3);
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
