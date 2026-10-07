use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

/// A separately negotiated external stage. Internal Reliquary-owned work remains native.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum RuntimeCapability {
    ConversationExecution,
    Insomnia,
    Dream,
    Embedding,
    ProjectEnvironment,
}

impl RuntimeCapability {
    /// A host claim cannot replace a stage without an executable result/receipt path.
    fn host_executable(self) -> bool {
        matches!(self, Self::ConversationExecution)
    }

    pub const ALL: [Self; 5] = [
        Self::ConversationExecution,
        Self::Insomnia,
        Self::Dream,
        Self::Embedding,
        Self::ProjectEnvironment,
    ];
}

/// Contract claims an adapter must satisfy before it may own upstream pending work.
///
/// These are adapter declarations, not independent proof of a real provider's
/// behavior. Real host integrations must verify the claims during their own
/// conformance work.
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

/// One capability's executable boundary and optional queue contract.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct HostCapabilityContract {
    /// The connected host executes this capability's stage.
    pub host_executes_stage: bool,
    /// Required only when the host asks to own pending queue admission.
    pub queue: HostQueueGuarantees,
}

impl HostCapabilityContract {
    pub fn may_own_pending_queue(self) -> bool {
        self.host_executes_stage && self.queue.complete()
    }
}

/// Contracts returned by one authenticated startup or reconnect handshake.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct HostConnection {
    pub capabilities: BTreeMap<RuntimeCapability, HostCapabilityContract>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostReconcileStatus {
    Ready,
    NoOutstandingWork,
    UnknownStatus,
}

/// Stable external operation identity and payload binding.
///
/// The gateway must persist its handoff-intent record before calling the
/// adapter handoff method. Retrying the same ID and digest must return the same
/// durable receipt; reusing an ID with a different digest is a conflict.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostOperation {
    pub stable_id: String,
    pub payload_digest: [u8; 32],
    /// Canonical immutable stage input. The digest is checked before every handoff.
    pub payload: Vec<u8>,
}

impl HostOperation {
    pub fn validate(&self) -> Result<(), HostContractError> {
        if self.stable_id.trim().is_empty() || self.stable_id.len() > 256 {
            return Err(HostContractError::InvalidOperationId);
        }
        if <[u8; 32]>::from(Sha256::digest(&self.payload)) != self.payload_digest {
            return Err(HostContractError::PayloadDigestMismatch);
        }
        Ok(())
    }
}

