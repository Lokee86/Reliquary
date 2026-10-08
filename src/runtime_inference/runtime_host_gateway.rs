use super::instances::{InstanceAuthorization, InstanceId, InstanceRuntime, OwnerGrant};
use super::{ReliquaryRuntimeHostError, ReliquaryRuntimeRoutes, operation};
#[path = "runtime_host_rel_execution.rs"]
mod rel_runtime;
use crate::{EpisodePolicy, InsomniaWorkerConfig, InteractionRuntime, Phylactery};
use rel_runtime::RelRuntime;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, RwLock};
use uuid::Uuid;

#[path = "workspace_management.rs"]
mod management;

#[path = "interaction_live.rs"]
mod live;

#[path = "runtime_host_gateway_admission.rs"]
mod admission;
#[path = "runtime_host_owner_operation_journal.rs"]
pub(super) mod owner_operations;
#[path = "runtime_host_phy_execution.rs"]
mod phy_runtime;
#[path = "runtime_host_owner_publication.rs"]
mod publication;
#[path = "runtime_host_owner_publication_codec.rs"]
mod publication_codec;
use phy_runtime::PhyRuntime;
#[path = "runtime_host_gateway_ambient.rs"]
mod ambient;
#[path = "runtime_host_gateway_mount.rs"]
pub(super) mod mount;
#[path = "runtime_host_gateway_phy_lifecycle.rs"]
mod phy_lifecycle;
#[path = "runtime_host_gateway_project_lifecycle.rs"]
mod project_lifecycle;
#[path = "runtime_host_gateway_semantic.rs"]
mod semantic;
pub use semantic::RuntimeKnowledgeProvenanceResolution;
#[path = "runtime_host_gateway_semantic_search.rs"]
mod semantic_search;
pub use admission::{GatewayAdmissionAdapter, VerifiedInstanceAdmission};
use mount::{MountClaim, MountedFileIdentity};
pub use semantic_search::{VisibleArchiveSearchOwner, VisibleArchiveSearchResult};
#[path = "runtime_host_gateway_host.rs"]
mod host;
pub use host::{
    CapabilityRouter, CapabilityState, GatewayHostAdapter, HandoffReceipt, HostCancelOutcome,
    HostCapabilityContract, HostConnection, HostContractError, HostOperation, HostOperationStatus,
    HostQueueGuarantees, HostReconcileStatus, RuntimeCapability, StageAuthority,
};
#[path = "runtime_host_gateway_convergence.rs"]
mod convergence;
#[cfg(test)]
#[path = "runtime_host_gateway_convergence_race_tests.rs"]
mod convergence_race_tests;
#[cfg(test)]
#[path = "runtime_host_gateway_convergence_tests.rs"]
mod convergence_tests;
#[path = "runtime_host_gateway_conversation.rs"]
pub(super) mod conversation;
#[path = "runtime_host_gateway_conversation_metadata.rs"]
mod conversation_metadata;
#[path = "runtime_host_gateway_conversation_selection.rs"]
mod conversation_selection;
#[path = "runtime_host_gateway_events.rs"]
pub(super) mod events;
#[path = "runtime_host_gateway_mutation_events.rs"]
mod mutation_events;
#[path = "runtime_host_gateway_observation.rs"]
pub(super) mod observation;
#[path = "runtime_host_gateway_reconnect.rs"]
mod reconnect;
#[path = "runtime_host_gateway_snapshots.rs"]
mod snapshots;
#[path = "runtime_host_gateway_streams.rs"]
pub(super) mod streams;
#[path = "runtime_host_gateway_subscriptions.rs"]
mod subscriptions;

/// Stable header identity used as the key for one gateway-owned REL or PHY.
pub type DurableOwnerId = String;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OwnerAdmission {
    Open,
    Closed,
}

#[derive(Debug)]
struct LeaseState {
    admission: OwnerAdmission,
    generation: u64,
    active: usize,
}

#[derive(Debug)]
struct LeaseGate {
    state: Mutex<LeaseState>,
    drained: Condvar,
}

impl LeaseGate {
    fn new(generation: u64) -> Self {
        Self {
            state: Mutex::new(LeaseState {
                admission: OwnerAdmission::Open,
                generation,
                active: 0,
            }),
            drained: Condvar::new(),
        }
    }

    fn acquire(self: &Arc<Self>, expected: u64) -> Result<LeaseGuard, ReliquaryRuntimeHostError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        if state.admission != OwnerAdmission::Open || state.generation != expected {
            return Err(operation("runtime admission is fenced"));
        }
        state.active = state
            .active
            .checked_add(1)
            .ok_or_else(|| operation("runtime lease count is exhausted"))?;
        Ok(LeaseGuard(Arc::clone(self)))
    }

    fn acquire_current(self: &Arc<Self>) -> Result<LeaseGuard, ReliquaryRuntimeHostError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        if state.admission != OwnerAdmission::Open {
            return Err(operation("runtime admission is fenced"));
        }
        state.active = state
            .active
            .checked_add(1)
            .ok_or_else(|| operation("runtime lease count is exhausted"))?;
        Ok(LeaseGuard(Arc::clone(self)))
    }

    fn close_and_drain(&self) -> Result<u64, ReliquaryRuntimeHostError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        state.admission = OwnerAdmission::Closed;
        while state.active != 0 {
            state = self
                .drained
                .wait(state)
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        }
        state.generation = state
            .generation
            .checked_add(1)
            .ok_or_else(|| operation("runtime fence generation is exhausted"))?;
        Ok(state.generation)
    }

    fn validate_reopen(&self) -> Result<(), ReliquaryRuntimeHostError> {
        let state = self
            .state
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        if state.admission != OwnerAdmission::Closed || state.active != 0 {
            return Err(operation(
                "reopen requires closed admission and drained leases",
            ));
        }
        state
            .generation
            .checked_add(1)
            .ok_or_else(|| operation("runtime fence generation is exhausted"))?;
        Ok(())
    }

    fn reopen(&self) -> Result<u64, ReliquaryRuntimeHostError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        if state.active != 0 {
            return Err(operation("cannot reopen a runtime with active leases"));
        }
        state.generation = state
            .generation
            .checked_add(1)
            .ok_or_else(|| operation("runtime fence generation is exhausted"))?;
        state.admission = OwnerAdmission::Open;
        Ok(state.generation)
    }

    fn snapshot(&self) -> Result<(OwnerAdmission, u64), ReliquaryRuntimeHostError> {
        self.state
            .lock()
            .map(|s| (s.admission, s.generation))
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)
    }

    fn snapshot_full(&self) -> Result<(OwnerAdmission, u64, usize), ReliquaryRuntimeHostError> {
        self.state
            .lock()
            .map(|s| (s.admission, s.generation, s.active))
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)
    }
}

