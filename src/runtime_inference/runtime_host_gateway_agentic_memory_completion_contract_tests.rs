use super::*;
use crate::{
    AgenticMemoryHandoffConfirmation, AgenticMemoryOutcome, DecisionEndpointError,
    GraphRelationKind, MemoryDraft, MemoryId, MemoryRef,
};
use serde_json::{Value, json};

struct ApproveEverything;
impl DecisionEndpoint for ApproveEverything {
    fn model(&self) -> &str {
        "completion-contract-test"
    }
    fn evaluate(&self, state: &Value, _: &Value) -> Result<Value, DecisionEndpointError> {
        Ok(
            json!({"version": 1, "approved": state["candidates"].as_array().unwrap()
            .iter().map(|candidate| candidate["memory"].clone()).collect::<Vec<_>>() }),
        )
    }
}

struct TwoOwnerFixture {
    gateway: GatewayRuntime,
    instance: Arc<InstanceRuntime>,
    controller: AgenticMemoryController,
    prepared: PreparedAgenticMemoryResult,
    root_owner: String,
    dependency_owner: String,
    root_neighbors: [MemoryId; 2],
    dependency_neighbor: MemoryId,
}

fn path(label: &str) -> PathBuf {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target/test-tmp")
        .join(format!(
            "agentic-completion-contract-{label}-{}",
            Uuid::new_v4()
        ));
    std::fs::create_dir_all(&root).unwrap();
    root
}

fn memory_ref(owner: &str, id: MemoryId) -> MemoryRef {
    MemoryRef {
        owner_id: owner.into(),
        memory_id: id,
    }
}

fn query_controller() -> AgenticMemoryController {
    let mut controller = AgenticMemoryController::new();
    controller
        .stage_query("complete query context")
        .unwrap()
        .commit(&mut controller)
        .unwrap();
    controller
}

fn successful(prepared: &PreparedAgenticMemoryResult) -> AgenticMemoryHandoffConfirmation {
    AgenticMemoryHandoffConfirmation::Succeeded {
        delivery_id: prepared.delivery_id().into(),
        manifest_digest: prepared.manifest_digest(),
    }
}

fn two_owner_fixture() -> TwoOwnerFixture {
    let root = path("two-owner");
    let mut dependency = crate::Cva::create_project(root.join("dependency.rel")).unwrap();
    let dependency_episode = crate::community_test_support::rel_episode(&mut dependency);
    let dependency_anchor = crate::community_test_support::rel_memory(
        &mut dependency,
        &dependency_episode,
        "dependency anchor",
    );
    let dependency_neighbor = crate::community_test_support::rel_memory(
        &mut dependency,
        &dependency_episode,
        "dependency delivery",
    );
    dependency
        .set_memory_relation(
            dependency_anchor,
            dependency_neighbor,
            GraphRelationKind::Topical,
            true,
            dependency.graph_version(),
        )
        .unwrap();
    let gateway = GatewayRuntime::new(
        root.join("state"),
        ReliquaryRuntimeRoutes::default()
            .with_memory_retrieval_decision(Some(Arc::new(ApproveEverything))),
        InsomniaWorkerConfig {
            workers: 0,
            ..Default::default()
        },
        EpisodePolicy::default(),
    );
    let dependency_owner = gateway
        .mount_rel(InteractionRuntime::new(dependency))
        .unwrap();

    let mut root_rel = crate::Cva::create_project(root.join("root.rel")).unwrap();
    let root_episode = crate::community_test_support::rel_episode(&mut root_rel);
    let root_anchor =
        crate::community_test_support::rel_memory(&mut root_rel, &root_episode, "root anchor");
    let root_a =
        crate::community_test_support::rel_memory(&mut root_rel, &root_episode, "root delivery a");
    let root_b =
        crate::community_test_support::rel_memory(&mut root_rel, &root_episode, "root delivery b");
    root_rel
        .set_memory_relation(
            root_anchor,
            root_a,
            GraphRelationKind::Causal,
            true,
            root_rel.graph_version(),
        )
        .unwrap();
    root_rel
        .set_memory_relation(
            root_anchor,
            root_b,
            GraphRelationKind::References,
            true,
            root_rel.graph_version(),
        )
        .unwrap();
    root_rel
        .set_rel_metadata(None, vec![dependency_owner.clone()])
        .unwrap();
    let root_owner = gateway
        .mount_rel(InteractionRuntime::new(root_rel))
        .unwrap();
    let instance = gateway
        .attach_instance(
            "completion-contract-reader".into(),
            None,
            BTreeMap::from([
                (root_owner.clone(), BTreeSet::from([OwnerGrant::ViewRel])),
                (
                    dependency_owner.clone(),
                    BTreeSet::from([OwnerGrant::ViewRel]),
                ),
            ]),
        )
        .unwrap();
    gateway.select_rel(&instance, &root_owner).unwrap();
    let mut controller = query_controller();
    let prepared = gateway
        .traverse_graph(
            &instance,
            &mut controller,
            &[
                memory_ref(&root_owner, root_anchor),
                memory_ref(&dependency_owner, dependency_anchor),
            ],
            "completion-contract-delivery",
        )
        .unwrap();
    TwoOwnerFixture {
        gateway,
        instance,
        controller,
        prepared,
        root_owner,
        dependency_owner,
        root_neighbors: [root_a, root_b],
        dependency_neighbor,
    }
}

