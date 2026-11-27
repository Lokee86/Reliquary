use crate::{
    OwnerId, OwnerKind, Phylactery, ProductError, ProductErrorKind, Rel, WorkspaceManagement,
};
use reliquary_runtime_gateway as runtime;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum OwnerGrant {
    ViewRel(OwnerId),
    OperateRel(OwnerId),
    ManageRel(OwnerId),
    ViewPhylactery(OwnerId),
    OperatePhylactery(OwnerId),
}

impl OwnerGrant {
    fn owner(self) -> OwnerId {
        match self {
            Self::ViewRel(id)
            | Self::OperateRel(id)
            | Self::ManageRel(id)
            | Self::ViewPhylactery(id)
            | Self::OperatePhylactery(id) => id,
        }
    }
    fn is_rel(self) -> bool {
        matches!(
            self,
            Self::ViewRel(_) | Self::OperateRel(_) | Self::ManageRel(_)
        )
    }
}

#[derive(Clone, Eq, PartialEq)]
struct MountedOwnerRecord {
    path: PathBuf,
    kind: OwnerKind,
    writable: bool,
    mounted: bool,
}

#[derive(Clone)]
pub struct Gateway {
    pub(crate) inner: runtime::Gateway,
    pub(crate) identity: Arc<runtime::IdentityLifecycle>,
    pub(crate) perception: Arc<runtime::PerceptionLifecycle>,
    mounted: Arc<Mutex<BTreeMap<OwnerId, MountedOwnerRecord>>>,
}

impl Default for Gateway {
    fn default() -> Self {
        Self::new()
    }
}

