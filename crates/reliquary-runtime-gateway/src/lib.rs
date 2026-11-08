//! Canonical read-only owner mounting and principal-scoped Instance access.
//!
//! Gateway owns the mounted-session registry. Instances carry immutable PHY
//! grants and never own or clone mutable domain stores.

use reliquary_application::{PreparedIngest, commit_prepared_native_ingest};
use reliquary_owner_session::{
    AcceptedEpisodeOrder, CompletionCheck, GraphAdmission, OwnerSessionError, PhySession,
    PhyWriter, RelInsomniaClaimInput, RelSession, RelWriter, records::DreamBeginAdmission,
};
use uuid::Uuid;

mod conversation_queue;
mod generation_stream;
mod host_lifecycle;
mod host_runtime;
use conversation_queue::ArchiveCommitProof;
pub use conversation_queue::{
    ArchiveReceipt, ConversationQueue, ConversationSubmissionId, DurableGeneration,
    DurableGenerationCheckpoint, DurableGenerationPhase, DurableHostOperation,
    DurableHostOperationPhase, EnqueueOutcome, FinalizeCapture, GenerationReplay, HandoffReceipt,
    HostCancelOutcome, HostOperationStatus, QueueError, QueueState, QueuedSubmission,
    RuntimeCapability, SubmissionId, SubmissionStatus,
};
pub use generation_stream::{
    GenerationStreamDelivery, GenerationStreamEvent, GenerationStreamPayload,
    GenerationStreamSnapshot, GenerationStreamSubscription,
};
pub use host_lifecycle::{
    CapabilityRouter, HostCapabilityContract, HostCapabilityState, HostConnection,
    HostExecutionMode,
};
pub use host_runtime::{
    HostDispatchOutcome, HostOperation, HostQueueGuarantees, HostReconcileStatus,
    TrustedHostAdapter,
};
use std::{
    collections::{BTreeSet, HashMap},
    fmt,
    path::Path,
    sync::{
        Arc, Mutex, RwLock,
        atomic::{AtomicBool, Ordering},
    },
};

const MIN_CREDENTIAL_BYTES: usize = 32;
const MAX_CREDENTIAL_BYTES: usize = 4096;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct OwnerId([u8; 16]);
impl OwnerId {
    pub const fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }
    pub const fn as_bytes(self) -> [u8; 16] {
        self.0
    }
}