fn advance_owner_to(
    gateway: &GatewayRuntime,
    instance: &InstanceRuntime,
    owner_id: &str,
    target: u64,
) {
    let envelope = gateway.capture_agentic_read(instance).unwrap();
    let token = &envelope
        .owners
        .iter()
        .find(|owner| owner.token.owner_id == owner_id)
        .unwrap()
        .token;
    gateway
        .with_rel(token, |runtime| {
            while runtime.cva.rel_turn_count() < target {
                let next = runtime.cva.rel_turn_count();
                runtime
                    .cva
                    .append_node(
                        format!("completion-contract-advance-{next}"),
                        "completion-contract".into(),
                        None,
                        "user".into(),
                        i64::try_from(next).unwrap(),
                        "advance accepted activity for retry",
                    )
                    .unwrap();
            }
            Ok(())
        })
        .unwrap();
}

#[test]
fn multi_owner_completion_groups_exact_owner_receipts_and_exact_retry_is_durable() {
    let fixture = two_owner_fixture();
    let mut controller = fixture.controller.clone();
    let payload = match &fixture.prepared.payload().outcome {
        AgenticMemoryOutcome::Memories(content) => content,
        other => panic!("expected delivered memories, got {other:?}"),
    };
    assert_eq!(payload.len(), 3);
    assert!(fixture.prepared.delivered().contains(&memory_ref(
        &fixture.dependency_owner,
        fixture.dependency_neighbor
    )));
    assert_eq!(
        payload
            .iter()
            .map(|item| item.memory.owner_id.clone())
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([fixture.root_owner.clone(), fixture.dependency_owner.clone()])
    );
    let envelope = fixture
        .gateway
        .capture_agentic_read(&fixture.instance)
        .unwrap();
    let checks = envelope
        .owners
        .iter()
        .map(|owner| {
            let token = owner.token.clone();
            let id = owner.token.owner_id.clone();
            let cut = fixture.prepared.owner_cuts[&id];
            let memories = fixture
                .prepared
                .delivered
                .iter()
                .filter(|item| item.owner_id == id)
                .map(|item| item.memory_id)
                .collect::<Vec<_>>();
            (token, id, cut, memories)
        })
        .collect::<Vec<_>>();
    fixture
        .gateway
        .complete_agentic_memory_delivery(
            &fixture.instance,
            &mut controller,
            &fixture.prepared,
            &successful(&fixture.prepared),
        )
        .unwrap();
    assert_eq!(controller.selected_ids(), fixture.prepared.delivered());
    let receipt_states = checks
        .iter()
        .map(|(token, id, cut, memories)| {
            fixture
                .gateway
                .with_rel(token, |runtime| {
                    let use_id = accepted_use_id(fixture.prepared.delivery_id(), id);
                    runtime
                        .cva
                        .freshness
                        .resolve_accepted_use(&use_id, *cut, memories)
                        .map(|receipt| receipt.is_some())
                        .map_err(operation)
                })
                .unwrap()
        })
        .collect::<Vec<_>>();
    assert_eq!(receipt_states, vec![true, true]);
    let after = checks
        .iter()
        .map(|(token, _, _, _)| {
            fixture
                .gateway
                .with_rel(token, |runtime| {
                    Ok(std::fs::metadata(runtime.cva.container.path())
                        .unwrap()
                        .len())
                })
                .unwrap()
        })
        .collect::<Vec<_>>();
    fixture
        .gateway
        .complete_agentic_memory_delivery(
            &fixture.instance,
            &mut controller,
            &fixture.prepared,
            &successful(&fixture.prepared),
        )
        .unwrap();
    let after_retry = checks
        .iter()
        .map(|(token, _, _, _)| {
            fixture
                .gateway
                .with_rel(token, |runtime| {
                    Ok(std::fs::metadata(runtime.cva.container.path())
                        .unwrap()
                        .len())
                })
                .unwrap()
        })
        .collect::<Vec<_>>();
    assert_eq!(after_retry, after);
}