struct LeaseGuard(Arc<LeaseGate>);
impl Drop for LeaseGuard {
    fn drop(&mut self) {
        if let Ok(mut state) = self.0.state.lock() {
            state.active = state.active.saturating_sub(1);
            if state.active == 0 {
                self.0.drained.notify_all();
            }
        }
    }
}

struct AttachmentEntry {
    instance: Arc<InstanceRuntime>,
    leases: Arc<LeaseGate>,
}

struct RelExecution {
    sequencer: Arc<events::RelSequencer>,
    file_identity: MountedFileIdentity,
    _mount_claim: MountClaim,
    owner: Arc<Mutex<RelRuntime>>,
    mount_epoch: AtomicU64,
    fence: Arc<LeaseGate>,
}

struct PhyExecution {
    file_identity: MountedFileIdentity,
    _mount_claim: MountClaim,
    owner: Arc<Mutex<Phylactery>>,
    runtime: Arc<Mutex<PhyRuntime>>,
    mount_epoch: AtomicU64,
    fence: Arc<LeaseGate>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OwnerKind {
    Rel,
    Phy,
}

/// Capability token bound to one authorized attachment and one owner mount.
/// Its private fields prevent callers from forging or retargeting a grant.
#[derive(Clone, Debug)]
pub struct OwnerAccessToken {
    instance_id: InstanceId,
    authorization: InstanceAuthorization,
    owner_id: String,
    kind: OwnerKind,
    grant: OwnerGrant,
    mount_epoch: u64,
    fence_generation: u64,
    owner_grant_generation: u64,
}

impl OwnerAccessToken {
    pub fn owner_id(&self) -> &str {
        &self.owner_id
    }
    pub fn grant(&self) -> OwnerGrant {
        self.grant
    }
}

/// Canonical in-process owner authority. One keyed execution exists per
/// mounted durable owner; instances contain only selection and authorization.
///
/// Instance-taking methods are the user-request admission surface. Constructors,
/// mount/unmount, raw owner enumeration, owner diagnostics, host reconnection,
/// shutdown, and instance-free quiesce/reopen are trusted embedding-service
/// controls: do not bind them directly to untrusted requests. Use instance-taking
/// management methods for client maintenance and viewable_rel_owners for listing.
pub struct GatewayRuntime {
    rel_executions: RwLock<BTreeMap<DurableOwnerId, RelExecution>>,
    phy_executions: Arc<RwLock<BTreeMap<DurableOwnerId, PhyExecution>>>,
    instances: RwLock<HashMap<InstanceId, AttachmentEntry>>,
    // Admission authority survives detach/unmount for this gateway lifetime.
    // Frozen publication plans additionally retain their exact durable destination.
    principal_phy_bindings: Arc<Mutex<BTreeMap<String, String>>>,
    publication_policy: Arc<RwLock<()>>,
    routes: Arc<RwLock<ReliquaryRuntimeRoutes>>,
    config: InsomniaWorkerConfig,
    episode_policy: EpisodePolicy,
    host_router: Option<CapabilityRouter>,
    admission_adapter: Option<Arc<dyn GatewayAdmissionAdapter>>,
    #[cfg(test)]
    project_io_test_hook: Mutex<Option<Arc<dyn Fn() + Send + Sync>>>,
    state_dir: PathBuf,
    next_mount_epoch: Mutex<u64>,
    lifecycle: Mutex<()>,
    topology_transition: Mutex<()>,
    shutting_down: AtomicBool,
    shutdown_complete: AtomicBool,
    conversation: conversation::ConversationRegistry,
}

impl GatewayRuntime {
    pub fn new(
        state_dir: PathBuf,
        routes: ReliquaryRuntimeRoutes,
        config: InsomniaWorkerConfig,
        episode_policy: EpisodePolicy,
    ) -> Self {
        Self::new_inner(state_dir, routes, config, episode_policy, None)
    }

    pub fn new_with_host(
        state_dir: PathBuf,
        routes: ReliquaryRuntimeRoutes,
        config: InsomniaWorkerConfig,
        episode_policy: EpisodePolicy,
        adapter: Arc<dyn GatewayHostAdapter>,
    ) -> Self {
        Self::new_inner(state_dir, routes, config, episode_policy, Some(adapter))
    }

    fn new_inner(
        state_dir: PathBuf,
        routes: ReliquaryRuntimeRoutes,
        config: InsomniaWorkerConfig,
        episode_policy: EpisodePolicy,
        host_adapter: Option<Arc<dyn GatewayHostAdapter>>,
    ) -> Self {
        let host_router = host_adapter.map(CapabilityRouter::connect);
        let gateway = Self {
            rel_executions: RwLock::new(BTreeMap::new()),
            phy_executions: Arc::new(RwLock::new(BTreeMap::new())),
            instances: RwLock::new(HashMap::new()),
            principal_phy_bindings: Arc::new(Mutex::new(BTreeMap::new())),
            publication_policy: Arc::new(RwLock::new(())),
            routes: Arc::new(RwLock::new(routes)),
            config,
            episode_policy,
            host_router,
            admission_adapter: None,
            #[cfg(test)]
            project_io_test_hook: Mutex::new(None),
            state_dir: state_dir.clone(),
            next_mount_epoch: Mutex::new(1),
            lifecycle: Mutex::new(()),
            topology_transition: Mutex::new(()),
            shutting_down: AtomicBool::new(false),
            shutdown_complete: AtomicBool::new(false),
            conversation: conversation::ConversationRegistry::new(state_dir.clone()),
        };
        gateway
    }

