use super::*;
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    mpsc::{self, Receiver, Sender},
};
use std::time::Duration;

struct MockHost {
    configured: BTreeSet<RuntimeCapability>,
    connection: Mutex<Result<HostConnection, String>>,
    reconciliation: Mutex<BTreeMap<RuntimeCapability, Result<HostReconcileStatus, String>>>,
    connects: AtomicUsize,
    handoffs: AtomicUsize,
}

impl MockHost {
    fn new(configured: &[RuntimeCapability]) -> Self {
        Self {
            configured: configured.iter().copied().collect(),
            connection: Mutex::new(Ok(HostConnection::default())),
            reconciliation: Mutex::new(BTreeMap::new()),
            connects: AtomicUsize::new(0),
            handoffs: AtomicUsize::new(0),
        }
    }

    fn advertise(&self, capability: RuntimeCapability, contract: HostCapabilityContract) {
        self.connection
            .lock()
            .unwrap()
            .as_mut()
            .unwrap()
            .capabilities
            .insert(capability, contract);
    }

    fn set_connection(&self, value: Result<HostConnection, String>) {
        *self.connection.lock().unwrap() = value;
    }

    fn set_reconciliation(
        &self,
        capability: RuntimeCapability,
        result: Result<HostReconcileStatus, String>,
    ) {
        self.reconciliation
            .lock()
            .unwrap()
            .insert(capability, result);
    }
}

impl GatewayHostAdapter for MockHost {
    fn configured_capabilities(&self) -> BTreeSet<RuntimeCapability> {
        self.configured.clone()
    }

    fn connect(&self) -> Result<HostConnection, String> {
        self.connects.fetch_add(1, Ordering::SeqCst);
        self.connection.lock().unwrap().clone()
    }

    fn reconcile(&self, capability: RuntimeCapability) -> Result<HostReconcileStatus, String> {
        self.reconciliation
            .lock()
            .unwrap()
            .get(&capability)
            .cloned()
            .unwrap_or(Ok(HostReconcileStatus::NoOutstandingWork))
    }

    fn handoff(
        &self,
        _capability: RuntimeCapability,
        operation: &HostOperation,
    ) -> Result<HandoffReceipt, String> {
        self.handoffs.fetch_add(1, Ordering::SeqCst);
        Ok(HandoffReceipt {
            stable_id: operation.stable_id.clone(),
            payload_digest: operation.payload_digest,
            durably_accepted: true,
        })
    }

    fn cancel(
        &self,
        _capability: RuntimeCapability,
        _stable_id: &str,
    ) -> Result<HostCancelOutcome, String> {
        Ok(HostCancelOutcome::CancelledBeforeStart)
    }

    fn status(
        &self,
        _capability: RuntimeCapability,
        _stable_id: &str,
    ) -> Result<HostOperationStatus, String> {
        Ok(HostOperationStatus::Pending)
    }
}

fn complete() -> HostCapabilityContract {
    HostCapabilityContract {
        host_executes_stage: true,
        queue: HostQueueGuarantees {
            durable_acceptance: true,
            stable_submission_ids: true,
            linearizable_cancel_dequeue: true,
            queryable_operation_status: true,
            idempotent_handoff: true,
        },
    }
}

#[test]
fn startup_connects_and_reconciles_before_granting_host_authority() {
    let adapter = Arc::new(MockHost::new(&[RuntimeCapability::ConversationExecution]));
    adapter.advertise(RuntimeCapability::ConversationExecution, complete());
    adapter.set_reconciliation(
        RuntimeCapability::ConversationExecution,
        Ok(HostReconcileStatus::Ready),
    );
    let router = CapabilityRouter::connect(adapter.clone());
    assert_eq!(adapter.connects.load(Ordering::SeqCst), 1);
    assert_eq!(
        router
            .state(RuntimeCapability::ConversationExecution)
            .unwrap(),
        CapabilityState::Ready
    );
    assert_eq!(
        router
            .authority(RuntimeCapability::ConversationExecution)
            .unwrap(),
        StageAuthority::Host
    );
    assert_eq!(
        router.authority(RuntimeCapability::Dream).unwrap(),
        StageAuthority::Native
    );
}

#[test]
fn incomplete_queue_stays_native_without_prior_host_work() {
    let adapter = Arc::new(MockHost::new(&[RuntimeCapability::ConversationExecution]));
    adapter.advertise(
        RuntimeCapability::ConversationExecution,
        HostCapabilityContract {
            host_executes_stage: true,
            ..Default::default()
        },
    );
    let router = CapabilityRouter::connect(adapter);
    assert_eq!(
        router
            .state(RuntimeCapability::ConversationExecution)
            .unwrap(),
        CapabilityState::NativeOnly
    );
    assert_eq!(
        router
            .authority(RuntimeCapability::ConversationExecution)
            .unwrap(),
        StageAuthority::Native
    );
}