/// Receipt that transfers a stage only after the receiver durably accepted the
/// same operation identity and payload fingerprint.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HandoffReceipt {
    pub stable_id: String,
    pub payload_digest: [u8; 32],
    pub durably_accepted: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum HostOperationStatus {
    Pending,
    Running,
    Completed,
    Cancelled,
    Unknown,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum HostCancelOutcome {
    /// Cancellation won the host's pending/dequeue serialization gate.
    CancelledBeforeStart,
    /// The operation already started; this is a cooperative request, not proof it stopped.
    RunningCancellationRequested,
    /// The host reports an existing terminal state.
    AlreadyTerminal(HostOperationStatus),
    /// The host cannot prove the operation's current state.
    Unknown,
}

/// Adapter boundary for host-owned stage execution.
///
/// Implementations must return durable acceptance and exact status by stable ID.
/// The cancel operation and provider dequeue must linearize on the same
/// pending-state transition. A cancel accepted before dequeue prevents execution;
/// if dequeue already won, the result must be RunningCancellationRequested. These
/// method contracts do not by themselves establish that a real external host conforms.
pub trait GatewayHostAdapter: Send + Sync {
    /// Capability intent configured by the application before connecting.
    /// On connect failure, only these dependent stages become unavailable.
    fn configured_capabilities(&self) -> BTreeSet<RuntimeCapability>;

    /// Authenticate/connect and advertise exact per-capability contracts.
    fn connect(&self) -> Result<HostConnection, String>;

    /// Reconcile prior work at one capability boundary before dispatch resumes.
    fn reconcile(&self, capability: RuntimeCapability) -> Result<HostReconcileStatus, String>;

    /// Accept or return the prior durable receipt for the same operation ID.
    fn handoff(
        &self,
        capability: RuntimeCapability,
        operation: &HostOperation,
    ) -> Result<HandoffReceipt, String>;

    /// Cancel/query by stable identity. Pending cancel must serialize with dequeue.
    fn cancel(
        &self,
        capability: RuntimeCapability,
        stable_id: &str,
    ) -> Result<HostCancelOutcome, String>;

    fn status(
        &self,
        capability: RuntimeCapability,
        stable_id: &str,
    ) -> Result<HostOperationStatus, String>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CapabilityState {
    /// No host stage is configured or the adapter lacks the full queue contract.
    NativeOnly,
    /// The complete advertised contract reconciled successfully.
    Ready,
    /// This stage's configured host boundary is unavailable.
    Unavailable(String),
    /// Work status is unknown. Never dispatch or replay this stage blindly.
    UnknownStatusHeld,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StageAuthority {
    Native,
    Host,
    Held,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HostContractError {
    InvalidOperationId,
    PayloadDigestMismatch,
    HostNotReady(RuntimeCapability),
    Adapter(String),
    InvalidHandoffReceipt,
    IdempotencyConflict,
    UnknownOperationStatus,
    HostCancelUnknown,
    LockPoisoned,
}

impl std::fmt::Display for HostContractError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidOperationId => {
                f.write_str("host operation ID must be non-empty and at most 256 bytes")
            }
            Self::PayloadDigestMismatch => {
                f.write_str("host operation payload digest does not match its payload")
            }
            Self::HostNotReady(capability) => write!(f, "host stage {capability:?} is not ready"),
            Self::Adapter(message) => f.write_str(message),
            Self::InvalidHandoffReceipt => {
                f.write_str("host handoff receipt did not durably accept the exact operation")
            }
            Self::IdempotencyConflict => {
                f.write_str("host operation ID was reused with a different payload digest")
            }
            Self::UnknownOperationStatus => {
                f.write_str("host operation status is unknown; replay is held")
            }
            Self::HostCancelUnknown => {
                f.write_str("host could not determine cancellation outcome; work is held")
            }
            Self::LockPoisoned => f.write_str("host capability router lock is poisoned"),
        }
    }
}

impl std::error::Error for HostContractError {}

#[derive(Debug)]
struct RouterState {
    contracts: BTreeMap<RuntimeCapability, HostCapabilityContract>,
    states: BTreeMap<RuntimeCapability, CapabilityState>,
    /// Capabilities that may still have host-owned work. This survives
    /// contract downgrades and adapter errors until reconciliation proves empty.
    ownership_debt: BTreeSet<RuntimeCapability>,
    owned_operation_ids: BTreeSet<(RuntimeCapability, String)>,
    // Validation cache only. Durable identity/recovery remains in the REL journal.
    operation_digests: BTreeMap<(RuntimeCapability, String), [u8; 32]>,
    mutation_generation: u64,
}

/// Single canonical readiness/authority map for host-integrated stages.
///
/// Native stages remain usable while unrelated host stages are unavailable or
/// held. Only capabilities with the full advertised queue contract can transfer
/// pending queue ownership to a host.
pub struct CapabilityRouter {
    adapter: Arc<dyn GatewayHostAdapter>,
    configured: BTreeSet<RuntimeCapability>,
    /// Serializes reconnect/reconciliation with each other. Adapter operation calls
    /// run outside this lock so cancellation can race an outstanding handoff.
    transition: Mutex<()>,
    state: Mutex<RouterState>,
}

impl CapabilityRouter {
    /// Constructing the router immediately connects and reconciles each advertised
    /// capability before any caller can ask for dispatch authority.
    pub fn connect(adapter: Arc<dyn GatewayHostAdapter>) -> Self {
        let configured = adapter
            .configured_capabilities()
            .into_iter()
            .filter(|capability| capability.host_executable())
            .collect::<BTreeSet<_>>();
        let router = Self {
            adapter,
            configured: configured.clone(),
            transition: Mutex::new(()),
            state: Mutex::new(RouterState {
                contracts: BTreeMap::new(),
                ownership_debt: BTreeSet::new(),
                owned_operation_ids: BTreeSet::new(),
                operation_digests: BTreeMap::new(),
                mutation_generation: 0,
                states: RuntimeCapability::ALL
                    .into_iter()
                    .map(|capability| (capability, CapabilityState::NativeOnly))
                    .collect(),
            }),
        };
        router.refresh();
        router
    }

    /// Retry a connection. Only configured capabilities are changed by failures.
    pub fn reconnect(&self) {
        self.refresh();
    }

    fn refresh(&self) {
        let Ok(_transition) = self.transition.lock() else {
            return;
        };

        let (previous_contracts, ownership_debt, owned_operation_ids, observed_generation) =
            match self.state.lock() {
                Ok(mut state) => {
                    // Fence configured host stages before making a potentially slow
                    // reconnect call. No observer may see stale Host authority meanwhile.
                    let affected = self
                        .configured
                        .union(&state.ownership_debt)
                        .copied()
                        .collect::<BTreeSet<_>>();
                    for capability in affected {
                        state.states.insert(
                            capability,
                            CapabilityState::Unavailable("host reconnect in progress".into()),
                        );
                    }
                    state.mutation_generation = state.mutation_generation.wrapping_add(1);
                    let generation = state.mutation_generation;
                    (
                        state.contracts.clone(),
                        state.ownership_debt.clone(),
                        state.owned_operation_ids.clone(),
                        generation,
                    )
                }
                Err(_) => return,
            };

        let connected = self.adapter.connect();
        let mut contracts = BTreeMap::new();
        let mut states = RuntimeCapability::ALL
            .into_iter()
            .map(|capability| (capability, CapabilityState::NativeOnly))
            .collect::<BTreeMap<_, _>>();
        let affected = self
            .configured
            .union(&ownership_debt)
            .copied()
            .collect::<BTreeSet<_>>();
        let mut next_debt = ownership_debt;
        // Only durable per-operation terminal recording may release known IDs.

        match connected {
            Err(error) => {
                // Keep prior contract and ownership debt. Failure cannot establish
                // that previously accepted work disappeared.
                contracts = previous_contracts;
                for capability in &affected {
                    states.insert(*capability, CapabilityState::Unavailable(error.clone()));
                }
            }
            Ok(connection) => {
                for capability in &affected {
                    let has_debt = next_debt.contains(capability);
                    let advertised = self
                        .configured
                        .contains(capability)
                        .then(|| connection.capabilities.get(capability).copied())
                        .flatten();

                    if let Some(contract) = advertised {
                        contracts.insert(*capability, contract);
                    }
                    // A withdrawn contract must also fence recovered handoff retries.
                    // Status/cancel remain available without a dispatch contract.

                    let complete =
                        advertised.is_some_and(HostCapabilityContract::may_own_pending_queue);
                    if !complete && !has_debt {
                        // An initially unsupported/incomplete stage remains native.
                        continue;
                    }

                    // Do not transfer pending-queue authority until the
                    // current host proves it can reconcile prior operations.
                    let reconciled = self.adapter.reconcile(*capability);
                    match reconciled {
                        Ok(HostReconcileStatus::NoOutstandingWork)
                            if !owned_operation_ids.iter().any(|(cap, _)| cap == capability) =>
                        {
                            next_debt.remove(capability);
                            states.insert(
                                *capability,
                                if complete {
                                    CapabilityState::Ready
                                } else {
                                    CapabilityState::NativeOnly
                                },
                            );
                        }
                        Ok(HostReconcileStatus::Ready) if complete && !has_debt => {
                            states.insert(*capability, CapabilityState::Ready);
                        }
                        Ok(
                            HostReconcileStatus::NoOutstandingWork
                            | HostReconcileStatus::Ready
                            | HostReconcileStatus::UnknownStatus,
                        ) => {
                            // The host has not disproved ownership of prior
                            // pending work. Never replay it through a native queue.
                            next_debt.insert(*capability);
                            states.insert(*capability, CapabilityState::UnknownStatusHeld);
                        }
                        Err(error) => {
                            states.insert(*capability, CapabilityState::Unavailable(error));
                        }
                    }
                }
            }
        }

        if let Ok(mut state) = self.state.lock() {
            if state.mutation_generation != observed_generation {
                // An operation call changed ownership while this refresh was away
                // from the state lock. Its snapshot is stale, so preserve all IDs
                // and fail closed until a later reconciliation succeeds.
                // Contract negotiation is current even when operation ownership
                // changed concurrently. Never retain a withdrawn dispatch contract.
                state.contracts = contracts;
                for capability in &affected {
                    state.ownership_debt.insert(*capability);
                    let held = match states.get(capability) {
                        Some(CapabilityState::Unavailable(error)) => {
                            CapabilityState::Unavailable(error.clone())
                        }
                        _ => CapabilityState::UnknownStatusHeld,
                    };
                    state.states.insert(*capability, held);
                }
                state.mutation_generation = state.mutation_generation.wrapping_add(1);
                return;
            }
            state.contracts = contracts;
            state.states = states;
            state.ownership_debt = next_debt;
            state.mutation_generation = state.mutation_generation.wrapping_add(1);
        }
    }

    pub fn state(
        &self,
        capability: RuntimeCapability,
    ) -> Result<CapabilityState, HostContractError> {
        Ok(self
            .state
            .lock()
            .map_err(|_| HostContractError::LockPoisoned)?
            .states
            .get(&capability)
            .cloned()
            .unwrap_or(CapabilityState::NativeOnly))
    }

    /// Queue dispatch authority, not evidence that an external host has
    /// fulfilled the actual handoff and cancellation contracts.
    pub fn authority(
        &self,
        capability: RuntimeCapability,
    ) -> Result<StageAuthority, HostContractError> {
        let state = self.state(capability)?;
        Ok(match state {
            CapabilityState::NativeOnly => StageAuthority::Native,
            CapabilityState::Ready => StageAuthority::Host,
            CapabilityState::Unavailable(_) | CapabilityState::UnknownStatusHeld => {
                StageAuthority::Held
            }
        })
    }

    /// Reconstruct known host-owned work from durable per-REL journal records.
    pub fn remember_operation(
        &self,
        capability: RuntimeCapability,
        stable_id: impl Into<String>,
    ) -> Result<(), HostContractError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| HostContractError::LockPoisoned)?;
        let id_added = state
            .owned_operation_ids
            .insert((capability, stable_id.into()));
        let debt_added = state.ownership_debt.insert(capability);
        if id_added || debt_added {
            state.mutation_generation = state.mutation_generation.wrapping_add(1);
        }
        Ok(())
    }

    /// Mark a capability held when a remote outcome is uncertain.
    pub fn hold_operation(
        &self,
        capability: RuntimeCapability,
        stable_id: impl Into<String>,
    ) -> Result<(), HostContractError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| HostContractError::LockPoisoned)?;
        state
            .owned_operation_ids
            .insert((capability, stable_id.into()));
        state.ownership_debt.insert(capability);
        state
            .states
            .insert(capability, CapabilityState::UnknownStatusHeld);
        state.mutation_generation = state.mutation_generation.wrapping_add(1);
        Ok(())
    }

    /// Clear one operation only after durable terminal status was recorded locally.
    pub fn release_operation(
        &self,
        capability: RuntimeCapability,
        stable_id: &str,
    ) -> Result<(), HostContractError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| HostContractError::LockPoisoned)?;
        if state
            .owned_operation_ids
            .remove(&(capability, stable_id.to_owned()))
        {
            state.mutation_generation = state.mutation_generation.wrapping_add(1);
        }
        Ok(())
    }

    /// Persist the intent before calling this method. Errors leave the capability held.
    fn mark_unknown_status(&self, capability: RuntimeCapability) -> Result<(), HostContractError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| HostContractError::LockPoisoned)?;
        state
            .states
            .insert(capability, CapabilityState::UnknownStatusHeld);
        state.ownership_debt.insert(capability);
        state.mutation_generation = state.mutation_generation.wrapping_add(1);
        Ok(())
    }

    fn bind_handoff(
        &self,
        capability: RuntimeCapability,
        operation: &HostOperation,
        retry: bool,
    ) -> Result<(), HostContractError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| HostContractError::LockPoisoned)?;
        let key = (capability, operation.stable_id.clone());
        if state
            .operation_digests
            .get(&key)
            .is_some_and(|digest| *digest != operation.payload_digest)
        {
            return Err(HostContractError::IdempotencyConflict);
        }
        if retry && !state.owned_operation_ids.contains(&key) {
            return Err(HostContractError::UnknownOperationStatus);
        }
        let expected = if retry {
            CapabilityState::UnknownStatusHeld
        } else {
            CapabilityState::Ready
        };
        let eligible = state.states.get(&capability) == Some(&expected)
            && capability.host_executable()
            && state
                .contracts
                .get(&capability)
                .copied()
                .is_some_and(HostCapabilityContract::may_own_pending_queue);
        if !eligible {
            if capability.host_executable() && !retry {
                state.owned_operation_ids.insert(key);
                state.ownership_debt.insert(capability);
                state
                    .states
                    .insert(capability, CapabilityState::UnknownStatusHeld);
                state.mutation_generation = state.mutation_generation.wrapping_add(1);
            }
            return Err(HostContractError::HostNotReady(capability));
        }
        state
            .operation_digests
            .insert(key.clone(), operation.payload_digest);
        state.owned_operation_ids.insert(key);
        state.ownership_debt.insert(capability);
        state.mutation_generation = state.mutation_generation.wrapping_add(1);
        Ok(())
    }

    pub fn handoff(
        &self,
        capability: RuntimeCapability,
        operation: &HostOperation,
    ) -> Result<HandoffReceipt, HostContractError> {
        operation.validate()?;
        self.bind_handoff(capability, operation, false)?;
        let result = self.adapter.handoff(capability, operation);
        let receipt = match result {
            Ok(receipt)
                if receipt.stable_id == operation.stable_id
                    && receipt.payload_digest == operation.payload_digest
                    && receipt.durably_accepted =>
            {
                receipt
            }
            Ok(_) => {
                self.hold_operation(capability, operation.stable_id.clone())?;
                self.mark_unknown_status(capability)?;
                return Err(HostContractError::InvalidHandoffReceipt);
            }
            Err(error) => {
                self.hold_operation(capability, operation.stable_id.clone())?;
                self.mark_unknown_status(capability)?;
                return Err(HostContractError::Adapter(error));
            }
        };
        self.remember_operation(capability, operation.stable_id.clone())?;
        Ok(receipt)
    }

    /// Retry only an already durable intent with the same stable ID and digest.
    /// The caller must compare the persisted intent before invoking this method.
    pub fn retry_handoff(
        &self,
        capability: RuntimeCapability,
        operation: &HostOperation,
    ) -> Result<HandoffReceipt, HostContractError> {
        operation.validate()?;
        self.bind_handoff(capability, operation, true)?;
        match self.adapter.handoff(capability, operation) {
            Ok(receipt)
                if receipt.stable_id == operation.stable_id
                    && receipt.payload_digest == operation.payload_digest
                    && receipt.durably_accepted =>
            {
                self.remember_operation(capability, operation.stable_id.clone())?;
                Ok(receipt)
            }
            Ok(_) => {
                self.hold_operation(capability, operation.stable_id.clone())?;
                Err(HostContractError::InvalidHandoffReceipt)
            }
            Err(error) => {
                self.hold_operation(capability, operation.stable_id.clone())?;
                Err(HostContractError::Adapter(error))
            }
        }
    }

    /// Query is allowed during downgrade/reconnect so prior work can be reconciled.
    pub fn status(
        &self,
        capability: RuntimeCapability,
        stable_id: &str,
    ) -> Result<HostOperationStatus, HostContractError> {
        match self.adapter.status(capability, stable_id) {
            Ok(status) => {
                match status {
                    HostOperationStatus::Pending | HostOperationStatus::Running => {
                        self.remember_operation(capability, stable_id.to_owned())?;
                    }
                    HostOperationStatus::Unknown => {
                        self.hold_operation(capability, stable_id.to_owned())?;
                        self.mark_unknown_status(capability)?;
                    }
                    HostOperationStatus::Completed | HostOperationStatus::Cancelled => {
                        self.remember_operation(capability, stable_id.to_owned())?;
                    }
                }
                Ok(status)
            }
            Err(error) => {
                self.hold_operation(capability, stable_id.to_owned())?;
                self.mark_unknown_status(capability)?;
                Err(HostContractError::Adapter(error))
            }
        }
    }

    /// Cancel is allowed during downgrade so a pending handoff can still be stopped.
    pub fn cancel(
        &self,
        capability: RuntimeCapability,
        stable_id: &str,
    ) -> Result<HostCancelOutcome, HostContractError> {
        match self.adapter.cancel(capability, stable_id) {
            Ok(outcome) => {
                match outcome {
                    HostCancelOutcome::CancelledBeforeStart
                    | HostCancelOutcome::AlreadyTerminal(
                        HostOperationStatus::Completed | HostOperationStatus::Cancelled,
                    ) => {
                        self.remember_operation(capability, stable_id.to_owned())?;
                    }
                    HostCancelOutcome::RunningCancellationRequested
                    | HostCancelOutcome::AlreadyTerminal(
                        HostOperationStatus::Pending | HostOperationStatus::Running,
                    ) => {
                        self.remember_operation(capability, stable_id.to_owned())?;
                    }
                    HostCancelOutcome::Unknown
                    | HostCancelOutcome::AlreadyTerminal(HostOperationStatus::Unknown) => {
                        self.hold_operation(capability, stable_id.to_owned())?;
                        self.mark_unknown_status(capability)?;
                    }
                }
                Ok(outcome)
            }
            Err(error) => {
                self.hold_operation(capability, stable_id.to_owned())?;
                self.mark_unknown_status(capability)?;
                Err(HostContractError::Adapter(error))
            }
        }
    }
}

#[cfg(test)]
#[path = "runtime_host_gateway_host_tests.rs"]
mod tests;