    pub fn state_dir(&self) -> &std::path::Path {
        &self.state_dir
    }

    pub fn capability_state(
        &self,
        capability: RuntimeCapability,
    ) -> Result<CapabilityState, ReliquaryRuntimeHostError> {
        match self.host_router.as_ref() {
            Some(router) => router.state(capability).map_err(operation),
            None => Ok(CapabilityState::NativeOnly),
        }
    }

    pub fn capability_authority(
        &self,
        capability: RuntimeCapability,
    ) -> Result<StageAuthority, ReliquaryRuntimeHostError> {
        match self.host_router.as_ref() {
            Some(router) => router.authority(capability).map_err(operation),
            None => Ok(StageAuthority::Native),
        }
    }

    /// Reconnect and reconcile per configured capability. The router preserves
    /// unknown and prior host-owned state until reconciliation proves safe.
    pub fn reconnect_host(&self) {
        if let Some(router) = self.host_router.as_ref() {
            // External connection/reconciliation never holds a REL distribution gate.
            router.reconnect();
            self.publish_host_capability_change();
        }
    }

    #[cfg(test)]
    pub(super) fn mount_rel(
        &self,
        runtime: InteractionRuntime,
    ) -> Result<DurableOwnerId, ReliquaryRuntimeHostError> {
        let _lifecycle = self
            .lifecycle
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        let _topology = self
            .topology_transition
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        let identity =
            MountedFileIdentity::from_container(&runtime.cva().container, OwnerKind::Rel)?;
        if self.existing_file_mount(&identity)? {
            return Ok(identity.owner_id);
        }
        let claim = MountClaim::acquire(&identity)?;
        self.mount_rel_inner(runtime, identity, claim)
    }

    fn mount_rel_inner(
        &self,
        mut runtime: InteractionRuntime,
        identity: MountedFileIdentity,
        claim: MountClaim,
    ) -> Result<DurableOwnerId, ReliquaryRuntimeHostError> {
        let owner_id = identity.owner_id.clone();
        let mount_epoch = self.allocate_mount_epoch()?;
        self.conversation.mount_rel(&owner_id)?;
        if let Err(error) = self.recover_rel_before_start(&owner_id, &mut runtime) {
            let _ = self.conversation.unmount_rel(&owner_id);
            return Err(error);
        }
        let operations = self.conversation.owner_journal(&owner_id)?;
        let restored = (|| {
            let journal = operations
                .lock()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
            let mut bindings = self
                .principal_phy_bindings
                .lock()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
            let mut proposed = bindings.clone();
            for record in journal.records() {
                let pairs = match &record.record {
                    owner_operations::OwnerOperationRecord::Attribution(a) => {
                        match (&a.admission.principal_id, &a.admission.phy_owner_id) {
                            (Some(principal), Some(phy)) => vec![(principal, phy)],
                            _ => Vec::new(),
                        }
                    }
                    owner_operations::OwnerOperationRecord::EpisodePlan(plan) => plan
                        .prepared
                        .personal_drafts
                        .iter()
                        .map(|draft| {
                            (
                                &draft.destination_principal,
                                &draft.destination_phy_owner_id,
                            )
                        })
                        .collect(),
                    _ => Vec::new(),
                };
                for (principal, phy) in pairs {
                    validate_owner_id(phy, "phy-")?;
                    if proposed.get(principal).is_some_and(|bound| bound != phy)
                        || proposed.iter().any(|(p, b)| p != principal && b == phy)
                    {
                        return Err(operation(
                            "durable personal route binding conflicts with gateway authority",
                        ));
                    }
                    proposed.insert(principal.clone(), phy.clone());
                }
            }
            *bindings = proposed;
            Ok(())
        })();
        if let Err(error) = restored {
            let _ = self.conversation.unmount_rel(&owner_id);
            return Err(error);
        }
        let fence = Arc::new(LeaseGate::new(1));
        let publication = Arc::new(publication::PublicationCoordinator {
            rel_id: owner_id.clone(),
            journal: operations,
            phys: Arc::clone(&self.phy_executions),
            bindings: Arc::clone(&self.principal_phy_bindings),
            policy: Arc::clone(&self.publication_policy),
            rel_fence: Arc::clone(&fence),
        });
        let sequencer = Arc::new(events::RelSequencer::new(&owner_id, mount_epoch)?);
        let owner = Arc::new(Mutex::new(RelRuntime::start(
            runtime,
            Arc::clone(&sequencer),
            Arc::clone(&self.routes),
            Arc::clone(&fence),
            self.config.clone(),
            self.episode_policy,
            publication,
        )));
        let mut mounted = self
            .rel_executions
            .write()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        if mounted
            .insert(
                owner_id.clone(),
                RelExecution {
                    sequencer,
                    file_identity: identity,
                    _mount_claim: claim,
                    owner,
                    mount_epoch: AtomicU64::new(mount_epoch),
                    fence,
                },
            )
            .is_some()
        {
            return Err(operation("REL owner was concurrently mounted"));
        }
        Ok(owner_id)
    }

    #[cfg(test)]
    pub(super) fn mount_phy(
        &self,
        owner: Phylactery,
    ) -> Result<DurableOwnerId, ReliquaryRuntimeHostError> {
        let _lifecycle = self
            .lifecycle
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        let _topology = self
            .topology_transition
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        let identity = MountedFileIdentity::from_container(&owner.container, OwnerKind::Phy)?;
        if self.existing_file_mount(&identity)? {
            return Ok(identity.owner_id);
        }
        let claim = MountClaim::acquire(&identity)?;
        self.mount_phy_inner(owner, identity, claim)
    }

