//! Exact physical copies prepared from a stable, read-only owner cut.
//!
//! This deliberately does not transform, promote, or delete source owners.

use super::ImportEpochStore;
use crate::staged_copy::StagedCopy;
use reliquary_container::{Container, ContainerIdentity, FileKind};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read},
    path::{Path, PathBuf},
};

const HASH_BUFFER_SIZE: usize = 64 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedOwnerCopyPlan {
    source_owner_id: [u8; 16],
    source_identity: ContainerIdentity,
    source_file_identity: (u64, u64),
    source_latest_version: u64,
    source_len: u64,
    source_digest: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedOwnerCopyReceipt {
    pub owner_id: [u8; 16],
    pub identity: ContainerIdentity,
    pub latest_version: u64,
    pub byte_len: u64,
    pub source_digest: [u8; 32],
    pub copy_digest: [u8; 32],
}

impl VerifiedOwnerCopyPlan {
    /// Select a copy source when one owner is an exact physical prefix of the
    /// other. Divergent histories require semantic reconciliation.
    pub fn prepare_prefix_reconciliation(
        left_path: &Path,
        right_path: &Path,
    ) -> Result<(Self, bool), String> {
        let left_plan = Self::prepare(left_path)?;
        let right_plan = Self::prepare(right_path)?;
        let left = Container::open_read_only(left_path).map_err(|e| e.to_string())?;
        let right = Container::open_read_only(right_path).map_err(|e| e.to_string())?;
        left_plan.verify_source(&left, left_path)?;
        right_plan.verify_source(&right, right_path)?;
        if left.identity() != right.identity() || left.owner_id() != right.owner_id() {
            return Err("reconciliation owners differ".into());
        }
        if crate::owner_history_generation(left_path)?
            != crate::owner_history_generation(right_path)?
        {
            return Err("reconciliation history generations differ".into());
        }
        let mut left = left.physical_cursor().map_err(|e| e.to_string())?;
        let mut right = right.physical_cursor().map_err(|e| e.to_string())?;
        let mut common_frames = 0u64;
        loop {
            match (
                left.next_frame().map_err(|e| e.to_string())?,
                right.next_frame().map_err(|e| e.to_string())?,
            ) {
                (None, Some(_)) => return Ok((right_plan, true)),
                (Some(_), None) | (None, None) => return Ok((left_plan, false)),
                (Some(mut a), Some(mut b)) => {
                    if a.object() != b.object() {
                        return Err(format!(
                            "reconciliation histories diverge after {common_frames} common frames; both owners are retained"
                        ));
                    }
                    let mut remaining = a.object().length;
                    let mut a_bytes = [0u8; HASH_BUFFER_SIZE];
                    let mut b_bytes = [0u8; HASH_BUFFER_SIZE];
                    while remaining != 0 {
                        let len = remaining.min(HASH_BUFFER_SIZE as u64) as usize;
                        a.read_exact(&mut a_bytes[..len])
                            .map_err(|e| e.to_string())?;
                        b.read_exact(&mut b_bytes[..len])
                            .map_err(|e| e.to_string())?;
                        if a_bytes[..len] != b_bytes[..len] {
                            return Err(format!(
                                "reconciliation histories diverge after {common_frames} common frames; both owners are retained"
                            ));
                        }
                        remaining -= len as u64;
                    }
                    common_frames += 1;
                }
            }
        }
    }

    /// Capture owner identity, physical identity, head, length, and a full-file
    /// digest while holding a shared read lease. An active writer is refused.
    pub fn prepare(source_path: impl AsRef<Path>) -> Result<Self, String> {
        let source_path = source_path.as_ref();
        let source = Container::open_read_only(source_path).map_err(|error| error.to_string())?;
        if ImportEpochStore::open(&source)
            .map_err(|error| format!("could not validate import fencing: {error}"))?
            .open_epoch()
            .is_some()
        {
            return Err("owner has an open import epoch".into());
        }
        let (source_digest, source_len) = digest_file(source_path).map_err(|e| e.to_string())?;
        Ok(Self {
            source_owner_id: source.owner_id(),
            source_identity: source.identity(),
            source_file_identity: source
                .physical_file_identity()
                .map_err(|error| error.to_string())?,
            source_latest_version: source.latest_version(),
            source_len,
            source_digest,
        })
    }

    pub fn owner_id(&self) -> [u8; 16] {
        self.source_owner_id
    }

    pub(crate) fn source_digest(&self) -> [u8; 32] {
        self.source_digest
    }

    pub fn file_kind(&self) -> FileKind {
        self.source_identity.file_kind
    }

    pub fn is_reliquary(&self) -> bool {
        self.source_identity.file_kind == FileKind::Reliquary
    }

    pub fn source_latest_version(&self) -> u64 {
        self.source_latest_version
    }

    /// Make an exact byte-for-byte copy at a new destination. The source stays
    /// read leased for the whole operation. The destination is independently
    /// reopened as a Container and its identity, head, length, and digest are
    /// checked before a receipt is returned. No promotion is performed.
    pub fn copy_to(
        &self,
        source_path: impl AsRef<Path>,
        destination_path: impl AsRef<Path>,
    ) -> Result<VerifiedOwnerCopyReceipt, String> {
        let source_path = source_path.as_ref();
        let destination_path = destination_path.as_ref();
        let source = Container::open_read_only(source_path).map_err(|error| error.to_string())?;
        self.verify_source(&source, source_path)?;

        let mut destination_created = false;
        let result = (|| {
            let mut input = File::open(source_path).map_err(|error| error.to_string())?;
            let mut output = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(destination_path)
                .map_err(|error| error.to_string())?;
            destination_created = true;
            io::copy(&mut input, &mut output).map_err(|error| error.to_string())?;
            output.sync_all().map_err(|error| error.to_string())?;
            drop(output);

            let copied =
                Container::open_read_only_as(destination_path, self.source_identity.file_kind)
                    .map_err(|error| error.to_string())?;
            let (copy_digest, byte_len) =
                digest_file(destination_path).map_err(|error| error.to_string())?;
            let copy_file_identity = copied
                .physical_file_identity()
                .map_err(|error| error.to_string())?;
            if copied.identity() != self.source_identity
                || copied.owner_id() != self.source_owner_id
                || copied.latest_version() != self.source_latest_version
                || byte_len != self.source_len
                || copy_digest != self.source_digest
                || copy_file_identity == self.source_file_identity
            {
                return Err(
                    "independently reopened owner copy failed exact identity or hash validation"
                        .into(),
                );
            }
            self.verify_source(&source, source_path)?;
            Ok(VerifiedOwnerCopyReceipt {
                owner_id: copied.owner_id(),
                identity: copied.identity(),
                latest_version: copied.latest_version(),
                byte_len,
                source_digest: self.source_digest,
                copy_digest,
            })
        })();

        if result.is_err() && destination_created {
            let _ = fs::remove_file(destination_path);
        }
        result
    }

    /// Publish a new destination only after the staged copy passes owner admission.
    pub fn copy_to_with_admission<T>(
        &self,
        source: impl AsRef<Path>,
        destination: &Path,
        admit: impl FnOnce(&Path, &VerifiedOwnerCopyReceipt) -> Result<T, String>,
    ) -> Result<(VerifiedOwnerCopyReceipt, T), String> {
        let staged = StagedCopy::create(destination)?;
        let receipt = self.copy_to(source, &staged.path)?;
        let admitted = admit(&staged.path, &receipt)?;
        let publication = Self::prepare(&staged.path)?
            .copy_to(&staged.path, destination)
            .map_err(|error| format!("could not publish verified owner copy: {error}"))?;
        if publication.owner_id != receipt.owner_id
            || publication.identity != receipt.identity
            || publication.byte_len != receipt.byte_len
            || publication.copy_digest != receipt.copy_digest
        {
            let _ = fs::remove_file(destination);
            return Err("published owner differs from the admitted candidate".into());
        }
        Ok((receipt, admitted))
    }

    pub(crate) fn verify_source(&self, source: &Container, path: &Path) -> Result<(), String> {
        if !source.names_same_file(path).map_err(|e| e.to_string())? {
            return Err("source path no longer names the leased owner".into());
        }
        let (digest, len) = digest_reader(source.physical_bytes().map_err(|e| e.to_string())?)
            .map_err(|error| error.to_string())?;
        if source.owner_id() != self.source_owner_id
            || source.identity() != self.source_identity
            || source.physical_file_identity().map_err(|e| e.to_string())?
                != self.source_file_identity
            || source.latest_version() != self.source_latest_version
            || len != self.source_len
            || digest != self.source_digest
        {
            return Err("source owner changed after the maintenance cut was prepared".into());
        }
        Ok(())
    }
}

fn digest_file(path: &Path) -> io::Result<([u8; 32], u64)> {
    digest_reader(File::open(path)?)
}

fn digest_reader(mut file: impl Read) -> io::Result<([u8; 32], u64)> {
    let mut hash = Sha256::new();
    let mut length = 0_u64;
    let mut buffer = [0; HASH_BUFFER_SIZE];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        length = length
            .checked_add(count as u64)
            .ok_or_else(|| io::Error::other("owner file length overflow"))?;
        hash.update(&buffer[..count]);
    }
    Ok((hash.finalize().into(), length))
}

