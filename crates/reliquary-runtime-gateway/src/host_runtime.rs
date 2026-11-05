//! Trusted host dispatch and reconciliation over the durable conversation queue.
//!
//! A host is called only after a queue handoff intent is synced. Once an intent
//! exists, recovery is status-only; this module never blindly repeats handoff.

use super::*;
use serde::Serialize;
use sha2::{Digest, Sha256};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct HostQueueGuarantees {
    pub durable_acceptance: bool,
    pub stable_submission_ids: bool,
    pub linearizable_cancel_dequeue: bool,
    pub queryable_operation_status: bool,
    pub idempotent_handoff: bool,
}

impl HostQueueGuarantees {
    pub fn complete(self) -> bool {
        self.durable_acceptance
            && self.stable_submission_ids
            && self.linearizable_cancel_dequeue
            && self.queryable_operation_status
            && self.idempotent_handoff
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostReconcileStatus {
    Ready,
    NoOutstandingWork,
    UnknownStatus,
}

/// Immutable host operation input. Payload digest is computed by the gateway.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostOperation {
    pub stable_id: String,
    pub payload_digest: [u8; 32],
    pub payload: Vec<u8>,
}

/// Trusted integration boundary. Implementations must satisfy the declared
/// durability and dequeue/cancel guarantees; gateway tests cannot prove a real
/// remote host's behavior.
pub trait TrustedHostAdapter: Send + Sync {
    /// Capability intent configured by the host application before connecting.
    fn configured_capabilities(&self) -> std::collections::BTreeSet<RuntimeCapability>;
    /// Authenticated connection handshake. Returned capability contracts are
    /// declarations only; they are not independent evidence of host behavior.
    fn connect(&self) -> Result<crate::host_lifecycle::HostConnection, String>;
    fn queue_guarantees(&self, capability: RuntimeCapability) -> HostQueueGuarantees;
    fn reconcile(&self, capability: RuntimeCapability) -> Result<HostReconcileStatus, String>;
    fn handoff(
        &self,
        capability: RuntimeCapability,
        operation: &HostOperation,
    ) -> Result<HandoffReceipt, String>;
    fn status(
        &self,
        capability: RuntimeCapability,
        stable_id: &str,
    ) -> Result<HostOperationStatus, String>;
    fn cancel(
        &self,
        capability: RuntimeCapability,
        stable_id: &str,
    ) -> Result<HostCancelOutcome, String>;

    /// Deliver selected Memory content and return only after actual external
    /// consumption has been confirmed by the trusted host.
    fn deliver_memory(
        &self,
        _operation: &crate::MemoryDeliveryOperation,
    ) -> Result<crate::MemoryDeliveryReceipt, String> {
        Err("trusted Memory delivery is unsupported".into())
    }

