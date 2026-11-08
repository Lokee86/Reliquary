use super::*;
use reliquary_application::{RetrievalAction, RetrievalConfig};
use reliquary_container::{Container, ReliquaryScope};
use reliquary_freshness_store::FreshnessStore;
use reliquary_memory_core::{MemoryDraft, MemoryStore};
use reliquary_switchboard::{DecisionEndpoint, DecisionEndpointError};
use serde_json::{Value, json};
use std::sync::Mutex;

struct Fixture {
    path: std::path::PathBuf,
    gateway: Gateway,
    owner: OwnerId,
}
impl Fixture {
    fn new() -> Self {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join(".test-tmp");
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join(format!("retrieval-{}.rel", Uuid::new_v4()));
        let raw = [133; 16];
        drop(RelWriter::create_path(&path, ReliquaryScope::Project, raw).unwrap());
        {
            let mut container = Container::open_writer(&path).unwrap();
            let mut memories = MemoryStore::open(&container).unwrap();
            let mut freshness = FreshnessStore::open(&container).unwrap();
            for (name, content) in [
                ("one", "needle first exact body"),
                ("two", "other second exact body"),
            ] {
                let published = memories
                    .publish_versioned_creation(
                        &mut container,
                        MemoryDraft {
                            title: name.into(),
                            content: content.into(),
                            category: "fact".into(),
                            memory_type: "semantic".into(),
                            authority_kind: "user".into(),
                            temporal_status: "current".into(),
                            scope: "project".into(),
                            lifecycle_state: "accepted".into(),
                            archived: false,
                            parent_id: None,
                            superseded_by: None,
                            source_node_id: None,
                            content_source_conversation_id: None,
                            content_source_node_id: None,
                            grounding_source_conversation_id: None,
                            grounding_source_node_id: None,
                            source_episode_id: None,
                            source_ref: None,
                            source_time_ns: None,
                            temporal_inference: None,
                            mutation_id: format!("fixture-{name}"),
                            created_at_ns: 1,
                            updated_at_ns: 1,
                        },
                        1,
                    )
                    .unwrap();
                freshness
                    .create_birth(
                        &mut container,
                        FreshnessMemoryId(published.revision.record.id.0),
                        0,
                        false,
                    )
                    .unwrap();
            }
        }
        let gateway = Gateway::new();
        let owner = gateway
            .mount_writable_reliquary(
                &path,
                &GraphAdmission::for_replay(raw, [], [], [], [], []),
                [],
                [],
            )
            .unwrap();
        Self {
            path,
            gateway,
            owner,
        }
    }
    fn alice(&self) -> Instance {
        self.gateway.instance("alice", [self.owner], []).unwrap()
    }
}

struct Approve;
impl DecisionEndpoint for Approve {
    fn model(&self) -> &str {
        "fixture"
    }
    fn evaluate(&self, state: &Value, _: &Value) -> Result<Value, DecisionEndpointError> {
        Ok(
            json!({"version":1,"approved":state["candidates"].as_array().unwrap().iter().map(|m|m["memory"].clone()).collect::<Vec<_>>()}),
        )
    }
}
fn route() -> MemoryRetrievalRoute {
    MemoryRetrievalRoute(Arc::new(Approve))
}
fn connect(gateway: &Gateway, host: Arc<dyn TrustedHostAdapter>) -> CapabilityRouter {
    TrustedHostAdmission::new([])
        .install(gateway)
        .unwrap()
        .0
        .connect_runtime_host(host)
}

