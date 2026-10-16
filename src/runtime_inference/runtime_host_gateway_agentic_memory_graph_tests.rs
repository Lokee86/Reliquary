use super::*;
use crate::{GraphRelationKind, MemoryId};

fn fixture() -> (GatewayRuntime, Arc<InstanceRuntime>, Vec<MemoryRef>) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target/test-tmp")
        .join(format!("agentic-graph-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let mut rel = crate::Cva::create_project(root.join("owner.rel")).unwrap();
    let episode = crate::community_test_support::rel_episode(&mut rel);
    let ids: Vec<MemoryId> = (0..4)
        .map(|n| {
            crate::community_test_support::rel_memory(&mut rel, &episode, &format!("graph-{n}"))
        })
        .collect();
    for (a, b, kind) in [
        (0, 1, GraphRelationKind::Topical),
        (1, 0, GraphRelationKind::Causal),
        (0, 1, GraphRelationKind::References),
        (1, 2, GraphRelationKind::Factual),
    ] {
        rel.set_memory_relation(ids[a], ids[b], kind, true, rel.graph_version())
            .unwrap();
    }
    let gateway = GatewayRuntime::new(
        root.join("state"),
        ReliquaryRuntimeRoutes::default(),
        InsomniaWorkerConfig {
            workers: 0,
            ..Default::default()
        },
        EpisodePolicy::default(),
    );
    let owner = gateway.mount_rel(InteractionRuntime::new(rel)).unwrap();
    let instance = gateway
        .attach_instance(
            "graph-reader".into(),
            None,
            BTreeMap::from([(owner.clone(), BTreeSet::from([OwnerGrant::ViewRel]))]),
        )
        .unwrap();
    gateway.select_rel(&instance, &owner).unwrap();
    (
        gateway,
        instance,
        ids.into_iter()
            .map(|memory_id| MemoryRef {
                owner_id: owner.clone(),
                memory_id,
            })
            .collect(),
    )
}

#[test]
fn agentic_graph_evidence_is_complete_bidirectional_one_hop_and_owner_qualified() {
    let (gateway, instance, ids) = fixture();
    let envelope = gateway.capture_agentic_read(&instance).unwrap();
    let evidence = gateway.agentic_anchor_evidence(&envelope, &ids[0]).unwrap();
    assert_eq!(evidence.len(), 3);
    assert!(evidence.iter().all(|edge| edge.neighbor == ids[1]));
    assert_eq!(
        evidence
            .iter()
            .filter(|edge| edge.direction == GraphDirection::Incoming)
            .count(),
        1
    );
    assert_eq!(
        evidence
            .iter()
            .filter(|edge| edge.direction == GraphDirection::Outgoing)
            .count(),
        2
    );
    let mut invalid = ids[0].clone();
    invalid.owner_id = "ungranted-owner".into();
    assert!(
        gateway
            .agentic_anchor_evidence(&envelope, &invalid)
            .is_err()
    );
    invalid = ids[0].clone();
    invalid.memory_id = MemoryId([255; 32]);
    assert!(
        gateway
            .agentic_anchor_evidence(&envelope, &invalid)
            .is_err()
    );
}

#[test]
fn agentic_graph_counts_use_snapshot_and_selected_or_declined_anchors_are_legal() {
    let (gateway, instance, ids) = fixture();
    let envelope = gateway.capture_agentic_read(&instance).unwrap();
    let mut controller = crate::AgenticMemoryController::new();
    controller
        .stage_query("complete query")
        .unwrap()
        .commit(&mut controller)
        .unwrap();
    controller.selected_ids.insert(ids[0].clone());
    controller.searched_ids.insert(ids[1].clone());
    let before = controller.clone();
    let anchor = gateway
        .prepare_agentic_anchor(&envelope, &controller, &ids[0])
        .unwrap();
    assert_eq!(anchor.report.unseen_neighbors, 0);
    assert_eq!(anchor.report.declined_neighbors, 1);
    assert!(anchor.candidates.is_empty());
    let bridge = gateway
        .prepare_agentic_anchor(&envelope, &controller, &ids[1])
        .unwrap();
    assert_eq!(bridge.report.unseen_neighbors, 1);
    assert_eq!(bridge.report.declined_neighbors, 0);
    assert_eq!(bridge.candidates[0].memory, ids[2]);
    assert_eq!(controller, before);
}

struct ConcurrentApproval {
    arrived: std::sync::Barrier,
    completed: std::sync::atomic::AtomicUsize,
    invalid_anchor: serde_json::Value,
}
impl DecisionEndpoint for ConcurrentApproval {
    fn model(&self) -> &str {
        "concurrent-anchor"
    }
    fn evaluate(
        &self,
        state: &serde_json::Value,
        _: &serde_json::Value,
    ) -> Result<serde_json::Value, crate::DecisionEndpointError> {
        self.arrived.wait();
        self.completed
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        if state["anchor"] == self.invalid_anchor {
            return Ok(serde_json::json!({"version":99,"approved":[]}));
        }
        Ok(
            serde_json::json!({"version":1,"approved":state["candidates"].as_array().unwrap()
            .iter().map(|c|c["memory"].clone()).collect::<Vec<_>>()}),
        )
    }
}
#[test]
fn agentic_graph_decisions_run_concurrently_and_gather_all_before_error() {
    let (gateway, instance, ids) = fixture();
    let route = Arc::new(ConcurrentApproval {
        arrived: std::sync::Barrier::new(2),
        completed: std::sync::atomic::AtomicUsize::new(0),
        invalid_anchor: crate::agentic_memory_retrieval_decision::memory_ref_value(&ids[2]),
    });
    *gateway.routes.write().unwrap() =
        ReliquaryRuntimeRoutes::default().with_memory_retrieval_decision(Some(route.clone()));
    let envelope = gateway.capture_agentic_read(&instance).unwrap();
    let mut controller = crate::AgenticMemoryController::new();
    controller
        .stage_query("query")
        .unwrap()
        .commit(&mut controller)
        .unwrap();
    let before = controller.clone();
    let anchors = [&ids[0], &ids[2]]
        .into_iter()
        .map(|id| {
            gateway
                .prepare_agentic_anchor(&envelope, &controller, id)
                .unwrap()
        })
        .collect();
    assert!(gateway.decide_agentic_anchors("query", anchors).is_err());
    assert_eq!(route.completed.load(std::sync::atomic::Ordering::SeqCst), 2);
    assert_eq!(controller, before);
}