    /// Reconcile delivery after an ambiguous result or process restart. Hosts
    /// must key this lookup by the stable operation ID.
    fn memory_delivery_status(
        &self,
        _stable_id: &str,
    ) -> Result<crate::MemoryDeliveryStatus, String> {
        Ok(crate::MemoryDeliveryStatus::Unknown)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HostDispatchOutcome {
    Dispatched(SubmissionStatus),
    Reconciled(SubmissionStatus),
}

#[derive(Serialize)]
struct CanonicalSubmission<'a> {
    owner_id: String,
    conversation_id: &'a str,
    submission_id: &'a str,
    principal_id: &'a str,
    content: &'a str,
}

impl Instance {
    /// Persist one handoff intent before calling the trusted host. An existing
    /// operation is reconciled by status and is never handed off a second time.
    pub fn dispatch_conversation_submission(
        &self,
        queue: &mut ConversationQueue,
        id: &ConversationSubmissionId,
        capability: RuntimeCapability,
        router: &CapabilityRouter,
    ) -> Result<HostDispatchOutcome, GatewayError> {
        self.authorize_host_submission(queue, id)?;
        require_executable(capability)?;

        if let Some(existing) = queue.host_operation(id, capability) {
            ensure_host_router(router, self.gateway_binding())?;
            // Rehydrate router ownership from the durable queue before querying
            // the host after process restart or router replacement.
            router.record_owned_operation(capability, existing.stable_id.clone());
            return self.reconcile_host_operation(queue, id, capability, router);
        }
        require_host_contract(router, capability, self.gateway_binding())?;
        match router
            .adapter()
            .reconcile(capability)
            .map_err(GatewayError::HostAdapter)?
        {
            HostReconcileStatus::UnknownStatus => {
                return Err(GatewayError::HostStatusUnknown);
            }
            HostReconcileStatus::Ready | HostReconcileStatus::NoOutstandingWork => {}
        }

        let submission = queue
            .submission(id)
            .cloned()
            .ok_or(conversation_queue::QueueError::MissingSubmission)?;
        let payload = serde_json::to_vec(&CanonicalSubmission {
            owner_id: hex(&queue.owner().as_bytes()),
            conversation_id: &submission.id.conversation_id,
            submission_id: submission.id.submission_id.as_str(),
            principal_id: &submission.principal_id,
            content: &submission.content,
        })
        .map_err(|error| {
            GatewayError::ConversationQueue(conversation_queue::QueueError::Serialization(error))
        })?;
        let payload_digest: [u8; 32] = Sha256::digest(&payload).into();
        let stable_id = stable_host_operation_id(queue.owner(), id, capability);

        // This append syncs before any externally visible host call.
        if !queue.begin_host_handoff(id, capability, stable_id.clone(), payload_digest)? {
            return self.reconcile_host_operation(queue, id, capability, router);
        }
        router.record_owned_operation(capability, stable_id.clone());
        let operation = HostOperation {
            stable_id: stable_id.clone(),
            payload_digest,
            payload,
        };
        let receipt = match router.adapter().handoff(capability, &operation) {
            Ok(receipt) => receipt,
            Err(error) => {
                // Intent remains durable. Later calls can only query status.
                return Err(GatewayError::HostAdapter(error));
            }
        };
        if receipt.stable_id != stable_id
            || receipt.payload_digest != payload_digest
            || !receipt.durably_accepted
        {
            let _ = queue.record_host_status(id, capability, HostOperationStatus::Unknown);
            return Err(GatewayError::InvalidHandoffReceipt);
        }
        let status = queue.record_host_handoff(id, capability, &receipt)?;
        Ok(HostDispatchOutcome::Dispatched(status))
    }

    /// Reconcile a previously dispatched operation. Unknown and adapter errors
    /// remain held; they never authorize redispatch.
    pub fn reconcile_host_operation(
        &self,
        queue: &mut ConversationQueue,
        id: &ConversationSubmissionId,
        capability: RuntimeCapability,
        router: &CapabilityRouter,
    ) -> Result<HostDispatchOutcome, GatewayError> {
        self.authorize_host_submission(queue, id)?;
        require_executable(capability)?;
        ensure_host_router(router, self.gateway_binding())?;
        let current = queue
            .host_operation(id, capability)
            .ok_or(conversation_queue::QueueError::MissingSubmission)?;
        match router.adapter().reconcile(capability) {
            Ok(HostReconcileStatus::UnknownStatus) => {
                return Ok(HostDispatchOutcome::Reconciled(queue.record_host_status(
                    id,
                    capability,
                    HostOperationStatus::Unknown,
                )?));
            }
            Ok(HostReconcileStatus::Ready | HostReconcileStatus::NoOutstandingWork) => {}
            Err(error) => {
                let _ = queue.record_host_status(id, capability, HostOperationStatus::Unknown);
                return Err(GatewayError::HostAdapter(error));
            }
        }
        let status = match router.adapter().status(capability, &current.stable_id) {
            Ok(status) => status,
            Err(error) => {
                let _ = queue.record_host_status(id, capability, HostOperationStatus::Unknown);
                return Err(GatewayError::HostAdapter(error));
            }
        };
        let status = queue.record_host_status(id, capability, status)?;
        if status.host_operations.get(&capability).is_some_and(|op| {
            matches!(
                op.phase,
                DurableHostOperationPhase::Completed | DurableHostOperationPhase::Cancelled
            )
        }) {
            router.record_terminal_operation(capability, &current.stable_id);
        }
        Ok(HostDispatchOutcome::Reconciled(status))
    }

