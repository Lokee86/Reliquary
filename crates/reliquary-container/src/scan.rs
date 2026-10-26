use super::{CHUNK_HEADER_LEN, ChunkRef, Container, ContainerError, ObjectRef, read_header};
use crate::version::observe_version_payload_state;
use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::Path;

/// One opaque physical frame and the global clock observed after accepting it.
pub struct PhysicalFrame {
    pub object: ObjectRef,
    pub offset: u64,
    pub payload: Vec<u8>,
    pub latest_global: u64,
    pub end: u64,
}

/// Caller-driven, bounded physical frame reader. It retains only clock metadata and
/// a fixed-size read buffer and the current frame; advancing releases the previous payload.
pub struct PhysicalCursor<'a> {
    container: &'a Container,
    _guard: super::GateGuard,
    offset: u64,
    file_len: u64,
    bounded: bool,
    standalone_reservations: BTreeSet<u64>,
    next_version: u64,
    transaction_times: BTreeMap<u64, i64>,
    done: bool,
    buffer_start: u64,
    read_buffer: Vec<u8>,
}

impl Container {
    /// Opens a read-only handle for bounded streaming queries. Header validation is
    /// immediate; payload framing and clock validation happen as the cursor advances.
    pub fn open_read_only_for_scan(path: impl AsRef<Path>) -> Result<Self, ContainerError> {
        let path = path.as_ref();
        let mut file = std::fs::OpenOptions::new().read(true).open(path)?;
        let lease = super::lease::Lease::shared(&file)?;
        let (version, identity, owner_uuid, header_len) = read_header(&mut file)?;
        Ok(Self {
            file,
            lease: Some(lease),
            mutation_gate: std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            path: path.to_path_buf(),
            version,
            identity,
            owner_uuid,
            header_len,
            next_version: 1,
            transaction_times: Default::default(),
            next_transaction_time_override: None,
            write_admission_closed: false,
            scope_active: std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            scope_id: *uuid::Uuid::new_v4().as_bytes(),
            mode: super::SessionMode::Read,
        })
    }