#[test]
fn failed_startup_holds_only_configured_host_stage_and_reconnect_recovers() {
    let adapter = Arc::new(MockHost::new(&[RuntimeCapability::ConversationExecution]));
    adapter.set_connection(Err("offline".to_owned()));
    let router = CapabilityRouter::connect(adapter.clone());
    assert_eq!(
        router
            .state(RuntimeCapability::ConversationExecution)
            .unwrap(),
        CapabilityState::Unavailable("offline".into())
    );
    assert_eq!(
        router
            .authority(RuntimeCapability::ConversationExecution)
            .unwrap(),
        StageAuthority::Held
    );
    assert_eq!(
        router.authority(RuntimeCapability::Dream).unwrap(),
        StageAuthority::Native
    );
    let mut contracts = BTreeMap::new();
    contracts.insert(RuntimeCapability::ConversationExecution, complete());
    adapter.set_connection(Ok(HostConnection {
        capabilities: contracts,
    }));
    router.reconnect();
    assert_eq!(adapter.connects.load(Ordering::SeqCst), 2);
    assert_eq!(
        router
            .authority(RuntimeCapability::ConversationExecution)
            .unwrap(),
        StageAuthority::Host
    );
}

#[test]
fn unknown_host_work_is_held_until_reconciliation_proves_empty() {
    let adapter = Arc::new(MockHost::new(&[RuntimeCapability::ConversationExecution]));
    adapter.advertise(RuntimeCapability::ConversationExecution, complete());
    let router = CapabilityRouter::connect(adapter.clone());

    // Model outstanding host ownership from an earlier durable handoff.
    router
        .state
        .lock()
        .unwrap()
        .ownership_debt
        .insert(RuntimeCapability::ConversationExecution);
    adapter.set_connection(Ok(HostConnection::default()));
    adapter.set_reconciliation(
        RuntimeCapability::ConversationExecution,
        Ok(HostReconcileStatus::UnknownStatus),
    );
    router.reconnect();
    assert_eq!(
        router
            .state(RuntimeCapability::ConversationExecution)
            .unwrap(),
        CapabilityState::UnknownStatusHeld
    );
    assert_eq!(
        router
            .authority(RuntimeCapability::ConversationExecution)
            .unwrap(),
        StageAuthority::Held
    );
    assert!(
        router
            .state
            .lock()
            .unwrap()
            .ownership_debt
            .contains(&RuntimeCapability::ConversationExecution)
    );

    adapter.set_reconciliation(
        RuntimeCapability::ConversationExecution,
        Ok(HostReconcileStatus::NoOutstandingWork),
    );
    router.reconnect();
    assert_eq!(
        router
            .authority(RuntimeCapability::ConversationExecution)
            .unwrap(),
        StageAuthority::Native
    );
    assert!(
        !router
            .state
            .lock()
            .unwrap()
            .ownership_debt
            .contains(&RuntimeCapability::ConversationExecution)
    );
}

#[test]
fn per_operation_release_does_not_clear_capability_debt_before_empty_reconciliation() {
    let capability = RuntimeCapability::ConversationExecution;
    let adapter = Arc::new(MockHost::new(&[capability]));
    adapter.advertise(capability, complete());
    let router = CapabilityRouter::connect(adapter.clone());
    router
        .hold_operation(capability, "stable-op".to_owned())
        .unwrap();
    router.release_operation(capability, "stable-op").unwrap();
    assert_eq!(router.authority(capability).unwrap(), StageAuthority::Held);

    adapter.set_reconciliation(capability, Ok(HostReconcileStatus::Ready));
    router.reconnect();
    assert_eq!(router.authority(capability).unwrap(), StageAuthority::Held);

    adapter.set_reconciliation(capability, Ok(HostReconcileStatus::NoOutstandingWork));
    router.reconnect();
    assert_eq!(router.authority(capability).unwrap(), StageAuthority::Host);
}

struct BlockingReconcileHost {
    capability: RuntimeCapability,
    block_next_reconcile: AtomicBool,
    withdraw_contract: AtomicBool,
    entered: Sender<()>,
    proceed: Mutex<Receiver<()>>,
}

impl GatewayHostAdapter for BlockingReconcileHost {
    fn configured_capabilities(&self) -> BTreeSet<RuntimeCapability> {
        [self.capability].into_iter().collect()
    }