    fn mount_phy_inner(
        &self,
        owner: Phylactery,
        identity: MountedFileIdentity,
        claim: MountClaim,
    ) -> Result<DurableOwnerId, ReliquaryRuntimeHostError> {
        let owner_id = identity.owner_id.clone();
        let mut mounted = self
            .phy_executions
            .write()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        let mount_epoch = self.allocate_mount_epoch()?;
        let owner = Arc::new(Mutex::new(owner));
        let fence = Arc::new(LeaseGate::new(1));
        let runtime = Arc::new(Mutex::new(PhyRuntime::start(
            Arc::clone(&owner),
            Arc::clone(&self.routes),
            Arc::clone(&fence),
        )?));
        mounted.insert(
            owner_id.clone(),
            PhyExecution {
                file_identity: identity,
                _mount_claim: claim,
                owner,
                runtime,
                mount_epoch: AtomicU64::new(mount_epoch),
                fence,
            },
        );
        Ok(owner_id)
    }

    fn allocate_mount_epoch(&self) -> Result<u64, ReliquaryRuntimeHostError> {
        let mut next = self
            .next_mount_epoch
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        let epoch = *next;
        *next = next
            .checked_add(1)
            .ok_or_else(|| operation("owner mount epoch is exhausted"))?;
        Ok(epoch)
    }

    /// Attach a host-verified principal, optional exact PHY binding, and explicit
    /// capabilities. Dependencies never grant access implicitly.
    pub(super) fn attach_instance(
        &self,
        principal_id: String,
        phy_id: Option<String>,
        grants: BTreeMap<String, BTreeSet<OwnerGrant>>,
    ) -> Result<Arc<InstanceRuntime>, ReliquaryRuntimeHostError> {
        let _lifecycle = self
            .lifecycle
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        if self.shutting_down.load(Ordering::SeqCst) {
            return Err(operation("gateway is shutting down"));
        }
        if principal_id.trim().is_empty() {
            return Err(operation("an instance requires a verified principal ID"));
        }
        self.validate_grants(&grants, phy_id.as_deref())?;
        if let Some(phy) = phy_id.as_deref() {
            if !self
                .phy_executions
                .read()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
                .contains_key(phy)
            {
                return Err(operation("bound PHY is not mounted at attachment"));
            }
        }
        let mut instances = self
            .instances
            .write()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        let mut bindings = self
            .principal_phy_bindings
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        if let Some(phy) = phy_id.as_ref() {
            if bindings
                .get(&principal_id)
                .is_some_and(|bound| bound != phy)
                || bindings
                    .iter()
                    .any(|(principal, bound)| principal != &principal_id && bound == phy)
            {
                return Err(operation(
                    "verified principal/PHY binding conflicts with gateway authority",
                ));
            }
            bindings.insert(principal_id.clone(), phy.clone());
        }
        let instance = Arc::new(InstanceRuntime::new(principal_id, phy_id, grants));
        instances.insert(
            instance.instance_id(),
            AttachmentEntry {
                instance: Arc::clone(&instance),
                leases: Arc::new(LeaseGate::new(1)),
            },
        );
        Ok(instance)
    }

    pub fn select_rel(
        &self,
        instance: &InstanceRuntime,
        owner_id: &str,
    ) -> Result<bool, ReliquaryRuntimeHostError> {
        let entry = self.attachment_entry(instance)?;
        let _lease = entry.leases.acquire_current()?;
        let fence = {
            let rels = self
                .rel_executions
                .read()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
            Arc::clone(
                &rels
                    .get(owner_id)
                    .ok_or_else(|| operation(format!("REL owner {owner_id} is not mounted")))?
                    .fence,
            )
        };
        let _owner_lease = fence.acquire_current()?;
        instance.select_rel(owner_id).map_err(operation)
    }

    pub fn authorize(
        &self,
        instance: &InstanceRuntime,
        owner_id: &str,
        grant: OwnerGrant,
    ) -> Result<OwnerAccessToken, ReliquaryRuntimeHostError> {
        let entry = self.attachment_entry(instance)?;
        let _lease = entry.leases.acquire_current()?;
        let authorization = instance.authorization().map_err(operation)?;
        instance
            .authorize(owner_id, grant, authorization)
            .map_err(operation)?;
        if let Some(rel) = self
            .rel_executions
            .read()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
            .get(owner_id)
        {
            let (admission, generation) = rel.fence.snapshot()?;
            if admission != OwnerAdmission::Open {
                return Err(operation("REL owner admission is fenced"));
            }
            return Ok(OwnerAccessToken {
                instance_id: instance.instance_id(),
                authorization,
                owner_id: owner_id.to_owned(),
                kind: OwnerKind::Rel,
                grant,
                mount_epoch: rel.mount_epoch.load(Ordering::SeqCst),
                fence_generation: generation,
                owner_grant_generation: instance
                    .owner_grant_generation(owner_id)
                    .map_err(operation)?,
            });
        }
        if grant != OwnerGrant::UseOwnPhy {
            return Err(operation("PHY owner access requires UseOwnPhy"));
        }
        let phys = self
            .phy_executions
            .read()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        let phy = phys
            .get(owner_id)
            .ok_or_else(|| operation(format!("owner {owner_id} is not mounted")))?;
        let (admission, generation) = phy.fence.snapshot()?;
        if admission != OwnerAdmission::Open {
            return Err(operation("PHY owner admission is fenced"));
        }
        Ok(OwnerAccessToken {
            instance_id: instance.instance_id(),
            authorization,
            owner_id: owner_id.to_owned(),
            kind: OwnerKind::Phy,
            grant,
            mount_epoch: phy.mount_epoch.load(Ordering::SeqCst),
            fence_generation: generation,
            owner_grant_generation: instance
                .owner_grant_generation(owner_id)
                .map_err(operation)?,
        })
    }

