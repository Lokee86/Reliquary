//! Verification of an admitted physical prefix and its append-only suffix.

use super::{
    Container, ContainerError, ContainerIdentity, FileKind, FormatVersion, HEADER_LEN, Mode,
    PhysicalClock, PositionedFile,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::Path;

const HASH_BUFFER_SIZE: usize = 1024 * 1024;

/// Version allocation and timestamp facts retained across a physical cut.
/// A version absent from the timestamp map has no recorded time.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CheckpointClock {
    pub next_version: u64,
    pub transaction_times: BTreeMap<u64, i64>,
    pub standalone_reservations: BTreeSet<u64>,
}

impl CheckpointClock {
    fn has_valid_frontier(&self) -> bool {
        self.next_version > 0
            && self
                .transaction_times
                .keys()
                .chain(&self.standalone_reservations)
                .all(|&version| (1..self.next_version).contains(&version))
    }
}

/// Physical state from a previously validated cut. Owner-session stores this
/// inside its private, versioned checkpoint.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PhysicalCheckpoint {
    pub identity: ContainerIdentity,
    pub version: FormatVersion,
    pub owner_id: [u8; 16],
    pub file_identity: (u64, u64),
    pub covered_end: u64,
    pub digest: [u8; 32],
    pub clock: CheckpointClock,
}

/// Proof and incremental hash state for a prefix on this particular lease.
pub struct VerifiedPrefix {
    lease_token: [u8; 16],
    covered_end: u64,
    hash: Sha256,
}

impl VerifiedPrefix {
    pub fn covered_end(&self) -> u64 {
        self.covered_end
    }
}

/// A leased, header-checked file. The append and read APIs become available
/// after validating the complete extent, with or without a checkpoint.
pub struct LeasedContainer {
    container: Container,
    extent: u64,
}

impl LeasedContainer {
    pub fn path(&self) -> &Path {
        self.container.path()
    }

    pub fn identity(&self) -> ContainerIdentity {
        self.container.identity()
    }

    pub fn owner_id(&self) -> [u8; 16] {
        self.container.owner_id()
    }

    pub fn physical_file_identity(&self) -> Result<(u64, u64), ContainerError> {
        self.container.physical_file_identity()
    }

    pub fn extent(&self) -> u64 {
        self.extent
    }

    /// A supplied checkpoint must have passed its private cache integrity and
    /// schema checks. An unusable physical prefix restarts validation at the
    /// native header with the same lease.
    pub fn finish(
        mut self,
        checkpoint: Option<PhysicalCheckpoint>,
    ) -> Result<(Container, Option<VerifiedPrefix>), ContainerError> {
        let mut verified = None;
        let mut clock = PhysicalClock::default();
        if let Some(checkpoint) = checkpoint
            && let Some(prefix) = self.verify_prefix(&checkpoint)?
        {
            clock = PhysicalClock::from_checkpoint(checkpoint.clock);
            verified = Some(prefix);
        }

        let from = verified
            .as_ref()
            .map_or(HEADER_LEN, VerifiedPrefix::covered_end);
        self.container
            .validate_extent_from(from, self.extent, clock)?;
        Ok((self.container, verified))
    }

    fn verify_prefix(
        &self,
        checkpoint: &PhysicalCheckpoint,
    ) -> Result<Option<VerifiedPrefix>, ContainerError> {
        let container = &self.container;
        if checkpoint.identity != container.identity()
            || checkpoint.version != container.version()
            || checkpoint.owner_id != container.owner_id()
            || !(HEADER_LEN..=self.extent).contains(&checkpoint.covered_end)
            || !checkpoint.clock.has_valid_frontier()
            || (checkpoint.covered_end == HEADER_LEN && checkpoint.clock.next_version != 1)
            || checkpoint.file_identity != container.physical_file_identity()?
        {
            return Ok(None);
        }
        container.require_extent(self.extent)?;
        let hash = container.hash_physical_bytes(0, checkpoint.covered_end, Sha256::new())?;
        container.require_extent(self.extent)?;
        let digest: [u8; 32] = hash.clone().finalize().into();
        if digest != checkpoint.digest {
            return Ok(None);
        }
        Ok(Some(VerifiedPrefix {
            lease_token: container.lease_token,
            covered_end: checkpoint.covered_end,
            hash,
        }))
    }
}