#[cfg(test)]
mod tests {
    use super::*;
    use reliquary_container::{ContainerIdentity, ReliquaryScope};
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT: AtomicU64 = AtomicU64::new(1);

    struct Temp(PathBuf);
    impl Temp {
        fn new(extension: &str) -> Self {
            let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join(".test-tmp");
            fs::create_dir_all(&directory).unwrap();
            Self(directory.join(format!(
                "verified-copy-{}-{}.{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed),
                extension,
            )))
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.0);
        }
    }

    #[test]
    fn prefix_reconciliation_preserves_sources_and_rejects_divergence() {
        let left = Temp::new("phy");
        let right = Temp::new("phy");
        let out = Temp::new("phy");
        let mut owner =
            Container::create(&left.0, ContainerIdentity::phylactery(), [31; 16]).unwrap();
        owner.append(b"common history").unwrap();
        owner.sync().unwrap();
        drop(owner);
        VerifiedOwnerCopyPlan::prepare(&left.0)
            .unwrap()
            .copy_to(&left.0, &right.0)
            .unwrap();
        let mut owner = Container::open_writer(&right.0).unwrap();
        owner.append(b"right continuation").unwrap();
        owner.sync().unwrap();
        drop(owner);
        let (plan, use_right) =
            VerifiedOwnerCopyPlan::prepare_prefix_reconciliation(&left.0, &right.0).unwrap();
        assert!(use_right);
        plan.copy_to(&right.0, &out.0).unwrap();
        assert_eq!(fs::read(&out.0).unwrap(), fs::read(&right.0).unwrap());
        let mut owner = Container::open_writer(&left.0).unwrap();
        owner.append(b"left continuation!").unwrap();
        owner.sync().unwrap();
        drop(owner);
        assert!(VerifiedOwnerCopyPlan::prepare_prefix_reconciliation(&left.0, &right.0).is_err());
    }

    #[test]
    fn exact_copy_preserves_both_owner_kinds_and_reopens_independently() {
        for identity in [
            ContainerIdentity::reliquary(ReliquaryScope::Project),
            ContainerIdentity::phylactery(),
        ] {
            let extension = match identity.file_kind {
                FileKind::Reliquary => "rel",
                FileKind::Phylactery => "phy",
            };
            let source_path = Temp::new(extension);
            let destination_path = Temp::new(extension);
            let owner_id = [31; 16];
            {
                let mut source = Container::create(&source_path.0, identity, owner_id).unwrap();
                source.reserve_version(Some(100)).unwrap();
                source.append(b"owner payload").unwrap();
                source.sync().unwrap();
            }
            let source_bytes = fs::read(&source_path.0).unwrap();
            let plan = VerifiedOwnerCopyPlan::prepare(&source_path.0).unwrap();
            let receipt = plan.copy_to(&source_path.0, &destination_path.0).unwrap();
            assert_eq!(fs::read(&destination_path.0).unwrap(), source_bytes);
            assert_eq!(receipt.owner_id, owner_id);
            assert_eq!(receipt.identity, identity);
            assert_eq!(receipt.latest_version, 1);
            assert_eq!(receipt.source_digest, receipt.copy_digest);
            let reopened =
                Container::open_read_only_as(&destination_path.0, identity.file_kind).unwrap();
            assert_eq!(reopened.owner_id(), owner_id);
            assert_eq!(reopened.latest_version(), 1);
        }
    }

    #[test]
    fn changed_source_and_existing_destination_are_refused_without_overwrite() {
        let source_path = Temp::new("phy");
        let destination_path = Temp::new("phy");
        {
            let mut source =
                Container::create(&source_path.0, ContainerIdentity::phylactery(), [32; 16])
                    .unwrap();
            source.append(b"before").unwrap();
            source.sync().unwrap();
        }
        let stale_plan = VerifiedOwnerCopyPlan::prepare(&source_path.0).unwrap();
        {
            let mut source = Container::open_writer(&source_path.0).unwrap();
            source.append(b"after").unwrap();
            source.sync().unwrap();
        }
        assert!(
            stale_plan
                .copy_to(&source_path.0, &destination_path.0)
                .is_err()
        );
        assert!(!destination_path.0.exists());

        let current_plan = VerifiedOwnerCopyPlan::prepare(&source_path.0).unwrap();
        fs::write(&destination_path.0, b"keep existing destination").unwrap();
        assert!(
            current_plan
                .copy_to(&source_path.0, &destination_path.0)
                .is_err()
        );
        assert_eq!(
            fs::read(&destination_path.0).unwrap(),
            b"keep existing destination"
        );
    }

    #[test]
    fn active_writer_prevents_maintenance_cut_preparation() {
        let source_path = Temp::new("phy");
        let source =
            Container::create(&source_path.0, ContainerIdentity::phylactery(), [33; 16]).unwrap();
        assert!(VerifiedOwnerCopyPlan::prepare(&source_path.0).is_err());
        drop(source);
    }

    #[test]
    fn open_import_epoch_fences_maintenance_cut_preparation() {
        let source_path = Temp::new("rel");
        let owner_id = [34; 16];
        {
            let mut source = Container::create(
                &source_path.0,
                ContainerIdentity::reliquary(ReliquaryScope::Project),
                owner_id,
            )
            .unwrap();
            let key = "ab".repeat(32);
            let mut imports = ImportEpochStore::open(&source).unwrap();
            imports
                .append_start(
                    &mut source,
                    super::super::ImportStart {
                        format_version: 2,
                        owner_uuid: owner_id,
                        import_id: super::super::import_id(owner_id, &key),
                        import_key: key,
                        source_namespace: "source/account".into(),
                        source_format: "fixture-v1".into(),
                        started_at_ns: 10,
                        pre: super::super::ImportWatermarks::default(),
                    },
                )
                .unwrap();
        }
        let error = VerifiedOwnerCopyPlan::prepare(&source_path.0).unwrap_err();
        assert!(error.contains("open import epoch"));
    }
}
