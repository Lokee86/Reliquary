use crate::{CompatibilityProfileId, EpisodePolicy, InsomniaWorkerConfig, Phylactery};
use std::collections::BTreeMap;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Condvar, Mutex, RwLock};
use std::time::Duration;

#[path = "runtime_host_archive_search.rs"]
mod archive_search;
#[path = "runtime_host_dependency_graph.rs"]
mod dependency_graph;
#[path = "runtime_host_dream.rs"]
mod dream;
#[path = "runtime_host_dream_owner.rs"]
mod dream_owner;
#[path = "runtime_host_files.rs"]
mod files;
#[path = "runtime_host_gateway.rs"]
mod gateway;
#[path = "runtime_host_insomnia.rs"]
mod insomnia;
#[path = "runtime_host_instances.rs"]
mod instances;
#[path = "runtime_host_interaction.rs"]
mod interaction;
#[path = "runtime_host_knowledge.rs"]
mod knowledge;
#[path = "runtime_host_knowledge_memory.rs"]
mod knowledge_memory;
#[path = "runtime_host_knowledge_relation.rs"]
mod knowledge_relation;
#[path = "runtime_host_lifecycle.rs"]
mod lifecycle;
#[path = "runtime_host_memory_provenance.rs"]
mod memory_provenance;
#[path = "runtime_host_memory_search.rs"]
pub mod memory_search;
#[path = "runtime_host_mount.rs"]
mod mount;
#[path = "runtime_host_owner_access.rs"]
mod owner_access;
#[path = "runtime_host_owner_archive.rs"]
mod owner_archive;
#[path = "runtime_host_owner_execution.rs"]
mod owner_execution;
#[path = "runtime_host_perception.rs"]
mod perception;
#[path = "runtime_host_perception_api.rs"]
mod perception_api;
#[path = "runtime_host_perception_flow.rs"]
mod perception_flow;
#[path = "runtime_host_perception_owner.rs"]
mod perception_owner;
#[path = "runtime_host_perception_queue.rs"]
mod perception_queue;
#[path = "runtime_host_perception_reconciliation.rs"]
mod perception_reconciliation;
#[path = "runtime_host_phylactery.rs"]
mod phylactery_host;
#[path = "runtime_host_project_environment.rs"]
mod project_environment;
#[path = "runtime_host_reconciliation.rs"]
mod reconciliation;
#[path = "runtime_host_routes.rs"]
mod routes;
#[path = "runtime_host_semantic_access.rs"]
mod semantic_access;
#[path = "runtime_host_semantic_echo.rs"]
mod semantic_echo;
#[path = "runtime_host_semantic_knowledge.rs"]
mod semantic_knowledge;
#[path = "runtime_host_semantic_provenance.rs"]
mod semantic_provenance;
#[path = "runtime_host_session.rs"]
mod session;
#[path = "runtime_host_status.rs"]
pub mod status;
#[path = "runtime_host_topology.rs"]
mod topology;
#[path = "runtime_host_vectors.rs"]
mod vectors;

pub(super) const IDLE_POLL: Duration = Duration::from_secs(1);
pub(super) const DREAM_RETRY_POLL: Duration = Duration::from_secs(60);
pub(super) const VECTOR_RETRY_POLL: Duration = Duration::from_secs(60);

#[derive(Debug)]
pub enum ReliquaryRuntimeHostError {
    Operation(String),
    LockPoisoned,
    ThreadPanicked,
}

impl std::fmt::Display for ReliquaryRuntimeHostError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Operation(message) => f.write_str(message),
            Self::LockPoisoned => f.write_str("Reliquary runtime host lock is poisoned"),
            Self::ThreadPanicked => f.write_str("Reliquary runtime host worker panicked"),
        }
    }
}

impl std::error::Error for ReliquaryRuntimeHostError {}

pub(super) struct Control {
    pub(super) stop: bool,
    pub(super) epoch: u64,
}

#[derive(Clone, Copy, Default)]
pub(super) struct RuntimeMemoryProfiles {
    pub(super) project: Option<CompatibilityProfileId>,
    pub(super) user: Option<CompatibilityProfileId>,
}

pub use archive_search::DEFAULT_ARCHIVE_SEARCH_RESULTS;
pub use gateway::conversation::{
    ConversationViewerToken, GatewayConversationAttachment, GatewayConversationSubmission,
    GatewaySubmissionReceipt,
};
use gateway::mount::MountClaim;
pub use gateway::{
    CapabilityRouter, CapabilityState, GatewayAdmissionAdapter, GatewayHostAdapter, GatewayRuntime,
    HandoffReceipt, HostCancelOutcome, HostCapabilityContract, HostConnection, HostContractError,
    HostOperation, HostOperationStatus, HostQueueGuarantees, HostReconcileStatus, OwnerAccessToken,
    OwnerAdmission, RuntimeCapability, RuntimeKnowledgeProvenanceResolution, StageAuthority,
    VerifiedInstanceAdmission, VisibleArchiveSearchOwner, VisibleArchiveSearchResult,
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
use owner_execution::{OwnerExecution, Shared};
pub use reconciliation::{RelReconciliationCandidateReport, RelReconciliationReport};
pub use routes::ReliquaryRuntimeRoutes;
pub use semantic_access::{RuntimeSemanticOwner, RuntimeSemanticOwnerKind};
pub use semantic_echo::{RuntimeEchoSource, RuntimeEchoTurn};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct LegacyManagedSessionState {
    pub(super) active_session_id: Option<String>,
    pub(super) reopened_session_pending: bool,
}

#[cfg(test)]
#[path = "runtime_host_legacy_isolation_tests.rs"]
mod legacy_isolation_tests;

pub struct ReliquaryRuntimeHost {
    executions: BTreeMap<String, OwnerExecution>,
    legacy_rel_claims: BTreeMap<String, MountClaim>,
    legacy_phy_claim: Mutex<Option<MountClaim>>,
    active_key: Option<String>,
    routes: Arc<RwLock<ReliquaryRuntimeRoutes>>,
    phylactery: Arc<Mutex<Option<Phylactery>>>,
    config: InsomniaWorkerConfig,
    episode_policy: EpisodePolicy,
    active_insomnia_enabled: AtomicBool,
    managed_sessions: Mutex<BTreeMap<String, LegacyManagedSessionState>>,
}

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

const LEGACY_OWNER_KEY: &str = "\0legacy";

pub(super) fn owner_key(owner_id: Option<&str>) -> String {
    owner_id.unwrap_or(LEGACY_OWNER_KEY).to_owned()
}
