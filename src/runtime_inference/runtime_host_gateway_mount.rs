//! Gateway file admission is read-only until identity and exclusive execution are established.
use super::*;
use crate::{Container, FileKind};
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum FileKey {
    Physical(u64, u64),
    PathScoped(PathBuf),
}

#[derive(Clone, Debug)]
pub(super) struct MountedFileIdentity {
    pub owner_id: String,
    pub kind: OwnerKind,
    pub key: FileKey,
    pub canonical_path: PathBuf,
}
impl MountedFileIdentity {
    pub(super) fn inspect(path: &Path, kind: OwnerKind) -> Result<Self, ReliquaryRuntimeHostError> {
        let canonical_path = std::fs::canonicalize(path).map_err(operation)?;
        let mut file = File::open(&canonical_path).map_err(operation)?;
        let (identity, owner_id) = Container::inspect_header(&mut file).map_err(operation)?;
        Self::validated(
            &file,
            canonical_path,
            identity.map(|i| i.file_kind),
            owner_id,
            kind,
        )
    }
    pub(super) fn from_container(
        container: &Container,
        kind: OwnerKind,
    ) -> Result<Self, ReliquaryRuntimeHostError> {
        Self::validated(
            container.backing_file(),
            std::fs::canonicalize(container.path()).map_err(operation)?,
            container.identity().map(|i| i.file_kind),
            container.owner_id(),
            kind,
        )
    }
    fn validated(
        file: &File,
        canonical_path: PathBuf,
        file_kind: Option<FileKind>,
        owner_id: Option<String>,
        kind: OwnerKind,
    ) -> Result<Self, ReliquaryRuntimeHostError> {
        let expected = match kind {
            OwnerKind::Rel => FileKind::Reliquary,
            OwnerKind::Phy => FileKind::Phylactery,
        };
        if file_kind != Some(expected) {
            return Err(operation(
                "gateway mount requires the exact typed owner kind",
            ));
        }
        let owner_id =
            owner_id.ok_or_else(|| operation("gateway mount requires a durable owner UUID"))?;
        validate_owner_id(
            &owner_id,
            match kind {
                OwnerKind::Rel => "rel-",
                OwnerKind::Phy => "phy-",
            },
        )?;
        let key = physical_key(file)
            .map_err(operation)?
            .unwrap_or_else(|| FileKey::PathScoped(canonical_path.clone()));
        Ok(Self {
            owner_id,
            kind,
            key,
            canonical_path,
        })
    }
    fn matches(&self, other: &Self) -> bool {
        self.owner_id == other.owner_id && self.kind == other.kind && self.key == other.key
    }
}

// Claims exclude concurrent Gateway executions in this process. They do not govern
// arbitrary direct file clients or other processes. Gateway owns the semantic registry.
#[derive(Clone, Debug)]
struct ClaimIdentity {
    key: FileKey,
    canonical_path: PathBuf,
}
static LIVE_MOUNTS: OnceLock<Mutex<BTreeMap<String, ClaimIdentity>>> = OnceLock::new();
pub(super) struct MountClaim(String);
impl MountClaim {
    pub(super) fn acquire(
        identity: &MountedFileIdentity,
    ) -> Result<Self, ReliquaryRuntimeHostError> {
        Self::acquire_identity(identity)
    }