    /// Record cancellation intent before calling the host's linearizable
    /// pending/dequeue cancellation gate.
    pub fn cancel_host_operation(
        &self,
        queue: &mut ConversationQueue,
        id: &ConversationSubmissionId,
        capability: RuntimeCapability,
        router: &CapabilityRouter,
    ) -> Result<SubmissionStatus, GatewayError> {
        self.authorize_host_submission(queue, id)?;
        require_executable(capability)?;
        ensure_host_router(router, self.gateway_binding())?;
        let current = queue
            .host_operation(id, capability)
            .ok_or(conversation_queue::QueueError::MissingSubmission)?;
        queue.begin_host_cancel(id, capability)?;
        let outcome = router
            .adapter()
            .cancel(capability, &current.stable_id)
            .map_err(GatewayError::HostAdapter)?;
        let status = queue.record_host_cancel(id, capability, outcome)?;
        if status.host_operations.get(&capability).is_some_and(|op| {
            matches!(
                op.phase,
                DurableHostOperationPhase::Completed | DurableHostOperationPhase::Cancelled
            )
        }) {
            router.record_terminal_operation(capability, &current.stable_id);
        }
        Ok(status)
    }

    fn authorize_host_submission(
        &self,
        queue: &ConversationQueue,
        id: &ConversationSubmissionId,
    ) -> Result<(), GatewayError> {
        self.authorize_queue_owner(queue.owner())?;
        self.ensure_credential_active()?;
        let principal = queue
            .submitter_principal(id)
            .ok_or(conversation_queue::QueueError::MissingSubmission)?;
        if principal != self.principal.as_ref() {
            return Err(GatewayError::UnauthorizedQueueSubmission {
                principal: self.principal.to_string(),
                owner: queue.owner(),
            });
        }
        Ok(())
    }
}

fn require_executable(capability: RuntimeCapability) -> Result<(), GatewayError> {
    if capability == RuntimeCapability::ConversationExecution {
        Ok(())
    } else {
        Err(GatewayError::UnsupportedHostCapability(capability))
    }
}

fn require_host_contract(
    router: &CapabilityRouter,
    capability: RuntimeCapability,
    gateway_binding: usize,
) -> Result<(), GatewayError> {
    ensure_host_router(router, gateway_binding)?;
    let contract_ready = router
        .contract(capability)
        .is_some_and(|contract| contract.queue.complete());
    match router.state(capability) {
        HostCapabilityState::Ready if contract_ready => Ok(()),
        HostCapabilityState::Unavailable(reason) => Err(GatewayError::HostAdapter(reason)),
        HostCapabilityState::UnknownStatusHeld => Err(GatewayError::HostStatusUnknown),
        _ => Err(GatewayError::IncompleteHostContract(capability)),
    }
}

fn ensure_host_router(
    router: &CapabilityRouter,
    gateway_binding: usize,
) -> Result<(), GatewayError> {
    if router.belongs_to(gateway_binding) {
        Ok(())
    } else {
        Err(GatewayError::HostAdapter(
            "host connection belongs to another Gateway".into(),
        ))
    }
}

fn stable_host_operation_id(
    owner: OwnerId,
    id: &ConversationSubmissionId,
    capability: RuntimeCapability,
) -> String {
    format!(
        "{}:{}:{}:{capability:?}",
        hex(&owner.as_bytes()),
        id.conversation_id,
        id.submission_id.as_str()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use reliquary_container::{Container, ContainerIdentity, ReliquaryScope};
    use reliquary_vector_store::{
        ArchiveVectorStore, CompatibilityProfileStore, MemoryVectorStore, PackedVectorStore,
        VectorGenerationStore,
    };
    use std::{
        collections::HashMap,
        path::{Path, PathBuf},
        sync::Mutex,
    };
    use uuid::Uuid;

    struct Temp(PathBuf);
    impl Temp {
        fn new() -> Self {
            let root = std::env::current_dir().unwrap().join(".test-tmp");
            std::fs::create_dir_all(&root).unwrap();
            Self(root.join(format!(
                "host-runtime-{}-{}",
                std::process::id(),
                Uuid::new_v4()
            )))
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn initialize_rel(path: &Path, owner: [u8; 16]) {
        let mut container = Container::create(
            path,
            ContainerIdentity::reliquary(ReliquaryScope::Project),
            owner,
        )
        .unwrap();
        PackedVectorStore::default()
            .initialize(&mut container)
            .unwrap();
        CompatibilityProfileStore::default()
            .initialize(&mut container)
            .unwrap();
        ArchiveVectorStore::default()
            .initialize(&mut container)
            .unwrap();
        MemoryVectorStore::default()
            .initialize(&mut container)
            .unwrap();
        VectorGenerationStore::empty()
            .initialize(&mut container)
            .unwrap();
        container.sync().unwrap();
    }

    fn setup() -> (
        Temp,
        Instance,
        OwnerId,
        ConversationQueue,
        ConversationSubmissionId,
    ) {
        let temp = Temp::new();
        std::fs::create_dir_all(&temp.0).unwrap();
        let bytes = [91; 16];
        initialize_rel(&temp.0.join("owner.rel"), bytes);
        let gateway = Gateway::new();
        let admission = GraphAdmission::for_replay(bytes, [], [], [], [], []);
        let owner = gateway
            .mount_writable_reliquary(&temp.0.join("owner.rel"), &admission, [], [])
            .unwrap();
        let instance = gateway.instance("alice", [owner], []).unwrap();
        let id =
            ConversationSubmissionId::new("conversation", SubmissionId::new("submission").unwrap())
                .unwrap();
        let mut queue = instance
            .open_conversation_queue(temp.0.join("queue"), owner)
            .unwrap();
        instance
            .submit_conversation(
                &mut queue,
                "conversation",
                id.submission_id.clone(),
                "trusted payload",
                0,
            )
            .unwrap();
        (temp, instance, owner, queue, id)
    }

    #[derive(Default)]
    struct FakeHost {
        operations: Mutex<HashMap<String, HostOperationStatus>>,
        handoffs: Mutex<usize>,
        status_unknown: Mutex<bool>,
        fail_handoff: Mutex<bool>,
    }
    impl TrustedHostAdapter for FakeHost {
        fn configured_capabilities(&self) -> std::collections::BTreeSet<RuntimeCapability> {
            [RuntimeCapability::ConversationExecution]
                .into_iter()
                .collect()
        }
        fn connect(&self) -> Result<crate::host_lifecycle::HostConnection, String> {
            Ok(crate::host_lifecycle::HostConnection {
                capabilities: [(
                    RuntimeCapability::ConversationExecution,
                    crate::host_lifecycle::HostCapabilityContract::pending_queue(
                        self.queue_guarantees(RuntimeCapability::ConversationExecution),
                    ),
                )]
                .into_iter()
                .collect(),
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
            Ok(HostReconcileStatus::Ready)
        }
        fn handoff(
            &self,
            _: RuntimeCapability,
            operation: &HostOperation,
        ) -> Result<HandoffReceipt, String> {
            *self.handoffs.lock().unwrap() += 1;
            if *self.fail_handoff.lock().unwrap() {
                return Err("transport lost after possible acceptance".into());
            }
            self.operations
                .lock()
                .unwrap()
                .insert(operation.stable_id.clone(), HostOperationStatus::Pending);
            Ok(HandoffReceipt {
                stable_id: operation.stable_id.clone(),
                payload_digest: operation.payload_digest,
                durably_accepted: true,
            })
        }
        fn status(
            &self,
            _: RuntimeCapability,
            stable_id: &str,
        ) -> Result<HostOperationStatus, String> {
            if *self.status_unknown.lock().unwrap() {
                return Ok(HostOperationStatus::Unknown);
            }
            Ok(self
                .operations
                .lock()
                .unwrap()
                .get(stable_id)
                .copied()
                .unwrap_or(HostOperationStatus::Unknown))
        }
        fn cancel(
            &self,
            _: RuntimeCapability,
            stable_id: &str,
        ) -> Result<HostCancelOutcome, String> {
            let mut operations = self.operations.lock().unwrap();
            let status = operations
                .get_mut(stable_id)
                .ok_or_else(|| "unknown operation".to_string())?;
            match *status {
                HostOperationStatus::Pending => {
                    *status = HostOperationStatus::Cancelled;
                    Ok(HostCancelOutcome::CancelledBeforeStart)
                }
                HostOperationStatus::Running => Ok(HostCancelOutcome::RunningCancellationRequested),
                terminal => Ok(HostCancelOutcome::AlreadyTerminal(terminal)),
            }
        }
    }

    #[test]
    fn uncertain_handoff_survives_reopen_and_is_reconciled_without_redispatch() {
        let (temp, instance, owner, mut queue, id) = setup();
        let host = Arc::new(FakeHost::default());
        let router = CapabilityRouter::connect(host.clone(), instance.gateway_binding());
        *host.fail_handoff.lock().unwrap() = true;
        assert!(matches!(
            instance.dispatch_conversation_submission(
                &mut queue,
                &id,
                RuntimeCapability::ConversationExecution,
                &router,
            ),
            Err(GatewayError::HostAdapter(_))
        ));
        assert_eq!(*host.handoffs.lock().unwrap(), 1);
        assert!(
            queue
                .host_operation(&id, RuntimeCapability::ConversationExecution)
                .unwrap()
                .requires_reconciliation()
        );
        drop(queue);

        let mut queue = instance
            .open_conversation_queue(temp.0.join("queue"), owner)
            .unwrap();
        *host.fail_handoff.lock().unwrap() = false;
        *host.status_unknown.lock().unwrap() = true;
        let outcome = instance
            .dispatch_conversation_submission(
                &mut queue,
                &id,
                RuntimeCapability::ConversationExecution,
                &router,
            )
            .unwrap();
        assert!(matches!(outcome, HostDispatchOutcome::Reconciled(_)));
        assert_eq!(*host.handoffs.lock().unwrap(), 1);
        let durable = queue
            .host_operation(&id, RuntimeCapability::ConversationExecution)
            .unwrap();
        assert_eq!(durable.phase, DurableHostOperationPhase::Held);
        assert_eq!(
            durable.last_known_phase,
            Some(DurableHostOperationPhase::HandoffIntent)
        );
    }

    #[test]
    fn accepted_dispatch_reconcile_and_cancel_use_same_stable_identity() {
        let (_temp, instance, _owner, mut queue, id) = setup();
        let host = Arc::new(FakeHost::default());
        let router = CapabilityRouter::connect(host.clone(), instance.gateway_binding());
        let dispatched = instance
            .dispatch_conversation_submission(
                &mut queue,
                &id,
                RuntimeCapability::ConversationExecution,
                &router,
            )
            .unwrap();
        assert!(matches!(dispatched, HostDispatchOutcome::Dispatched(_)));
        let stable_id = queue
            .host_operation(&id, RuntimeCapability::ConversationExecution)
            .unwrap()
            .stable_id;
        let reconciled = instance
            .dispatch_conversation_submission(
                &mut queue,
                &id,
                RuntimeCapability::ConversationExecution,
                &router,
            )
            .unwrap();
        assert!(matches!(reconciled, HostDispatchOutcome::Reconciled(_)));
        assert_eq!(*host.handoffs.lock().unwrap(), 1);

        let cancelled = instance
            .cancel_host_operation(
                &mut queue,
                &id,
                RuntimeCapability::ConversationExecution,
                &router,
            )
            .unwrap();
        assert_eq!(
            cancelled.host_operations[&RuntimeCapability::ConversationExecution].phase,
            DurableHostOperationPhase::Cancelled
        );
        assert_eq!(
            queue
                .host_operation(&id, RuntimeCapability::ConversationExecution)
                .unwrap()
                .stable_id,
            stable_id
        );
    }

    #[test]
    fn host_router_from_another_gateway_is_rejected() {
        let (_temp, instance, _owner, mut queue, id) = setup();
        let host = Arc::new(FakeHost::default());
        let router =
            CapabilityRouter::connect(host.clone(), instance.gateway_binding().wrapping_add(1));
        assert!(matches!(
            instance.dispatch_conversation_submission(
                &mut queue,
                &id,
                RuntimeCapability::ConversationExecution,
                &router
            ),
            Err(GatewayError::HostAdapter(_))
        ));
        assert_eq!(*host.handoffs.lock().unwrap(), 0);
    }
}