#[derive(Default)]
struct Host {
    operations: Mutex<HashMap<String, MemoryDeliveryOperation>>,
    pending: AtomicBool,
    forged: AtomicBool,
    rejected: AtomicBool,
    calls: std::sync::atomic::AtomicUsize,
    revoke: Mutex<Option<Arc<AtomicBool>>>,
}
impl TrustedHostAdapter for Host {
    fn configured_capabilities(&self) -> BTreeSet<RuntimeCapability> {
        [RuntimeCapability::MemoryDelivery].into()
    }
    fn connect(&self) -> Result<HostConnection, String> {
        Ok(HostConnection {
            capabilities: [(
                RuntimeCapability::MemoryDelivery,
                HostCapabilityContract::pending_queue(
                    self.queue_guarantees(RuntimeCapability::MemoryDelivery),
                ),
            )]
            .into(),
        })
    }
    fn queue_guarantees(&self, _: RuntimeCapability) -> HostQueueGuarantees {
        HostQueueGuarantees {
            durable_acceptance: true,
            stable_submission_ids: true,
            linearizable_cancel_dequeue: true,
            queryable_operation_status: true,
            idempotent_handoff: true,
        }
    }
    fn reconcile(&self, _: RuntimeCapability) -> Result<HostReconcileStatus, String> {
        Ok(HostReconcileStatus::NoOutstandingWork)
    }
    fn handoff(&self, _: RuntimeCapability, _: &HostOperation) -> Result<HandoffReceipt, String> {
        Err("not a conversation host".into())
    }
    fn status(&self, _: RuntimeCapability, _: &str) -> Result<HostOperationStatus, String> {
        Ok(HostOperationStatus::Unknown)
    }
    fn cancel(&self, _: RuntimeCapability, _: &str) -> Result<HostCancelOutcome, String> {
        Ok(HostCancelOutcome::Unknown)
    }
    fn deliver_memory(
        &self,
        operation: &MemoryDeliveryOperation,
    ) -> Result<MemoryDeliveryReceipt, String> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        self.operations
            .lock()
            .unwrap()
            .insert(operation.stable_id.clone(), operation.clone());
        if let Some(active) = self.revoke.lock().unwrap().as_ref() {
            active.store(false, Ordering::Release);
        }
        Ok(MemoryDeliveryReceipt {
            stable_id: operation.stable_id.clone(),
            payload_digest: if self.forged.load(Ordering::Relaxed) {
                [0; 32]
            } else {
                operation.payload_digest
            },
            confirmed_consumption: !self.pending.load(Ordering::Relaxed),
        })
    }
    fn memory_delivery_status(&self, id: &str) -> Result<MemoryDeliveryStatus, String> {
        if self.rejected.load(Ordering::Relaxed) {
            return Ok(MemoryDeliveryStatus::Rejected);
        }
        Ok(match self.operations.lock().unwrap().get(id) {
            Some(_) if self.pending.load(Ordering::Relaxed) => MemoryDeliveryStatus::Pending,
            Some(operation) => MemoryDeliveryStatus::Confirmed(operation.clone()),
            None => MemoryDeliveryStatus::NotFound,
        })
    }
}

