use super::{StorageUpgradeError, StorageUpgradePlan, StorageUpgradeRequest, StorageUpgradeResult};
use crate::Container;
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
};

fn reject_existing_output(path: &Path) -> Result<(), StorageUpgradeError> {
    match fs::symlink_metadata(path) {
        Ok(_) => Err(StorageUpgradeError::OutputExists(path.to_path_buf())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

// Streaming, with an optional version normalization solely for copy comparison.
fn digest(file: &File, as_v1: bool) -> Result<([u8; 32], u64), StorageUpgradeError> {
    let mut input = file.try_clone()?;
    let expected_length = input.metadata()?.len();
    input.seek(SeekFrom::Start(0))?;
    let mut input = input.take(expected_length);
    let mut header = [0; 12];
    input.read_exact(&mut header)?;
    if as_v1 {
        header[8..10].copy_from_slice(&1_u16.to_le_bytes());
        header[10..12].copy_from_slice(&0_u16.to_le_bytes());
    }
    let mut hash = Sha256::new();
    hash.update(header);
    let mut length = 12;
    let mut buffer = [0; 64 * 1024];
    loop {
        let count = input.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
        length += count as u64;
    }
    if length != expected_length {
        return Err(StorageUpgradeError::SourceChanged);
    }
    Ok((hash.finalize().into(), length))
}

pub(crate) fn plan_storage_upgrade(
    request: StorageUpgradeRequest,
) -> Result<StorageUpgradePlan, StorageUpgradeError> {
    reject_existing_output(&request.destination)?;
    let source = Container::open_v1_read_only(&request.source)?;
    let identity = source
        .identity()
        .ok_or(StorageUpgradeError::MissingOwnerIdentity)?;
    let owner_uuid = source
        .owner_uuid()
        .ok_or(StorageUpgradeError::MissingOwnerIdentity)?;
    let (source_digest, source_len) = digest(source.backing_file(), false)?;
    Ok(StorageUpgradePlan {
        request,
        source_digest,
        source_len,
        identity,
        owner_uuid,
    })
}

/// Owns a disposable sibling file. There is deliberately no promotion method:
/// a physical copy alone cannot certify the later owner-codec migration.
#[derive(Debug)]
pub(crate) struct StagedStorageUpgrade {
    pub(super) path: PathBuf,
    pub evidence: StorageUpgradeResult,
}
impl StagedStorageUpgrade {
    #[cfg(test)]
    pub(crate) fn test_path(&self) -> &Path {
        &self.path
    }
}
impl Drop for StagedStorageUpgrade {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
        if let Some(directory) = self.path.parent() {
            let _ = fs::remove_dir(directory);
        }
    }
}

pub(crate) fn stage_storage_upgrade(
    plan: &StorageUpgradePlan,
) -> Result<StagedStorageUpgrade, StorageUpgradeError> {
    stage_with_check(plan, || Ok(()))
}

pub(crate) fn stage_memory_storage_upgrade(
    plan: &StorageUpgradePlan,
) -> Result<StagedStorageUpgrade, StorageUpgradeError> {
    stage_impl(plan, || Ok(()), OwnerMigration::Memory)
}

pub(crate) fn stage_entity_storage_upgrade(
    plan: &StorageUpgradePlan,
) -> Result<StagedStorageUpgrade, StorageUpgradeError> {
    stage_impl(plan, || Ok(()), OwnerMigration::MemoryAndEntity)
}

pub(crate) fn stage_relationship_storage_upgrade(
    plan: &StorageUpgradePlan,
) -> Result<StagedStorageUpgrade, StorageUpgradeError> {
    stage_impl(plan, || Ok(()), OwnerMigration::MemoryEntityAndRelationship)
}

pub(crate) fn stage_ego_storage_upgrade(
    plan: &StorageUpgradePlan,
) -> Result<StagedStorageUpgrade, StorageUpgradeError> {
    stage_impl(
        plan,
        || Ok(()),
        OwnerMigration::MemoryEntityRelationshipAndEgo,
    )
}

pub(crate) fn stage_graph_storage_upgrade(
    plan: &StorageUpgradePlan,
) -> Result<StagedStorageUpgrade, StorageUpgradeError> {
    stage_impl(plan, || Ok(()), OwnerMigration::ThroughGraph)
}

enum OwnerMigration {
    None,
    Memory,
    MemoryAndEntity,
    MemoryEntityAndRelationship,
    MemoryEntityRelationshipAndEgo,
    ThroughGraph,
}

// The callback is a bounded test seam for aborts and concurrent source/output
// changes. It does not grant access to either open container handle.
pub(super) fn stage_with_check(
    plan: &StorageUpgradePlan,
    check: impl FnOnce() -> Result<(), StorageUpgradeError>,
) -> Result<StagedStorageUpgrade, StorageUpgradeError> {
    stage_impl(plan, check, OwnerMigration::None)
}

fn stage_impl(
    plan: &StorageUpgradePlan,
    check: impl FnOnce() -> Result<(), StorageUpgradeError>,
    migration: OwnerMigration,
) -> Result<StagedStorageUpgrade, StorageUpgradeError> {
    reject_existing_output(&plan.request.destination)?;
    let mut source = Container::open_v1_read_only(&plan.request.source)?;
    if source.identity() != Some(plan.identity)
        || source.owner_uuid() != Some(plan.owner_uuid)
        || digest(source.backing_file(), false)? != (plan.source_digest, plan.source_len)
    {
        return Err(StorageUpgradeError::SourceChanged);
    }
    let parent = plan
        .request
        .destination
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let directory = parent.join(format!(".storage-upgrade-{}.partial", uuid::Uuid::new_v4()));
    // Own the directory exclusively before arming cleanup; then even a header
    // write failure is cleaned up, and a collision never deletes another file.
    fs::create_dir(&directory)?;
    let path = directory.join("owner.partial");
    let staged = StagedStorageUpgrade {
        path,
        evidence: StorageUpgradeResult {
            identity: plan.identity,
            owner_uuid: plan.owner_uuid,
            source_bytes: plan.source_len,
            global_head: source.latest_version(),
        },
    };
    let mut destination =
        Container::create_with_identity_and_uuid(&staged.path, plan.identity, plan.owner_uuid)?;
    match migration {
        OwnerMigration::Memory => super::memory::transform(&mut source, &mut destination)?,
        OwnerMigration::MemoryAndEntity => {
            super::memory::transform_with_entities(&mut source, &mut destination)?
        }
        OwnerMigration::MemoryEntityAndRelationship => {
            super::memory::transform_with_relationships(&mut source, &mut destination)?
        }
        OwnerMigration::MemoryEntityRelationshipAndEgo => {
            super::memory::transform_with_ego(&mut source, &mut destination)?
        }
        OwnerMigration::ThroughGraph => {
            super::memory::transform_with_graph(&mut source, &mut destination)?
        }
        OwnerMigration::None => {
            source.copy_upgrade_shell_frames(&mut destination)?;
            if digest(destination.backing_file(), true)? != (plan.source_digest, plan.source_len) {
                return Err(StorageUpgradeError::CopyMismatch);
            }
        }
    }
    destination.sync()?;
    drop(destination);
    let reopened = Container::open_compact(&staged.path)?;
    if reopened.identity() != Some(plan.identity)
        || reopened.owner_uuid() != Some(plan.owner_uuid)
        || reopened.latest_version() != staged.evidence.global_head
    {
        return Err(StorageUpgradeError::CopyMismatch);
    }
    drop(reopened);
    check()?;
    if digest(source.backing_file(), false)? != (plan.source_digest, plan.source_len)
        || digest(&File::open(&plan.request.source)?, false)?
            != (plan.source_digest, plan.source_len)
    {
        return Err(StorageUpgradeError::SourceChanged);
    }
    reject_existing_output(&plan.request.destination)?;
    Ok(staged)
}