#[test]
fn changed_owner_cut_with_matching_new_manifest_confirmation_conflicts_with_existing_receipt() {
    let fixture = two_owner_fixture();
    let mut controller = fixture.controller.clone();
    fixture
        .gateway
        .complete_agentic_memory_delivery(
            &fixture.instance,
            &mut controller,
            &fixture.prepared,
            &successful(&fixture.prepared),
        )
        .unwrap();
    let mut changed = fixture.prepared.clone();
    let cut = changed.owner_cuts.get_mut(&fixture.root_owner).unwrap();
    *cut += 1;
    advance_owner_to(
        &fixture.gateway,
        &fixture.instance,
        &fixture.root_owner,
        *cut,
    );
    let confirmation = successful(&changed);
    assert!(
        fixture
            .gateway
            .complete_agentic_memory_delivery(
                &fixture.instance,
                &mut controller,
                &changed,
                &confirmation
            )
            .is_err()
    );
    assert!(
        fixture
            .prepared
            .delivered()
            .is_subset(controller.selected_ids())
    );
}

#[test]
fn partial_receipt_failure_keeps_selection_and_exact_retry_finishes_all_owners() {
    let fixture = two_owner_fixture();
    let mut controller = fixture.controller.clone();
    let mut retry_manifest = fixture.prepared.clone();
    let retry_cut = retry_manifest
        .owner_cuts
        .get_mut(&fixture.root_owner)
        .unwrap();
    *retry_cut += 1;
    let retry_turn = *retry_cut;
    let confirmation = successful(&retry_manifest);

    assert!(
        fixture
            .gateway
            .complete_agentic_memory_delivery(
                &fixture.instance,
                &mut controller,
                &retry_manifest,
                &confirmation,
            )
            .is_err()
    );
    assert_eq!(controller.selected_ids(), retry_manifest.delivered());

    advance_owner_to(
        &fixture.gateway,
        &fixture.instance,
        &fixture.root_owner,
        retry_turn,
    );
    fixture
        .gateway
        .complete_agentic_memory_delivery(
            &fixture.instance,
            &mut controller,
            &retry_manifest,
            &confirmation,
        )
        .unwrap();
    assert_eq!(controller.selected_ids(), retry_manifest.delivered());

    let envelope = fixture
        .gateway
        .capture_agentic_read(&fixture.instance)
        .unwrap();
    for owner in &envelope.owners {
        let delivered = retry_manifest
            .delivered()
            .iter()
            .filter(|memory| memory.owner_id == owner.token.owner_id)
            .map(|memory| memory.memory_id)
            .collect::<Vec<_>>();
        let use_id = accepted_use_id(retry_manifest.delivery_id(), &owner.token.owner_id);
        let accepted_turn = retry_manifest.owner_cuts[&owner.token.owner_id];
        let resolved = fixture
            .gateway
            .with_rel(&owner.token, |runtime| {
                runtime
                    .cva
                    .freshness
                    .resolve_accepted_use(&use_id, accepted_turn, &delivered)
                    .map(|receipt| receipt.is_some())
                    .map_err(operation)
            })
            .unwrap();
        assert!(
            resolved,
            "retry should complete the receipt for each delivered owner"
        );
    }
}