    pub fn physical_cursor(&self) -> Result<PhysicalCursor<'_>, ContainerError> {
        self.physical_cursor_through(u64::MAX)
    }

    pub fn physical_cursor_through(
        &self,
        prefix_end: u64,
    ) -> Result<PhysicalCursor<'_>, ContainerError> {
        let guard = self.read_gate()?;
        self.validate_path_identity()?;
        let physical_len = self.file.metadata()?.len();
        if prefix_end != u64::MAX && prefix_end < self.header_len {
            return Err(ContainerError::TruncatedChunk(prefix_end));
        }
        let file_len = physical_len;
        let file_len = if prefix_end == u64::MAX {
            file_len
        } else {
            if prefix_end > file_len {
                return Err(ContainerError::TruncatedChunk(prefix_end));
            }
            prefix_end
        };
        Ok(PhysicalCursor {
            offset: self.header_len,
            file_len,
            bounded: file_len < physical_len,
            container: self,
            _guard: guard,
            standalone_reservations: BTreeSet::new(),
            next_version: 1,
            transaction_times: BTreeMap::new(),
            done: false,
            buffer_start: 0,
            read_buffer: Vec::new(),
        })
    }

    /// Validates the complete physical prefix and reports the start of an incomplete tail.
    /// Complete malformed frames and invalid clock records remain hard errors.
    pub(crate) fn inspect_tail(&self) -> Result<Option<u64>, ContainerError> {
        let mut cursor = self.physical_cursor()?;
        loop {
            match cursor.next_frame() {
                Ok(Some(_)) => {}
                Ok(None) => return Ok(None),
                Err(ContainerError::RepairRequired { valid_end }) => return Ok(Some(valid_end)),
                Err(error) => return Err(error),
            }
        }
    }

    pub(super) fn read_exact_at(
        &self,
        mut buffer: &mut [u8],
        mut offset: u64,
    ) -> Result<(), ContainerError> {
        while !buffer.is_empty() {
            #[cfg(unix)]
            let read = {
                use std::os::unix::fs::FileExt;
                self.file.read_at(buffer, offset)?
            };
            #[cfg(windows)]
            let read = {
                use std::os::windows::fs::FileExt;
                self.file.seek_read(buffer, offset)?
            };
            #[cfg(not(any(unix, windows)))]
            let read = return Err(ContainerError::Io(io::Error::new(
                io::ErrorKind::Unsupported,
                "positional file reads are unsupported on this platform",
            )));
            if read == 0 {
                return Err(ContainerError::Io(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "physical frame changed while reading",
                )));
            }
            offset += read as u64;
            buffer = &mut buffer[read..];
        }
        Ok(())
    }

    /// Opens a coherent shared read session; concurrent writable sessions refuse.
    pub fn open_read_only(path: impl AsRef<Path>) -> Result<Self, ContainerError> {
        let mut container = Self::open_read_only_for_scan(path)?;
        container.validate_path_identity()?;
        let clock_state = {
            let mut cursor = container.physical_cursor()?;
            while cursor.next_frame()?.is_some() {}
            cursor.finish_open_scan()?
        };
        container.apply_scan_clock(clock_state);
        Ok(container)
    }

    pub fn open(path: impl AsRef<Path>) -> Result<Self, ContainerError> {
        Self::open_writer_validated(path.as_ref())
    }

    pub fn open_writer_validated(path: impl AsRef<Path>) -> Result<Self, ContainerError> {
        let path = path.as_ref();
        let mut file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)?;
        let lease = super::lease::Lease::exclusive(&file)?;
        let (version, identity, owner_uuid, header_len) = read_header(&mut file)?;
        let mut container = Self {
            file,
            lease: Some(lease),
            mutation_gate: std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            path: path.to_path_buf(),
            version,
            identity,
            owner_uuid,
            header_len,
            next_version: 1,
            transaction_times: Default::default(),
            next_transaction_time_override: None,
            write_admission_closed: false,
            scope_active: std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            scope_id: *uuid::Uuid::new_v4().as_bytes(),
            mode: super::SessionMode::Writer,
        };
        container.validate_path_identity()?;
        let clock_state = {
            let mut cursor = container.physical_cursor()?;
            while cursor.next_frame()?.is_some() {}
            cursor.finish_open_scan()?
        };
        container.apply_scan_clock(clock_state);
        Ok(container)
    }

    /// Holds writer exclusion while the owner pulls and validates frames in order.
    /// No mutation is admitted until the complete cursor is accepted.
    pub fn open_for_replay(path: impl AsRef<Path>) -> Result<Self, ContainerError> {
        let mut container = Self::open_for_repair(path)?;
        container.mode = super::SessionMode::Replay;
        Ok(container)
    }

    pub fn open_for_repair(path: impl AsRef<Path>) -> Result<Self, ContainerError> {
        let path = path.as_ref();
        let mut file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)?;
        let lease = super::lease::Lease::exclusive(&file)?;
        let (version, identity, owner_uuid, header_len) = read_header(&mut file)?;
        let container = Self {
            file,
            lease: Some(lease),
            mutation_gate: std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            path: path.to_path_buf(),
            version,
            identity,
            owner_uuid,
            header_len,
            next_version: 1,
            transaction_times: Default::default(),
            next_transaction_time_override: None,
            write_admission_closed: true,
            scope_active: std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            scope_id: *uuid::Uuid::new_v4().as_bytes(),
            mode: super::SessionMode::Repair,
        };
        container.validate_path_identity()?;
        Ok(container)
    }
}