    fn connect(&self) -> Result<HostConnection, String> {
        let mut capabilities = BTreeMap::new();
        if !self.withdraw_contract.load(Ordering::SeqCst) {
            capabilities.insert(self.capability, complete());
        }
        Ok(HostConnection { capabilities })
    }

    fn reconcile(&self, _capability: RuntimeCapability) -> Result<HostReconcileStatus, String> {
        if self.block_next_reconcile.swap(false, Ordering::SeqCst) {
            self.entered.send(()).map_err(|error| error.to_string())?;
            self.proceed
                .lock()
                .map_err(|error| error.to_string())?
                .recv_timeout(Duration::from_secs(5))
                .map_err(|error| error.to_string())?;
        }
        Ok(HostReconcileStatus::NoOutstandingWork)
    }

    fn handoff(
        &self,
        _capability: RuntimeCapability,
        operation: &HostOperation,
    ) -> Result<HandoffReceipt, String> {
        Ok(HandoffReceipt {
            stable_id: operation.stable_id.clone(),
            payload_digest: operation.payload_digest,
            durably_accepted: true,
        })
    }

    fn cancel(
        &self,
        _capability: RuntimeCapability,
        _stable_id: &str,
    ) -> Result<HostCancelOutcome, String> {
        Ok(HostCancelOutcome::Unknown)
    }

    fn status(
        &self,
        _capability: RuntimeCapability,
        _stable_id: &str,
    ) -> Result<HostOperationStatus, String> {
        Ok(HostOperationStatus::Unknown)
    }
}

#[test]
fn refresh_cannot_erase_unknown_status_observed_during_reconcile() {
    let capability = RuntimeCapability::ConversationExecution;
    let (entered_tx, entered_rx) = mpsc::channel();
    let (proceed_tx, proceed_rx) = mpsc::channel();
    let host = Arc::new(BlockingReconcileHost {
        capability,
        block_next_reconcile: AtomicBool::new(false),
        withdraw_contract: AtomicBool::new(false),
        entered: entered_tx,
        proceed: Mutex::new(proceed_rx),
    });
    let router = Arc::new(CapabilityRouter::connect(host.clone()));
    host.block_next_reconcile.store(true, Ordering::SeqCst);

    let refresh_router = Arc::clone(&router);
    let refresh = std::thread::spawn(move || refresh_router.reconnect());
    entered_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("reconcile should block");
    assert_eq!(
        router.status(capability, "stable-op").unwrap(),
        HostOperationStatus::Unknown
    );
    proceed_tx.send(()).unwrap();
    refresh.join().unwrap();

    assert_eq!(router.authority(capability).unwrap(), StageAuthority::Held);
    let state = router.state.lock().unwrap();
    assert!(state.ownership_debt.contains(&capability));
    assert!(
        state
            .owned_operation_ids
            .contains(&(capability, "stable-op".into()))
    );
}

#[test]
fn concurrent_operation_update_cannot_restore_a_withdrawn_dispatch_contract() {
    let capability = RuntimeCapability::ConversationExecution;
    let (entered_tx, entered_rx) = mpsc::channel();
    let (proceed_tx, proceed_rx) = mpsc::channel();
    let host = Arc::new(BlockingReconcileHost {
        capability,
        block_next_reconcile: AtomicBool::new(false),
        withdraw_contract: AtomicBool::new(false),
        entered: entered_tx,
        proceed: Mutex::new(proceed_rx),
    });
    let router = Arc::new(CapabilityRouter::connect(host.clone()));
    router.hold_operation(capability, "intent").unwrap();
    host.withdraw_contract.store(true, Ordering::SeqCst);
    host.block_next_reconcile.store(true, Ordering::SeqCst);
    let reconnect_router = Arc::clone(&router);
    let reconnect = std::thread::spawn(move || reconnect_router.reconnect());
    entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    router.hold_operation(capability, "another-intent").unwrap();
    proceed_tx.send(()).unwrap();
    reconnect.join().unwrap();
    let payload = b"durable intent".to_vec();
    let operation = HostOperation {
        stable_id: "intent".into(),
        payload_digest: Sha256::digest(&payload).into(),
        payload,
    };
    assert_eq!(router.authority(capability).unwrap(), StageAuthority::Held);
    assert_eq!(
        router.retry_handoff(capability, &operation),
        Err(HostContractError::HostNotReady(capability))
    );
    assert_eq!(router.state.lock().unwrap().owned_operation_ids.len(), 2);
}

