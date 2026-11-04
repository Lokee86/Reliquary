//! Canonical read-only owner mounting and principal-scoped Instance access.
//!
//! Gateway owns the mounted-session registry. Instances carry immutable PHY
//! grants and never own or clone mutable domain stores.

use reliquary_application::{PreparedIngest, prepare_routed_user_memory};
use reliquary_owner_session::{
    AcceptedEpisodeOrder, CompletionCheck, GraphAdmission, InsomniaLeaseToken, OwnerSessionError,
    PhySession, PhyWriter, RelSession, RelWriter, RoutedUserMemoryProof,
    records::{DreamBeginAdmission, GroupedExternalMemoryRef, GroupedInsomniaCompletion},
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
pub use host_runtime::{
    HostDispatchOutcome, HostOperation, HostQueueGuarantees, HostReconcileStatus,
    TrustedHostAdapter,
};
pub use host_lifecycle::{CapabilityRouter, HostCapabilityContract, HostCapabilityState, HostConnection, HostExecutionMode};
use std::{
    collections::{BTreeSet, HashMap},
    fmt,
    path::Path,
    sync::{
        Arc, RwLock,
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
    IncompleteHostContract(RuntimeCapability),
    UnsupportedHostCapability(RuntimeCapability),
    RoutedMemory(String),
    RegistryPoisoned,
    WriterUnavailable,
}
impl fmt::Display for GatewayError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OwnerSession(e) => e.fmt(f),
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
            Self::IncompleteHostContract(capability) => {
                write!(f, "host queue contract is incomplete for {capability:?}")
            }
            Self::UnsupportedHostCapability(capability) => {
                write!(f, "host execution is not enabled for {capability:?}")
            }
            Self::RoutedMemory(reason) => write!(f, "routed user Memory failed: {reason}"),
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
enum RoutedSource {
    Snapshot(Arc<RelSession>),
    Writer(Arc<MountedRelWriter>),
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
}
impl Gateway {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn mount_reliquary(
        &self,
        path: impl AsRef<Path>,
        graph_admission: &GraphAdmission,
        accepted_episodes: impl IntoIterator<Item = AcceptedEpisodeOrder>,
        dream_admissions: impl IntoIterator<Item = DreamBeginAdmission>,
    ) -> Result<OwnerId, GatewayError> {
        let session = Arc::new(RelSession::open_path(
            path,
            graph_admission,
            accepted_episodes,
            dream_admissions,
        )?);
        let owner = OwnerId(session.owner_id());
        self.check_admission(owner, graph_admission)?;
        self.insert(owner, MountedOwner::Rel(session))?;
        Ok(owner)
    }

    /// Mount a REL with one owner-local writer lease. The mutable Container
    /// remains inside owner-session; request code receives only concrete ops.
    pub fn mount_writable_reliquary(
        &self,
        path: impl AsRef<Path>,
        graph_admission: &GraphAdmission,
        accepted_episodes: impl IntoIterator<Item = AcceptedEpisodeOrder>,
        dream_admissions: impl IntoIterator<Item = DreamBeginAdmission>,
    ) -> Result<OwnerId, GatewayError> {
        let writer = RelWriter::open_path(
            path,
            graph_admission.clone(),
            accepted_episodes,
            dream_admissions,
        )?;
        let owner = OwnerId(writer.owner_id());
        self.check_admission(owner, graph_admission)?;
        let mounted = MountedRelWriter {
            writer: std::sync::Mutex::new(writer),
        };
        self.insert(owner, MountedOwner::RelWriter(Arc::new(mounted)))?;
        Ok(owner)
    }

    pub fn mount_phylactery(
        &self,
        path: impl AsRef<Path>,
        graph_admission: &GraphAdmission,
        dream_admissions: impl IntoIterator<Item = DreamBeginAdmission>,
    ) -> Result<OwnerId, GatewayError> {
        let session = Arc::new(PhySession::open_path(
            path,
            graph_admission,
            dream_admissions,
        )?);
        let owner = OwnerId(session.owner_id());
        self.check_admission(owner, graph_admission)?;
        self.insert(owner, MountedOwner::Phy(session))?;
        Ok(owner)
    }

    /// Mount a PHY with one owner-local writer lease for authenticated routed
    /// user Memory. General mutation stays inside owner-session.
    pub fn mount_writable_phylactery(
        &self,
        path: impl AsRef<Path>,
        graph_admission: &GraphAdmission,
        dream_admissions: impl IntoIterator<Item = DreamBeginAdmission>,
    ) -> Result<OwnerId, GatewayError> {
        let writer = PhyWriter::open_path(path, graph_admission, dream_admissions)?;
        let owner = OwnerId(writer.owner_id());
        self.check_admission(owner, graph_admission)?;
        self.insert(
            owner,
            MountedOwner::PhyWriter(Arc::new(MountedPhyWriter {
                writer: std::sync::Mutex::new(writer),
            })),
        )?;
        Ok(owner)
    }

    /// Private construction path used only after trusted host authentication.
    #[allow(dead_code)]
    pub(crate) fn instance(
        &self,
        principal: impl Into<String>,
        rel_grants: impl IntoIterator<Item = OwnerId>,
        phy_grants: impl IntoIterator<Item = OwnerId>,
    ) -> Result<Instance, GatewayError> {
        let principal = principal.into();
        validate_principal(&principal)?;
        let rel_grants: BTreeSet<_> = rel_grants.into_iter().collect();
        let phy_grants: BTreeSet<_> = phy_grants.into_iter().collect();
        self.validate_grants(&rel_grants, &phy_grants)?;
        self.instance_with_revocation(principal, rel_grants, phy_grants, None)
    }

    fn instance_with_revocation(
        &self,
        principal: String,
        rel_grants: BTreeSet<OwnerId>,
        phy_grants: BTreeSet<OwnerId>,
        credential_active: Option<Arc<AtomicBool>>,
    ) -> Result<Instance, GatewayError> {
        Ok(Instance {
            owners: Arc::clone(&self.owners),
            principal: Arc::from(principal),
            instance_id: Uuid::new_v4(),
            rel_grants: Arc::new(rel_grants),
            phy_grants: Arc::new(phy_grants),
            credential_active,
        })
    }

    fn validate_grants(
        &self,
        rel_grants: &BTreeSet<OwnerId>,
        phy_grants: &BTreeSet<OwnerId>,
    ) -> Result<(), GatewayError> {
        let owners = self
            .owners
            .read()
            .map_err(|_| GatewayError::RegistryPoisoned)?;
        for owner in rel_grants {
            match owners.get(owner) {
                None => return Err(GatewayError::UnknownOwner(*owner)),
                Some(mounted) if !mounted.is_rel() => {
                    return Err(GatewayError::WrongOwnerKind(*owner));
                }
                Some(_) => {}
            }
        }
        for owner in phy_grants {
            match owners.get(owner) {
                None => return Err(GatewayError::UnknownOwner(*owner)),
                Some(mounted) if mounted.is_rel() => {
                    return Err(GatewayError::WrongOwnerKind(*owner));
                }
                Some(_) => {}
            }
        }
        Ok(())
    }

    pub fn mounted_owner_count(&self) -> Result<usize, GatewayError> {
        self.owners
            .read()
            .map(|owners| owners.len())
            .map_err(|_| GatewayError::RegistryPoisoned)
    }

    fn check_admission(
        &self,
        owner: OwnerId,
        admission: &GraphAdmission,
    ) -> Result<(), GatewayError> {
        let actual = admission.owner_id().map(OwnerId);
        if actual != Some(owner) {
            return Err(GatewayError::AdmissionOwnerMismatch {
                expected: owner,
                actual,
            });
        }
        Ok(())
    }
    fn insert(&self, owner: OwnerId, mounted: MountedOwner) -> Result<(), GatewayError> {
        let mut owners = self
            .owners
            .write()
            .map_err(|_| GatewayError::RegistryPoisoned)?;
        if owners.contains_key(&owner) {
            return Err(GatewayError::DuplicateOwner(owner));
        }
        owners.insert(owner, mounted);
        Ok(())
    }
}

fn validate_principal(principal: &str) -> Result<(), GatewayError> {
    if principal.trim().is_empty() || principal.chars().any(char::is_control) {
        Err(GatewayError::InvalidPrincipal)
    } else {
        Ok(())
    }
}

fn route_user_proofs(
    rel: &RelSession,
    destination: &PhyWriter,
    prepared: &PreparedIngest,
    principal: &str,
) -> Result<Vec<RoutedUserMemoryProof>, GatewayError> {
    if rel.owner_id() != prepared.owner_id() {
        return Err(GatewayError::RoutedMemory(
            "prepared ingest does not belong to the mounted REL".into(),
        ));
    }
    let episode = rel
        .episode(prepared.episode_id())
        .ok_or_else(|| GatewayError::RoutedMemory("prepared Episode is unavailable".into()))?;
    if episode.conversation_id != prepared.conversation_id() {
        return Err(GatewayError::RoutedMemory(
            "prepared Episode conversation differs from Archive".into(),
        ));
    }
    let mut proofs = Vec::with_capacity(prepared.user_memories().count());
    for candidate in prepared.user_memories() {
        let source_turn = rel
            .resolved_archive_turn(prepared.conversation_id(), &candidate.source_node_id)?
            .ok_or_else(|| {
                GatewayError::RoutedMemory("user Memory source turn is unavailable".into())
            })?;
        let (draft, _) = prepare_routed_user_memory(prepared, candidate, &source_turn, principal)
            .map_err(|error| GatewayError::RoutedMemory(error.to_string()))?;
        proofs.push(rel.prove_routed_user_memory(destination, &draft, &candidate.source_quote)?);
    }
    Ok(proofs)
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
    pub fn connect_runtime_host(
        &self,
        adapter: Arc<dyn TrustedHostAdapter>,
    ) -> CapabilityRouter {
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
impl Instance {
    pub(crate) fn gateway_binding(&self) -> usize {
        Arc::as_ptr(&self.owners) as usize
    }

    pub fn principal(&self) -> &str {
        &self.principal
    }

    /// Attempt routed publication through this Instance's exact REL and PHY
    /// grants. The owner-session boundary currently rejects publication until
    /// semantic candidate evidence can be sealed across the application API.
    pub fn publish_user_owned_ingest(
        &self,
        source_rel_owner: OwnerId,
        destination_phy_owner: OwnerId,
        prepared: &PreparedIngest,
    ) -> Result<Vec<GroupedExternalMemoryRef>, GatewayError> {
        self.authorize_routed_owners(source_rel_owner, destination_phy_owner)?;
        if prepared.owner_id() != source_rel_owner.as_bytes() {
            return Err(GatewayError::RoutedMemory(
                "prepared ingest source owner differs from its REL grant".into(),
            ));
        }
        let (source, destination) = {
            let owners = self
                .owners
                .read()
                .map_err(|_| GatewayError::RegistryPoisoned)?;
            let source = match owners.get(&source_rel_owner) {
                Some(MountedOwner::Rel(session)) => RoutedSource::Snapshot(Arc::clone(session)),
                Some(MountedOwner::RelWriter(writer)) => RoutedSource::Writer(Arc::clone(writer)),
                Some(_) => return Err(GatewayError::WrongOwnerKind(source_rel_owner)),
                None => return Err(GatewayError::UnknownOwner(source_rel_owner)),
            };
            let destination = match owners.get(&destination_phy_owner) {
                Some(MountedOwner::PhyWriter(writer)) => Arc::clone(writer),
                Some(MountedOwner::Phy(_)) => return Err(GatewayError::WriterUnavailable),
                Some(_) => return Err(GatewayError::WrongOwnerKind(destination_phy_owner)),
                None => return Err(GatewayError::UnknownOwner(destination_phy_owner)),
            };
            (source, destination)
        };

        let mut destination = destination
            .writer
            .lock()
            .map_err(|_| GatewayError::RegistryPoisoned)?;
        let proofs = match source {
            RoutedSource::Snapshot(session) => {
                route_user_proofs(&session, &destination, prepared, &self.principal)?
            }
            RoutedSource::Writer(writer) => {
                let writer = writer
                    .writer
                    .lock()
                    .map_err(|_| GatewayError::RegistryPoisoned)?;
                route_user_proofs(
                    writer.read_session(),
                    &destination,
                    prepared,
                    &self.principal,
                )?
            }
        };
        self.authorize_routed_owners(source_rel_owner, destination_phy_owner)?;
        let destination_id = hex(&destination_phy_owner.0);
        let mut refs = Vec::with_capacity(proofs.len());
        let mut seen = BTreeSet::new();
        for proof in proofs {
            self.authorize_routed_owners(source_rel_owner, destination_phy_owner)?;
            let revision = destination.publish_routed_user_memory(proof)?;
            let id = revision.record.id.0;
            if !seen.insert(id) {
                return Err(GatewayError::RoutedMemory(
                    "duplicate routed Memory identity".into(),
                ));
            }
            let source = revision.record.source_ref.as_ref().ok_or_else(|| {
                GatewayError::RoutedMemory("PHY revision lost source proof".into())
            })?;
            if source.owner_id != hex(&source_rel_owner.0)
                || source.principal_id.as_deref() != Some(self.principal.as_ref())
            {
                return Err(GatewayError::RoutedMemory(
                    "published PHY revision differs from the active source grant".into(),
                ));
            }
            refs.push(GroupedExternalMemoryRef {
                owner_id: destination_id.clone(),
                memory_id: id,
            });
        }
        Ok(refs)
    }

    fn authorize_routed_owners(
        &self,
        source_rel_owner: OwnerId,
        destination_phy_owner: OwnerId,
    ) -> Result<(), GatewayError> {
        self.ensure_credential_active()?;
        if !self.rel_grants.contains(&source_rel_owner) {
            return Err(GatewayError::UnauthorizedRel {
                principal: self.principal.to_string(),
                owner: source_rel_owner,
            });
        }
        if !self.phy_grants.contains(&destination_phy_owner) {
            return Err(GatewayError::UnauthorizedPhy {
                principal: self.principal.to_string(),
                owner: destination_phy_owner,
            });
        }
        Ok(())
    }

    /// Admit a provisional user submission using only this Instance's verified
    /// principal and exact REL grant. Queue contents never enter Archive here.
    pub fn submit_conversation(
        &self,
        queue: &mut ConversationQueue,
        conversation_id: impl Into<String>,
        submission_id: SubmissionId,
        content: impl Into<String>,
        expected_revision: u64,
    ) -> Result<EnqueueOutcome, GatewayError> {
        self.authorize_queue_owner(queue.owner())?;
        let id = ConversationSubmissionId::new(conversation_id, submission_id)?;
        let submission = QueuedSubmission::new(id, self.principal.to_string(), content)?;
        Ok(queue.enqueue(submission, expected_revision)?)
    }

    /// Win the finalize race, sync the accepted turn and branch through the
    /// mounted REL Archive, then record the receipt in the durable queue.
    pub fn finalize_conversation_submission(
        &self,
        queue: &mut ConversationQueue,
        id: &ConversationSubmissionId,
        expected_revision: u64,
        capture: FinalizeCapture,
    ) -> Result<SubmissionStatus, GatewayError> {
        self.authorize_queue_owner(queue.owner())?;
        let submitter = queue
            .submitter_principal(id)
            .ok_or(conversation_queue::QueueError::MissingSubmission)?;
        if submitter != self.principal.as_ref() {
            return Err(GatewayError::UnauthorizedQueueSubmission {
                principal: self.principal.to_string(),
                owner: queue.owner(),
            });
        }
        if let Some(status) = queue.status(id)
            && status.state == QueueState::Committed
        {
            if status.finalize_capture.as_ref() == Some(&capture) {
                return Ok(status);
            }
            return Err(conversation_queue::QueueError::ArchiveIdentityConflict.into());
        }
        let submission = queue
            .submission(id)
            .cloned()
            .ok_or(conversation_queue::QueueError::MissingSubmission)?;
        let mounted = {
            let owners = self
                .owners
                .read()
                .map_err(|_| GatewayError::RegistryPoisoned)?;
            match owners.get(&queue.owner()) {
                None => return Err(GatewayError::UnknownOwner(queue.owner())),
                Some(MountedOwner::RelWriter(writer)) => Arc::clone(writer),
                Some(_) => return Err(GatewayError::WriterUnavailable),
            }
        };
        let turn = reliquary_archive::TurnInput {
            id: submission.id.submission_id.as_str().to_owned(),
            conversation_id: submission.id.conversation_id.clone(),
            parent_id: capture.parent_node_id.clone(),
            role: "user".to_owned(),
            principal_id: Some(submission.principal_id.clone()),
            timestamp_ns: capture.committed_timestamp_ns,
            content: submission.content,
            attachments: Vec::new(),
            project_attachments: Vec::new(),
        };
        let branch = reliquary_archive::Branch {
            id: capture.branch_id.clone(),
            conversation_id: id.conversation_id.clone(),
            leaf_node_id: id.submission_id.as_str().to_owned(),
            canonical: true,
        };
        let (finalizing, node) = {
            let mut writer = mounted
                .writer
                .lock()
                .map_err(|_| GatewayError::RegistryPoisoned)?;
            self.ensure_credential_active()?;
            writer.validate_turn_branch(&turn, &branch)?;
            let finalizing = queue.claim_finalize(id, expected_revision, capture.clone())?;
            let node = writer.accept_turn_and_advance_branch(turn, branch)?;
            (finalizing, node)
        };

        let receipt = ArchiveReceipt {
            node_id: node.id,
            branch_id: capture.branch_id,
            parent_node_id: node.parent_id,
            principal_id: node
                .principal_id
                .ok_or(conversation_queue::QueueError::ArchiveIdentityConflict)?,
            committed_timestamp_ns: node.timestamp_ns,
            payload_hash: submission.payload_hash,
        };
        if finalizing.state != QueueState::Finalizing {
            return Err(conversation_queue::QueueError::InvalidState.into());
        }
        queue.record_archive_commit(id, ArchiveCommitProof::from_archive(receipt))?;
        Ok(queue.status(id).expect("committed entry remains projected"))
    }

    /// Open the queue for a mounted REL only after checking this Instance's grant.
    pub(crate) fn open_conversation_queue(
        &self,
        state_dir: impl AsRef<Path>,
        owner: OwnerId,
    ) -> Result<ConversationQueue, GatewayError> {
        self.authorize_queue_owner(owner)?;
        {
            let owners = self
                .owners
                .read()
                .map_err(|_| GatewayError::RegistryPoisoned)?;
            match owners.get(&owner) {
                Some(mounted) if mounted.is_rel() => {}
                Some(_) => return Err(GatewayError::WrongOwnerKind(owner)),
                None => return Err(GatewayError::UnknownOwner(owner)),
            }
        }
        self.ensure_credential_active()?;
        Ok(ConversationQueue::open(state_dir, owner)?)
    }

    /// Read one operation status through the same owner authorization check used
    /// for submission and cancellation.
    pub fn conversation_submission_status(
        &self,
        queue: &ConversationQueue,
        id: &ConversationSubmissionId,
    ) -> Result<Option<SubmissionStatus>, GatewayError> {
        self.authorize_queue_owner(queue.owner())?;
        Ok(queue.status(id))
    }

    pub fn host_operation_status(
        &self,
        queue: &ConversationQueue,
        id: &ConversationSubmissionId,
        capability: RuntimeCapability,
    ) -> Result<Option<DurableHostOperation>, GatewayError> {
        self.authorize_queue_owner(queue.owner())?;
        Ok(queue.host_operation(id, capability))
    }

    /// Read the latest full text prefix only after checking the current REL grant.
    pub fn generation_checkpoint_snapshot(
        &self,
        queue: &ConversationQueue,
        id: &ConversationSubmissionId,
    ) -> Result<Option<DurableGenerationCheckpoint>, GatewayError> {
        self.authorize_queue_owner(queue.owner())?;
        Ok(queue.generation_checkpoint(id))
    }

    /// Replay bounded, already-synced full-prefix checkpoints for this REL.
    pub fn replay_generation_checkpoints(
        &self,
        queue: &ConversationQueue,
        id: &ConversationSubmissionId,
        after_sequence: u64,
        max_items: usize,
    ) -> Result<GenerationReplay, GatewayError> {
        self.authorize_queue_owner(queue.owner())?;
        Ok(queue.replay_generation_checkpoints(id, after_sequence, max_items)?)
    }

    /// Cancel only a pending submission originally admitted under this principal.
    pub fn cancel_own_pending_submission(
        &self,
        queue: &mut ConversationQueue,
        id: &ConversationSubmissionId,
        expected_revision: u64,
    ) -> Result<SubmissionStatus, GatewayError> {
        self.authorize_queue_owner(queue.owner())?;
        let submitter = queue
            .submitter_principal(id)
            .ok_or(conversation_queue::QueueError::MissingSubmission)?;
        if submitter != self.principal.as_ref() {
            return Err(GatewayError::UnauthorizedQueueSubmission {
                principal: self.principal.to_string(),
                owner: queue.owner(),
            });
        }
        Ok(queue.cancel(id, expected_revision)?)
    }

    fn authorize_queue_owner(&self, owner: OwnerId) -> Result<(), GatewayError> {
        self.ensure_credential_active()?;
        if !self.rel_grants.contains(&owner) {
            return Err(GatewayError::UnauthorizedRel {
                principal: self.principal.to_string(),
                owner,
            });
        }
        Ok(())
    }
    pub fn reliquary(&self, owner: OwnerId) -> Result<Arc<RelSession>, GatewayError> {
        self.ensure_credential_active()?;
        if !self.rel_grants.contains(&owner) {
            return Err(GatewayError::UnauthorizedRel {
                principal: self.principal.to_string(),
                owner,
            });
        }
        let owners = self
            .owners
            .read()
            .map_err(|_| GatewayError::RegistryPoisoned)?;
        match owners.get(&owner) {
            None => Err(GatewayError::UnknownOwner(owner)),
            Some(MountedOwner::Rel(session)) => Ok(Arc::clone(session)),
            Some(MountedOwner::RelWriter(_)) => Err(GatewayError::WriterUnavailable),
            Some(MountedOwner::Phy(_)) => Err(GatewayError::WrongOwnerKind(owner)),
            Some(MountedOwner::PhyWriter(_)) => Err(GatewayError::WrongOwnerKind(owner)),
        }
    }

    /// Execute bounded owner-local read work against a writable mount's current
    /// projection. Do not call providers, network APIs, or external callbacks here.
    pub(crate) fn with_writable_reliquary<T>(
        &self,
        owner: OwnerId,
        read: impl FnOnce(&RelSession) -> T,
    ) -> Result<T, GatewayError> {
        self.ensure_credential_active()?;
        if !self.rel_grants.contains(&owner) {
            return Err(GatewayError::UnauthorizedRel {
                principal: self.principal.to_string(),
                owner,
            });
        }
        let mounted = {
            let owners = self
                .owners
                .read()
                .map_err(|_| GatewayError::RegistryPoisoned)?;
            match owners.get(&owner) {
                None => return Err(GatewayError::UnknownOwner(owner)),
                Some(MountedOwner::RelWriter(writer)) => Arc::clone(writer),
                Some(MountedOwner::Rel(_)) => return Err(GatewayError::WriterUnavailable),
                Some(MountedOwner::Phy(_)) => return Err(GatewayError::WrongOwnerKind(owner)),
                Some(MountedOwner::PhyWriter(_)) => {
                    return Err(GatewayError::WrongOwnerKind(owner));
                }
            }
        };
        let writer = mounted
            .writer
            .lock()
            .map_err(|_| GatewayError::RegistryPoisoned)?;
        self.ensure_credential_active()?;
        Ok(read(writer.read_session()))
    }

    pub fn commit_grouped_insomnia_completion(
        &self,
        owner: OwnerId,
        token: InsomniaLeaseToken,
        completion: GroupedInsomniaCompletion,
        now_ns: i64,
    ) -> Result<CompletionCheck, GatewayError> {
        self.ensure_credential_active()?;
        if !self.rel_grants.contains(&owner) {
            return Err(GatewayError::UnauthorizedRel {
                principal: self.principal.to_string(),
                owner,
            });
        }
        let mounted = {
            let owners = self
                .owners
                .read()
                .map_err(|_| GatewayError::RegistryPoisoned)?;
            match owners.get(&owner) {
                None => return Err(GatewayError::UnknownOwner(owner)),
                Some(MountedOwner::RelWriter(writer)) => Arc::clone(writer),
                Some(MountedOwner::Rel(_)) => return Err(GatewayError::WriterUnavailable),
                Some(MountedOwner::Phy(_)) => return Err(GatewayError::WrongOwnerKind(owner)),
                Some(MountedOwner::PhyWriter(_)) => {
                    return Err(GatewayError::WrongOwnerKind(owner));
                }
            }
        };
        let mut writer = mounted
            .writer
            .lock()
            .map_err(|_| GatewayError::RegistryPoisoned)?;
        self.ensure_credential_active()?;
        Ok(writer.commit_grouped_insomnia_completion(token, completion, now_ns)?)
    }
    pub fn phylactery(&self, owner: OwnerId) -> Result<Arc<PhySession>, GatewayError> {
        self.ensure_credential_active()?;
        if !self.phy_grants.contains(&owner) {
            return Err(GatewayError::UnauthorizedPhy {
                principal: self.principal.to_string(),
                owner,
            });
        }
        let owners = self
            .owners
            .read()
            .map_err(|_| GatewayError::RegistryPoisoned)?;
        match owners.get(&owner) {
            None => Err(GatewayError::UnknownOwner(owner)),
            Some(MountedOwner::Phy(session)) => Ok(Arc::clone(session)),
            Some(MountedOwner::Rel(_)) => Err(GatewayError::WrongOwnerKind(owner)),
            Some(MountedOwner::RelWriter(_)) => Err(GatewayError::WrongOwnerKind(owner)),
            Some(MountedOwner::PhyWriter(_)) => Err(GatewayError::WriterUnavailable),
        }
    }

    fn ensure_credential_active(&self) -> Result<(), GatewayError> {
        if self
            .credential_active
            .as_ref()
            .is_some_and(|active| !active.load(Ordering::Acquire))
        {
            Err(GatewayError::CredentialRevoked)
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use reliquary_container::{Container, ContainerIdentity, ReliquaryScope};
    use reliquary_vector_store::{
        ArchiveVectorStore, CompatibilityProfileStore, MemoryVectorStore, PackedVectorStore,
        VectorGenerationStore,
    };

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

    #[test]
    fn writable_mount_queue_finalization_and_reopen_retry_are_end_to_end() {
        let root = std::env::current_dir()
            .unwrap()
            .join(".test-tmp")
            .join(format!(
                "gateway-finalize-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
        std::fs::create_dir_all(&root).unwrap();
        let rel_path = root.join("owner.rel");
        let queue_path = root.join("queue");
        let raw_owner = [61; 16];
        initialize_rel(&rel_path, raw_owner);

        let gateway = Gateway::new();
        let admission = GraphAdmission::for_replay(raw_owner, [], [], [], [], []);
        let owner = gateway
            .mount_writable_reliquary(&rel_path, &admission, [], [])
            .unwrap();
        let instance = gateway.instance("alice", [owner], []).unwrap();
        let id = ConversationSubmissionId::new(
            "conversation-1",
            SubmissionId::new("submission-1").unwrap(),
        )
        .unwrap();
        let capture = FinalizeCapture {
            branch_id: "main".to_owned(),
            parent_node_id: None,
            committed_timestamp_ns: 42,
            verified_principal_id: "alice".to_owned(),
        };

        let mut queue = instance
            .open_conversation_queue(&queue_path, owner)
            .unwrap();
        instance
            .submit_conversation(
                &mut queue,
                "conversation-1",
                id.submission_id.clone(),
                "durable conversation text",
                0,
            )
            .unwrap();
        let other_principal = gateway.instance("bob", [owner], []).unwrap();
        assert!(matches!(
            other_principal.finalize_conversation_submission(&mut queue, &id, 1, capture.clone(),),
            Err(GatewayError::UnauthorizedQueueSubmission { .. })
        ));
        assert_eq!(
            instance
                .conversation_submission_status(&queue, &id)
                .unwrap()
                .unwrap()
                .state,
            QueueState::Pending
        );
        let committed = instance
            .finalize_conversation_submission(&mut queue, &id, 1, capture.clone())
            .unwrap();
        assert_eq!(committed.state, QueueState::Committed);
        assert_eq!(
            committed.archive_receipt.as_ref().unwrap().principal_id,
            "alice"
        );
        assert!(matches!(
            other_principal.finalize_conversation_submission(&mut queue, &id, 1, capture.clone(),),
            Err(GatewayError::UnauthorizedQueueSubmission { .. })
        ));
        assert!(
            instance
                .with_writable_reliquary(owner, |rel| {
                    rel.archive_node("conversation-1", "submission-1").is_some()
                })
                .unwrap()
        );

        let followup_id = ConversationSubmissionId::new(
            "conversation-1",
            SubmissionId::new("submission-2").unwrap(),
        )
        .unwrap();
        instance
            .submit_conversation(
                &mut queue,
                "conversation-1",
                followup_id.submission_id.clone(),
                "forked follow-up",
                3,
            )
            .unwrap();
        let stale_parent = FinalizeCapture {
            branch_id: "main".to_owned(),
            parent_node_id: None,
            committed_timestamp_ns: 43,
            verified_principal_id: "alice".to_owned(),
        };
        assert!(matches!(
            instance.finalize_conversation_submission(&mut queue, &followup_id, 4, stale_parent,),
            Err(GatewayError::OwnerSession(_))
        ));
        assert_eq!(
            instance
                .conversation_submission_status(&queue, &followup_id)
                .unwrap()
                .unwrap()
                .state,
            QueueState::Pending
        );
        assert!(
            !instance
                .with_writable_reliquary(owner, |rel| {
                    rel.archive_node("conversation-1", "submission-2").is_some()
                })
                .unwrap()
        );
        let fork_capture = FinalizeCapture {
            branch_id: "fork".to_owned(),
            parent_node_id: Some("submission-1".to_owned()),
            committed_timestamp_ns: 43,
            verified_principal_id: "alice".to_owned(),
        };
        let forked = instance
            .finalize_conversation_submission(&mut queue, &followup_id, 4, fork_capture.clone())
            .unwrap();
        assert_eq!(forked.state, QueueState::Committed);

        let outsider = gateway.instance("bob", [], []).unwrap();
        assert!(matches!(
            outsider.open_conversation_queue(&queue_path, owner),
            Err(GatewayError::UnauthorizedRel { .. })
        ));

        drop(queue);
        let mut recovered = instance
            .open_conversation_queue(&queue_path, owner)
            .unwrap();
        assert_eq!(
            instance
                .finalize_conversation_submission(&mut recovered, &followup_id, 4, fork_capture)
                .unwrap(),
            forked
        );
        let (accepted_turn_count, content) = instance
            .with_writable_reliquary(owner, |rel| {
                (
                    rel.accepted_turn_sources().len(),
                    rel.resolved_archive_turn("conversation-1", "submission-2")
                        .unwrap()
                        .unwrap()
                        .content,
                )
            })
            .unwrap();
        assert_eq!(accepted_turn_count, 2);
        assert_eq!(content, "forked follow-up");
        drop(recovered);
        drop(instance);
        drop(gateway);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn archive_commit_without_queue_receipt_reconciles_after_branch_advances() {
        let root = std::env::current_dir()
            .unwrap()
            .join(".test-tmp")
            .join(format!(
                "gateway-archive-reconcile-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
        std::fs::create_dir_all(&root).unwrap();
        let rel_path = root.join("owner.rel");
        let queue_path = root.join("queue");
        let raw_owner = [62; 16];
        initialize_rel(&rel_path, raw_owner);

        let gateway = Gateway::new();
        let admission = GraphAdmission::for_replay(raw_owner, [], [], [], [], []);
        let owner = gateway
            .mount_writable_reliquary(&rel_path, &admission, [], [])
            .unwrap();
        let instance = gateway.instance("alice", [owner], []).unwrap();
        let first_id = ConversationSubmissionId::new(
            "conversation-1",
            SubmissionId::new("submission-1").unwrap(),
        )
        .unwrap();
        let second_id = ConversationSubmissionId::new(
            "conversation-1",
            SubmissionId::new("submission-2").unwrap(),
        )
        .unwrap();
        let first_capture = FinalizeCapture {
            branch_id: "main".to_owned(),
            parent_node_id: None,
            committed_timestamp_ns: 43,
            verified_principal_id: "alice".to_owned(),
        };
        let second_capture = FinalizeCapture {
            branch_id: "main".to_owned(),
            parent_node_id: Some("submission-1".to_owned()),
            committed_timestamp_ns: 44,
            verified_principal_id: "alice".to_owned(),
        };
        let mut queue = instance
            .open_conversation_queue(&queue_path, owner)
            .unwrap();
        instance
            .submit_conversation(
                &mut queue,
                "conversation-1",
                first_id.submission_id.clone(),
                "first turn",
                0,
            )
            .unwrap();
        queue
            .claim_finalize(&first_id, 1, first_capture.clone())
            .unwrap();

        // Simulate process loss after both Archive frames sync but before the
        // queue's ArchiveCommitted event is appended.
        {
            let owners = instance.owners.read().unwrap();
            let Some(MountedOwner::RelWriter(mounted)) = owners.get(&owner) else {
                panic!("writable REL mount disappeared");
            };
            mounted
                .writer
                .lock()
                .unwrap()
                .accept_turn_and_advance_branch(
                    reliquary_archive::TurnInput {
                        id: "submission-1".to_owned(),
                        conversation_id: "conversation-1".to_owned(),
                        parent_id: None,
                        role: "user".to_owned(),
                        principal_id: Some("alice".to_owned()),
                        timestamp_ns: 43,
                        content: "first turn".to_owned(),
                        attachments: Vec::new(),
                        project_attachments: Vec::new(),
                    },
                    reliquary_archive::Branch {
                        id: "main".to_owned(),
                        conversation_id: "conversation-1".to_owned(),
                        leaf_node_id: "submission-1".to_owned(),
                        canonical: true,
                    },
                )
                .unwrap();
        }
        assert!(queue.status(&first_id).unwrap().archive_receipt.is_none());
        instance
            .submit_conversation(
                &mut queue,
                "conversation-1",
                second_id.submission_id.clone(),
                "second turn",
                2,
            )
            .unwrap();
        instance
            .finalize_conversation_submission(&mut queue, &second_id, 3, second_capture)
            .unwrap();
        drop(queue);

        let mut recovered = instance
            .open_conversation_queue(&queue_path, owner)
            .unwrap();
        let first_status = instance
            .finalize_conversation_submission(&mut recovered, &first_id, 1, first_capture)
            .unwrap();
        assert_eq!(first_status.state, QueueState::Committed);
        assert!(first_status.archive_receipt.is_some());
        let accepted_count = instance
            .with_writable_reliquary(owner, |rel| rel.accepted_turn_sources().len())
            .unwrap();
        assert_eq!(accepted_count, 2);

        drop(recovered);
        drop(instance);
        drop(gateway);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn ungranted_owner_access_is_denied_before_lookup() {
        let gateway = Gateway::new();
        let owner = OwnerId::from_bytes([41; 16]);
        let instance = gateway.instance("alice", [], []).unwrap();
        assert!(matches!(
            instance.reliquary(owner),
            Err(GatewayError::UnauthorizedRel { .. })
        ));
        assert!(matches!(
            instance.phylactery(owner),
            Err(GatewayError::UnauthorizedPhy { .. })
        ));
    }

    #[test]
    fn grants_cannot_name_unmounted_owners() {
        let gateway = Gateway::new();
        let rel = OwnerId::from_bytes([42; 16]);
        let phy = OwnerId::from_bytes([43; 16]);
        assert!(
            matches!(gateway.instance("alice", [rel], []), Err(GatewayError::UnknownOwner(id)) if id == rel)
        );
        assert!(
            matches!(gateway.instance("alice", [], [phy]), Err(GatewayError::UnknownOwner(id)) if id == phy)
        );
    }

    #[test]
    fn blank_or_control_character_principals_are_rejected() {
        let gateway = Gateway::new();
        assert!(matches!(
            gateway.instance("  ", [], []),
            Err(GatewayError::InvalidPrincipal)
        ));
        assert!(matches!(
            gateway.instance("alice\nadmin", [], []),
            Err(GatewayError::InvalidPrincipal)
        ));
    }

    #[test]
    fn host_admission_binds_bearer_credential_to_configured_principal() {
        let gateway = Gateway::new();
        let token = "correct horse battery staple with enough entropy";
        let host = TrustedHostAdmission::new([HostCredentialBinding::new("alice", token, [], [])]);
        let (adapter, _control) = host.install(&gateway).unwrap();

        assert_eq!(adapter.authenticate(token).unwrap().principal(), "alice");
        assert!(matches!(
            adapter.authenticate("attacker supplied a different sufficiently long token"),
            Err(GatewayError::InvalidCredential)
        ));
    }

    #[test]
    fn host_admission_rejects_bad_credentials_and_duplicate_tokens() {
        let gateway = Gateway::new();
        assert!(matches!(
            TrustedHostAdmission::new([HostCredentialBinding::new("alice", "short", [], [])])
                .install(&gateway),
            Err(GatewayError::InvalidCredential)
        ));
        let token = "shared credential with at least thirty two bytes";
        assert!(matches!(
            TrustedHostAdmission::new([
                HostCredentialBinding::new("alice", token, [], []),
                HostCredentialBinding::new("bob", token, [], []),
            ])
            .install(&gateway),
            Err(GatewayError::DuplicateCredential)
        ));
    }

    #[test]
    fn host_revocation_invalidates_existing_instances_and_future_admission() {
        let gateway = Gateway::new();
        let token = "revocable host credential with enough bytes";
        let host = TrustedHostAdmission::new([HostCredentialBinding::new("alice", token, [], [])]);
        let (adapter, control) = host.install(&gateway).unwrap();
        let instance = adapter.authenticate(token).unwrap();

        control.revoke(token).unwrap();
        assert!(matches!(
            instance.reliquary(OwnerId::from_bytes([41; 16])),
            Err(GatewayError::CredentialRevoked)
        ));
        assert!(matches!(
            adapter.authenticate(token),
            Err(GatewayError::CredentialRevoked)
        ));
    }

    #[test]
    fn ungranted_instance_cannot_submit_to_an_owner_queue() {
        let dir = std::env::current_dir()
            .unwrap()
            .join(".test-tmp")
            .join(format!(
                "gateway-denied-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
        std::fs::create_dir_all(&dir).unwrap();
        let gateway = Gateway::new();
        let owner = OwnerId::from_bytes([23; 16]);
        let instance = gateway.instance("alice", [], []).unwrap();
        let mut queue = ConversationQueue::open(&dir, owner).unwrap();
        let result = instance.submit_conversation(
            &mut queue,
            "conversation",
            SubmissionId::new("submission").unwrap(),
            "content",
            0,
        );
        assert!(matches!(result, Err(GatewayError::UnauthorizedRel { .. })));
        assert_eq!(queue.sequence(), 0);
        drop(queue);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn ungranted_instance_cannot_read_generation_checkpoint_or_replay() {
        let dir = std::env::current_dir()
            .unwrap()
            .join(".test-tmp")
            .join(format!(
                "gateway-stream-denied-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
        std::fs::create_dir_all(&dir).unwrap();
        let gateway = Gateway::new();
        let owner = OwnerId::from_bytes([28; 16]);
        let instance = gateway.instance("alice", [], []).unwrap();
        let queue = ConversationQueue::open(&dir, owner).unwrap();
        let id =
            ConversationSubmissionId::new("conversation", SubmissionId::new("submission").unwrap())
                .unwrap();
        assert!(matches!(
            instance.generation_checkpoint_snapshot(&queue, &id),
            Err(GatewayError::UnauthorizedRel { .. })
        ));
        assert!(matches!(
            instance.replay_generation_checkpoints(&queue, &id, 0, 1),
            Err(GatewayError::UnauthorizedRel { .. })
        ));
        drop(queue);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
