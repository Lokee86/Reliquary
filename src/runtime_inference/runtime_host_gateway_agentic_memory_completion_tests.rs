use super::*;
use crate::{DecisionEndpointError, GraphRelationKind, MemoryRef};
use serde_json::{Value, json};
struct Approve;
impl DecisionEndpoint for Approve {
    fn model(&self) -> &str {
        "completion-test"
    }
    fn evaluate(&self, state: &Value, _: &Value) -> Result<Value, DecisionEndpointError> {
        Ok(
            json!({"version":1,"approved":state["candidates"].as_array().unwrap().iter()
            .map(|body|body["memory"].clone()).collect::<Vec<_>>()}),
        )
    }
}
fn fixture() -> (
    GatewayRuntime,
    Arc<InstanceRuntime>,
    AgenticMemoryController,
    PreparedAgenticMemoryResult,
) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target/test-tmp")
        .join(format!("agentic-completion-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let mut rel = crate::Cva::create_project(root.join("owner.rel")).unwrap();
    let episode = crate::community_test_support::rel_episode(&mut rel);
    let a = crate::community_test_support::rel_memory(&mut rel, &episode, "anchor");
    let b = crate::community_test_support::rel_memory(&mut rel, &episode, "delivered");
    rel.set_memory_relation(a, b, GraphRelationKind::Topical, true, rel.graph_version())
        .unwrap();
    let gateway = GatewayRuntime::new(
        root.join("state"),
        ReliquaryRuntimeRoutes::default().with_memory_retrieval_decision(Some(Arc::new(Approve))),
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
    controller
        .stage_query("full query")
        .unwrap()
        .commit(&mut controller)
        .unwrap();
    let prepared = gateway
        .traverse_graph(
            &instance,
            &mut controller,
            &[MemoryRef {
                owner_id: owner,
                memory_id: a,
            }],
            "turn-1/tool-1",
        )
        .unwrap();
    (gateway, instance, controller, prepared)
}
fn success(prepared: &PreparedAgenticMemoryResult) -> AgenticMemoryHandoffConfirmation {
    AgenticMemoryHandoffConfirmation::Succeeded {
        delivery_id: prepared.delivery_id().into(),
        manifest_digest: prepared.manifest_digest(),
    }
}
#[test]
fn agentic_completion_rejects_non_success_or_mismatched_confirmation_without_state_change() {
    let (gateway, instance, mut controller, prepared) = fixture();
    let before = controller.clone();
    for confirmation in [
        AgenticMemoryHandoffConfirmation::Failed,
        AgenticMemoryHandoffConfirmation::Cancelled,
        AgenticMemoryHandoffConfirmation::Unknown,
        AgenticMemoryHandoffConfirmation::Succeeded {
            delivery_id: "other".into(),
            manifest_digest: prepared.manifest_digest(),
        },
        AgenticMemoryHandoffConfirmation::Succeeded {
            delivery_id: prepared.delivery_id().into(),
            manifest_digest: "wrong".into(),
        },
    ] {
        assert!(
            gateway
                .complete_agentic_memory_delivery(
                    &instance,
                    &mut controller,
                    &prepared,
                    &confirmation
                )
                .is_err()
        );
        assert_eq!(controller, before);
    }
}
#[test]
fn agentic_completion_selects_exact_delivered_refs_only_after_success_and_retries_safely() {
    let (gateway, instance, mut controller, prepared) = fixture();
    let before = controller.clone();
    assert!(before.selected_ids().is_empty());
    gateway
        .complete_agentic_memory_delivery(
            &instance,
            &mut controller,
            &prepared,
            &success(&prepared),
        )
        .unwrap();
    assert_eq!(controller.selected_ids(), prepared.delivered());
    assert_eq!(controller.current_query(), before.current_query());
    assert_eq!(controller.searched_ids(), before.searched_ids());
    let selected = controller.clone();
    gateway
        .complete_agentic_memory_delivery(
            &instance,
            &mut controller,
            &prepared,
            &success(&prepared),
        )
        .unwrap();
    assert_eq!(controller, selected);
}
#[test]
fn agentic_completion_rejects_changed_payload_and_inconsistent_manifest() {
    let (gateway, instance, mut controller, prepared) = fixture();
    let before = controller.clone();
    let confirmation = success(&prepared);
    let mut changed = prepared.clone();
    let AgenticMemoryOutcome::Memories(content) = &mut changed.payload.outcome else {
        panic!()
    };
    content[0].content.push('x');
    assert!(
        gateway
            .complete_agentic_memory_delivery(&instance, &mut controller, &changed, &confirmation)
            .is_err()
    );
    changed = prepared;
    changed.delivered.clear();
    assert!(
        gateway
            .complete_agentic_memory_delivery(
                &instance,
                &mut controller,
                &changed,
                &success(&changed)
            )
            .is_err()
    );
    assert_eq!(controller, before);
}