#[test]
fn reconnect_preserves_recovered_debt_for_a_no_longer_configured_capability() {
    let capability = RuntimeCapability::ConversationExecution;
    let adapter = Arc::new(MockHost::new(&[]));
    let router = CapabilityRouter::connect(adapter.clone());
    router.hold_operation(capability, "recovered").unwrap();
    router.reconnect();
    assert_eq!(router.authority(capability).unwrap(), StageAuthority::Held);
    adapter.set_connection(Err("offline".into()));
    router.reconnect();
    assert_eq!(router.authority(capability).unwrap(), StageAuthority::Held);
    adapter.set_connection(Ok(HostConnection::default()));
    router.release_operation(capability, "recovered").unwrap();
    router.reconnect();
    assert_eq!(
        router.authority(capability).unwrap(),
        StageAuthority::Native
    );
}

#[test]
fn reconnect_cannot_clear_any_locally_unresolved_operation() {
    let capability = RuntimeCapability::ConversationExecution;
    let adapter = Arc::new(MockHost::new(&[capability]));
    adapter.advertise(capability, complete());
    let router = CapabilityRouter::connect(adapter.clone());
    router.hold_operation(capability, "first").unwrap();
    router.hold_operation(capability, "second").unwrap();
    router.reconnect();
    assert_eq!(router.authority(capability).unwrap(), StageAuthority::Held);
    router.release_operation(capability, "first").unwrap();
    router.reconnect();
    assert_eq!(router.authority(capability).unwrap(), StageAuthority::Held);
    assert!(
        router
            .state
            .lock()
            .unwrap()
            .owned_operation_ids
            .contains(&(capability, "second".into()))
    );
    router.release_operation(capability, "second").unwrap();
    router.reconnect();
    assert_eq!(router.authority(capability).unwrap(), StageAuthority::Host);
}

#[test]
fn withdrawn_contract_fences_recovered_intent_retry_but_allows_status_and_cancel() {
    let capability = RuntimeCapability::ConversationExecution;
    let adapter = Arc::new(MockHost::new(&[capability]));
    adapter.advertise(capability, complete());
    let router = CapabilityRouter::connect(adapter.clone());
    router.hold_operation(capability, "intent").unwrap();
    adapter.set_connection(Ok(HostConnection::default()));
    router.reconnect();
    let payload = b"durable intent".to_vec();
    let operation = HostOperation {
        stable_id: "intent".into(),
        payload_digest: Sha256::digest(&payload).into(),
        payload,
    };
    assert_eq!(router.authority(capability).unwrap(), StageAuthority::Held);
    assert_eq!(
        router.retry_handoff(capability, &operation),
        Err(HostContractError::HostNotReady(capability))
    );
    assert_eq!(
        router.status(capability, "intent").unwrap(),
        HostOperationStatus::Pending
    );
    assert_eq!(
        router.cancel(capability, "intent").unwrap(),
        HostCancelOutcome::CancelledBeforeStart
    );
    router.reconnect();
    assert_eq!(router.authority(capability).unwrap(), StageAuthority::Held);
    router.release_operation(capability, "intent").unwrap();
    router.reconnect();
    assert_eq!(
        router.authority(capability).unwrap(),
        StageAuthority::Native
    );
}

#[test]
fn reconciliation_error_fences_prior_host_work_and_preserves_other_stages() {
    let adapter = Arc::new(MockHost::new(&[RuntimeCapability::ConversationExecution]));
    adapter.advertise(RuntimeCapability::ConversationExecution, complete());
    adapter.set_reconciliation(
        RuntimeCapability::ConversationExecution,
        Err("recovery unavailable".into()),
    );
    let router = CapabilityRouter::connect(adapter);
    assert_eq!(
        router
            .state(RuntimeCapability::ConversationExecution)
            .unwrap(),
        CapabilityState::Unavailable("recovery unavailable".into())
    );
    assert_eq!(
        router
            .authority(RuntimeCapability::ConversationExecution)
            .unwrap(),
        StageAuthority::Held
    );
    assert_eq!(
        router.authority(RuntimeCapability::Embedding).unwrap(),
        StageAuthority::Native
    );
}