#[test]
fn passive_reads_and_preparation_never_credit_and_confirmed_retry_does_once() {
    let fixture = Fixture::new();
    let alice = fixture.alice();
    let turn = alice
        .begin_memory_retrieval_turn(fixture.owner, "turn-1")
        .unwrap();
    let route = route();
    let snapshot = alice.retrieval_snapshot(fixture.owner, None).unwrap();
    let rank = alice
        .rank_rel_memories(
            fixture.owner,
            "needle",
            None,
            None,
            RetrievalConfig::default(),
        )
        .unwrap();
    assert_eq!(rank.selected[0].body.content, "needle first exact body");
    let outcome = alice
        .prepare_agentic_memory_delivery(
            &turn,
            &route,
            RetrievalAction::Query("needle".into()),
            None,
            None,
        )
        .unwrap();
    assert_eq!(
        snapshot,
        alice.retrieval_snapshot(fixture.owner, None).unwrap()
    );
    let token = outcome.prepared.unwrap();
    let host = Arc::new(Host::default());
    host.pending.store(true, Ordering::Relaxed);
    let router = connect(&fixture.gateway, host.clone());
    assert!(
        alice
            .deliver_agentic_memory(&turn, &token, &router)
            .is_err()
    );
    assert_eq!(
        snapshot,
        alice.retrieval_snapshot(fixture.owner, None).unwrap()
    );
    host.pending.store(false, Ordering::Relaxed);
    assert!(
        alice
            .deliver_agentic_memory(&turn, &token, &router)
            .unwrap()
    );
    assert!(
        !alice
            .deliver_agentic_memory(&turn, &token, &router)
            .unwrap()
    );
    let operation = host
        .operations
        .lock()
        .unwrap()
        .values()
        .next()
        .unwrap()
        .clone();
    let manifest: Value = serde_json::from_slice(&operation.payload).unwrap();
    assert_eq!(
        manifest["memories"][0]["content"],
        "needle first exact body"
    );
    assert_eq!(manifest["memories"][0]["revision"], 1);
    assert_eq!(host.calls.load(Ordering::Relaxed), 1);
    assert!(
        !alice
            .recover_agentic_memory_delivery(&turn, &operation.stable_id, &router)
            .unwrap()
    );
    let again = alice
        .prepare_agentic_memory_delivery(
            &turn,
            &route,
            RetrievalAction::Query("needle".into()),
            None,
            None,
        )
        .unwrap();
    assert!(again.prepared.is_none());
    assert_eq!(again.already_provided.len(), 1);
    let second = alice
        .prepare_agentic_memory_delivery(
            &turn,
            &route,
            RetrievalAction::Query("other".into()),
            None,
            None,
        )
        .unwrap()
        .prepared
        .unwrap();
    assert!(
        alice
            .deliver_agentic_memory(&turn, &second, &router)
            .unwrap()
    );
    assert_eq!(host.operations.lock().unwrap().len(), 2);
    assert!(
        !alice
            .recover_agentic_memory_delivery(&turn, &operation.stable_id, &router)
            .unwrap()
    );
    {
        let state = turn.state.lock().unwrap();
        let state = &state.turn;
        assert_eq!(state.query(), Some("other"));
        assert_eq!(state.searched().len(), 1);
        assert_eq!(state.selected().len(), 2);
    }
    let other = alice
        .prepare_agentic_memory_delivery(
            &turn,
            &route,
            RetrievalAction::Query("other".into()),
            None,
            None,
        )
        .unwrap();
    assert!(other.prepared.is_none());
    assert_eq!(other.already_provided.len(), 1);
    drop(alice);
    drop(router);
    let owner = fixture.owner;
    let path = fixture.path.clone();
    drop(fixture);
    let reopened = Gateway::new();
    reopened
        .mount_writable_reliquary(
            &path,
            &GraphAdmission::for_replay(owner.as_bytes(), [], [], [], [], []),
            [],
            [],
        )
        .unwrap();
    let alice = reopened.instance("alice", [owner], []).unwrap();
    let turn = alice.begin_memory_retrieval_turn(owner, "turn-1").unwrap();
    let router = connect(&reopened, host.clone());
    assert!(
        !alice
            .recover_agentic_memory_delivery(&turn, &operation.stable_id, &router)
            .unwrap()
    );
    assert_eq!(host.calls.load(Ordering::Relaxed), 2);
}

