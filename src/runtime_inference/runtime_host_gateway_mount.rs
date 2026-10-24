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
        let canonical_path = std::fs::canonicalize(container.path()).map_err(operation)?;
        Self::validated_identity(
            container.backing_file_identity().map_err(operation)?,
            canonical_path,
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
        Self::validated_identity(
            Container::inspect_file_identity(file).map_err(operation)?,
            canonical_path,
            file_kind,
            owner_id,
            kind,
        )
    }
    fn validated_identity(
        identity: Option<(u64, u64)>,
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
        match kind {
            OwnerKind::Rel => validate_rel_owner_id(&owner_id)?,
            OwnerKind::Phy => validate_owner_id(&owner_id, "phy-")?,
        }
        let key = identity
            .map(|(volume, index)| FileKey::Physical(volume, index))
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
pub(crate) struct MountClaim(String);
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
        OwnerKind::Rel => REL_OWNER_PREFIXES,
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
        let cva =
            crate::RelExecutionOwner::open_mounted(&identity.canonical_path).map_err(operation)?;
        if !identity.matches(&MountedFileIdentity::from_container(
            &cva.container,
            OwnerKind::Rel,
        )?) {
            return Err(operation("REL file changed during mount admission"));
        }
        self.mount_rel_inner(RelExecution::from_owner(cva), identity, claim)
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
        let mut phy = crate::phy_execution_owner::PhyExecutionOwner::open_unvalidated(
            &identity.canonical_path,
        )
        .map_err(operation)?;
        let history = crate::owner_lifecycle::HistorySource::new(&phy.container)
            .map_err(|error| operation(crate::PhylacteryError::Repack(error.to_string())))?;
        crate::history::validate_phylactery_prune_evidence(
            &phy.owner_maintenance,
            &history,
            &phy.graph,
            &phy.memories,
            &phy.entities,
            &phy.relationships,
            &phy.compatibility_profiles,
            &phy.memory_vectors,
            &phy.packed_vectors,
        )
        .map_err(operation)?;
        phy.validate_open_owner().map_err(operation)?;
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
    fn legacy_typed_rel_prefixes_pass_mount_identity_validation() {
        let path = crate::runtime_host_test_support::test_path("legacy-owner.rel");
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap();
        let uuid = uuid::Uuid::new_v4();
        for prefix in REL_OWNER_PREFIXES {
            let owner_id = format!("{prefix}{uuid}");
            let identity = MountedFileIdentity::validated(
                &file,
                PathBuf::from(format!("legacy-{prefix}{uuid}.rel")),
                Some(FileKind::Reliquary),
                Some(owner_id.clone()),
                OwnerKind::Rel,
            )
            .unwrap();
            assert_eq!(identity.owner_id, owner_id);
        }
        drop(file);
        std::fs::remove_file(path).unwrap();
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