impl PhysicalCursor<'_> {
    // Keep sequential replay buffered without sharing or changing the file cursor.
    // Refills stop at this cursor's frozen prefix, including when the next frame
    // lies outside the caller's accepted boundary.
    fn read_frame_bytes(
        &mut self,
        mut output: &mut [u8],
        mut offset: u64,
    ) -> Result<(), ContainerError> {
        const BUFFER_BYTES: u64 = 64 * 1024;
        while !output.is_empty() {
            let cached_end = self.buffer_start + self.read_buffer.len() as u64;
            if offset < self.buffer_start || offset >= cached_end {
                let remaining = self
                    .file_len
                    .checked_sub(offset)
                    .ok_or(ContainerError::InvalidPrefixBoundary)?;
                let length = remaining.min(BUFFER_BYTES) as usize;
                if length == 0 {
                    return Err(ContainerError::TruncatedChunk(offset));
                }
                self.read_buffer.resize(length, 0);
                self.container
                    .read_exact_at(&mut self.read_buffer, offset)?;
                self.buffer_start = offset;
            }
            let position = (offset - self.buffer_start) as usize;
            let take = output.len().min(self.read_buffer.len() - position);
            output[..take].copy_from_slice(&self.read_buffer[position..position + take]);
            offset += take as u64;
            output = &mut output[take..];
        }
        Ok(())
    }

    /// Publishes clock state after a caller has consumed the entire physical file
    /// during an owner-open replay. Historical bounded reads must not call this.
    pub fn finish_open_scan(self) -> Result<ScanClockState, ContainerError> {
        if !self.done
            || self.bounded
            || self.offset != self.file_len
            || self.container.physical_length()? != self.file_len
        {
            return Err(ContainerError::InvalidIdentity);
        }
        Ok(ScanClockState {
            lease_token: self
                .container
                .lease
                .as_ref()
                .ok_or(ContainerError::ReadOnly)?
                .token(),
            file_len: self.file_len,
            next_version: self.next_version,
            transaction_times: self.transaction_times,
        })
    }

    pub fn next_frame(&mut self) -> Result<Option<PhysicalFrame>, ContainerError> {
        if self.done || self.offset == self.file_len {
            self.done = true;
            return Ok(None);
        }
        if self.offset > self.file_len || self.file_len - self.offset < CHUNK_HEADER_LEN {
            self.done = true;
            if self.bounded {
                return Err(ContainerError::TruncatedChunk(self.file_len));
            }
            return Err(ContainerError::RepairRequired {
                valid_end: self.offset,
            });
        }
        let start = self.offset;
        let mut length_bytes = [0_u8; CHUNK_HEADER_LEN as usize];
        self.read_frame_bytes(&mut length_bytes, start)?;
        let len = u64::from_le_bytes(length_bytes);
        let end = start
            .checked_add(CHUNK_HEADER_LEN)
            .and_then(|v| v.checked_add(len))
            .ok_or(ContainerError::ChunkTooLarge)?;
        if end > self.file_len {
            self.done = true;
            if self.bounded {
                return Err(ContainerError::TruncatedChunk(start));
            }
            return Err(ContainerError::RepairRequired { valid_end: start });
        }
        let payload_len = usize::try_from(len).map_err(|_| ContainerError::ChunkTooLarge)?;
        let mut payload = vec![0; payload_len];
        self.read_frame_bytes(&mut payload, start + CHUNK_HEADER_LEN)?;
        observe_version_payload_state(
            &mut self.next_version,
            &mut self.transaction_times,
            &mut self.standalone_reservations,
            &payload,
        )?;
        self.offset = end;
        Ok(Some(PhysicalFrame {
            object: ObjectRef::from_chunk(ChunkRef { offset: start, len }),
            offset: start,
            payload,
            latest_global: self.next_version.saturating_sub(1),
            end,
        }))
    }
}

pub struct ScanClockState {
    lease_token: [u8; 16],
    file_len: u64,
    next_version: u64,
    transaction_times: BTreeMap<u64, i64>,
}

impl Container {
    /// Accepts only a fully consumed scan from this held replay session.
    pub fn accept_replay_scan(&mut self, state: ScanClockState) -> Result<(), ContainerError> {
        self.require_writer_lease()?;
        if self.mode != super::SessionMode::Replay
            || self.lease.as_ref().unwrap().token() != state.lease_token
            || self.physical_length()? != state.file_len
        {
            return Err(ContainerError::UnstableSnapshot);
        }
        self.apply_scan_clock(state);
        self.mode = super::SessionMode::Writer;
        self.write_admission_closed = false;
        Ok(())
    }

    pub(crate) fn apply_scan_clock(&mut self, state: ScanClockState) {
        self.next_version = state.next_version;
        self.transaction_times = state.transaction_times;
    }
}