#[test]
fn known_pending_handoff_debt_survives_contract_downgrade() {
    let capability = RuntimeCapability::ConversationExecution;
    let adapter = Arc::new(MockHost::new(&[capability]));
    adapter.advertise(capability, complete());
    let router = CapabilityRouter::connect(adapter.clone());
    let payload = b"durable stage input".to_vec();
    router
        .handoff(
            capability,
            &HostOperation {
                stable_id: "known-pending".into(),
                payload_digest: Sha256::digest(&payload).into(),
                payload,
            },
        )
        .unwrap();
    adapter.set_connection(Ok(HostConnection::default()));
    adapter.set_reconciliation(capability, Ok(HostReconcileStatus::UnknownStatus));
    router.reconnect();
    assert_eq!(router.authority(capability).unwrap(), StageAuthority::Held);
    adapter.set_reconciliation(capability, Ok(HostReconcileStatus::NoOutstandingWork));
    router.reconnect();
    assert_eq!(router.authority(capability).unwrap(), StageAuthority::Held);
    router
        .release_operation(capability, "known-pending")
        .unwrap();
    router.reconnect();
    assert_eq!(
        router.authority(capability).unwrap(),
        StageAuthority::Native
    );
}

#[test]
fn declared_background_capabilities_without_executable_stages_remain_native() {
    let capabilities = [
        RuntimeCapability::Insomnia,
        RuntimeCapability::Dream,
        RuntimeCapability::Embedding,
        RuntimeCapability::ProjectEnvironment,
    ];
    let adapter = Arc::new(MockHost::new(&capabilities));
    for capability in capabilities {
        adapter.advertise(capability, complete());
    }
    let router = CapabilityRouter::connect(adapter.clone());
    let payload = b"unsupported stage".to_vec();
    let operation = HostOperation {
        stable_id: "unsupported".into(),
        payload_digest: Sha256::digest(&payload).into(),
        payload,
    };
    for capability in capabilities {
        assert_eq!(
            router.authority(capability).unwrap(),
            StageAuthority::Native
        );
        assert_eq!(
            router.handoff(capability, &operation),
            Err(HostContractError::HostNotReady(capability))
        );
        assert_eq!(
            router.authority(capability).unwrap(),
            StageAuthority::Native
        );
    }
    assert_eq!(adapter.handoffs.load(Ordering::SeqCst), 0);
    adapter.set_connection(Err("offline".into()));
    router.reconnect();
    for capability in capabilities {
        assert_eq!(
            router.authority(capability).unwrap(),
            StageAuthority::Native
        );
    }
}

#[test]
fn unsupported_stage_filter_cannot_erase_recovered_host_ownership_debt() {
    let capability = RuntimeCapability::Dream;
    let adapter = Arc::new(MockHost::new(&[capability]));
    adapter.advertise(capability, complete());
    let router = CapabilityRouter::connect(adapter.clone());
    router
        .hold_operation(capability, "recovered-dream")
        .unwrap();
    router.reconnect();
    assert_eq!(router.authority(capability).unwrap(), StageAuthority::Held);
    let payload = b"durable old intent".to_vec();
    let operation = HostOperation {
        stable_id: "recovered-dream".into(),
        payload_digest: Sha256::digest(&payload).into(),
        payload,
    };
    assert_eq!(
        router.retry_handoff(capability, &operation),
        Err(HostContractError::HostNotReady(capability))
    );
    assert_eq!(adapter.handoffs.load(Ordering::SeqCst), 0);
    router
        .release_operation(capability, "recovered-dream")
        .unwrap();
    router.reconnect();
    assert_eq!(
        router.authority(capability).unwrap(),
        StageAuthority::Native
    );
}

#[test]
fn exact_digest_retry_is_allowed_but_changed_digest_never_reaches_adapter() {
    let capability = RuntimeCapability::ConversationExecution;
    let adapter = Arc::new(MockHost::new(&[capability]));
    adapter.advertise(capability, complete());
    let router = CapabilityRouter::connect(adapter.clone());
    let payload = b"immutable input".to_vec();
    let operation = HostOperation {
        stable_id: "stable".into(),
        payload_digest: Sha256::digest(&payload).into(),
        payload,
    };
    let first = router.handoff(capability, &operation).unwrap();
    assert!(first.durably_accepted);
    assert_eq!(router.handoff(capability, &operation).unwrap(), first);
    router.hold_operation(capability, "stable").unwrap();
    assert_eq!(router.retry_handoff(capability, &operation).unwrap(), first);
    let payload = b"changed input".to_vec();
    let changed = HostOperation {
        stable_id: "stable".into(),
        payload_digest: Sha256::digest(&payload).into(),
        payload,
    };
    assert_eq!(
        router.retry_handoff(capability, &changed),
        Err(HostContractError::IdempotencyConflict)
    );
    assert_eq!(adapter.handoffs.load(Ordering::SeqCst), 3);
    let mut unrelated = operation;
    unrelated.stable_id = "never-admitted".into();
    assert_eq!(
        router.retry_handoff(capability, &unrelated),
        Err(HostContractError::UnknownOperationStatus)
    );
    assert_eq!(adapter.handoffs.load(Ordering::SeqCst), 3);
}