#[derive(Debug)]
pub enum GatewayError {
    OwnerSession(OwnerSessionError),
    Management(reliquary_application::ManagementError),
    ConversationQueue(conversation_queue::QueueError),
    DuplicateOwner(OwnerId),
    AdmissionOwnerMismatch {
        expected: OwnerId,
        actual: Option<OwnerId>,
    },
    UnknownOwner(OwnerId),
    WrongOwnerKind(OwnerId),
    UnauthorizedRel {
        principal: String,
        owner: OwnerId,
    },
    UnauthorizedPhy {
        principal: String,
        owner: OwnerId,
    },
    UnauthorizedQueueSubmission {
        principal: String,
        owner: OwnerId,
    },
    InvalidPrincipal,
    InvalidCredential,
    DuplicateCredential,
    CredentialRevoked,
    InvalidGenerationSubscription,
    HostAdapter(String),
    HostStatusUnknown,
    InvalidHandoffReceipt,
    InvalidPreparedRetrieval,
    StaleRetrieval,
    Retrieval(String),
    RetrievalUnavailable(&'static str),
    IncompleteHostContract(RuntimeCapability),
    UnsupportedHostCapability(RuntimeCapability),
    RoutedMemory(String),
    ProjectSource(String),
    ExternalSource(reliquary_owner_session::records::external_index::model::ExternalSourceError),
    NativeImportConflict,
    RegistryPoisoned,
    WriterUnavailable,
}
impl fmt::Display for GatewayError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OwnerSession(e) => e.fmt(f),
            Self::Management(e) => e.fmt(f),
            Self::ConversationQueue(e) => e.fmt(f),
            Self::DuplicateOwner(id) => write!(f, "owner {:?} is already mounted", id.0),
            Self::AdmissionOwnerMismatch { expected, actual } => write!(
                f,
                "graph admission owner {:?} does not match mounted owner {:?}",
                actual.map(|id| id.0),
                expected.0
            ),
            Self::UnknownOwner(id) => write!(f, "owner {:?} is not mounted", id.0),
            Self::WrongOwnerKind(id) => write!(f, "owner {:?} has the wrong file kind", id.0),
            Self::UnauthorizedRel { principal, owner } => write!(
                f,
                "principal {principal:?} has no grant for REL owner {:?}",
                owner.0
            ),
            Self::UnauthorizedPhy { principal, owner } => write!(
                f,
                "principal {principal:?} has no grant for PHY owner {:?}",
                owner.0
            ),
            Self::UnauthorizedQueueSubmission { principal, owner } => write!(
                f,
                "principal {principal:?} cannot cancel another principal's pending submission for REL owner {:?}",
                owner.0
            ),
            Self::InvalidPrincipal => {
                f.write_str("principal must be non-empty and contain no control characters")
            }
            Self::InvalidCredential => f.write_str("credential is invalid"),
            Self::DuplicateCredential => f.write_str("credential is already configured"),
            Self::CredentialRevoked => f.write_str("credential has been revoked"),
            Self::InvalidGenerationSubscription => {
                f.write_str("generation subscription belongs to another instance or stream")
            }
            Self::HostAdapter(reason) => write!(f, "trusted host adapter failed: {reason}"),
            Self::HostStatusUnknown => {
                f.write_str("host status is unknown; operation remains held")
            }
            Self::InvalidHandoffReceipt => {
                f.write_str("host handoff receipt does not match the durable operation")
            }
            Self::InvalidPreparedRetrieval => {
                f.write_str("prepared retrieval result is invalid for this Instance")
            }
            Self::StaleRetrieval => {
                f.write_str("retrieval result no longer matches current authorized owner state")
            }
            Self::Retrieval(reason) => write!(f, "agentic retrieval failed: {reason}"),
            Self::RetrievalUnavailable(reason) => f.write_str(reason),
            Self::IncompleteHostContract(capability) => {
                write!(f, "host queue contract is incomplete for {capability:?}")
            }
            Self::UnsupportedHostCapability(capability) => {
                write!(f, "host execution is not enabled for {capability:?}")
            }
            Self::RoutedMemory(reason) => write!(f, "routed user Memory failed: {reason}"),
            Self::ProjectSource(reason) => write!(f, "project source failed: {reason}"),
            Self::ExternalSource(error) => error.fmt(f),
            Self::NativeImportConflict => {
                f.write_str("native import conflicts with the mounted owner's source or open epoch")
            }
            Self::RegistryPoisoned => f.write_str("gateway owner registry is unavailable"),
            Self::WriterUnavailable => f.write_str("owner writer session is unavailable"),
        }
    }
}
impl std::error::Error for GatewayError {}
impl From<OwnerSessionError> for GatewayError {
    fn from(e: OwnerSessionError) -> Self {
        Self::OwnerSession(e)
    }
}
impl From<conversation_queue::QueueError> for GatewayError {
    fn from(error: conversation_queue::QueueError) -> Self {
        Self::ConversationQueue(error)
    }
}

#[derive(Clone)]
enum MountedOwner {
    Rel(Arc<RelSession>),
    RelWriter(Arc<MountedRelWriter>),
    Phy(Arc<PhySession>),
    PhyWriter(Arc<MountedPhyWriter>),
}
struct MountedRelWriter {
    writer: std::sync::Mutex<RelWriter>,
}
struct MountedPhyWriter {
    writer: std::sync::Mutex<PhyWriter>,
}

impl MountedOwner {
    fn is_rel(&self) -> bool {
        matches!(self, Self::Rel(_) | Self::RelWriter(_))
    }
}

/// One canonical registry of mounted owner snapshots.
#[derive(Clone, Default)]
pub struct Gateway {
    owners: Arc<RwLock<HashMap<OwnerId, MountedOwner>>>,
    owner_worker_leases: Arc<Mutex<BTreeSet<(OwnerId, u8)>>>,
}
fn validate_principal(principal: &str) -> Result<(), GatewayError> {
    if principal.trim().is_empty() || principal.chars().any(char::is_control) {
        Err(GatewayError::InvalidPrincipal)
    } else {
        Ok(())
    }
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(DIGITS[usize::from(byte >> 4)] as char);
        output.push(DIGITS[usize::from(byte & 0x0f)] as char);
    }
    output
}