    fn acquire_identity(identity: &MountedFileIdentity) -> Result<Self, ReliquaryRuntimeHostError> {
        let registry_id = claim_registry_id(identity);
        let mut live = LIVE_MOUNTS
            .get_or_init(|| Mutex::new(BTreeMap::new()))
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        if live.contains_key(&registry_id)
            || live.values().any(|claim| {
                claim.key == identity.key || claim.canonical_path == identity.canonical_path
            })
        {
            return Err(operation("owner already has a live execution"));
        }
        live.insert(
            registry_id.clone(),
            ClaimIdentity {
                key: identity.key.clone(),
                canonical_path: identity.canonical_path.clone(),
            },
        );
        Ok(Self(registry_id))
    }
}
fn claim_registry_id(identity: &MountedFileIdentity) -> String {
    let prefixes: &[&str] = match identity.kind {
        OwnerKind::Rel => &["rel-", "proj-", "org-", "con-"],
        OwnerKind::Phy => &["phy-"],
    };
    for prefix in prefixes {
        if let Some(suffix) = identity.owner_id.strip_prefix(prefix)
            && let Ok(uuid) = uuid::Uuid::parse_str(suffix)
        {
            return format!(
                "{}-{uuid}",
                if identity.kind == OwnerKind::Rel {
                    "rel"
                } else {
                    "phy"
                }
            );
        }
    }
    identity.owner_id.clone()
}
impl Drop for MountClaim {
    fn drop(&mut self) {
        if let Some(live) = LIVE_MOUNTS.get() {
            if let Ok(mut live) = live.lock() {
                live.remove(&self.0);
            }
        }
    }
}
#[cfg(unix)]
fn physical_key(file: &File) -> std::io::Result<Option<FileKey>> {
    use std::os::unix::fs::MetadataExt;
    let metadata = file.metadata()?;
    Ok(Some(FileKey::Physical(metadata.dev(), metadata.ino())))
}
#[cfg(windows)]
fn physical_key(file: &File) -> std::io::Result<Option<FileKey>> {
    use std::os::windows::io::AsRawHandle;
    // BY_HANDLE_FILE_INFORMATION: thirteen DWORDs, including three FILETIMEs.
    #[repr(C)]
    struct Information {
        attributes: u32,
        creation: [u32; 2],
        access: [u32; 2],
        write: [u32; 2],
        volume: u32,
        size_high: u32,
        size_low: u32,
        links: u32,
        index_high: u32,
        index_low: u32,
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetFileInformationByHandle(
            handle: *mut std::ffi::c_void,
            information: *mut Information,
        ) -> i32;
    }
    let mut information = std::mem::MaybeUninit::<Information>::uninit();
    // SAFETY: the live File owns this handle and the OS writes this repr(C) structure.
    if unsafe { GetFileInformationByHandle(file.as_raw_handle(), information.as_mut_ptr()) } == 0 {
        return Ok(None);
    }
    // SAFETY: success initializes every field.
    let information = unsafe { information.assume_init() };
    let index = (u64::from(information.index_high) << 32) | u64::from(information.index_low);
    Ok((index != 0).then_some(FileKey::Physical(u64::from(information.volume), index)))
}
#[cfg(not(any(unix, windows)))]
fn physical_key(_: &File) -> std::io::Result<Option<FileKey>> {
    Ok(None)
}

