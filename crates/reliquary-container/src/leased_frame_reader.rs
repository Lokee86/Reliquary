//! Bounded lookups of records indexed by the owner stores.

use super::{Container, ContainerError, FRAME_HEADER_LEN, HEADER_LEN, Mode, ObjectRef, read_at};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, Weak};

#[derive(Debug)]
pub(super) struct ReadLease {
    file: File,
    // Windows positional reads also move the shared system cursor. Exclude
    // them for the full seek-and-append sequence on this lease.
    cursor: Mutex<()>,
    write_fenced: AtomicBool,
}

impl ReadLease {
    pub(super) fn lock(&self) -> Result<MutexGuard<'_, ()>, ContainerError> {
        self.cursor
            .lock()
            .map_err(|_| ContainerError::WriteRecoveryRequired)
    }

    pub(super) fn is_fenced(&self) -> bool {
        self.write_fenced.load(Ordering::Acquire)
    }

    fn fence(&self) {
        self.write_fenced.store(true, Ordering::Release);
    }

    fn read_range(
        &self,
        object: ObjectRef,
        offset: u64,
        length: u64,
        digest: [u8; 32],
    ) -> Result<Vec<u8>, ContainerError> {
        let invalid = || ContainerError::InvalidFrameReference;
        let extent = self.file.metadata()?.len();
        let start = object
            .offset
            .checked_add(FRAME_HEADER_LEN)
            .ok_or_else(invalid)?;
        let end = start.checked_add(object.length).ok_or_else(invalid)?;
        if object.offset < HEADER_LEN
            || end > extent
            || offset
                .checked_add(length)
                .is_none_or(|end| end > object.length)
        {
            return Err(invalid());
        }

        let mut header = [0; FRAME_HEADER_LEN as usize];
        self.read_exact_at(object.offset, &mut header)?;
        if u64::from_le_bytes(header) != object.length {
            return Err(invalid());
        }
        let length = usize::try_from(length).map_err(|_| invalid())?;
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(length).map_err(|_| invalid())?;
        bytes.resize(length, 0);
        self.read_exact_at(start + offset, &mut bytes)?;
        let actual: [u8; 32] = Sha256::digest(&bytes).into();
        if actual != digest || self.file.metadata()?.len() != extent {
            return Err(invalid());
        }
        Ok(bytes)
    }

    fn read_exact_at(&self, mut offset: u64, mut output: &mut [u8]) -> Result<(), ContainerError> {
        while !output.is_empty() {
            match read_at(&self.file, offset, output) {
                Ok(0) => return Err(ContainerError::InvalidFrameReference),
                Ok(read) => {
                    offset += read as u64;
                    output = &mut output[read..];
                }
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(error.into()),
            }
        }
        Ok(())
    }
}

/// A positional reader on an existing lease. Storing one in a projection
/// does not keep the owner file open after its Container has been dropped.
#[derive(Clone, Debug)]
pub struct LeasedFrameReader {
    lease: Weak<ReadLease>,
}

impl LeasedFrameReader {
    pub fn read_frame(
        &self,
        object: ObjectRef,
        digest: [u8; 32],
    ) -> Result<Vec<u8>, ContainerError> {
        self.read_frame_range(object, 0, object.length, digest)
    }

    /// Read a subrange of a validated frame payload. The expected digest
    /// covers the requested range, and the length prefix must still match the
    /// admitted enclosing frame. An invalid reference fences further writes.
    pub fn read_frame_range(
        &self,
        object: ObjectRef,
        payload_offset: u64,
        length: u64,
        digest: [u8; 32],
    ) -> Result<Vec<u8>, ContainerError> {
        let lease = self
            .lease
            .upgrade()
            .ok_or(ContainerError::UnstableSnapshot)?;
        let _guard = lease.lock()?;
        let result = lease.read_range(object, payload_offset, length, digest);
        if result.is_err() {
            lease.fence();
        }
        result
    }

    /// Fence a writer when a domain codec rejects a verified backing record.
    pub fn fence_writes(&self) {
        if let Some(lease) = self.lease.upgrade() {
            lease.fence();
        }
    }
}

impl Container {
    pub fn leased_frame_reader(&self) -> Result<LeasedFrameReader, ContainerError> {
        if self.mode == Mode::Repair {
            return Err(ContainerError::ReadOnly);
        }
        let lease = if let Some(lease) = self.read_lease.get() {
            lease
        } else {
            let file = self.file.try_clone()?;
            self.read_lease.get_or_init(|| {
                Arc::new(ReadLease {
                    file,
                    cursor: Mutex::new(()),
                    write_fenced: AtomicBool::new(false),
                })
            })
        };
        Ok(LeasedFrameReader {
            lease: Arc::downgrade(lease),
        })
    }
}