/// One trusted host-side credential-to-grant configuration entry.
///
/// Keep these values in host configuration; never populate them from a request.
pub struct HostCredentialBinding {
    principal: String,
    token: String,
    rel_grants: BTreeSet<OwnerId>,
    phy_grants: BTreeSet<OwnerId>,
}
impl HostCredentialBinding {
    pub fn new(
        principal: impl Into<String>,
        token: impl Into<String>,
        rel_grants: impl IntoIterator<Item = OwnerId>,
        phy_grants: impl IntoIterator<Item = OwnerId>,
    ) -> Self {
        Self {
            principal: principal.into(),
            token: token.into(),
            rel_grants: rel_grants.into_iter().collect(),
            phy_grants: phy_grants.into_iter().collect(),
        }
    }
}

/// Trusted startup configuration that binds host credentials to exact owner grants.
///
/// Keep this capability in the host configuration layer. Pass only the resulting
/// `HostAdmissionAdapter` to request handlers; a `Gateway` or `Instance` alone
/// cannot create or alter grant bindings.
pub struct TrustedHostAdmission {
    bindings: Vec<HostCredentialBinding>,
}
impl TrustedHostAdmission {
    pub fn new(bindings: impl IntoIterator<Item = HostCredentialBinding>) -> Self {
        Self {
            bindings: bindings.into_iter().collect(),
        }
    }

    /// Install the fixed host configuration against the mounted owner registry.
    pub fn install(
        &self,
        gateway: &Gateway,
    ) -> Result<(HostAdmissionAdapter, HostAdmissionControl), GatewayError> {
        let mut credentials = HashMap::new();
        for binding in &self.bindings {
            if !(MIN_CREDENTIAL_BYTES..=MAX_CREDENTIAL_BYTES).contains(&binding.token.len())
                || binding.token.chars().any(char::is_control)
            {
                return Err(GatewayError::InvalidCredential);
            }
            validate_principal(&binding.principal)?;
            let token_hash = *blake3::hash(binding.token.as_bytes()).as_bytes();
            if credentials.contains_key(&token_hash) {
                return Err(GatewayError::DuplicateCredential);
            }
            // Validate all grants at installation time so a bad binding can
            // never partially admit a request.
            gateway.validate_grants(&binding.rel_grants, &binding.phy_grants)?;
            let active = Arc::new(AtomicBool::new(true));
            credentials.insert(
                token_hash,
                AdmissionRecord {
                    principal: binding.principal.clone(),
                    rel_grants: binding.rel_grants.clone(),
                    phy_grants: binding.phy_grants.clone(),
                    active,
                },
            );
        }
        let active = credentials
            .iter()
            .map(|(hash, record)| (*hash, Arc::clone(&record.active)))
            .collect();
        Ok((
            HostAdmissionAdapter {
                gateway: gateway.clone(),
                credentials,
            },
            HostAdmissionControl { active },
        ))
    }
}

struct AdmissionRecord {
    principal: String,
    rel_grants: BTreeSet<OwnerId>,
    phy_grants: BTreeSet<OwnerId>,
    active: Arc<AtomicBool>,
}

/// Immutable request-time authentication against host-configured credentials.
pub struct HostAdmissionAdapter {
    gateway: Gateway,
    credentials: HashMap<[u8; 32], AdmissionRecord>,
}

/// Host-only revocation capability. Keep it out of request handlers.
pub struct HostAdmissionControl {
    active: HashMap<[u8; 32], Arc<AtomicBool>>,
}
impl HostAdmissionControl {
    /// Revoke all Instances previously admitted with this credential.
    pub fn revoke(&self, token: &str) -> Result<(), GatewayError> {
        let hash = *blake3::hash(token.as_bytes()).as_bytes();
        let active = self
            .active
            .get(&hash)
            .ok_or(GatewayError::InvalidCredential)?;
        active.store(false, Ordering::Release);
        Ok(())
    }
}

impl HostAdmissionAdapter {
    /// Connect an explicitly configured runtime host. Request Instances have no
    /// corresponding method, so adapters cannot self-register during a call.
    /// Negotiated guarantees remain declarations requiring host-side conformance.
    pub fn connect_runtime_host(&self, adapter: Arc<dyn TrustedHostAdapter>) -> CapabilityRouter {
        CapabilityRouter::connect(adapter, Arc::as_ptr(&self.gateway.owners) as usize)
    }

