//! Canonical read-only owner mounting and principal-scoped Instance access.
//!
//! Gateway owns the mounted-session registry. Instances carry immutable PHY
//! grants and never own or clone mutable domain stores.

use reliquary_owner_session::{
    AcceptedEpisodeOrder, CompletionCheck, GraphAdmission, InsomniaLeaseToken, OwnerSessionError,
    PhySession, RelSession, RelWriter,
    records::{DreamBeginAdmission, GroupedInsomniaCompletion},
};

mod conversation_queue;
pub use conversation_queue::{
    ArchiveReceipt, ConversationQueue, ConversationSubmissionId, DurableGeneration,
    DurableGenerationCheckpoint, DurableGenerationPhase, DurableHostOperation,
    DurableHostOperationPhase, EnqueueOutcome, FinalizeCapture, GenerationReplay, HandoffReceipt,
    HostCancelOutcome, HostOperationStatus, QueueError, QueueState, QueuedSubmission,
    RuntimeCapability, SubmissionId, SubmissionStatus,
};
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
}
struct MountedRelWriter {
    writer: std::sync::Mutex<RelWriter>,
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
    rel_grants: Arc<BTreeSet<OwnerId>>,
    phy_grants: Arc<BTreeSet<OwnerId>>,
    credential_active: Option<Arc<AtomicBool>>,
}
impl Instance {
    pub fn principal(&self) -> &str {
        &self.principal
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

    /// Open the queue for a mounted REL only after checking this Instance's grant.
    pub fn open_conversation_queue(
        &self,
        state_dir: impl AsRef<Path>,
        owner: OwnerId,
    ) -> Result<ConversationQueue, GatewayError> {
        self.authorize_queue_owner(owner)?;
        self.reliquary(owner)?;
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