impl GatewayRuntime {
    /// Reuse aliases before opening another mutable owner; reject conflicting UUID copies.
    pub fn mount_rel_path(
        &self,
        path: impl AsRef<Path>,
    ) -> Result<String, ReliquaryRuntimeHostError> {
        let _lifecycle = self
            .lifecycle
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        let _topology = self
            .topology_transition
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        let identity = MountedFileIdentity::inspect(path.as_ref(), OwnerKind::Rel)?;
        if self.existing_file_mount(&identity)? {
            return Ok(identity.owner_id);
        }
        let claim = MountClaim::acquire(&identity)?;
        let cva = crate::Cva::open(&identity.canonical_path).map_err(operation)?;
        if !identity.matches(&MountedFileIdentity::from_container(
            &cva.container,
            OwnerKind::Rel,
        )?) {
            return Err(operation("REL file changed during mount admission"));
        }
        self.mount_rel_inner(InteractionRuntime::new(cva), identity, claim)
    }
    pub fn mount_phy_path(
        &self,
        path: impl AsRef<Path>,
    ) -> Result<String, ReliquaryRuntimeHostError> {
        let _lifecycle = self
            .lifecycle
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        let _topology = self
            .topology_transition
            .lock()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        let identity = MountedFileIdentity::inspect(path.as_ref(), OwnerKind::Phy)?;
        if self.existing_file_mount(&identity)? {
            return Ok(identity.owner_id);
        }
        let claim = MountClaim::acquire(&identity)?;
        let phy = Phylactery::open(&identity.canonical_path).map_err(operation)?;
        if !identity.matches(&MountedFileIdentity::from_container(
            &phy.container,
            OwnerKind::Phy,
        )?) {
            return Err(operation("PHY file changed during mount admission"));
        }
        self.mount_phy_inner(phy, identity, claim)
    }
    pub(super) fn existing_file_mount(
        &self,
        candidate: &MountedFileIdentity,
    ) -> Result<bool, ReliquaryRuntimeHostError> {
        if self.shutting_down.load(Ordering::SeqCst) {
            return Err(operation("gateway is shutting down"));
        }
        let rels = self
            .rel_executions
            .read()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        let phys = self
            .phy_executions
            .read()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        for mounted in rels
            .values()
            .map(|r| &r.file_identity)
            .chain(phys.values().map(|p| &p.file_identity))
        {
            if mounted.key == candidate.key {
                if mounted.matches(candidate) {
                    return Ok(true);
                }
                return Err(operation(
                    "mounted physical file reports inconsistent owner identity",
                ));
            }
            if mounted.owner_id == candidate.owner_id {
                return Err(operation(
                    "owner UUID is mounted from a different file; reconcile the copy offline",
                ));
            }
            if mounted.canonical_path == candidate.canonical_path {
                return Err(operation(
                    "mounted path was replaced; quiesce and unmount before replacement",
                ));
            }
        }
        Ok(false)
    }
    /// A true result explicitly limits alias deduplication to canonical path identity.
    pub fn owner_mount_is_path_scoped(
        &self,
        owner_id: &str,
    ) -> Result<bool, ReliquaryRuntimeHostError> {
        let rels = self
            .rel_executions
            .read()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        if let Some(rel) = rels.get(owner_id) {
            return Ok(matches!(rel.file_identity.key, FileKey::PathScoped(_)));
        }
        drop(rels);
        let phys = self
            .phy_executions
            .read()
            .map_err(|_| ReliquaryRuntimeHostError::LockPoisoned)?;
        let phy = phys
            .get(owner_id)
            .ok_or_else(|| operation("owner is not mounted"))?;
        Ok(matches!(phy.file_identity.key, FileKey::PathScoped(_)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity(owner_id: String, kind: OwnerKind, path: &str) -> MountedFileIdentity {
        MountedFileIdentity {
            owner_id,
            kind,
            key: FileKey::PathScoped(PathBuf::from(path)),
            canonical_path: PathBuf::from(path),
        }
    }

    #[test]
    fn legacy_typed_rel_prefixes_collide_with_canonical_rel_uuid_claims() {
        let uuid = uuid::Uuid::new_v4();
        for prefix in ["proj-", "org-", "con-"] {
            let legacy = identity(
                format!("{prefix}{uuid}"),
                OwnerKind::Rel,
                &format!("legacy-{prefix}{uuid}.rel"),
            );
            let canonical_copy = identity(
                format!("rel-{uuid}"),
                OwnerKind::Rel,
                &format!("canonical-{uuid}.rel"),
            );
            let claim = MountClaim::acquire(&legacy).unwrap();
            assert!(
                MountClaim::acquire(&canonical_copy).is_err(),
                "legacy prefix {prefix} must share the canonical REL UUID claim"
            );
            drop(claim);
        }
    }

    #[test]
    fn replacing_a_claimed_canonical_path_requires_unmount() {
        let first = identity(
            format!("rel-{}", uuid::Uuid::new_v4()),
            OwnerKind::Rel,
            "same.rel",
        );
        let mut replacement = identity(
            format!("rel-{}", uuid::Uuid::new_v4()),
            OwnerKind::Rel,
            "same.rel",
        );
        replacement.key = FileKey::PathScoped(PathBuf::from("different-file-at-same-path"));
        let claim = MountClaim::acquire(&first).unwrap();
        assert!(MountClaim::acquire(&replacement).is_err());
        drop(claim);
        assert!(MountClaim::acquire(&replacement).is_ok());
    }

    #[test]
    fn rel_and_phy_claim_namespaces_remain_distinct() {
        let uuid = uuid::Uuid::new_v4();
        let rel = identity(
            format!("rel-{uuid}"),
            OwnerKind::Rel,
            &format!("rel-{uuid}.rel"),
        );
        let phy = identity(
            format!("phy-{uuid}"),
            OwnerKind::Phy,
            &format!("phy-{uuid}.phy"),
        );
        let rel_claim = MountClaim::acquire(&rel).unwrap();
        let phy_claim = MountClaim::acquire(&phy).unwrap();
        drop((rel_claim, phy_claim));
    }
}