#[test]
fn owner_instance_and_registered_host_bindings_are_enforced() {
    assert!(
        MemoryRetrievalRoute::resolve(&ModelSwitchboardConfig::default(), None, None, None)
            .unwrap()
            .is_none()
    );
    let fixture = Fixture::new();
    let alice = fixture.alice();
    let turn = alice
        .begin_memory_retrieval_turn(fixture.owner, "turn")
        .unwrap();
    let route = route();
    let denied = fixture.gateway.instance("alice", [], []).unwrap();
    assert!(matches!(
        denied.begin_memory_retrieval_turn(fixture.owner, "denied"),
        Err(GatewayError::UnauthorizedRel { .. })
    ));
    let token = alice
        .prepare_agentic_memory_delivery(
            &turn,
            &route,
            RetrievalAction::Query("needle".into()),
            None,
            None,
        )
        .unwrap()
        .prepared
        .unwrap();
    let host = Arc::new(Host::default());
    let router = connect(&fixture.gateway, host.clone());
    let other = fixture.alice();
    assert!(
        other
            .deliver_agentic_memory(&turn, &token, &router)
            .is_err()
    );
    let another_turn = alice
        .begin_memory_retrieval_turn(fixture.owner, "turn")
        .unwrap();
    assert!(
        alice
            .deliver_agentic_memory(&another_turn, &token, &router)
            .is_err()
    );
    let unrelated = connect(&Gateway::new(), host.clone());
    assert!(
        alice
            .deliver_agentic_memory(&turn, &token, &unrelated)
            .is_err()
    );
    assert_eq!(host.calls.load(Ordering::Relaxed), 0);
    host.forged.store(true, Ordering::Relaxed);
    assert!(
        alice
            .deliver_agentic_memory(&turn, &token, &router)
            .is_err()
    );
    let operation = host
        .operations
        .lock()
        .unwrap()
        .values()
        .next()
        .unwrap()
        .clone();
    assert!(
        alice
            .with_writable_reliquary(fixture.owner, |rel| rel
                .accepted_external_use(&operation.stable_id)
                .is_none())
            .unwrap()
    );
    assert!(
        alice
            .deliver_agentic_memory(&turn, &token, &router)
            .unwrap()
    );
    assert_eq!(host.calls.load(Ordering::Relaxed), 1);
}

#[test]
fn authorization_is_rechecked_after_model_and_after_host_confirmation() {
    struct Revoke(Arc<AtomicBool>);
    impl DecisionEndpoint for Revoke {
        fn model(&self) -> &str {
            "revoke"
        }
        fn evaluate(
            &self,
            state: &Value,
            questions: &Value,
        ) -> Result<Value, DecisionEndpointError> {
            self.0.store(false, Ordering::Release);
            Approve.evaluate(state, questions)
        }
    }
    let fixture = Fixture::new();
    let token = "a".repeat(32);
    let (admission, _) = TrustedHostAdmission::new([HostCredentialBinding::new(
        "alice",
        &token,
        [fixture.owner],
        [],
    )])
    .install(&fixture.gateway)
    .unwrap();
    let alice = admission.authenticate(&token).unwrap();
    let turn = alice
        .begin_memory_retrieval_turn(fixture.owner, "model-revoke")
        .unwrap();
    let revoke_route =
        MemoryRetrievalRoute(Arc::new(Revoke(alice.credential_active.clone().unwrap())));
    assert!(matches!(
        alice.prepare_agentic_memory_delivery(
            &turn,
            &revoke_route,
            RetrievalAction::Query("needle".into()),
            None,
            None
        ),
        Err(GatewayError::CredentialRevoked)
    ));
    drop(alice);
    drop(admission);

    let token = "b".repeat(32);
    let (admission, _) = TrustedHostAdmission::new([HostCredentialBinding::new(
        "alice",
        &token,
        [fixture.owner],
        [],
    )])
    .install(&fixture.gateway)
    .unwrap();
    let alice = admission.authenticate(&token).unwrap();
    let turn = alice
        .begin_memory_retrieval_turn(fixture.owner, "host-revoke")
        .unwrap();
    let prepared = alice
        .prepare_agentic_memory_delivery(
            &turn,
            &route(),
            RetrievalAction::Query("needle".into()),
            None,
            None,
        )
        .unwrap()
        .prepared
        .unwrap();
    let host = Arc::new(Host::default());
    *host.revoke.lock().unwrap() = alice.credential_active.clone();
    let router = admission.connect_runtime_host(host.clone());
    assert!(matches!(
        alice.deliver_agentic_memory(&turn, &prepared, &router),
        Err(GatewayError::CredentialRevoked)
    ));
    let operation = host
        .operations
        .lock()
        .unwrap()
        .values()
        .next()
        .unwrap()
        .clone();
    let replacement = fixture.alice();
    assert!(
        replacement
            .with_writable_reliquary(fixture.owner, |rel| rel
                .accepted_external_use(&operation.stable_id)
                .is_none())
            .unwrap()
    );
    let path = fixture.path.clone();
    let owner = fixture.owner;
    drop(replacement);
    drop(alice);
    drop(admission);
    drop(router);
    drop(fixture);
    let reopened = Gateway::new();
    reopened
        .mount_writable_reliquary(
            &path,
            &GraphAdmission::for_replay(owner.as_bytes(), [], [], [], [], []),
            [],
            [],
        )
        .unwrap();
    let replacement = reopened.instance("alice", [owner], []).unwrap();
    let turn = replacement
        .begin_memory_retrieval_turn(owner, "host-revoke")
        .unwrap();
    let router = connect(&reopened, host.clone());
    assert!(
        replacement
            .recover_agentic_memory_delivery(&turn, &operation.stable_id, &router)
            .unwrap()
    );
    assert!(
        !replacement
            .recover_agentic_memory_delivery(&turn, &operation.stable_id, &router)
            .unwrap()
    );
    assert_eq!(host.calls.load(Ordering::Relaxed), 1);
}

