//! Coherent held read sources and local physical access exclusion.
use super::{Container, ContainerError, ObjectRef, scan};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::File,
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

const MUTATION_ACTIVE: usize = usize::MAX;

#[derive(Debug)]
pub struct PhysicalReadSource {
    inner: Arc<Container>,
}

impl Clone for PhysicalReadSource {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

#[derive(Debug)]
pub(crate) struct GateGuard {
    gate: Arc<AtomicUsize>,
    writer: bool,
}

impl Drop for GateGuard {
    fn drop(&mut self) {
        if self.writer {
            self.gate.store(0, Ordering::Release);
        } else {
            self.gate.fetch_sub(1, Ordering::Release);
        }
    }
}

impl PhysicalReadSource {
    pub fn file_identity(&self) -> Result<Option<(u64, u64)>, ContainerError> {
        self.inner.file_identity()
    }

    pub fn path(&self) -> &Path {
        self.inner.path()
    }
    pub fn header_len(&self) -> u64 {
        self.inner.header_len
    }
    pub fn physical_length(&self) -> Result<u64, ContainerError> {
        self.inner.physical_length()
    }
    pub fn validate_path_identity(&self) -> Result<(), ContainerError> {
        self.inner.validate_path_identity()
    }

    pub fn physical_cursor(&self) -> Result<scan::PhysicalCursor<'_>, ContainerError> {
        self.inner.physical_cursor()
    }
    pub fn physical_cursor_through(
        &self,
        prefix_end: u64,
    ) -> Result<scan::PhysicalCursor<'_>, ContainerError> {
        self.inner.physical_cursor_through(prefix_end)
    }
    pub fn read(&self, object: ObjectRef) -> Result<Vec<u8>, ContainerError> {
        self.inner.read(object)
    }
}

impl Container {
    /// Returns a path's operating-system identity without exposing its file handle.
    pub fn inspect_file_identity(
        path: impl AsRef<Path>,
    ) -> Result<Option<(u64, u64)>, ContainerError> {
        let file = File::open(path)?;
        super::lease::file_identity(&file).map(|identity| identity.as_pair())
    }

    pub fn file_identity(&self) -> Result<Option<(u64, u64)>, ContainerError> {
        self.lease
            .as_ref()
            .ok_or(ContainerError::ReadOnly)
            .map(super::lease::Lease::identity_pair)
    }

    pub(super) fn read_gate(&self) -> Result<GateGuard, ContainerError> {
        let mut state = self.mutation_gate.load(Ordering::Acquire);
        loop {
            if state == MUTATION_ACTIVE {
                return Err(ContainerError::UnstableSnapshot);
            }
            let next = state
                .checked_add(1)
                .ok_or(ContainerError::UnstableSnapshot)?;
            match self.mutation_gate.compare_exchange_weak(
                state,
                next,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    return Ok(GateGuard {
                        gate: Arc::clone(&self.mutation_gate),
                        writer: false,
                    });
                }
                Err(actual) => state = actual,
            }
        }
    }

    pub(super) fn mutation_gate(&self) -> Result<GateGuard, ContainerError> {
        self.mutation_gate
            .compare_exchange(0, MUTATION_ACTIVE, Ordering::AcqRel, Ordering::Acquire)
            .map(|_| GateGuard {
                gate: Arc::clone(&self.mutation_gate),
                writer: true,
            })
            .map_err(|_| ContainerError::UnstableSnapshot)
    }

    pub fn read_source(&self) -> Result<PhysicalReadSource, ContainerError> {
        if self.lease.is_none() {
            return Err(ContainerError::ReadOnly);
        }
        self.validate_path_identity()?;
        let _guard = self.read_gate()?;
        let file = self.file.try_clone()?;
        let lease = self
            .lease
            .as_ref()
            .expect("writer lease checked")
            .read_view();
        let clone = Self {
            file,
            lease: Some(lease),
            mutation_gate: Arc::clone(&self.mutation_gate),
            path: self.path.clone(),
            version: self.version,
            identity: self.identity,
            owner_uuid: self.owner_uuid,
            header_len: self.header_len,
            // Read sources replay the physical stream independently and do
            // not inherit the writer's potentially unbounded clock history.
            next_version: 1,
            transaction_times: BTreeMap::new(),
            next_transaction_time_override: None,
            write_admission_closed: true,
            scope_active: std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            scope_id: *uuid::Uuid::new_v4().as_bytes(),
            mode: super::SessionMode::Read,
        };
        Ok(PhysicalReadSource {
            inner: Arc::new(clone),
        })
    }

    pub fn physical_length(&self) -> Result<u64, ContainerError> {
        Ok(self.file.metadata()?.len())
    }

    pub fn physical_digest(&self) -> Result<[u8; 32], ContainerError> {
        let _read = self.read_gate()?;
        self.validate_path_identity()?;
        let length = self.file.metadata()?.len();
        let mut offset = 0;
        let mut hash = Sha256::new();
        let mut buf = [0_u8; 64 * 1024];
        while offset < length {
            let count = usize::try_from((length - offset).min(buf.len() as u64)).unwrap();
            self.read_exact_at(&mut buf[..count], offset)?;
            hash.update(&buf[..count]);
            offset += count as u64;
        }
        if self.file.metadata()?.len() != length {
            return Err(ContainerError::UnstableSnapshot);
        }
        self.validate_path_identity()?;
        Ok(hash.finalize().into())
    }

    pub fn validate_path_identity(&self) -> Result<(), ContainerError> {
        self.lease
            .as_ref()
            .ok_or(ContainerError::ReadOnly)?
            .ensure_path_identity(&self.path)
    }

    /// Verifies that a published alias names the held, leased physical file.
    pub fn validate_alias_identity(&self, path: impl AsRef<Path>) -> Result<(), ContainerError> {
        self.lease
            .as_ref()
            .ok_or(ContainerError::ReadOnly)?
            .ensure_path_identity(path.as_ref())
    }

    pub fn relocate_published_path(
        &mut self,
        new_path: impl AsRef<Path>,
    ) -> Result<(), ContainerError> {
        let _mutation = self.mutation_gate()?;
        self.lease
            .as_ref()
            .filter(|lease| lease.is_exclusive())
            .ok_or(ContainerError::ReadOnly)?
            .ensure_path_identity(new_path.as_ref())?;
        self.path = new_path.as_ref().to_path_buf();
        Ok(())
    }
}
