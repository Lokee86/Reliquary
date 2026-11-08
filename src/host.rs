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

#[derive(Clone)]
struct MountedOwnerRecord {
    path: PathBuf,
    kind: OwnerKind,
    writable: bool,
    mounted: bool,
}

#[derive(Clone)]
pub struct Gateway {
    pub(crate) inner: runtime::Gateway,
    mounted: Arc<Mutex<BTreeMap<OwnerId, MountedOwnerRecord>>>,
}

impl Default for Gateway {
    fn default() -> Self {
        Self::new()
    }
}

impl Gateway {
    pub fn new() -> Self {
        Self {
            inner: runtime::Gateway::new(),
            mounted: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }

    pub fn mount_rel(&self, rel: &Rel, writable: bool) -> Result<OwnerId, ProductError> {
        let runtime_id = if writable {
            self.inner.mount_writable_reliquary_current(rel.path())?
        } else {
            self.inner.mount_reliquary_current(rel.path())?
        };
        let id = OwnerId::from(runtime_id);
        self.record_mount(id, rel.path(), OwnerKind::Rel, writable)?;
        Ok(id)
    }

    pub fn mount_phylactery(
        &self,
        phy: &Phylactery,
        writable: bool,
    ) -> Result<OwnerId, ProductError> {
        let runtime_id = if writable {
            self.inner.mount_writable_phylactery_current(phy.path())?
        } else {
            self.inner.mount_phylactery_current(phy.path())?
        };
        let id = OwnerId::from(runtime_id);
        self.record_mount(id, phy.path(), OwnerKind::Phylactery, writable)?;
        Ok(id)
    }

    fn record_mount(
        &self,
        id: OwnerId,
        path: &Path,
        kind: OwnerKind,
        writable: bool,
    ) -> Result<(), ProductError> {
        let mut mounted = self.mounted.lock().map_err(|_| {
            ProductError::new(
                ProductErrorKind::Unavailable,
                "mounted owner registry is unavailable",
            )
        })?;
        mounted.insert(
            id,
            MountedOwnerRecord {
                path: path.to_path_buf(),
                kind,
                writable,
                mounted: true,
            },
        );
        Ok(())
    }

    pub(crate) fn quiesce(&self, owner: OwnerId) -> Result<(), ProductError> {
        self.inner.quiesce_owner(owner.into())?;
        let mut mounted = self.mounted.lock().map_err(|_| {
            ProductError::new(
                ProductErrorKind::Unavailable,
                "mounted owner registry is unavailable",
            )
        })?;
        let record = mounted.get_mut(&owner).ok_or_else(|| {
            ProductError::new(ProductErrorKind::Unavailable, "owner is not registered")
        })?;
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
        let reopened = match (record.kind, record.writable) {
            (OwnerKind::Rel, true) => self.inner.mount_writable_reliquary_current(&record.path)?,
            (OwnerKind::Rel, false) => self.inner.mount_reliquary_current(&record.path)?,
            (OwnerKind::Phylactery, true) => {
                self.inner.mount_writable_phylactery_current(&record.path)?
            }
            (OwnerKind::Phylactery, false) => self.inner.mount_phylactery_current(&record.path)?,
        };
        if OwnerId::from(reopened) != owner {
            return Err(ProductError::new(
                ProductErrorKind::Admission,
                "reopened owner identity changed",
            ));
        }
        self.mounted
            .lock()
            .map_err(|_| {
                ProductError::new(
                    ProductErrorKind::Unavailable,
                    "mounted owner registry is unavailable",
                )
            })?
            .get_mut(&owner)
            .expect("registered owner remains present")
            .mounted = true;
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

#[path = "host_admission.rs"]
mod admission;
pub use admission::{
    HostAdmissionAdapter, HostAdmissionControl, HostCredentialBinding, TrustedHostAdmission,
};
#[path = "host_instance.rs"]
mod instance;
pub use instance::{ConversationQueue, Instance};

#[path = "host_context.rs"]
pub(crate) mod context;
#[path = "host_external_source.rs"]
pub(crate) mod external_source;
#[path = "host_project_source.rs"]
pub(crate) mod project_source;
