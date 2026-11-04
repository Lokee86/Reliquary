//! Host connection negotiation and per-capability readiness.
//!
//! A negotiated declaration is only a contract claim. Production integrations
//! must independently verify the remote implementation's behavior.
use crate::{
    HandoffReceipt, HostCancelOutcome, HostOperationStatus, RuntimeCapability,
    host_runtime::{HostOperation, HostQueueGuarantees, HostReconcileStatus, TrustedHostAdapter},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Mutex},
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum HostExecutionMode {
    #[default]
    PendingQueue,
    ExecutionOnly,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct HostCapabilityContract {
    pub execution_mode: HostExecutionMode,
    pub host_executes_stage: bool,
    pub queue: HostQueueGuarantees,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    struct MockHost {
        connection: Mutex<Result<HostConnection, String>>,
        reconcile: Mutex<Result<HostReconcileStatus, String>>,
    }
    impl MockHost {
        fn full_contract() -> HostConnection {
            let guarantees = HostQueueGuarantees {
                durable_acceptance: true,
                stable_submission_ids: true,
                linearizable_cancel_dequeue: true,
                queryable_operation_status: true,
                idempotent_handoff: true,
            };
            HostConnection {
                capabilities: [(
                    RuntimeCapability::ConversationExecution,
                    HostCapabilityContract::pending_queue(guarantees),
                )]
                .into_iter()
                .collect(),
            }
        }
    }
    impl TrustedHostAdapter for MockHost {
        fn configured_capabilities(&self) -> BTreeSet<RuntimeCapability> {
            [RuntimeCapability::ConversationExecution]
                .into_iter()
                .collect()
        }
        fn connect(&self) -> Result<HostConnection, String> {
            self.connection.lock().unwrap().clone()
        }
        fn queue_guarantees(&self, _: RuntimeCapability) -> HostQueueGuarantees {
            HostQueueGuarantees::default()
        }
        fn reconcile(&self, _: RuntimeCapability) -> Result<HostReconcileStatus, String> {
            self.reconcile.lock().unwrap().clone()
        }
        fn handoff(
            &self,
            _: RuntimeCapability,
            operation: &HostOperation,
        ) -> Result<HandoffReceipt, String> {
            Ok(HandoffReceipt {
                stable_id: operation.stable_id.clone(),
                payload_digest: operation.payload_digest,
                durably_accepted: true,
            })
        }
        fn status(&self, _: RuntimeCapability, _: &str) -> Result<HostOperationStatus, String> {
            Ok(HostOperationStatus::Unknown)
        }
        fn cancel(&self, _: RuntimeCapability, _: &str) -> Result<HostCancelOutcome, String> {
            Ok(HostCancelOutcome::Unknown)
        }
    }

    #[test]
    fn connect_requires_complete_contract_and_reconciliation() {
        let host = Arc::new(MockHost {
            connection: Mutex::new(Ok(MockHost::full_contract())),
            reconcile: Mutex::new(Ok(HostReconcileStatus::NoOutstandingWork)),
        });
        let router = CapabilityRouter::connect(host, 1);
        assert_eq!(
            router.state(RuntimeCapability::ConversationExecution),
            HostCapabilityState::Ready
        );
        assert_eq!(
            router.state(RuntimeCapability::Dream),
            HostCapabilityState::NativeOnly
        );
    }

    #[test]
    fn unknown_status_holds_authority_until_clean_reconciliation() {
        let host = Arc::new(MockHost {
            connection: Mutex::new(Ok(MockHost::full_contract())),
            reconcile: Mutex::new(Ok(HostReconcileStatus::UnknownStatus)),
        });
        let router = CapabilityRouter::connect(Arc::clone(&host) as Arc<dyn TrustedHostAdapter>, 1);
        assert_eq!(
            router.state(RuntimeCapability::ConversationExecution),
            HostCapabilityState::UnknownStatusHeld
        );
        *host.reconcile.lock().unwrap() = Ok(HostReconcileStatus::NoOutstandingWork);
        router.reconnect();
        assert_eq!(
            router.state(RuntimeCapability::ConversationExecution),
            HostCapabilityState::Ready
        );
    }

    #[test]
    fn empty_host_reconciliation_cannot_release_known_operation_debt() {
        let host = Arc::new(MockHost {
            connection: Mutex::new(Ok(MockHost::full_contract())),
            reconcile: Mutex::new(Ok(HostReconcileStatus::NoOutstandingWork)),
        });
        let router = CapabilityRouter::connect(Arc::clone(&host) as Arc<dyn TrustedHostAdapter>, 1);
        router.record_owned_operation(RuntimeCapability::ConversationExecution, "op-1");
        router.reconnect();
        assert_eq!(
            router.state(RuntimeCapability::ConversationExecution),
            HostCapabilityState::UnknownStatusHeld
        );
        router.record_terminal_operation(RuntimeCapability::ConversationExecution, "op-1");
        router.reconnect();
        assert_eq!(
            router.state(RuntimeCapability::ConversationExecution),
            HostCapabilityState::Ready
        );
    }

    #[test]
    fn connection_failure_fences_configured_capability() {
        let host = Arc::new(MockHost {
            connection: Mutex::new(Ok(MockHost::full_contract())),
            reconcile: Mutex::new(Ok(HostReconcileStatus::NoOutstandingWork)),
        });
        let router = CapabilityRouter::connect(Arc::clone(&host) as Arc<dyn TrustedHostAdapter>, 1);
        *host.connection.lock().unwrap() = Err("offline".into());
        router.reconnect();
        assert_eq!(
            router.state(RuntimeCapability::ConversationExecution),
            HostCapabilityState::Unavailable("offline".into())
        );
        assert!(
            router
                .contract(RuntimeCapability::ConversationExecution)
                .is_some()
        );
    }

    #[test]
    fn incomplete_declaration_does_not_gain_host_authority() {
        let host = Arc::new(MockHost {
            connection: Mutex::new(Ok(HostConnection {
                capabilities: [(
                    RuntimeCapability::ConversationExecution,
                    HostCapabilityContract::default(),
                )]
                .into_iter()
                .collect(),
            })),
            reconcile: Mutex::new(Ok(HostReconcileStatus::Ready)),
        });
        let router = CapabilityRouter::connect(host, 1);
        assert_eq!(
            router.state(RuntimeCapability::ConversationExecution),
            HostCapabilityState::NativeOnly
        );
    }
}
impl HostCapabilityContract {
    pub fn pending_queue(queue: HostQueueGuarantees) -> Self {
        Self {
            execution_mode: HostExecutionMode::PendingQueue,
            host_executes_stage: true,
            queue,
        }
    }
    pub fn execution_only() -> Self {
        Self {
            execution_mode: HostExecutionMode::ExecutionOnly,
            host_executes_stage: true,
            queue: HostQueueGuarantees::default(),
        }
    }
    fn queue_ready(self) -> bool {
        self.host_executes_stage
            && self.execution_mode == HostExecutionMode::PendingQueue
            && self.queue.complete()
    }
    fn execution_ready(self) -> bool {
        self.host_executes_stage && self.execution_mode == HostExecutionMode::ExecutionOnly
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct HostConnection {
    pub capabilities: BTreeMap<RuntimeCapability, HostCapabilityContract>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HostCapabilityState {
    NativeOnly,
    ExecutionOnlyReady,
    Ready,
    Unavailable(String),
    UnknownStatusHeld,
}

#[derive(Debug)]
struct State {
    contracts: BTreeMap<RuntimeCapability, HostCapabilityContract>,
    states: BTreeMap<RuntimeCapability, HostCapabilityState>,
    ownership_debt: BTreeSet<RuntimeCapability>,
    owned_operation_ids: BTreeSet<(RuntimeCapability, String)>,
}

/// A single host connection owner. It fences configured stages during reconnect
/// and never restores authority while prior host work has unknown ownership.
pub struct CapabilityRouter {
    adapter: Arc<dyn TrustedHostAdapter>,
    gateway_binding: usize,
    configured: BTreeSet<RuntimeCapability>,
    transition: Mutex<()>,
    state: Mutex<State>,
}
impl CapabilityRouter {
    pub(crate) fn connect(adapter: Arc<dyn TrustedHostAdapter>, gateway_binding: usize) -> Self {
        let configured = adapter.configured_capabilities();
        let router = Self {
            adapter,
            gateway_binding,
            configured,
            transition: Mutex::new(()),
            state: Mutex::new(State {
                contracts: BTreeMap::new(),
                states: RuntimeCapability::ALL
                    .into_iter()
                    .map(|c| (c, HostCapabilityState::NativeOnly))
                    .collect(),
                ownership_debt: BTreeSet::new(),
                owned_operation_ids: BTreeSet::new(),
            }),
        };
        router.refresh();
        router
    }
    pub fn reconnect(&self) {
        self.refresh();
    }
    pub fn state(&self, capability: RuntimeCapability) -> HostCapabilityState {
        self.state
            .lock()
            .map(|s| {
                s.states
                    .get(&capability)
                    .cloned()
                    .unwrap_or(HostCapabilityState::NativeOnly)
            })
            .unwrap_or_else(|_| {
                HostCapabilityState::Unavailable("host router lock poisoned".into())
            })
    }
    pub fn contract(&self, capability: RuntimeCapability) -> Option<HostCapabilityContract> {
        self.state.lock().ok()?.contracts.get(&capability).copied()
    }
    pub(crate) fn adapter(&self) -> &dyn TrustedHostAdapter {
        self.adapter.as_ref()
    }
    pub(crate) fn belongs_to(&self, gateway_binding: usize) -> bool {
        self.gateway_binding == gateway_binding
    }
    /// Track an operation once durable host ownership is possible. Only a
    /// durable terminal observation may release this fence.
    pub fn record_owned_operation(
        &self,
        capability: RuntimeCapability,
        stable_id: impl Into<String>,
    ) {
        if let Ok(mut state) = self.state.lock() {
            state
                .owned_operation_ids
                .insert((capability, stable_id.into()));
            state.ownership_debt.insert(capability);
        }
    }
    pub fn record_terminal_operation(&self, capability: RuntimeCapability, stable_id: &str) {
        if let Ok(mut state) = self.state.lock() {
            state
                .owned_operation_ids
                .remove(&(capability, stable_id.to_owned()));
        }
    }

    fn refresh(&self) {
        let Ok(_transition) = self.transition.lock() else {
            return;
        };
        let (debt, owned_ids, previous) = match self.state.lock() {
            Ok(mut state) => {
                let affected = self
                    .configured
                    .union(&state.ownership_debt)
                    .copied()
                    .collect::<Vec<_>>();
                for capability in affected {
                    state.states.insert(
                        capability,
                        HostCapabilityState::Unavailable("host reconnect in progress".into()),
                    );
                }
                (
                    state.ownership_debt.clone(),
                    state.owned_operation_ids.clone(),
                    state.contracts.clone(),
                )
            }
            Err(_) => return,
        };
        let affected = self
            .configured
            .union(&debt)
            .copied()
            .collect::<BTreeSet<_>>();
        let mut contracts = BTreeMap::new();
        let mut states: BTreeMap<_, _> = RuntimeCapability::ALL
            .into_iter()
            .map(|c| (c, HostCapabilityState::NativeOnly))
            .collect();
        let mut next_debt = debt.clone();
        match self.adapter.connect() {
            Err(error) => {
                contracts = previous;
                for c in &affected {
                    states.insert(*c, HostCapabilityState::Unavailable(error.clone()));
                }
            }
            Ok(connection) => {
                for capability in affected {
                    let advertised = self
                        .configured
                        .contains(&capability)
                        .then(|| connection.capabilities.get(&capability).copied())
                        .flatten();
                    if let Some(contract) = advertised {
                        contracts.insert(capability, contract);
                    }
                    let queue = advertised.is_some_and(HostCapabilityContract::queue_ready);
                    let execution = advertised.is_some_and(HostCapabilityContract::execution_ready);
                    if !queue && !execution && !debt.contains(&capability) {
                        continue;
                    }
                    match self.adapter.reconcile(capability) {
                        // Clear ownership debt only after host-wide empty state and
                        // no durable operation IDs remain in this router.
                        Ok(HostReconcileStatus::NoOutstandingWork)
                            if !owned_ids.iter().any(|(cap, _)| *cap == capability) =>
                        {
                            next_debt.remove(&capability);
                            states.insert(
                                capability,
                                if queue {
                                    HostCapabilityState::Ready
                                } else if execution {
                                    HostCapabilityState::ExecutionOnlyReady
                                } else {
                                    HostCapabilityState::NativeOnly
                                },
                            );
                        }
                        Ok(HostReconcileStatus::Ready)
                            if !debt.contains(&capability)
                                && !owned_ids.iter().any(|(cap, _)| *cap == capability)
                                && (queue || execution) =>
                        {
                            states.insert(
                                capability,
                                if queue {
                                    HostCapabilityState::Ready
                                } else {
                                    HostCapabilityState::ExecutionOnlyReady
                                },
                            );
                        }
                        Ok(_) => {
                            next_debt.insert(capability);
                            states.insert(capability, HostCapabilityState::UnknownStatusHeld);
                        }
                        Err(error) => {
                            states.insert(capability, HostCapabilityState::Unavailable(error));
                        }
                    }
                }
            }
        }
        if let Ok(mut state) = self.state.lock() {
            state.contracts = contracts;
            state.states = states;
            state.ownership_debt = next_debt;
        }
    }
}
