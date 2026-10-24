//! Physical promotion, rollback and lease ownership for a reconciled REL.
use crate::owner_lifecycle::ReliquaryOwner;
use crate::{CvaReconcileError, CvaReconcileResult};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) struct PromotionSource {
    canonical: PathBuf,
    conflicted_guard: crate::Container,
    before: FileFingerprint,
    temp: PathBuf,
    backup: PathBuf,
}
impl PromotionSource {
    pub(crate) fn prepare(canonical: &Path, conflicted: &Path) -> Result<Self, CvaReconcileError> {
        if fs::canonicalize(canonical)? == fs::canonicalize(conflicted)? {
            return Err(CvaReconcileError::PromotionPathsMustDiffer);
        }
        let conflicted_guard = crate::Container::open_read_only_for_scan(conflicted)?;
        conflicted_guard.validate_path_identity()?;
        let before = {
            let mut source = crate::Container::open_read_only(canonical)?;
            fingerprint(&mut source)?
        };
        Ok(Self {
            canonical: canonical.to_owned(),
            conflicted_guard,
            before,
            temp: sibling_path(canonical, "merge.tmp"),
            backup: sibling_path(canonical, "merge.bak"),
        })
    }
    pub(crate) fn candidate_path(&self) -> &Path {
        &self.temp
    }
    pub(crate) fn promote(
        self,
        result: CvaReconcileResult,
        mut candidate: ReliquaryOwner,
    ) -> Result<CvaReconcileResult, CvaReconcileError> {
        let canonical = self.canonical.as_path();
        let temp = self.temp;
        let backup = self.backup;
        let conflicted_guard = self.conflicted_guard;
        let before = self.before;
        if !result.canonical_change_required {
            candidate.container.validate_path_identity()?;
            let _ = fs::remove_file(&temp);
            drop(candidate);
            return Ok(result);
        }

        let mut backup_source = None;
        let promote_result = (|| {
            // Revalidate the exact canonical file under exclusive authority.
            // Candidate and backup retain their own leases across replacement.
            let mut canonical_guard = crate::Container::open(canonical)?;
            if fingerprint(&mut canonical_guard)? != before {
                return Err(CvaReconcileError::CanonicalChangedDuringPromotion);
            }
            candidate.container.sync()?;
            let mut backup_guard = canonical_guard.create_empty_like(&backup)?;
            backup_source = Some(backup_guard.read_source()?);
            {
                let mut frames = canonical_guard.physical_cursor()?;
                while let Some(frame) = frames.next_frame()? {
                    backup_guard.append(&frame.payload)?;
                }
            }
            backup_guard.sync()?;
            if backup_guard.physical_length()? != before.len
                || backup_guard.physical_digest()? != before.digest
            {
                return Err(CvaReconcileError::CanonicalChangedDuringPromotion);
            }
            canonical_guard.validate_path_identity()?;
            conflicted_guard.validate_path_identity()?;
            candidate.container.validate_path_identity()?;
            let parent = canonical.parent().unwrap_or_else(|| Path::new("."));
            atomic_replace(&temp, canonical)?;
            candidate.container.relocate_published_path(canonical)?;
            if let Err(failure) =
                sync_parent(parent).and_then(|_| candidate.container.sync().map_err(Into::into))
            {
                let failure = failure.to_string();
                let recovery = (|| {
                    atomic_replace(&backup, canonical)?;
                    backup_guard.relocate_published_path(canonical)?;
                    sync_parent(parent)?;
                    let restored = crate::maintenance::reconciliation_owner_access::open_reconciliation_container(backup_guard)?;
                    restored.container.sync()?;
                    Ok::<(), CvaReconcileError>(())
                })();
                if let Err(recovery) = recovery {
                    return Err(CvaReconcileError::PromotionRecoveryFailed {
                        failure,
                        recovery: recovery.to_string(),
                    });
                }
                return Err(CvaReconcileError::PromotionFinalizationFailed(failure));
            }
            let _ = fs::remove_file(&backup);
            drop(backup_guard);
            sync_parent(parent)?;
            Ok(())
        })();

        if let Err(error) = promote_result {
            if candidate.container.validate_alias_identity(&temp).is_ok() {
                let _ = fs::remove_file(&temp);
            }
            drop(candidate);
            // A failed restoration may leave the backup as the only intact
            // original. Preserve it for explicit recovery.
            if !matches!(error, CvaReconcileError::PromotionRecoveryFailed { .. })
                && backup_source
                    .as_ref()
                    .is_some_and(|source| source.validate_path_identity().is_ok())
            {
                let _ = fs::remove_file(&backup);
            }
            return Err(error);
        }
        Ok(result)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FileFingerprint {
    len: u64,
    digest: [u8; 32],
    identity: (u64, u64),
}

fn fingerprint(container: &mut crate::Container) -> Result<FileFingerprint, CvaReconcileError> {
    container.validate_path_identity()?;
    let identity = container
        .backing_file_identity()?
        .ok_or(crate::ContainerError::LockingUnsupported)?;
    let len = container.physical_length()?;
    let digest = container.physical_digest()?;
    container.validate_path_identity()?;
    Ok(FileFingerprint {
        len,
        digest,
        identity,
    })
}

fn sibling_path(path: &Path, suffix: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("workspace.cva");
    path.with_file_name(format!(".{name}.{}.{}.{suffix}", std::process::id(), stamp))
}

fn atomic_replace(from: &Path, to: &Path) -> Result<(), CvaReconcileError> {
    crate::atomic_publication::atomic_replace(from, to).map_err(CvaReconcileError::from)
}

fn sync_parent(parent: &Path) -> Result<(), CvaReconcileError> {
    crate::atomic_publication::sync_parent(parent).map_err(CvaReconcileError::from)
}