    /// Revalidate authorization at the actual owner operation boundary. Keep
    /// this lease short and never perform inference or provider I/O in the closure.
    fn with_rel<T>(
        &self,
        token: &OwnerAccessToken,
        f: impl FnOnce(&mut InteractionRuntime) -> Result<T, ReliquaryRuntimeHostError>,
    ) -> Result<T, ReliquaryRuntimeHostError> {
        if token.kind != OwnerKind::Rel {
            return Err(operation("owner token is not for a REL"));
        }
        let attachment = self.attachment_by_id(token.instance_id)?;
        let _attachment_lease = attachment.leases.acquire_current()?;
        attachment
            .instance
            .authorize_owner_generation(
                &token.owner_id,
                token.grant,
                token.authorization,
                token.owner_grant_generation,
            )
            .map_err(operation)?;
        let rels = self
            .rel_executions
            .read()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        let rel = rels
            .get(&token.owner_id)
            .ok_or_else(|| operation(format!("REL owner {} is not mounted", token.owner_id)))?;
        if rel.mount_epoch.load(Ordering::SeqCst) != token.mount_epoch {
            return Err(operation("owner token belongs to a previous REL mount"));
        }
        let _owner_lease = rel.fence.acquire(token.fence_generation)?;
        let owner = Arc::clone(&rel.owner);
        drop(rels);
        let runtime = owner
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
            .runtime
            .as_ref()
            .cloned()
            .ok_or_else(|| operation("REL runtime is unavailable"))?;
        let mut runtime = runtime
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        attachment
            .instance
            .authorize_owner_generation(
                &token.owner_id,
                token.grant,
                token.authorization,
                token.owner_grant_generation,
            )
            .map_err(operation)?;
        f(&mut runtime)
    }

    fn with_phy<T>(
        &self,
        token: &OwnerAccessToken,
        f: impl FnOnce(&mut Phylactery) -> Result<T, ReliquaryRuntimeHostError>,
    ) -> Result<T, ReliquaryRuntimeHostError> {
        if token.kind != OwnerKind::Phy || token.grant != OwnerGrant::UseOwnPhy {
            return Err(operation("owner token is not authorized for a PHY"));
        }
        let attachment = self.attachment_by_id(token.instance_id)?;
        let _attachment_lease = attachment.leases.acquire_current()?;
        attachment
            .instance
            .authorize_owner_generation(
                &token.owner_id,
                token.grant,
                token.authorization,
                token.owner_grant_generation,
            )
            .map_err(operation)?;
        let (owner, mount_epoch, fence) = {
            let phys = self
                .phy_executions
                .read()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
            let phy = phys
                .get(&token.owner_id)
                .ok_or_else(|| operation(format!("PHY owner {} is not mounted", token.owner_id)))?;
            (
                Arc::clone(&phy.owner),
                phy.mount_epoch.load(Ordering::SeqCst),
                Arc::clone(&phy.fence),
            )
        };
        if mount_epoch != token.mount_epoch {
            return Err(operation("owner token belongs to a previous PHY mount"));
        }
        let _owner_lease = fence.acquire(token.fence_generation)?;
        let mut owner = owner
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        attachment
            .instance
            .authorize_owner_generation(
                &token.owner_id,
                token.grant,
                token.authorization,
                token.owner_grant_generation,
            )
            .map_err(operation)?;
        f(&mut owner)
    }

    fn validate_grants(
        &self,
        grants: &BTreeMap<String, BTreeSet<OwnerGrant>>,
        phy_id: Option<&str>,
    ) -> Result<(), ReliquaryRuntimeHostError> {
        let rels = self
            .rel_executions
            .read()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        let phys = self
            .phy_executions
            .read()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        for (owner, caps) in grants {
            if owner.starts_with("rel-") {
                validate_owner_id(owner, "rel-")?;
                if caps.contains(&OwnerGrant::UseOwnPhy) {
                    return Err(operation("UseOwnPhy is not a REL capability"));
                }
                if !rels.contains_key(owner) {
                    return Err(operation(format!(
                        "authorized REL owner {owner} is not mounted"
                    )));
                }
            } else if owner.starts_with("phy-") {
                validate_owner_id(owner, "phy-")?;
                if caps.iter().any(|cap| *cap != OwnerGrant::UseOwnPhy) {
                    return Err(operation("REL capabilities cannot authorize PHY access"));
                }
                if !phys.contains_key(owner) {
                    return Err(operation(format!(
                        "authorized PHY owner {owner} is not mounted"
                    )));
                }
                if caps.contains(&OwnerGrant::UseOwnPhy) && phy_id != Some(owner.as_str()) {
                    return Err(operation(
                        "UseOwnPhy may name only the verified instance PHY",
                    ));
                }
            } else {
                return Err(operation(format!("owner ID {owner} is not canonical")));
            }
        }
        if let Some(id) = phy_id {
            validate_owner_id(id, "phy-")?;
        }
        Ok(())
    }