#[test]
fn readonly_preparation_cannot_deliver_and_terminal_rejection_releases_turn() {
    let fixture = Fixture::new();
    let alice = fixture.alice();
    let turn = alice
        .begin_memory_retrieval_turn(fixture.owner, "turn")
        .unwrap();
    let host = Arc::new(Host::default());
    let router = connect(&fixture.gateway, host.clone());
    let prepared = alice
        .prepare_agentic_memory_delivery(
            &turn,
            &route(),
            RetrievalAction::Query("needle".into()),
            None,
            None,
        )
        .unwrap()
        .prepared
        .unwrap();
    host.rejected.store(true, Ordering::Relaxed);
    assert!(
        alice
            .deliver_agentic_memory(&turn, &prepared, &router)
            .is_err()
    );
    let next = alice
        .prepare_agentic_memory_delivery(
            &turn,
            &route(),
            RetrievalAction::Query("other".into()),
            None,
            None,
        )
        .unwrap();
    assert!(next.prepared.is_some());
    assert_eq!(host.calls.load(Ordering::Relaxed), 0);
    let path = fixture.path.clone();
    let owner = fixture.owner;
    drop(alice);
    drop(router);
    drop(fixture);
    let gateway = Gateway::new();
    gateway
        .mount_reliquary(
            &path,
            &GraphAdmission::for_replay(owner.as_bytes(), [], [], [], [], []),
            [],
            [],
        )
        .unwrap();
    let alice = gateway.instance("alice", [owner], []).unwrap();
    let turn = alice
        .begin_memory_retrieval_turn(owner, "read-only")
        .unwrap();
    assert_eq!(
        alice
            .rank_rel_memories(owner, "needle", None, None, RetrievalConfig::default())
            .unwrap()
            .selected
            .len(),
        1
    );
    let prepared = alice
        .prepare_agentic_memory_delivery(
            &turn,
            &route(),
            RetrievalAction::Query("needle".into()),
            None,
            None,
        )
        .unwrap()
        .prepared
        .unwrap();
    let router = connect(&gateway, host.clone());
    assert!(matches!(
        alice.deliver_agentic_memory(&turn, &prepared, &router),
        Err(GatewayError::WriterUnavailable)
    ));
    assert_eq!(host.calls.load(Ordering::Relaxed), 0);
}

#[path = "inspection_tests.rs"]
mod inspection_tests;
