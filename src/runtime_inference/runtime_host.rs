use std::sync::{Condvar, Mutex};
use std::time::Duration;

#[path = "runtime_host_archive_search.rs"]
mod archive_search;
#[path = "runtime_host_gateway.rs"]
mod gateway;
#[path = "runtime_host_instances.rs"]
mod instances;
#[path = "runtime_host_knowledge.rs"]
mod knowledge;
#[path = "runtime_host_memory_search.rs"]
pub mod memory_search;
#[path = "runtime_host_reconciliation.rs"]
mod reconciliation;
#[path = "runtime_host_semantic_access.rs"]
mod semantic_access;
#[path = "runtime_host_semantic_echo.rs"]
mod semantic_echo;
#[path = "runtime_host_status.rs"]
pub mod status;

pub(super) const IDLE_POLL: Duration = Duration::from_secs(1);
pub(super) const DREAM_RETRY_POLL: Duration = Duration::from_secs(60);
pub(super) const VECTOR_RETRY_POLL: Duration = Duration::from_secs(60);

pub use crate::runtime_shared::{ReliquaryRuntimeHostError, ReliquaryRuntimeRoutes};

pub(super) struct Control {
    pub(super) stop: bool,
    pub(super) epoch: u64,
}

pub use archive_search::DEFAULT_ARCHIVE_SEARCH_RESULTS;
pub use gateway::conversation::{
    ConversationViewerToken, GatewayConversationAttachment, GatewayConversationSubmission,
    GatewaySubmissionReceipt,
};
pub use gateway::{
    CapabilityRouter, CapabilityState, GatewayAdmissionAdapter, GatewayHostAdapter, GatewayRuntime,
    HandoffReceipt, HostCancelOutcome, HostCapabilityContract, HostConnection, HostContractError,
    HostExecutionMode, HostOperation, HostOperationStatus, HostQueueGuarantees,
    HostReconcileStatus, OwnerAccessToken, OwnerAdmission, RuntimeCapability,
    RuntimeKnowledgeProvenanceResolution, StageAuthority, VerifiedInstanceAdmission,
    VisibleArchiveSearchOwner, VisibleArchiveSearchResult,
};
pub use gateway::{
    CommunitySummary, ConversationInspection, MAX_MANAGEMENT_FILE_BYTES, MAX_MANAGEMENT_PAGE_BYTES,
    MAX_MANAGEMENT_PAGE_ITEMS, ManagedBranch, ManagementError, ManagementPage, ManagementVersions,
    MemorySummary, RelManagementHealth, SemanticOwnerStatistics, SourceFile,
    UnsupportedManagementOperation, WorkspaceManagement,
};
pub use gateway::{
    LiveCapability, LiveCheckpoint, LiveCheckpointShape, LiveDiagnostics, LiveGeneratedArtifact,
    LiveGeneration, LiveGenerationStatus, LiveResumeToken, LiveRuntimeAdapter, LiveRuntimeError,
    LiveSession, LiveStreamCursor, LiveStreamEvent, LiveStreamSnapshot, LiveToolEvent,
    LiveToolEventKind,
};
pub use instances::{
    InstanceAuthorization, InstanceId, InstanceRuntime, InstanceRuntimeError,
    ManagedConversationState, OwnerGrant,
};
pub use knowledge::RuntimeKnowledgeState;
pub use reconciliation::{RelReconciliationCandidateReport, RelReconciliationReport};
pub use semantic_access::{RuntimeSemanticOwner, RuntimeSemanticOwnerKind};
pub use semantic_echo::{RuntimeEchoSource, RuntimeEchoTurn};

pub(super) fn stopped(
    signal: &(Mutex<Control>, Condvar),
) -> Result<bool, ReliquaryRuntimeHostError> {
    signal
        .0
        .lock()
        .map(|state| state.stop)
        .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)
}

pub(super) fn wait_for_work(
    signal: &(Mutex<Control>, Condvar),
    seen_epoch: u64,
) -> Result<u64, ReliquaryRuntimeHostError> {
    wait_for_work_timeout(signal, seen_epoch, IDLE_POLL)
}

pub(super) fn wait_for_work_timeout(
    signal: &(Mutex<Control>, Condvar),
    seen_epoch: u64,
    timeout: Duration,
) -> Result<u64, ReliquaryRuntimeHostError> {
    let mut control = signal
        .0
        .lock()
        .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
    if control.epoch == seen_epoch && !control.stop {
        let (next, _) = signal
            .1
            .wait_timeout(control, timeout)
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        control = next;
    }
    Ok(control.epoch)
}

pub(super) fn notify_work(
    signal: &(Mutex<Control>, Condvar),
) -> Result<(), ReliquaryRuntimeHostError> {
    let mut control = signal
        .0
        .lock()
        .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
    control.epoch = control.epoch.wrapping_add(1);
    signal.1.notify_all();
    Ok(())
}

pub(super) fn operation(error: impl std::fmt::Display) -> ReliquaryRuntimeHostError {
    ReliquaryRuntimeHostError::Operation(error.to_string())
}

#[cfg(test)]
#[path = "../project_environment_tests.rs"]
mod project_environment_tests;

#[cfg(test)]
#[path = "../project_environment_git_tests.rs"]
mod project_environment_git_tests;

#[cfg(test)]
#[path = "../project_environment_lore_tests.rs"]
mod project_environment_lore_tests;

#[cfg(test)]
#[path = "../semantic_graph/semantic_graph_runtime_host_tests.rs"]
mod semantic_graph_runtime_host_tests;

#[cfg(test)]
#[path = "runtime_host_backpressure_tests.rs"]
mod runtime_host_backpressure_tests;
