use super::*;
use crate::{
    AgenticMemoryController, AgenticMemoryHandoffConfirmation, AgenticMemoryOutcome,
    DecisionEndpoint, DecisionEndpointError, GraphRelationKind, MemoryRef,
    PreparedAgenticMemoryResult, SimulatedEmbeddingEndpoint, VectorNormalization,
};
use serde_json::{Value, json};

struct ApproveAndRewrite;

impl DecisionEndpoint for ApproveAndRewrite {
    fn model(&self) -> &str {
        "agentic-completion-authorization-test"
    }

    fn evaluate(&self, state: &Value, _: &Value) -> Result<Value, DecisionEndpointError> {
        if state["operation"] == "refine_query_rewrite" {
            return Ok(json!({"version": 1, "query": "corrected complete query"}));
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

#[derive(Clone, Debug, Eq, PartialEq)]
struct OwnerState {
    accepted_turn: u64,
    freshness_records: String,
    receipt_exists: bool,
}

struct ReceiptProbe {
    use_id: String,
    accepted_turn: u64,
    memory_ids: Vec<crate::MemoryId>,
}

fn receipt_probe(prepared: &PreparedAgenticMemoryResult, owner: &str) -> ReceiptProbe {
    ReceiptProbe {
        use_id: super::accepted_use_id(prepared.delivery_id(), owner),
        accepted_turn: prepared.owner_cuts[owner],
        memory_ids: prepared
            .delivered
            .iter()
            .filter(|memory| memory.owner_id == owner)
            .map(|memory| memory.memory_id)
            .collect(),
    }
}

fn state_from_cva(cva: &crate::Cva, probe: &ReceiptProbe) -> Result<OwnerState, String> {
    Ok(OwnerState {
        accepted_turn: cva.rel_turn_count(),
        freshness_records: format!("{:?}", cva.freshness.records()),
        receipt_exists: cva
            .freshness
            .resolve_accepted_use(&probe.use_id, probe.accepted_turn, &probe.memory_ids)?
            .is_some(),
    })
}

fn owner_state(
    gateway: &GatewayRuntime,
    instance: &InstanceRuntime,
    owner: &str,
    probe: &ReceiptProbe,
) -> OwnerState {
    let envelope = gateway.capture_agentic_read(instance).unwrap();
    let read = envelope
        .owners
        .iter()
        .find(|read| read.token.owner_id == owner)
        .expect("observer has access to the owner");
    gateway
        .with_rel(&read.token, |runtime| {
            state_from_cva(&runtime.cva, probe).map_err(operation)
        })
        .unwrap()
}

fn assert_owner_state_unchanged(
    gateway: &GatewayRuntime,
    owner: &str,
    probe: &ReceiptProbe,
    before: &OwnerState,
) {
    let observer = gateway
        .attach_instance(
            "completion-post-check".into(),
            None,
            BTreeMap::from([(owner.to_owned(), BTreeSet::from([OwnerGrant::ViewRel]))]),
        )
        .unwrap();
    gateway.select_rel(&observer, owner).unwrap();
    let after = owner_state(gateway, &observer, owner, probe);
    assert_eq!(before, &after);
}

fn success(prepared: &PreparedAgenticMemoryResult) -> AgenticMemoryHandoffConfirmation {
    AgenticMemoryHandoffConfirmation::Succeeded {
        delivery_id: prepared.delivery_id().into(),
        manifest_digest: prepared.manifest_digest(),
    }
}

struct CompletionFixture {
    gateway: GatewayRuntime,
    instance: Arc<InstanceRuntime>,
    owner: String,
    other_owner: String,
    controller: AgenticMemoryController,
    prepared: PreparedAgenticMemoryResult,
}

fn completion_fixture() -> CompletionFixture {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target/test-tmp")
        .join(format!("agentic-completion-auth-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let mut rel = crate::Cva::create_project(root.join("owner.rel")).unwrap();
    let episode = crate::community_test_support::rel_episode(&mut rel);
    let anchor = crate::community_test_support::rel_memory(&mut rel, &episode, "anchor");
    let delivered = crate::community_test_support::rel_memory(&mut rel, &episode, "delivered");
    rel.set_memory_relation(
        anchor,
        delivered,
        GraphRelationKind::Topical,
        true,
        rel.graph_version(),
    )
    .unwrap();
    let mut other_rel = crate::Cva::create_project(root.join("other.rel")).unwrap();
    let other_episode = crate::community_test_support::rel_episode(&mut other_rel);
    crate::community_test_support::rel_memory(&mut other_rel, &other_episode, "other owner");

    let gateway = GatewayRuntime::new(
        root.join("state"),
        ReliquaryRuntimeRoutes::default()
            .with_memory_retrieval_decision(Some(Arc::new(ApproveAndRewrite))),
        InsomniaWorkerConfig {
            workers: 0,
            ..Default::default()
        },
        EpisodePolicy::default(),
    );
    let owner = gateway.mount_rel(InteractionRuntime::new(rel)).unwrap();
    let other_owner = gateway
        .mount_rel(InteractionRuntime::new(other_rel))
        .unwrap();
    let grants = BTreeMap::from([
        (owner.clone(), BTreeSet::from([OwnerGrant::ViewRel])),
        (other_owner.clone(), BTreeSet::from([OwnerGrant::ViewRel])),
    ]);
    let instance = gateway
        .attach_instance("completion-reader".into(), None, grants)
        .unwrap();
    gateway.select_rel(&instance, &owner).unwrap();
    let mut controller = AgenticMemoryController::new();
    controller
        .stage_query("full graph query")
        .unwrap()
        .commit(&mut controller)
        .unwrap();
    let prepared = gateway
        .traverse_graph(
            &instance,
            &mut controller,
            &[MemoryRef {
                owner_id: owner.clone(),
                memory_id: anchor,
            }],
            "completion-auth-delivery",
        )
        .unwrap();
    CompletionFixture {
        gateway,
        instance,
        owner,
        other_owner,
        controller,
        prepared,
    }
}

#[derive(Clone, Copy, Debug)]
enum AuthorityChange {
    Revoke,
    Detach,
    ChangeSelectedScope,
    UnmountOwner,
}

fn completion_rejects_stale_authority_without_selection_or_receipt(change: AuthorityChange) {
    let fixture = completion_fixture();
    let probe = receipt_probe(&fixture.prepared, &fixture.owner);
    let before = owner_state(&fixture.gateway, &fixture.instance, &fixture.owner, &probe);
    assert!(!before.receipt_exists);
    let mut controller = fixture.controller.clone();
    let before_controller = controller.clone();
    let unmounted = match change {
        AuthorityChange::Revoke => {
            fixture
                .gateway
                .replace_instance_grants(&fixture.instance, BTreeMap::new())
                .unwrap();
            None
        }
        AuthorityChange::Detach => {
            assert!(fixture.gateway.detach_instance(&fixture.instance).unwrap());
            None
        }
        AuthorityChange::ChangeSelectedScope => {
            fixture
                .gateway
                .select_rel(&fixture.instance, &fixture.other_owner)
                .unwrap();
            None
        }
        AuthorityChange::UnmountOwner => Some(fixture.gateway.unmount_rel(&fixture.owner).unwrap()),
    };

    assert!(
        fixture
            .gateway
            .complete_agentic_memory_delivery(
                &fixture.instance,
                &mut controller,
                &fixture.prepared,
                &success(&fixture.prepared),
            )
            .is_err()
    );
    assert_eq!(controller, before_controller);
    if let Some(cva) = unmounted.as_ref() {
        assert_eq!(before, state_from_cva(cva, &probe).unwrap());
    } else {
        assert_owner_state_unchanged(&fixture.gateway, &fixture.owner, &probe, &before);
    }
}

#[test]
fn completion_rejects_revoked_authority_without_receipt_or_selected_ids() {
    completion_rejects_stale_authority_without_selection_or_receipt(AuthorityChange::Revoke);
}

#[test]
fn completion_rejects_detached_authority_without_receipt_or_selected_ids() {
    completion_rejects_stale_authority_without_selection_or_receipt(AuthorityChange::Detach);
}

#[test]
fn completion_rejects_changed_selected_scope_without_receipt_or_selected_ids() {
    completion_rejects_stale_authority_without_selection_or_receipt(
        AuthorityChange::ChangeSelectedScope,
    );
}

#[test]
fn completion_rejects_unmounted_authority_without_receipt_or_selected_ids() {
    completion_rejects_stale_authority_without_selection_or_receipt(AuthorityChange::UnmountOwner);
}

#[test]
fn failed_cancelled_and_unknown_handoffs_do_not_write_freshness_receipts() {
    let fixture = completion_fixture();
    let probe = receipt_probe(&fixture.prepared, &fixture.owner);
    let before = owner_state(&fixture.gateway, &fixture.instance, &fixture.owner, &probe);
    assert!(!before.receipt_exists);
    let mut controller = fixture.controller.clone();
    let before_controller = controller.clone();
    for confirmation in [
        AgenticMemoryHandoffConfirmation::Failed,
        AgenticMemoryHandoffConfirmation::Cancelled,
        AgenticMemoryHandoffConfirmation::Unknown,
    ] {
        assert!(
            fixture
                .gateway
                .complete_agentic_memory_delivery(
                    &fixture.instance,
                    &mut controller,
                    &fixture.prepared,
                    &confirmation,
                )
                .is_err()
        );
        assert_eq!(controller, before_controller);
        assert_eq!(
            owner_state(&fixture.gateway, &fixture.instance, &fixture.owner, &probe,),
            before
        );
    }
}

struct InternalFixture {
    gateway: GatewayRuntime,
    instance: Arc<InstanceRuntime>,
    owner: String,
    ids: Vec<crate::MemoryId>,
}

fn internal_fixture() -> InternalFixture {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target/test-tmp")
        .join(format!("agentic-internal-completion-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let mut rel = crate::Cva::create_project(root.join("owner.rel")).unwrap();
    let episode = crate::community_test_support::rel_episode(&mut rel);
    let ids = (0..4)
        .map(|n| {
            crate::community_test_support::rel_memory(&mut rel, &episode, &format!("needle-{n}"))
        })
        .collect::<Vec<_>>();
    rel.set_memory_relation(
        ids[0],
        ids[1],
        GraphRelationKind::Topical,
        true,
        rel.graph_version(),
    )
    .unwrap();
    let embedding = Arc::new(SimulatedEmbeddingEndpoint::new(
        8,
        VectorNormalization::L2,
        731,
    ));
    let profile = rel
        .establish_compatibility_profile(embedding.as_ref())
        .unwrap();
    rel.build_missing_memory_vectors(profile.id, embedding.as_ref())
        .unwrap();
    let gateway = GatewayRuntime::new(
        root.join("state"),
        ReliquaryRuntimeRoutes::new(None, None, None, None, Some(embedding))
            .with_memory_retrieval_decision(Some(Arc::new(ApproveAndRewrite))),
        InsomniaWorkerConfig {
            workers: 0,
            ..Default::default()
        },
        EpisodePolicy::default(),
    );
    let owner = gateway.mount_rel(InteractionRuntime::new(rel)).unwrap();
    let instance = gateway
        .attach_instance(
            "internal-operation-reader".into(),
            None,
            BTreeMap::from([(owner.clone(), BTreeSet::from([OwnerGrant::ViewRel]))]),
        )
        .unwrap();
    gateway.select_rel(&instance, &owner).unwrap();
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
    InternalFixture {
        gateway,
        instance,
        owner,
        ids,
    }
}

fn assert_no_internal_accepted_use(
    fixture: &InternalFixture,
    controller: &AgenticMemoryController,
    prepared: &PreparedAgenticMemoryResult,
    before: &OwnerState,
) {
    assert!(controller.selected_ids().is_empty());
    let probe = receipt_probe(prepared, &fixture.owner);
    assert!(
        !owner_state(&fixture.gateway, &fixture.instance, &fixture.owner, &probe).receipt_exists
    );
    let envelope = fixture
        .gateway
        .capture_agentic_read(&fixture.instance)
        .unwrap();
    let after = fixture
        .gateway
        .with_rel(&envelope.owners[0].token, |runtime| {
            Ok(OwnerState {
                accepted_turn: runtime.cva.rel_turn_count(),
                freshness_records: format!("{:?}", runtime.cva.freshness.records()),
                receipt_exists: false,
            })
        })
        .unwrap();
    assert_eq!(before.accepted_turn, after.accepted_turn);
    assert_eq!(before.freshness_records, after.freshness_records);
}

#[test]
fn internal_query_refinement_and_graph_preparation_never_reinforce_freshness() {
    let fixture = internal_fixture();

    let mut query_controller = AgenticMemoryController::new();
    query_controller
        .stage_query("old query")
        .unwrap()
        .commit(&mut query_controller)
        .unwrap();
    let query_before = owner_state(
        &fixture.gateway,
        &fixture.instance,
        &fixture.owner,
        &ReceiptProbe {
            use_id: "uncommitted-query-probe".into(),
            accepted_turn: 0,
            memory_ids: Vec::new(),
        },
    );
    let query = fixture
        .gateway
        .memory_query(
            &fixture.instance,
            &mut query_controller,
            "needle",
            "internal-query",
        )
        .unwrap();
    assert!(matches!(
        &query.payload().outcome,
        AgenticMemoryOutcome::Memories(_)
    ));
    assert_no_internal_accepted_use(&fixture, &query_controller, &query, &query_before);

    let mut refine_controller = AgenticMemoryController::new();
    let mut staged = refine_controller.stage_query("old query").unwrap();
    staged
        .record_evaluated(fixture.ids.iter().map(|memory_id| MemoryRef {
            owner_id: fixture.owner.clone(),
            memory_id: *memory_id,
        }))
        .unwrap();
    staged.commit(&mut refine_controller).unwrap();
    let refine_before = owner_state(
        &fixture.gateway,
        &fixture.instance,
        &fixture.owner,
        &ReceiptProbe {
            use_id: "uncommitted-refinement-probe".into(),
            accepted_turn: 0,
            memory_ids: Vec::new(),
        },
    );
    let refined = fixture
        .gateway
        .refine_query(
            &fixture.instance,
            &mut refine_controller,
            "correct the query",
            "internal-refinement",
        )
        .unwrap();
    assert!(matches!(
        &refined.payload().outcome,
        AgenticMemoryOutcome::Memories(_)
    ));
    assert_no_internal_accepted_use(&fixture, &refine_controller, &refined, &refine_before);

    let mut graph_controller = AgenticMemoryController::new();
    graph_controller
        .stage_query("graph query")
        .unwrap()
        .commit(&mut graph_controller)
        .unwrap();
    let graph_before = owner_state(
        &fixture.gateway,
        &fixture.instance,
        &fixture.owner,
        &ReceiptProbe {
            use_id: "uncommitted-graph-probe".into(),
            accepted_turn: 0,
            memory_ids: Vec::new(),
        },
    );
    let traversed = fixture
        .gateway
        .traverse_graph(
            &fixture.instance,
            &mut graph_controller,
            &[MemoryRef {
                owner_id: fixture.owner.clone(),
                memory_id: fixture.ids[0],
            }],
            "internal-graph",
        )
        .unwrap();
    assert!(matches!(
        &traversed.payload().outcome,
        AgenticMemoryOutcome::Memories(_)
    ));
    assert_no_internal_accepted_use(&fixture, &graph_controller, &traversed, &graph_before);
}