    /// Authenticate a bearer token and create an Instance with its configured grants.
    /// Unknown, malformed, or changed credentials all fail closed.
    pub fn authenticate(&self, token: &str) -> Result<Instance, GatewayError> {
        if !(MIN_CREDENTIAL_BYTES..=MAX_CREDENTIAL_BYTES).contains(&token.len())
            || token.chars().any(char::is_control)
        {
            return Err(GatewayError::InvalidCredential);
        }
        let hash = *blake3::hash(token.as_bytes()).as_bytes();
        let record = self
            .credentials
            .get(&hash)
            .ok_or(GatewayError::InvalidCredential)?;
        if !record.active.load(Ordering::Acquire) {
            return Err(GatewayError::CredentialRevoked);
        }
        self.gateway.instance_with_revocation(
            record.principal.clone(),
            record.rel_grants.clone(),
            record.phy_grants.clone(),
            Some(Arc::clone(&record.active)),
        )
    }
}

/// A principal-scoped view. Owner authorization is checked on every access.
pub struct Instance {
    owners: Arc<RwLock<HashMap<OwnerId, MountedOwner>>>,
    principal: Arc<str>,
    instance_id: Uuid,
    rel_grants: Arc<BTreeSet<OwnerId>>,
    phy_grants: Arc<BTreeSet<OwnerId>>,
    credential_active: Option<Arc<AtomicBool>>,
}
#[cfg(test)]
mod g4_completed_maintenance;
#[cfg(test)]
mod g4_export;
#[cfg(test)]
mod g4_gateway_acceptance;
#[cfg(test)]
mod g4_maintenance_observations;
#[cfg(test)]
mod g4_retrieval_acceptance;
#[cfg(test)]
mod g4_semantic_acceptance;
#[cfg(test)]
mod g4_semantic_completion;
#[cfg(test)]
mod g4_semantic_negatives;
#[cfg(test)]
mod g4_terminal_acceptance;
#[cfg(test)]
mod g4_test_cohort;
#[cfg(test)]
mod tests;

mod gateway_mounts;
mod insomnia_worker;
mod instance_context;
mod instance_conversation;
mod instance_dream;
mod owner_workers;
mod workflow_workers;
pub use instance_context::{
    AuthorizedContextCheckpoint, AuthorizedContextWorkflow, ContextMemoryOwnerKind,
    ContextMemoryRef, ProjectContextAttachmentRef,
};
mod instance_management;
mod instance_native_import;
mod instance_owner;
mod instance_passive_retrieval;
pub use instance_passive_retrieval::{SearchMode, SearchResult};
mod instance_processing;
mod live_processing;
mod processing_stages;
mod processing_vectors;
pub use instance_processing::{
    InsomniaRunOptions, ProcessingJob, ProcessingReport, ProcessingState,
};
pub use processing_vectors::VectorCoverage;
mod instance_project_correlation;
pub use instance_project_correlation::{AuthorizedProjectCorrelation, TrustedProjectSource};
mod instance_routing;
pub use insomnia_worker::{InsomniaWorkerConfig, InsomniaWorkerHandle};
pub use owner_workers::{OwnerWorkerConfig, OwnerWorkerCoordinator, OwnerWorkerKind};
pub use workflow_workers::{
    DreamWorker, EntityWorker, FreshnessWorker, OwnerWorkflowSet, VectorWorker,
};

mod host_memory_delivery;
mod instance_entity;
mod instance_perception_lifecycle;
mod instance_vectors;
pub use instance_vectors::PreparedVectorPublication;
mod instance_retrieval;

mod instance_external_source;

pub use host_memory_delivery::{
    MemoryDeliveryOperation, MemoryDeliveryReceipt, MemoryDeliveryStatus,
};
pub use instance_external_source::{AuthorizedExternalLexicalIndex, ExternalHostSource};
pub use instance_retrieval::{
    MemoryRetrievalRoute, MemoryRetrievalTurn, PreparedMemoryDelivery, RetrievalCut,
    RetrievalMemory, RetrievalMode, RetrievalOutcome, RetrievalPreview, RetrievalSource,
};
