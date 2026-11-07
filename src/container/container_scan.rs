use super::{CHUNK_HEADER_LEN, ChunkRef, Container, ContainerError, ObjectRef, read_header};
use crate::container_version::observe_version_payload_state;
use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io;
use std::path::Path;

/// One opaque physical frame and the global clock observed after accepting it.
pub(crate) struct PhysicalFrame {
    pub object: ObjectRef,
    pub payload: Vec<u8>,
    pub latest_global: u64,
    pub end: u64,
}

/// Caller-driven, bounded physical frame reader. It retains only clock metadata and
/// the current frame; advancing releases the previous payload.
pub(crate) struct PhysicalCursor<'a> {
    container: &'a Container,
    _guard: super::GateGuard,
    offset: u64,
    file_len: u64,
    bounded: bool,
    standalone_reservations: BTreeSet<u64>,
    next_version: u64,
    transaction_times: BTreeMap<u64, i64>,
    max_frame_payload_bytes: Option<u64>,
    max_clock_state_entries: Option<usize>,
    done: bool,
}

impl Container {
    /// Opens a read-only handle for bounded streaming queries. Header validation is
    /// immediate; payload framing and clock validation happen as the cursor advances.
    pub(crate) fn open_read_only_for_scan(path: impl AsRef<Path>) -> Result<Self, ContainerError> {
        let path = path.as_ref();
        let file = std::fs::OpenOptions::new().read(true).open(path)?;
        Self::open_read_only_for_scan_from_file(path, file)
    }

    /// Reads the header from a caller-held read-only file handle. This keeps the
    /// parsed header bound to the exact file object the caller verified and locked.
    pub(crate) fn open_read_only_for_scan_from_file(
        path: impl AsRef<Path>,
        mut file: File,
    ) -> Result<Self, ContainerError> {
        let path = path.as_ref();
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
            owner_write_scope: false,
        })
    }

    pub(crate) fn physical_cursor(&self) -> Result<PhysicalCursor<'_>, ContainerError> {
        self.physical_cursor_through(u64::MAX)
    }

    pub(crate) fn physical_cursor_through(
        &self,
        prefix_end: u64,
    ) -> Result<PhysicalCursor<'_>, ContainerError> {
        self.physical_cursor_with_limits(prefix_end, None, None)
    }

    pub(crate) fn physical_cursor_for_source_witness(
        &self,
        max_frame_payload_bytes: u64,
        max_clock_state_entries: usize,
    ) -> Result<PhysicalCursor<'_>, ContainerError> {
        self.physical_cursor_with_limits(
            u64::MAX,
            Some(max_frame_payload_bytes),
            Some(max_clock_state_entries),
        )
    }

    fn physical_cursor_with_limits(
        &self,
        prefix_end: u64,
        max_frame_payload_bytes: Option<u64>,
        max_clock_state_entries: Option<usize>,
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
            max_frame_payload_bytes,
            max_clock_state_entries,
            done: false,
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

    pub(crate) fn open_writer_validated(path: impl AsRef<Path>) -> Result<Self, ContainerError> {
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
            owner_write_scope: false,
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

    pub(crate) fn open_for_repair(path: impl AsRef<Path>) -> Result<Self, ContainerError> {
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
            write_admission_closed: false,
            owner_write_scope: false,
        };
        container.validate_path_identity()?;
        Ok(container)
    }
}

impl PhysicalCursor<'_> {
    /// Publishes clock state after a caller has consumed the entire physical file
    /// during an owner-open replay. Historical bounded reads must not call this.
    pub(crate) fn finish_open_scan(self) -> Result<ScanClockState, ContainerError> {
        if !self.done || self.offset != self.file_len {
            return Err(ContainerError::InvalidIdentity);
        }
        Ok(ScanClockState {
            next_version: self.next_version,
            transaction_times: self.transaction_times,
        })
    }

    pub(crate) fn next_frame(&mut self) -> Result<Option<PhysicalFrame>, ContainerError> {
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
        self.container.read_exact_at(&mut length_bytes, start)?;
        let len = u64::from_le_bytes(length_bytes);
        if self
            .max_frame_payload_bytes
            .is_some_and(|limit| len > limit)
        {
            return Err(ContainerError::ScanResourceLimitExceeded);
        }
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
        self.container
            .read_exact_at(&mut payload, start + CHUNK_HEADER_LEN)?;
        observe_version_payload_state(
            &mut self.next_version,
            &mut self.transaction_times,
            &mut self.standalone_reservations,
            &payload,
            self.max_clock_state_entries,
        )?;
        self.offset = end;
        Ok(Some(PhysicalFrame {
            object: ObjectRef::from_chunk(ChunkRef { offset: start, len }),
            payload,
            latest_global: self.next_version.saturating_sub(1),
            end,
        }))
    }
}

pub(crate) struct ScanClockState {
    next_version: u64,
    transaction_times: BTreeMap<u64, i64>,
}

impl Container {
    pub(crate) fn apply_scan_clock(&mut self, state: ScanClockState) {
        self.next_version = state.next_version;
        self.transaction_times = state.transaction_times;
    }
}