impl Container {
    pub fn prepare_checkpoint_open(
        path: impl AsRef<Path>,
        writable: bool,
        kind: FileKind,
        owner: Option<[u8; 16]>,
    ) -> Result<LeasedContainer, ContainerError> {
        let mode = if writable { Mode::Write } else { Mode::Read };
        let container = Self::open_leased(path.as_ref(), mode, Some(kind), owner)?;
        let extent = container.extent()?;
        Ok(LeasedContainer { container, extent })
    }

    pub fn extent(&self) -> Result<u64, ContainerError> {
        Ok(self.file.metadata()?.len())
    }

    pub fn require_extent(&self, extent: u64) -> Result<(), ContainerError> {
        if self.extent()? != extent {
            return Err(ContainerError::UnstableSnapshot);
        }
        Ok(())
    }

    /// Run the authoritative physical checks from the header on the same file.
    pub fn validate_from_start(&mut self) -> Result<(), ContainerError> {
        self.validate_extent_from(HEADER_LEN, self.extent()?, PhysicalClock::default())
    }

    fn validate_extent_from(
        &mut self,
        offset: u64,
        extent: u64,
        clock: PhysicalClock,
    ) -> Result<(), ContainerError> {
        self.validate_frames_from(offset, extent)?;
        self.rebuild_clock_from(offset, extent, clock)?;
        self.require_extent(extent)
    }

    /// Capture a synced, semantically admitted cut. Reuse hash state only if
    /// it came from this lease and still covers a prefix of the captured extent.
    pub fn capture_checkpoint(
        &self,
        previous: Option<&VerifiedPrefix>,
    ) -> Result<(PhysicalCheckpoint, VerifiedPrefix), ContainerError> {
        self.require_write()?;
        if self.synced_epoch != self.write_epoch {
            return Err(ContainerError::WriteRecoveryRequired);
        }
        let covered_end = self.extent()?;
        let (from, hash) = previous
            .filter(|old| old.lease_token == self.lease_token && old.covered_end <= covered_end)
            .map_or_else(
                || (0, Sha256::new()),
                |old| (old.covered_end, old.hash.clone()),
            );
        let hash = self.hash_physical_bytes(from, covered_end, hash)?;
        self.require_extent(covered_end)?;
        let digest = hash.clone().finalize().into();
        Ok((
            PhysicalCheckpoint {
                identity: self.identity(),
                version: self.version(),
                owner_id: self.owner_id(),
                file_identity: self.physical_file_identity()?,
                covered_end,
                digest,
                clock: CheckpointClock {
                    next_version: self.next_version,
                    transaction_times: self.transaction_times.clone(),
                    standalone_reservations: self.standalone_reservations.clone(),
                },
            },
            VerifiedPrefix {
                lease_token: self.lease_token,
                covered_end,
                hash,
            },
        ))
    }

    fn hash_physical_bytes(
        &self,
        offset: u64,
        end: u64,
        mut hash: Sha256,
    ) -> Result<Sha256, ContainerError> {
        if offset == end {
            return Ok(hash);
        }
        let mut file = PositionedFile {
            file: self.file.try_clone()?,
            offset,
        };
        let mut remaining = end - offset;
        let mut bytes = vec![0; HASH_BUFFER_SIZE.min(remaining.try_into().unwrap_or(usize::MAX))];
        while remaining > 0 {
            let len = remaining.min(bytes.len() as u64) as usize;
            file.read_exact(&mut bytes[..len]).map_err(|error| {
                if error.kind() == std::io::ErrorKind::UnexpectedEof {
                    ContainerError::UnstableSnapshot
                } else {
                    ContainerError::Io(error)
                }
            })?;
            hash.update(&bytes[..len]);
            remaining -= len as u64;
        }
        Ok(hash)
    }
}