    pub(super) fn replace_instance_grants(
        &self,
        instance: &InstanceRuntime,
        grants: BTreeMap<String, BTreeSet<OwnerGrant>>,
    ) -> Result<(), ReliquaryRuntimeHostError> {
        let _lifecycle = self
            .lifecycle
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        if self.shutting_down.load(Ordering::SeqCst) {
            return Err(operation("gateway is shutting down"));
        }
        let entry = self.attachment_entry(instance)?;
        self.validate_grants(&grants, instance.attached_phy_owner_id())?;
        entry.leases.close_and_drain()?;
        let old_grants = instance.grants_snapshot().map_err(operation)?;
        let mut revoked_view = Vec::new();
        let mut revoked_submit = BTreeSet::new();
        let mut revoked_phy = None;
        for (owner_id, previous) in &old_grants {
            let replacement = grants.get(owner_id);
            let has_next = |grant| replacement.is_some_and(|next| next.contains(&grant));
            if previous.contains(&OwnerGrant::ViewRel) && !has_next(OwnerGrant::ViewRel) {
                revoked_view.push(owner_id.as_str());
            }
            if previous.contains(&OwnerGrant::SubmitConversation)
                && !has_next(OwnerGrant::SubmitConversation)
            {
                revoked_submit.insert(owner_id.clone());
            }
            if previous.contains(&OwnerGrant::UseOwnPhy) && !has_next(OwnerGrant::UseOwnPhy) {
                revoked_phy = Some(owner_id.as_str());
            }
        }
        let update = (|| {
            // Wait for admitted personal writes, then sync withdrawals before
            // new routed writes or the changed grant state can become visible.
            let _policy = self
                .publication_policy
                .write()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
            self.conversation.revoke_personal_authority(
                instance.instance_id(),
                &revoked_submit,
                revoked_phy,
            )?;
            for owner_id in revoked_view {
                self.conversation
                    .remove_instance_owner_viewers(instance.instance_id(), owner_id)?;
            }
            for rel in self
                .rel_executions
                .read()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
                .values()
            {
                rel.sequencer.remove_instance(instance.instance_id())?;
            }
            instance.replace_grants(grants).map_err(operation)?;
            Ok(())
        })();
        match update {
            Ok(()) => {
                entry.leases.reopen()?;
            }
            Err(error) => {
                let _ = entry.leases.reopen();
                return Err(error);
            }
        }
        Ok(())
    }

    pub fn detach_instance(
        &self,
        instance: &InstanceRuntime,
    ) -> Result<bool, ReliquaryRuntimeHostError> {
        let _lifecycle = self
            .lifecycle
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        if self.shutting_down.load(Ordering::SeqCst) {
            return Err(operation("gateway is shutting down"));
        }
        let mut instances = self
            .instances
            .write()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        let Some(entry) = instances.get(&instance.instance_id()) else {
            return Ok(false);
        };
        if !std::ptr::eq(Arc::as_ptr(&entry.instance), instance) {
            return Err(operation(
                "instance attachment does not belong to this gateway",
            ));
        }
        let entry = instances
            .remove(&instance.instance_id())
            .expect("checked instance entry");
        drop(instances);
        entry.leases.close_and_drain()?;
        self.conversation
            .detach_instance_viewers(instance.instance_id())?;
        for rel in self
            .rel_executions
            .read()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
            .values()
        {
            rel.sequencer.remove_instance(instance.instance_id())?;
        }
        instance.detach().map_err(operation)
    }

    pub fn active_rel_id(
        &self,
        instance: &InstanceRuntime,
    ) -> Result<Option<String>, ReliquaryRuntimeHostError> {
        self.attachment_entry(instance)?;
        let selected = instance.selected_rel().map_err(operation)?;
        if let Some(owner) = selected.as_deref() {
            if !self
                .rel_executions
                .read()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
                .contains_key(owner)
            {
                return Err(operation("instance selection refers to an unmounted REL"));
            }
        }
        Ok(selected)
    }

    pub fn instance_managed_state(
        &self,
        instance: &InstanceRuntime,
        owner_id: &str,
    ) -> Result<super::instances::ManagedConversationState, ReliquaryRuntimeHostError> {
        self.attachment_entry(instance)?;
        if !self
            .rel_executions
            .read()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
            .contains_key(owner_id)
        {
            return Err(operation(format!("REL owner {owner_id} is not mounted")));
        }
        instance
            .managed_conversation_state(owner_id)
            .map_err(operation)
    }

    fn set_instance_managed_state(
        &self,
        instance: &InstanceRuntime,
        owner_id: &str,
        state: super::instances::ManagedConversationState,
    ) -> Result<(), ReliquaryRuntimeHostError> {
        let entry = self.attachment_entry(instance)?;
        let _lease = entry.leases.acquire_current()?;
        if !self
            .rel_executions
            .read()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
            .contains_key(owner_id)
        {
            return Err(operation(format!("REL owner {owner_id} is not mounted")));
        }
        instance
            .set_managed_conversation_state(owner_id, state)
            .map_err(operation)
    }