#[test]
fn changed_owner_qualified_refs_with_matching_confirmation_conflict_and_keep_selection() {
    let fixture = two_owner_fixture();
    let mut controller = fixture.controller.clone();
    fixture
        .gateway
        .complete_agentic_memory_delivery(
            &fixture.instance,
            &mut controller,
            &fixture.prepared,
            &successful(&fixture.prepared),
        )
        .unwrap();
    let mut changed = fixture.prepared.clone();
    let AgenticMemoryOutcome::Memories(content) = &mut changed.payload.outcome else {
        panic!()
    };
    let removed = memory_ref(&fixture.root_owner, fixture.root_neighbors[0]);
    content.retain(|item| item.memory != removed);
    changed.delivered.remove(&removed);
    let confirmation = successful(&changed);
    assert!(
        fixture
            .gateway
            .complete_agentic_memory_delivery(
                &fixture.instance,
                &mut controller,
                &changed,
                &confirmation
            )
            .is_err()
    );
    assert!(
        fixture
            .prepared
            .delivered()
            .is_subset(controller.selected_ids())
    );
}

fn legacy_memory_draft() -> MemoryDraft {
    MemoryDraft {
        category: "fact".into(),
        memory_type: "project".into(),
        authority_kind: "direct".into(),
        temporal_status: "current".into(),
        title: "Legacy without Freshness".into(),
        content: "legacy candidate has no Freshness record".into(),
        scope: "test".into(),
        lifecycle_state: "knowledge".into(),
        archived: false,
        superseded_by: None,
        parent_id: None,
        source_node_id: None,
        content_source_conversation_id: None,
        content_source_node_id: None,
        grounding_source_conversation_id: None,
        grounding_source_node_id: None,
        source_episode_id: None,
        source_time_ns: None,
        mutation_id: "completion-contract-legacy".into(),
        created_at_ns: 1,
        updated_at_ns: 1,
    }
}

#[test]
fn missing_freshness_receipt_failure_keeps_selected_delivery_without_enrolling_memory() {
    let root = path("missing-freshness");
    let mut rel = crate::Cva::create_project(root.join("legacy.rel")).unwrap();
    let episode = crate::community_test_support::rel_episode(&mut rel);
    let anchor = crate::community_test_support::rel_memory(&mut rel, &episode, "legacy anchor");
    let (legacy, created) = crate::cva_memory_publish::publish_memory_parts(
        &rel.archive,
        &mut rel.memories,
        &mut rel.container,
        None,
        0,
        legacy_memory_draft(),
    )
    .unwrap();
    assert!(created);
    assert!(rel.freshness_record(legacy.id).is_none());
    rel.set_memory_relation(
        anchor,
        legacy.id,
        GraphRelationKind::Topical,
        true,
        rel.graph_version(),
    )
    .unwrap();
    let gateway = GatewayRuntime::new(
        root.join("state"),
        ReliquaryRuntimeRoutes::default()
            .with_memory_retrieval_decision(Some(Arc::new(ApproveEverything))),
        InsomniaWorkerConfig {
            workers: 0,
            ..Default::default()
        },
        EpisodePolicy::default(),
    );
    let owner = gateway.mount_rel(InteractionRuntime::new(rel)).unwrap();
    let instance = gateway
        .attach_instance(
            "legacy-reader".into(),
            None,
            BTreeMap::from([(owner.clone(), BTreeSet::from([OwnerGrant::ViewRel]))]),
        )
        .unwrap();
    gateway.select_rel(&instance, &owner).unwrap();
    let mut controller = query_controller();
    let prepared = gateway
        .traverse_graph(
            &instance,
            &mut controller,
            &[memory_ref(&owner, anchor)],
            "legacy-delivery",
        )
        .unwrap();
    assert_eq!(
        prepared.delivered(),
        &BTreeSet::from([memory_ref(&owner, legacy.id)])
    );
    let envelope = gateway.capture_agentic_read(&instance).unwrap();
    let token = envelope.owners[0].token.clone();
    let error = gateway
        .complete_agentic_memory_delivery(
            &instance,
            &mut controller,
            &prepared,
            &successful(&prepared),
        )
        .unwrap_err();
    assert!(matches!(error, AgenticMemoryError::Handoff(_)));
    assert!(
        controller
            .selected_ids()
            .contains(&memory_ref(&owner, legacy.id))
    );
    assert!(
        gateway
            .with_rel(&token, |runtime| Ok(runtime
                .cva
                .freshness_record(legacy.id)
                .is_none()))
            .unwrap()
    );
}