impl Gateway {
    pub fn new() -> Self {
        let inner = runtime::Gateway::new();
        let perception = Arc::new(inner.perception_lifecycle());
        Self {
            inner,
            identity: Arc::default(),
            perception,
            mounted: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }

    /// Set the configured runtime for Gateway-managed work. Entity comparisons use
    /// its Entity route, including when a previously recorded pair needs a new judgment.
    pub fn set_runtime(
        &self,
        runtime: Option<&crate::ConfiguredRuntime>,
    ) -> Result<(), ProductError> {
        let endpoint = runtime.and_then(crate::ConfiguredRuntime::entity_endpoint);
        let route = runtime
            .map(|runtime| {
                blake3::Hash::from_bytes(runtime.processing_route_identity().entity_identity)
                    .to_hex()
                    .to_string()
            })
            .unwrap_or_default();
        self.identity.set_route(endpoint, route)?;
        self.set_perception_runtime(runtime)
    }

    /// Bind the same-REL Perception worker while leaving Entity routing independent.
    pub fn set_perception_runtime(
        &self,
        runtime: Option<&crate::ConfiguredRuntime>,
    ) -> Result<(), ProductError> {
        self.perception.set_routes(runtime.and_then(|runtime| {
            use reliquary_switchboard::DecisionConsumer;
            Some(runtime::PerceptionRoutes {
                general: runtime.general_endpoint()?,
                contribution: runtime
                    .decision_endpoint_for(DecisionConsumer::ObservationContribution)?,
                verification: runtime
                    .decision_endpoint_for(DecisionConsumer::ObservationVerification)?,
                embedding: runtime.embedding_endpoint()?,
                identity: runtime.processing_route_identity().perception,
                embedding_identity: runtime.processing_route_identity().perception_embedding,
            })
        }))?;
        Ok(())
    }

    pub(crate) fn admit_perception(
        &self,
        instance: &Arc<runtime::Instance>,
        managed: Vec<runtime::OwnerId>,
    ) -> Result<(), ProductError> {
        Ok(self.perception.admit(instance, managed)?)
    }

    pub fn mount_rel(&self, rel: &Rel, writable: bool) -> Result<OwnerId, ProductError> {
        self.mount_owner_path(rel.path(), OwnerKind::Rel, writable, Some(rel.owner_id()))
    }

    pub fn mount_phylactery(
        &self,
        phy: &Phylactery,
        writable: bool,
    ) -> Result<OwnerId, ProductError> {
        self.mount_owner_path(
            phy.path(),
            OwnerKind::Phylactery,
            writable,
            Some(phy.owner_id()),
        )
    }

    /// Admit and mount one owner on the same file handle. Hosts with a reviewed
    /// or persisted assignment supply its expected owner ID.
    pub fn mount_owner_path(
        &self,
        path: impl AsRef<Path>,
        kind: OwnerKind,
        writable: bool,
        expected_owner: Option<OwnerId>,
    ) -> Result<OwnerId, ProductError> {
        let path = path.as_ref();
        let prepared = prepare_owner_mount(path, kind, writable, expected_owner)?;
        let mut mounted = self.mounted.lock().map_err(|_| {
            ProductError::new(
                ProductErrorKind::Unavailable,
                "mounted owner registry is unavailable",
            )
        })?;
        let id: OwnerId = self.inner.install_owner_mount(prepared)?.into();
        mounted.insert(
            id,
            MountedOwnerRecord {
                path: path.to_path_buf(),
                kind,
                writable,
                mounted: true,
            },
        );
        Ok(id)
    }

    pub(crate) fn quiesce(&self, owner: OwnerId) -> Result<(), ProductError> {
        self.quiesce_mount(owner, None)
    }

    pub(crate) fn detach_read_only_phylactery(&self, owner: OwnerId) -> Result<(), ProductError> {
        self.quiesce_mount(owner, Some(false))
    }

    fn detach_writable_phylactery(&self, owner: OwnerId) -> Result<(), ProductError> {
        self.quiesce_mount(owner, Some(true))
    }

    fn quiesce_mount(
        &self,
        owner: OwnerId,
        phy_writable: Option<bool>,
    ) -> Result<(), ProductError> {
        let mut mounted = self.mounted.lock().map_err(|_| {
            ProductError::new(
                ProductErrorKind::Unavailable,
                "mounted owner registry is unavailable",
            )
        })?;
        let record = mounted.get_mut(&owner).ok_or_else(|| {
            ProductError::new(ProductErrorKind::Unavailable, "owner is not registered")
        })?;
        if phy_writable == Some(true)
            && (record.kind != OwnerKind::Phylactery || !record.mounted || !record.writable)
        {
            return Err(ProductError::new(
                ProductErrorKind::Conflict,
                "owner must be a mounted writable PHY",
            ));
        }
        if phy_writable == Some(false) {
            self.inner.detach_read_only_phylactery(owner.into())?;
        } else {
            self.inner.quiesce_owner(owner.into())?;
        }
        record.mounted = false;
        Ok(())
    }

    pub(crate) fn reopen(&self, owner: OwnerId) -> Result<(), ProductError> {
        let record = self
            .mounted
            .lock()
            .map_err(|_| {
                ProductError::new(
                    ProductErrorKind::Unavailable,
                    "mounted owner registry is unavailable",
                )
            })?
            .get(&owner)
            .cloned()
            .ok_or_else(|| {
                ProductError::new(ProductErrorKind::Unavailable, "owner is not registered")
            })?;
        if record.mounted {
            return Err(ProductError::new(
                ProductErrorKind::Conflict,
                "owner is already mounted",
            ));
        }
        let prepared =
            prepare_owner_mount(&record.path, record.kind, record.writable, Some(owner))?;
        let mut mounted = self.mounted.lock().map_err(|_| {
            ProductError::new(
                ProductErrorKind::Unavailable,
                "mounted owner registry is unavailable",
            )
        })?;
        let current = mounted
            .get_mut(&owner)
            .filter(|current| **current == record)
            .ok_or_else(|| {
                ProductError::new(
                    ProductErrorKind::Conflict,
                    "owner registration changed before reopen",
                )
            })?;
        self.inner.install_owner_mount(prepared)?;
        current.mounted = true;
        Ok(())
    }

    pub(crate) fn record(
        &self,
        owner: OwnerId,
    ) -> Result<(PathBuf, OwnerKind, bool), ProductError> {
        let record = self
            .mounted
            .lock()
            .map_err(|_| {
                ProductError::new(
                    ProductErrorKind::Unavailable,
                    "mounted owner registry is unavailable",
                )
            })?
            .get(&owner)
            .cloned()
            .ok_or_else(|| {
                ProductError::new(ProductErrorKind::Unavailable, "owner is not registered")
            })?;
        Ok((record.path, record.kind, record.mounted))
    }
}

fn prepare_owner_mount(
    path: &Path,
    kind: OwnerKind,
    writable: bool,
    expected: Option<OwnerId>,
) -> Result<runtime::PreparedOwnerMount, ProductError> {
    use runtime::OwnerMountMode::*;
    let mode = match (kind, writable) {
        (OwnerKind::Rel, false) => ReadOnlyRel,
        (OwnerKind::Rel, true) => WritableRel,
        (OwnerKind::Phylactery, false) => ReadOnlyPhy,
        (OwnerKind::Phylactery, true) => WritablePhy,
    };
    Ok(runtime::Gateway::prepare_owner_mount(
        path,
        mode,
        expected.map(Into::into),
    )?)
}

#[path = "host_admission.rs"]
mod admission;
pub use admission::{
    HostAdmissionAdapter, HostAdmissionControl, HostCredentialBinding, TrustedHostAdmission,
};
#[path = "host_instance.rs"]
mod instance;
pub use instance::{ConversationQueue, Instance};

#[path = "host_entity_review.rs"]
mod entity_review;
pub use entity_review::{EntityReviewAuthority, EntityReviewSession};

#[path = "host_context.rs"]
pub(crate) mod context;
#[path = "host_ego.rs"]
pub(crate) mod ego;
#[path = "host_external_source.rs"]
pub(crate) mod external_source;
#[path = "host_project_source.rs"]
pub(crate) mod project_source;