    pub fn mounted_rel_ids(&self) -> Result<Vec<String>, ReliquaryRuntimeHostError> {
        self.rel_executions
            .read()
            .map(|r| r.keys().cloned().collect())
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)
    }
    pub fn mounted_phy_ids(&self) -> Result<Vec<String>, ReliquaryRuntimeHostError> {
        self.phy_executions
            .read()
            .map(|p| p.keys().cloned().collect())
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)
    }

    pub fn owner_admission(
        &self,
        owner_id: &str,
    ) -> Result<OwnerAdmission, ReliquaryRuntimeHostError> {
        if let Some(rel) = self
            .rel_executions
            .read()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
            .get(owner_id)
        {
            return Ok(rel.fence.snapshot()?.0);
        }
        if let Some(phy) = self
            .phy_executions
            .read()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
            .get(owner_id)
        {
            return Ok(phy.fence.snapshot()?.0);
        }
        Err(operation(format!("owner {owner_id} is not mounted")))
    }

    /// Close admission and drain short REL commit leases. This is an in-process
    /// fence only; direct Cva writers and other processes remain outside scope.
    pub fn quiesce_rel(&self, owner_id: &str) -> Result<(), ReliquaryRuntimeHostError> {
        let _lifecycle = self
            .lifecycle
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        if self.shutting_down.load(Ordering::SeqCst) {
            return Err(operation("gateway is shutting down"));
        }
        let (fence, owner, sequencer) = {
            let rels = self
                .rel_executions
                .read()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
            let rel = rels
                .get(owner_id)
                .ok_or_else(|| operation(format!("REL owner {owner_id} is not mounted")))?;
            (
                Arc::clone(&rel.fence),
                Arc::clone(&rel.owner),
                Arc::clone(&rel.sequencer),
            )
        };
        fence.close_and_drain()?;
        sequencer.close()?;
        owner
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
            .stop_workers()?;
        Ok(())
    }

    pub fn reopen_rel(&self, owner_id: &str) -> Result<u64, ReliquaryRuntimeHostError> {
        let _lifecycle = self
            .lifecycle
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        if self.shutting_down.load(Ordering::SeqCst) {
            return Err(operation("gateway is shutting down"));
        }
        let (fence, owner, sequencer) = {
            let rels = self
                .rel_executions
                .read()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
            let rel = rels
                .get(owner_id)
                .ok_or_else(|| operation(format!("REL owner {owner_id} is not mounted")))?;
            (
                Arc::clone(&rel.fence),
                Arc::clone(&rel.owner),
                Arc::clone(&rel.sequencer),
            )
        };
        if fence.snapshot()?.0 != OwnerAdmission::Closed {
            return Err(operation("REL must be quiesced before it can be reopened"));
        }
        // Preflight before reconciliation or restarting workers: an exhausted
        // fence must leave this owner fully closed and stopped.
        fence.validate_reopen()?;
        self.reconcile_rel(owner_id)?;
        let epoch = self.allocate_mount_epoch()?;
        {
            let rels = self
                .rel_executions
                .read()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
            let rel = rels
                .get(owner_id)
                .ok_or_else(|| operation(format!("REL owner {owner_id} is not mounted")))?;
            rel.mount_epoch.store(epoch, Ordering::SeqCst);
        }
        self.reopen_rel_distribution(owner_id, epoch, &sequencer)?;
        let (generation, restarted) = {
            let mut execution = owner
                .lock()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
            let generation = fence.reopen()?;
            (generation, execution.restart_workers())
        };
        if let Err(error) = restarted {
            sequencer.close()?;
            fence.close_and_drain()?;
            owner
                .lock()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
                .stop_workers()?;
            return Err(error);
        }
        Ok(generation)
    }

    pub(super) fn with_quiesced_rel<T>(
        &self,
        owner_id: &str,
        f: impl FnOnce(&mut InteractionRuntime) -> Result<T, ReliquaryRuntimeHostError>,
    ) -> Result<T, ReliquaryRuntimeHostError> {
        let _lifecycle = self
            .lifecycle
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        self.with_quiesced_rel_inner(owner_id, f)
    }

    /// Internal transition path for lifecycle code that already owns the
    /// gateway lifecycle mutex, such as quiesced Archive reconciliation.
    pub(super) fn with_quiesced_rel_during_transition<T>(
        &self,
        owner_id: &str,
        f: impl FnOnce(&mut InteractionRuntime) -> Result<T, ReliquaryRuntimeHostError>,
    ) -> Result<T, ReliquaryRuntimeHostError> {
        self.with_quiesced_rel_inner(owner_id, f)
    }

    fn with_quiesced_rel_inner<T>(
        &self,
        owner_id: &str,
        f: impl FnOnce(&mut InteractionRuntime) -> Result<T, ReliquaryRuntimeHostError>,
    ) -> Result<T, ReliquaryRuntimeHostError> {
        if self.shutting_down.load(Ordering::SeqCst) {
            return Err(operation("gateway is shutting down"));
        }
        let (owner, fence) = {
            let rels = self
                .rel_executions
                .read()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
            let rel = rels
                .get(owner_id)
                .ok_or_else(|| operation(format!("REL owner {owner_id} is not mounted")))?;
            (Arc::clone(&rel.owner), Arc::clone(&rel.fence))
        };
        let (admission, _, active) = fence.snapshot_full()?;
        if admission != OwnerAdmission::Closed || active != 0 {
            return Err(operation(
                "quiesced maintenance requires closed admission and drained leases",
            ));
        }
        let owner = owner
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        if !owner.workers_stopped() {
            return Err(operation(
                "owner workers must be stopped before quiesced maintenance",
            ));
        }
        let runtime = owner
            .runtime
            .as_ref()
            .cloned()
            .ok_or_else(|| operation("REL runtime is unavailable"))?;
        let mut runtime = runtime
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        f(&mut runtime)
    }

    pub fn shutdown(&self) -> Result<(), ReliquaryRuntimeHostError> {
        let _lifecycle = self
            .lifecycle
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        let _topology = self
            .topology_transition
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        if self.shutdown_complete.load(Ordering::SeqCst) {
            return Ok(());
        }
        self.shutting_down.store(true, Ordering::SeqCst);
        let rels = self
            .rel_executions
            .read()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        let owners = rels
            .values()
            .map(|rel| {
                (
                    Arc::clone(&rel.fence),
                    Arc::clone(&rel.owner),
                    Arc::clone(&rel.sequencer),
                )
            })
            .collect::<Vec<_>>();
        drop(rels);
        let phys = self
            .phy_executions
            .read()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        let phy_owners = phys
            .values()
            .map(|phy| (Arc::clone(&phy.fence), Arc::clone(&phy.runtime)))
            .collect::<Vec<_>>();
        drop(phys);
        let mut first_failure = None;
        for (fence, _, _) in &owners {
            if let Err(error) = fence.close_and_drain() {
                if first_failure.is_none() {
                    first_failure = Some(error);
                }
            }
        }
        for (gate, _) in &phy_owners {
            if let Err(error) = gate.close_and_drain() {
                if first_failure.is_none() {
                    first_failure = Some(error);
                }
            }
        }
        for (_, _, sequencer) in &owners {
            if let Err(error) = sequencer.close() {
                if first_failure.is_none() {
                    first_failure = Some(error);
                }
            }
        }
        for (_, owner, _) in owners {
            let stopped = match owner.lock() {
                Ok(mut owner) => owner.stop_workers(),
                Err(_) => Err(ReliquaryRuntimeHostError::LockPoisoned),
            };
            if let Err(error) = stopped {
                if first_failure.is_none() {
                    first_failure = Some(error);
                }
            }
        }
        for (_, runtime) in phy_owners {
            let stopped = match runtime.lock() {
                Ok(mut runtime) => runtime.stop_workers(),
                Err(_) => Err(ReliquaryRuntimeHostError::LockPoisoned),
            };
            if let Err(error) = stopped {
                if first_failure.is_none() {
                    first_failure = Some(error);
                }
            }
        }
        let instances = self
            .instances
            .read()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        let attachments = instances
            .values()
            .map(|entry| (Arc::clone(&entry.leases), Arc::clone(&entry.instance)))
            .collect::<Vec<_>>();
        drop(instances);
        for (gate, _) in &attachments {
            if let Err(error) = gate.close_and_drain() {
                if first_failure.is_none() {
                    first_failure = Some(error);
                }
            }
        }
        for (_, instance) in attachments {
            if let Err(error) = instance.detach() {
                if first_failure.is_none() {
                    first_failure = Some(operation(error));
                }
            }
        }
        if let Err(error) = self.conversation.shutdown() {
            if first_failure.is_none() {
                first_failure = Some(error);
            }
        }
        if let Some(error) = first_failure {
            return Err(error);
        }
        self.shutdown_complete.store(true, Ordering::SeqCst);
        Ok(())
    }

    pub fn unmount_phy(&self, owner_id: &str) -> Result<Phylactery, ReliquaryRuntimeHostError> {
        let _lifecycle = self
            .lifecycle
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        let _topology = self
            .topology_transition
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        if self.shutting_down.load(Ordering::SeqCst) {
            return Err(operation("gateway is shutting down"));
        }
        let (fence, owner, runtime) = {
            let phys = self
                .phy_executions
                .read()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
            let phy = phys
                .get(owner_id)
                .ok_or_else(|| operation(format!("PHY owner {owner_id} is not mounted")))?;
            (
                Arc::clone(&phy.fence),
                Arc::clone(&phy.owner),
                Arc::clone(&phy.runtime),
            )
        };
        fence.close_and_drain()?;
        runtime
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
            .stop_workers()?;
        let removed = self
            .phy_executions
            .write()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
            .remove(owner_id)
            .ok_or_else(|| operation(format!("PHY owner {owner_id} is not mounted")))?;
        drop(removed);
        drop(runtime);
        let owner = Arc::try_unwrap(owner)
            .map_err(|_| operation("PHY execution still has active readers"))?;
        owner
            .into_inner()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)
    }

    pub fn unmount_rel(&self, owner_id: &str) -> Result<crate::Cva, ReliquaryRuntimeHostError> {
        let _lifecycle = self
            .lifecycle
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        let _topology = self
            .topology_transition
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        if self.shutting_down.load(Ordering::SeqCst) {
            return Err(operation("gateway is shutting down"));
        }
        let (fence, owner, sequencer) = {
            let rels = self
                .rel_executions
                .read()
                .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
            let rel = rels
                .get(owner_id)
                .ok_or_else(|| operation(format!("REL owner {owner_id} is not mounted")))?;
            (
                Arc::clone(&rel.fence),
                Arc::clone(&rel.owner),
                Arc::clone(&rel.sequencer),
            )
        };
        fence.close_and_drain()?;
        sequencer.close()?;
        owner
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
            .stop_workers()?;
        self.conversation.unmount_rel(owner_id)?;
        let removed = self
            .rel_executions
            .write()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?
            .remove(owner_id)
            .ok_or_else(|| operation(format!("REL owner {owner_id} is not mounted")))?;
        drop(removed);
        let owner = Arc::try_unwrap(owner)
            .map_err(|_| operation("REL execution still has active owner references"))?;
        let owner = owner
            .into_inner()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        owner.into_cva()
    }

    fn attachment_entry(
        &self,
        instance: &InstanceRuntime,
    ) -> Result<AttachmentEntryRef, ReliquaryRuntimeHostError> {
        let entry = self.attachment_by_id(instance.instance_id())?;
        if !std::ptr::eq(Arc::as_ptr(&entry.instance), instance) {
            return Err(operation(
                "instance attachment does not belong to this gateway",
            ));
        }
        Ok(entry)
    }

    fn attachment_by_id(
        &self,
        id: InstanceId,
    ) -> Result<AttachmentEntryRef, ReliquaryRuntimeHostError> {
        if self.shutting_down.load(Ordering::SeqCst) {
            return Err(operation("gateway is shutting down"));
        }
        let instances = self
            .instances
            .read()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        let entry = instances
            .get(&id)
            .ok_or_else(|| operation("instance is not attached to this gateway"))?;
        Ok(AttachmentEntryRef {
            instance: Arc::clone(&entry.instance),
            leases: Arc::clone(&entry.leases),
        })
    }
}

struct AttachmentEntryRef {
    instance: Arc<InstanceRuntime>,
    leases: Arc<LeaseGate>,
}

fn validate_owner_id(owner_id: &str, prefix: &str) -> Result<(), ReliquaryRuntimeHostError> {
    let Some(uuid) = owner_id.strip_prefix(prefix) else {
        return Err(operation(format!(
            "owner ID {owner_id} must use canonical {prefix}<uuid> form"
        )));
    };
    let parsed = Uuid::parse_str(uuid)
        .map_err(|_| operation(format!("owner ID {owner_id} has an invalid durable UUID")))?;
    if parsed.is_nil() || parsed.to_string() != uuid {
        return Err(operation(format!("owner ID {owner_id} is not canonical")));
    }
    Ok(())
}

#[cfg(test)]
#[path = "runtime_host_gateway_admission_tests.rs"]
mod admission_tests;
#[cfg(test)]
#[path = "runtime_host_gateway_phy_tests.rs"]
mod phy_tests;
#[cfg(test)]
#[path = "runtime_host_gateway_tests.rs"]
mod tests;

impl Drop for GatewayRuntime {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}
