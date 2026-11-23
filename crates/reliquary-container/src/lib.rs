//! Physical REL/PHY container framing for the canonical 2.1 format.
//!
//! This crate intentionally admits one signature/version pair. Format conversion
//! belongs in a separate offline program, never in normal container open paths.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use uuid::Uuid;

const MAGIC: [u8; 8] = *b"REL\0\r\n\x1a\n";
const HEADER_LEN: u64 = 40;
const FRAME_HEADER_LEN: u64 = 8;
const SCAN_BUFFER_SIZE: usize = 64 * 1024;
const CURRENT_VERSION: FormatVersion = FormatVersion { major: 2, minor: 1 };

mod checkpoint;
pub use checkpoint::{CheckpointClock, LeasedContainer, PhysicalCheckpoint, VerifiedPrefix};
mod grouped_clock;
mod leased_frame_reader;
pub use leased_frame_reader::LeasedFrameReader;
use leased_frame_reader::ReadLease;
mod publication_envelope;
pub use publication_envelope::{PublicationEnvelope, decode_publication, replace_publication_body};
mod version_clock;
pub use version_clock::PhysicalClock;

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FormatVersion {
    pub major: u16,
    pub minor: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum FileKind {
    Reliquary,
    Phylactery,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ReliquaryScope {
    Unspecified,
    Organization,
    Project,
    Connection,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ContainerIdentity {
    pub file_kind: FileKind,
    pub scope: ReliquaryScope,
}

/// Opaque proof tying an owner session to one live Container writer lease and
/// its current append epoch. Only Container can issue a valid proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WriterLeaseProof {
    token: [u8; 16],
    epoch: u64,
}

impl ContainerIdentity {
    pub const fn reliquary(scope: ReliquaryScope) -> Self {
        Self {
            file_kind: FileKind::Reliquary,
            scope,
        }
    }

    pub const fn phylactery() -> Self {
        Self {
            file_kind: FileKind::Phylactery,
            scope: ReliquaryScope::Unspecified,
        }
    }
}

#[derive(Debug)]
pub enum ContainerError {
    Io(io::Error),
    InvalidSignature,
    UnsupportedFormatSignature,
    UnsupportedVersion(FormatVersion),
    InvalidHeaderLength(u32),
    InvalidIdentity,
    WrongFileKind {
        expected: FileKind,
        actual: FileKind,
    },
    WrongOwner,
    TruncatedHeader,
    TruncatedFrame {
        offset: u64,
    },
    InvalidFrameLength {
        offset: u64,
        length: u64,
    },
    InvalidFrameReference,
    RepairRequired {
        valid_end: u64,
    },
    WriteRecoveryRequired,
    UnstableSnapshot,
    WriterConflict,
    ReadOnly,
    UnstableRepairPlan,
    InvalidRepairBoundary,
    InvalidVersionRecord,
    VersionExhausted,
    InvalidTransactionTime,
    ChunkTooLarge,
}

impl fmt::Display for ContainerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "container I/O error: {e}"),
            Self::InvalidSignature => f.write_str("invalid REL container signature"),
            Self::UnsupportedFormatSignature => f.write_str("unsupported container signature"),
            Self::UnsupportedVersion(v) => {
                write!(f, "unsupported REL format {}.{}", v.major, v.minor)
            }
            Self::InvalidHeaderLength(n) => write!(f, "invalid container header length {n}"),
            Self::InvalidIdentity => f.write_str("invalid container identity"),
            Self::WrongFileKind { expected, actual } => write!(
                f,
                "wrong container kind: expected {expected:?}, found {actual:?}"
            ),
            Self::WrongOwner => f.write_str("container owner does not match the requested owner"),
            Self::TruncatedHeader => f.write_str("truncated container header"),
            Self::TruncatedFrame { offset } => write!(f, "truncated frame at {offset}"),
            Self::InvalidFrameLength { offset, length } => {
                write!(f, "invalid frame length {length} at {offset}")
            }
            Self::InvalidFrameReference => f.write_str("indexed frame payload verification failed"),
            Self::RepairRequired { valid_end } => write!(
                f,
                "incomplete tail requires explicit repair after {valid_end}"
            ),
            Self::WriteRecoveryRequired => {
                f.write_str("a prior append failed; reopen and inspect or repair before writing")
            }
            Self::UnstableSnapshot => f.write_str("container changed while it was being read"),
            Self::WriterConflict => f.write_str("container has an incompatible active lease"),
            Self::ReadOnly => f.write_str("container was opened read-only"),
            Self::UnstableRepairPlan => {
                f.write_str("repair evidence is stale or belongs to another lease")
            }
            Self::InvalidRepairBoundary => {
                f.write_str("repair boundary is outside the accepted prefix")
            }
            Self::InvalidVersionRecord => f.write_str("invalid physical version record"),
            Self::VersionExhausted => f.write_str("physical version space exhausted"),
            Self::InvalidTransactionTime => f.write_str("invalid transaction time"),
            Self::ChunkTooLarge => f.write_str("physical publication is too large"),
        }
    }
}

impl std::error::Error for ContainerError {}

impl From<io::Error> for ContainerError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

fn acquire_shared(file: &File) -> Result<(), ContainerError> {
    file.try_lock_shared().map_err(|error| match error {
        std::fs::TryLockError::WouldBlock => ContainerError::WriterConflict,
        std::fs::TryLockError::Error(error) => ContainerError::Io(error),
    })
}

fn acquire_exclusive(file: &File) -> Result<(), ContainerError> {
    file.try_lock().map_err(|error| match error {
        std::fs::TryLockError::WouldBlock => ContainerError::WriterConflict,
        std::fs::TryLockError::Error(error) => ContainerError::Io(error),
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ObjectRef {
    pub offset: u64,
    pub length: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PhysicalFrame {
    pub object: ObjectRef,
    pub payload: Vec<u8>,
}

/// One frame borrowed from the scan's reusable payload buffer.
pub struct ScannedFrame<'a> {
    pub object: ObjectRef,
    pub payload: &'a [u8],
}

pub struct FrameScan<'a> {
    cursor: PhysicalCursor<'a>,
    payload: Vec<u8>,
}

impl FrameScan<'_> {
    pub fn next_frame(&mut self) -> Result<Option<ScannedFrame<'_>>, ContainerError> {
        let Some(object) = self.cursor.read_frame_into(&mut self.payload)? else {
            return Ok(None);
        };
        Ok(Some(ScannedFrame {
            object,
            payload: &self.payload,
        }))
    }
}

// Cloned File handles share a system cursor. Read at explicit offsets so
// separate scans can use the same leased file even when their reads interleave.
struct PositionedFile {
    file: File,
    offset: u64,
}

impl Read for PositionedFile {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        let read = read_at(&self.file, self.offset, output)?;
        self.offset += read as u64;
        Ok(read)
    }
}

fn read_at(file: &File, offset: u64, output: &mut [u8]) -> io::Result<usize> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::FileExt;
        file.read_at(output, offset)
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::FileExt;
        file.seek_read(output, offset)
    }
    #[cfg(not(any(unix, windows)))]
    {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "positional file reads are unsupported on this platform",
        ))
    }
}

/// Streaming physical frame cursor. Each scan uses a fixed read buffer and
/// exposes each payload through a bounded `Read` implementation.
pub struct PhysicalCursor<'a> {
    _container: &'a Container,
    reader: BufReader<PositionedFile>,
    next_offset: u64,
    file_len: u64,
    done: bool,
}

pub struct FramePayloadReader<'a> {
    reader: &'a mut BufReader<PositionedFile>,
    object: ObjectRef,
    remaining: u64,
}

impl FramePayloadReader<'_> {
    pub fn object(&self) -> ObjectRef {
        self.object
    }
}

impl io::Read for FramePayloadReader<'_> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        if self.remaining == 0 || output.is_empty() {
            return Ok(0);
        }
        let limit =
            usize::try_from(self.remaining.min(output.len() as u64)).unwrap_or(output.len());
        let read = self.reader.read(&mut output[..limit])?;
        if read == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "frame payload changed while reading",
            ));
        }
        self.remaining -= read as u64;
        Ok(read)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mode {
    Read,
    Write,
    Repair,
}

/// Evidence for one incomplete physical tail, tied to the active repair lease.
pub struct TailRepairPlan {
    valid_end: u64,
    observed_len: u64,
    lease_token: [u8; 16],
}

impl TailRepairPlan {
    pub fn valid_end(&self) -> u64 {
        self.valid_end
    }
    pub fn observed_len(&self) -> u64 {
        self.observed_len
    }
}

/// Open file and lease. A shared lease backs readers; writers and repair sessions
/// hold an exclusive lease for their full lifetime.
pub struct Container {
    file: File,
    path: PathBuf,
    identity: ContainerIdentity,
    owner_id: [u8; 16],
    mode: Mode,
    lease_token: [u8; 16],
    write_epoch: u64,
    synced_epoch: u64,
    write_blocked: bool,
    read_lease: OnceLock<Arc<ReadLease>>,
    next_version: u64,
    transaction_times: BTreeMap<u64, i64>,
    standalone_reservations: BTreeSet<u64>,
    #[cfg(test)]
    fail_next_append_after: Option<usize>,
}

impl Container {
    pub fn create(
        path: impl AsRef<Path>,
        identity: ContainerIdentity,
        owner_id: [u8; 16],
    ) -> Result<Self, ContainerError> {
        validate_identity(identity)?;
        let path = path.as_ref();
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(path)?;
        acquire_exclusive(&file)?;
        let header = encode_header(identity, owner_id);
        file.write_all(&header)?;
        file.sync_all()?;
        file.seek(SeekFrom::Start(HEADER_LEN))?;
        Ok(Self {
            file,
            path: path.to_path_buf(),
            identity,
            owner_id,
            mode: Mode::Write,
            lease_token: *Uuid::new_v4().as_bytes(),
            write_epoch: 0,
            synced_epoch: 0,
            write_blocked: false,
            read_lease: OnceLock::new(),
            next_version: 1,
            transaction_times: BTreeMap::new(),
            standalone_reservations: BTreeSet::new(),
            #[cfg(test)]
            fail_next_append_after: None,
        })
    }

    pub fn open_read_only(path: impl AsRef<Path>) -> Result<Self, ContainerError> {
        Self::open_current(path.as_ref(), Mode::Read, None, None)
    }

    pub fn open_read_only_as(
        path: impl AsRef<Path>,
        expected: FileKind,
    ) -> Result<Self, ContainerError> {
        Self::open_read_only_for(path, expected, None)
    }

    pub fn open_read_only_for(
        path: impl AsRef<Path>,
        kind: FileKind,
        owner: Option<[u8; 16]>,
    ) -> Result<Self, ContainerError> {
        Self::open_current(path.as_ref(), Mode::Read, Some(kind), owner)
    }

    pub fn open_writer(path: impl AsRef<Path>) -> Result<Self, ContainerError> {
        Self::open_current(path.as_ref(), Mode::Write, None, None)
    }

    pub fn open_writer_as(
        path: impl AsRef<Path>,
        expected: FileKind,
    ) -> Result<Self, ContainerError> {
        Self::open_writer_for(path, expected, None)
    }

    pub fn open_writer_for(
        path: impl AsRef<Path>,
        kind: FileKind,
        owner: Option<[u8; 16]>,
    ) -> Result<Self, ContainerError> {
        Self::open_current(path.as_ref(), Mode::Write, Some(kind), owner)
    }

    fn open_current(
        path: &Path,
        mode: Mode,
        kind: Option<FileKind>,
        owner: Option<[u8; 16]>,
    ) -> Result<Self, ContainerError> {
        let mut container = Self::open_leased(path, mode, kind, owner)?;
        container.validate_from_start()?;
        Ok(container)
    }

    /// Opens an exclusive repair session. It validates only the immutable header.
    pub fn open_for_repair(
        path: impl AsRef<Path>,
        expected: FileKind,
    ) -> Result<Self, ContainerError> {
        Self::open_leased(path.as_ref(), Mode::Repair, Some(expected), None)
    }

    fn open_leased(
        path: &Path,
        mode: Mode,
        kind: Option<FileKind>,
        owner: Option<[u8; 16]>,
    ) -> Result<Self, ContainerError> {
        let mut file = OpenOptions::new()
            .read(true)
            .write(mode != Mode::Read)
            .open(path)?;
        if mode == Mode::Read {
            acquire_shared(&file)?;
        } else {
            acquire_exclusive(&file)?;
        }
        let (identity, owner_id) = read_header(&mut file)?;
        check_kind(identity, kind)?;
        if owner.is_some_and(|expected| expected != owner_id) {
            return Err(ContainerError::WrongOwner);
        }
        Ok(Self {
            file,
            path: path.to_path_buf(),
            identity,
            owner_id,
            mode,
            lease_token: *Uuid::new_v4().as_bytes(),
            write_epoch: 0,
            synced_epoch: 0,
            write_blocked: false,
            read_lease: OnceLock::new(),
            next_version: 1,
            transaction_times: BTreeMap::new(),
            standalone_reservations: BTreeSet::new(),
            #[cfg(test)]
            fail_next_append_after: None,
        })
    }

    pub fn identity(&self) -> ContainerIdentity {
        self.identity
    }
    pub fn owner_id(&self) -> [u8; 16] {
        self.owner_id
    }
    /// Proof of this live writer lease and its append epoch. A different
    /// Container handle cannot reuse the proof.
    pub fn writer_lease_proof(&self) -> Option<WriterLeaseProof> {
        (self.mode == Mode::Write).then_some(WriterLeaseProof {
            token: self.lease_token,
            epoch: self.write_epoch,
        })
    }

    /// Appends on this handle since the lease was admitted.
    pub fn append_epoch(&self) -> u64 {
        self.write_epoch
    }
    pub fn version(&self) -> FormatVersion {
        CURRENT_VERSION
    }
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Stable identity of the opened physical file, independent of its path.
    pub fn physical_file_identity(&self) -> Result<(u64, u64), ContainerError> {
        Self::file_identity(&self.file)
    }

    pub fn names_same_file(&self, path: &Path) -> Result<bool, ContainerError> {
        Ok(Self::file_identity(&File::open(path)?)? == self.physical_file_identity()?)
    }

    /// Read exact physical bytes through the existing owner lease.
    pub fn physical_bytes(&self) -> Result<impl Read + '_, ContainerError> {
        Ok(PositionedFile {
            file: self.file.try_clone()?,
            offset: 0,
        })
    }

    fn file_identity(file: &File) -> Result<(u64, u64), ContainerError> {
        #[cfg(windows)]
        {
            use std::os::windows::io::AsRawHandle;
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
            // SAFETY: `file` owns a live handle and Windows fills the structure.
            if unsafe { GetFileInformationByHandle(file.as_raw_handle(), information.as_mut_ptr()) }
                == 0
            {
                return Err(ContainerError::Io(std::io::Error::last_os_error()));
            }
            // SAFETY: a successful call initializes all structure fields.
            let information = unsafe { information.assume_init() };
            let index =
                (u64::from(information.index_high) << 32) | u64::from(information.index_low);
            Ok((u64::from(information.volume), index))
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let metadata = file.metadata()?;
            Ok((metadata.dev(), metadata.ino()))
        }
        #[cfg(not(any(windows, unix)))]
        {
            Err(ContainerError::InvalidIdentity)
        }
    }

    /// Highest allocated or grouped physical version observed in this file.
    pub fn latest_version(&self) -> u64 {
        self.next_version.saturating_sub(1)
    }

    pub fn transaction_time_ns(&self, version: u64) -> Option<i64> {
        self.transaction_times.get(&version).copied()
    }

    /// Whether a current clock ticket is still available to bind.
    pub fn has_unbound_version_reservation(&self, version: u64) -> bool {
        self.standalone_reservations.contains(&version)
    }

    /// Reserve the next physical version as a standalone ticket. A later
    /// publication envelope may bind to this reservation at its own frame.
    pub fn reserve_version(
        &mut self,
        transaction_time_ns: Option<i64>,
    ) -> Result<u64, ContainerError> {
        self.require_write()?;
        let version = self.next_version;
        let next = version
            .checked_add(1)
            .ok_or(ContainerError::VersionExhausted)?;
        let bytes = version_clock::encode(version, transaction_time_ns);
        self.append(&bytes)?;
        self.next_version = next;
        self.standalone_reservations.insert(version);
        if let Some(time) = transaction_time_ns {
            self.transaction_times.insert(version, time);
        }
        Ok(version)
    }

    /// Append an opaque owner body with an allocated global clock.
    pub fn append_publication_envelope(
        &mut self,
        body: &[u8],
        transaction_time_ns: Option<i64>,
    ) -> Result<(u64, ObjectRef), ContainerError> {
        self.require_write()?;
        let version = self.next_version;
        let next = version
            .checked_add(1)
            .ok_or(ContainerError::VersionExhausted)?;
        let bytes = publication_envelope::encode(version, transaction_time_ns, body)?;
        let object = self.append(&bytes)?;
        self.next_version = next;
        if let Some(time) = transaction_time_ns {
            self.transaction_times.insert(version, time);
        }
        Ok((version, object))
    }

    /// Append an opaque publication bound to an earlier standalone reservation.
    pub fn append_bound_publication_envelope(
        &mut self,
        version: u64,
        body: &[u8],
    ) -> Result<ObjectRef, ContainerError> {
        self.require_write()?;
        if !self.standalone_reservations.contains(&version) {
            return Err(ContainerError::InvalidVersionRecord);
        }
        let bytes = publication_envelope::encode_bound(version, body)?;
        let object = self.append(&bytes)?;
        self.standalone_reservations.remove(&version);
        Ok(object)
    }

    /// Bind a grouped carrier's already embedded contiguous range into the
    /// physical frontier after its frame has been appended.
    pub fn commit_embedded_version_range_at(
        &mut self,
        start: u64,
        count: usize,
        transaction_time_ns: Option<i64>,
    ) -> Result<(), ContainerError> {
        self.require_write()?;
        let count = u64::try_from(count).map_err(|_| ContainerError::VersionExhausted)?;
        if count > 0 && start != self.next_version {
            return Err(ContainerError::InvalidVersionRecord);
        }
        let next = if count == 0 {
            self.next_version
        } else {
            start
                .checked_add(count)
                .ok_or(ContainerError::VersionExhausted)?
        };
        if let Some(time) = transaction_time_ns {
            for offset in 0..count {
                let version = start
                    .checked_add(offset)
                    .ok_or(ContainerError::VersionExhausted)?;
                if self
                    .transaction_times
                    .get(&version)
                    .is_some_and(|existing| *existing != time)
                {
                    return Err(ContainerError::InvalidVersionRecord);
                }
            }
            for offset in 0..count {
                self.transaction_times.insert(start + offset, time);
            }
        }
        self.next_version = next;
        Ok(())
    }

    fn require_write(&self) -> Result<(), ContainerError> {
        if self.mode != Mode::Write {
            return Err(ContainerError::ReadOnly);
        }
        if self.write_blocked || self.read_lease.get().is_some_and(|lease| lease.is_fenced()) {
            return Err(ContainerError::WriteRecoveryRequired);
        }
        Ok(())
    }

    fn rebuild_clock_from(
        &mut self,
        from: u64,
        extent: u64,
        mut clock: PhysicalClock,
    ) -> Result<(), ContainerError> {
        let mut cursor = self.physical_cursor_at(from, extent)?;
        let mut bytes = Vec::new();
        while cursor.read_frame_into(&mut bytes)?.is_some() {
            clock.observe_frame(&bytes)?;
        }
        drop(cursor);
        (
            self.next_version,
            self.transaction_times,
            self.standalone_reservations,
        ) = clock.into_state();
        Ok(())
    }

    pub fn append(&mut self, payload: &[u8]) -> Result<ObjectRef, ContainerError> {
        let _read_guard = self
            .read_lease
            .get()
            .map(|lease| lease.lock())
            .transpose()?;
        self.require_write()?;
        let len = u64::try_from(payload.len()).map_err(|_| ContainerError::InvalidFrameLength {
            offset: 0,
            length: u64::MAX,
        })?;
        let offset = self.file.metadata()?.len();
        self.file.seek(SeekFrom::Start(offset))?;
        #[cfg(test)]
        if let Some(prefix_len) = self.fail_next_append_after.take() {
            self.file
                .write_all(&len.to_le_bytes())
                .map_err(ContainerError::Io)?;
            self.file
                .write_all(&payload[..prefix_len.min(payload.len())])
                .map_err(ContainerError::Io)?;
            self.write_blocked = true;
            return Err(ContainerError::Io(io::Error::other(
                "injected partial append failure",
            )));
        }
        if let Err(error) = self
            .file
            .write_all(&len.to_le_bytes())
            .and_then(|()| self.file.write_all(payload))
        {
            self.write_blocked = true;
            return Err(ContainerError::Io(error));
        }
        let Some(next_epoch) = self.write_epoch.checked_add(1) else {
            self.write_blocked = true;
            return Err(ContainerError::VersionExhausted);
        };
        self.write_epoch = next_epoch;
        Ok(ObjectRef {
            offset,
            length: len,
        })
    }

    pub fn sync(&mut self) -> Result<(), ContainerError> {
        let _read_guard = self
            .read_lease
            .get()
            .map(|lease| lease.lock())
            .transpose()?;
        self.require_write()?;
        if let Err(error) = self.file.sync_all() {
            self.write_blocked = true;
            return Err(ContainerError::Io(error));
        }
        self.synced_epoch = self.write_epoch;
        Ok(())
    }

    pub fn read_frames(&self) -> Result<Vec<PhysicalFrame>, ContainerError> {
        let mut cursor = self.physical_cursor()?;
        let mut frames = Vec::new();
        let mut payload = Vec::new();
        while let Some(object) = cursor.read_frame_into(&mut payload)? {
            frames.push(PhysicalFrame {
                object,
                payload: std::mem::take(&mut payload),
            });
        }
        Ok(frames)
    }

    /// Replay frames in physical order using one reusable payload buffer.
    pub fn scan_frames(&self) -> Result<FrameScan<'_>, ContainerError> {
        self.scan_frames_from(HEADER_LEN)
    }

    /// Replay from an admitted physical frame boundary, through the current extent.
    pub fn scan_frames_from(&self, offset: u64) -> Result<FrameScan<'_>, ContainerError> {
        Ok(FrameScan {
            cursor: self.physical_cursor_at(offset, self.extent()?)?,
            payload: Vec::new(),
        })
    }

    pub fn physical_cursor(&self) -> Result<PhysicalCursor<'_>, ContainerError> {
        self.physical_cursor_from(HEADER_LEN)
    }

    /// Stream frames from a previously admitted frame boundary.
    pub fn physical_cursor_from(&self, offset: u64) -> Result<PhysicalCursor<'_>, ContainerError> {
        self.physical_cursor_at(offset, self.extent()?)
    }

    fn physical_cursor_at(
        &self,
        offset: u64,
        extent: u64,
    ) -> Result<PhysicalCursor<'_>, ContainerError> {
        if !(HEADER_LEN..=extent).contains(&offset) {
            return Err(ContainerError::InvalidFrameReference);
        }
        self.require_extent(extent)?;
        let file = self.file.try_clone()?;
        Ok(PhysicalCursor {
            _container: self,
            reader: BufReader::with_capacity(SCAN_BUFFER_SIZE, PositionedFile { file, offset }),
            next_offset: offset,
            file_len: extent,
            done: false,
        })
    }

    fn validate_frames_from(&self, offset: u64, extent: u64) -> Result<(), ContainerError> {
        let mut cursor = self.physical_cursor_at(offset, extent)?;
        while let Some(frame) = cursor.next_frame()? {
            drop(frame);
        }
        Ok(())
    }

    pub fn plan_tail_repair(&mut self) -> Result<Option<TailRepairPlan>, ContainerError> {
        if self.mode != Mode::Repair {
            return Err(ContainerError::ReadOnly);
        }
        let file_len = self.file.metadata()?.len();
        let (end, incomplete) = self.inspect_accepted_end()?;
        if incomplete {
            Ok(Some(TailRepairPlan {
                valid_end: end,
                observed_len: file_len,
                lease_token: self.lease_token,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn repair_incomplete_tail(&mut self, plan: TailRepairPlan) -> Result<(), ContainerError> {
        if self.mode != Mode::Repair {
            return Err(ContainerError::ReadOnly);
        }
        if plan.lease_token != self.lease_token
            || self.file.metadata()?.len() != plan.observed_len
            || self.inspect_accepted_end()? != (plan.valid_end, true)
        {
            return Err(ContainerError::UnstableRepairPlan);
        }
        if plan.valid_end < HEADER_LEN || plan.valid_end > plan.observed_len {
            return Err(ContainerError::InvalidRepairBoundary);
        }
        self.file.set_len(plan.valid_end)?;
        self.file.sync_all()?;
        Ok(())
    }

    fn inspect_accepted_end(&self) -> Result<(u64, bool), ContainerError> {
        let mut cursor = self.physical_cursor()?;
        loop {
            match cursor.next_frame() {
                Ok(Some(frame)) => drop(frame),
                Ok(None) => return Ok((cursor.next_offset, false)),
                Err(ContainerError::RepairRequired { valid_end }) => return Ok((valid_end, true)),
                Err(error) => return Err(error),
            }
        }
    }
}

impl PhysicalCursor<'_> {
    fn read_frame_into(
        &mut self,
        payload: &mut Vec<u8>,
    ) -> Result<Option<ObjectRef>, ContainerError> {
        let Some(mut reader) = self.next_frame()? else {
            return Ok(None);
        };
        payload.clear();
        reader.read_to_end(payload)?;
        Ok(Some(reader.object()))
    }

    /// Advances to the next frame. Dropping a payload reader skips its unread
    /// suffix when the cursor advances again.
    pub fn next_frame(&mut self) -> Result<Option<FramePayloadReader<'_>>, ContainerError> {
        if self.done {
            return Ok(None);
        }
        let offset = self.next_offset;
        if self.file_len.saturating_sub(offset) < FRAME_HEADER_LEN {
            self.check_length()?;
            if offset == self.file_len {
                self.done = true;
                return Ok(None);
            }
            return Err(ContainerError::RepairRequired { valid_end: offset });
        }

        // A dropped payload reader can leave bytes unread. Advance within the
        // buffer when possible and reposition the next read for a larger skip.
        let buffered = self.reader.buffer().len();
        let buffer_end = self.reader.get_ref().offset;
        let buffer_start = buffer_end - buffered as u64;
        if (buffer_start..=buffer_end).contains(&offset) {
            self.reader.consume((offset - buffer_start) as usize);
        } else {
            self.reader.consume(buffered);
            self.reader.get_mut().offset = offset;
        }
        if self.reader.buffer().len() < FRAME_HEADER_LEN as usize {
            self.check_length()?;
        }
        let mut length_bytes = [0; FRAME_HEADER_LEN as usize];
        self.reader.read_exact(&mut length_bytes).map_err(|error| {
            if error.kind() == io::ErrorKind::UnexpectedEof {
                ContainerError::UnstableSnapshot
            } else {
                ContainerError::Io(error)
            }
        })?;
        let length = u64::from_le_bytes(length_bytes);
        let payload_start = offset
            .checked_add(FRAME_HEADER_LEN)
            .ok_or(ContainerError::InvalidFrameLength { offset, length })?;
        let frame_end = payload_start
            .checked_add(length)
            .ok_or(ContainerError::InvalidFrameLength { offset, length })?;
        if frame_end > self.file_len {
            self.check_length()?;
            return Err(ContainerError::RepairRequired { valid_end: offset });
        }
        self.next_offset = frame_end;
        Ok(Some(FramePayloadReader {
            reader: &mut self.reader,
            object: ObjectRef { offset, length },
            remaining: length,
        }))
    }

    fn check_length(&self) -> Result<(), ContainerError> {
        if self.reader.get_ref().file.metadata()?.len() != self.file_len {
            return Err(ContainerError::UnstableSnapshot);
        }
        Ok(())
    }
}

fn encode_header(identity: ContainerIdentity, owner_id: [u8; 16]) -> [u8; HEADER_LEN as usize] {
    let mut bytes = [0; HEADER_LEN as usize];
    bytes[..8].copy_from_slice(&MAGIC);
    bytes[8..10].copy_from_slice(&CURRENT_VERSION.major.to_le_bytes());
    bytes[10..12].copy_from_slice(&CURRENT_VERSION.minor.to_le_bytes());
    bytes[12..16].copy_from_slice(&(HEADER_LEN as u32).to_le_bytes());
    bytes[16] = match identity.file_kind {
        FileKind::Reliquary => 1,
        FileKind::Phylactery => 2,
    };
    bytes[17] = match identity.scope {
        ReliquaryScope::Unspecified => 0,
        ReliquaryScope::Organization => 1,
        ReliquaryScope::Project => 2,
        ReliquaryScope::Connection => 3,
    };
    bytes[24..40].copy_from_slice(&owner_id);
    bytes
}

fn read_header(file: &mut File) -> Result<(ContainerIdentity, [u8; 16]), ContainerError> {
    let mut bytes = [0; HEADER_LEN as usize];
    file.seek(SeekFrom::Start(0))?;
    file.read_exact(&mut bytes).map_err(|e| {
        if e.kind() == io::ErrorKind::UnexpectedEof {
            ContainerError::TruncatedHeader
        } else {
            ContainerError::Io(e)
        }
    })?;
    if bytes[..8] != MAGIC {
        let version = FormatVersion {
            major: u16::from_le_bytes([bytes[8], bytes[9]]),
            minor: u16::from_le_bytes([bytes[10], bytes[11]]),
        };
        if version == (FormatVersion { major: 2, minor: 0 }) {
            return Err(ContainerError::UnsupportedFormatSignature);
        }
        return Err(ContainerError::InvalidSignature);
    }
    let version = FormatVersion {
        major: u16::from_le_bytes([bytes[8], bytes[9]]),
        minor: u16::from_le_bytes([bytes[10], bytes[11]]),
    };
    if version != CURRENT_VERSION {
        return Err(ContainerError::UnsupportedVersion(version));
    }
    let length = u32::from_le_bytes(bytes[12..16].try_into().unwrap());
    if length != HEADER_LEN as u32 {
        return Err(ContainerError::InvalidHeaderLength(length));
    }
    if bytes[18..24].iter().any(|b| *b != 0) {
        return Err(ContainerError::InvalidIdentity);
    }
    let file_kind = match bytes[16] {
        1 => FileKind::Reliquary,
        2 => FileKind::Phylactery,
        _ => return Err(ContainerError::InvalidIdentity),
    };
    let scope = match bytes[17] {
        0 => ReliquaryScope::Unspecified,
        1 => ReliquaryScope::Organization,
        2 => ReliquaryScope::Project,
        3 => ReliquaryScope::Connection,
        _ => return Err(ContainerError::InvalidIdentity),
    };
    let identity = ContainerIdentity { file_kind, scope };
    validate_identity(identity)?;
    Ok((identity, bytes[24..40].try_into().unwrap()))
}

fn validate_identity(identity: ContainerIdentity) -> Result<(), ContainerError> {
    if identity.file_kind == FileKind::Phylactery && identity.scope != ReliquaryScope::Unspecified {
        return Err(ContainerError::InvalidIdentity);
    }
    Ok(())
}

fn check_kind(
    identity: ContainerIdentity,
    expected: Option<FileKind>,
) -> Result<(), ContainerError> {
    if let Some(expected) = expected {
        if expected != identity.file_kind {
            return Err(ContainerError::WrongFileKind {
                expected,
                actual: identity.file_kind,
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::{BufRead, BufReader};
    use std::process::{Command, Stdio};

    struct Dir(PathBuf);
    impl Dir {
        fn new() -> Self {
            let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".test-tmp");
            fs::create_dir_all(&base).unwrap();
            let path = base.join(format!("rel-container-{}", Uuid::new_v4()));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn rel() -> ContainerIdentity {
        ContainerIdentity::reliquary(ReliquaryScope::Project)
    }

    #[test]
    fn version_reservations_and_bound_envelopes_replay_in_2_1() {
        let d = Dir::new();
        let p = d.0.join("clock.rel");
        let mut c = Container::create(&p, rel(), [5; 16]).unwrap();
        assert_eq!(c.version(), FormatVersion { major: 2, minor: 1 });
        assert_eq!(c.reserve_version(Some(100)).unwrap(), 1);
        c.append_bound_publication_envelope(1, b"MEMORY01body")
            .unwrap();
        let (allocated, _) = c
            .append_publication_envelope(b"GRAPH001body", Some(120))
            .unwrap();
        assert_eq!(allocated, 2);
        assert_eq!(c.latest_version(), 2);
        assert_eq!(c.transaction_time_ns(1), Some(100));
        c.sync().unwrap();
        drop(c);
        let c = Container::open_read_only(&p).unwrap();
        assert_eq!(c.latest_version(), 2);
        assert_eq!(c.transaction_time_ns(1), Some(100));
        assert_eq!(c.transaction_time_ns(2), Some(120));
        let frames = c.read_frames().unwrap();
        assert!(frames[0].payload.starts_with(b"RELVERS2"));
        assert!(frames[1].payload.starts_with(b"RELPUB01"));
    }

    #[test]
    fn checkpoint_replays_a_bound_reservation_and_group_after_the_cut() {
        let d = Dir::new();
        let p = d.0.join("checkpoint.rel");
        let mut writer = Container::create(&p, rel(), [5; 16]).unwrap();
        let reserved = writer.reserve_version(Some(-3)).unwrap();
        let undated = writer.reserve_version(None).unwrap();
        assert!(matches!(
            writer.capture_checkpoint(None),
            Err(ContainerError::WriteRecoveryRequired)
        ));
        writer.sync().unwrap();
        let (checkpoint, _) = writer.capture_checkpoint(None).unwrap();
        let bound = writer
            .append_bound_publication_envelope(reserved, b"MEMORY01body")
            .unwrap();
        let mut group = vec![0; 84];
        group[..8].copy_from_slice(b"RELINSC7");
        group[64..72].copy_from_slice(&3_u64.to_le_bytes());
        group[72..76].copy_from_slice(&2_u32.to_le_bytes());
        group[76..84].copy_from_slice(&9_i64.to_le_bytes());
        writer.append(&group).unwrap();
        writer
            .commit_embedded_version_range_at(3, 2, Some(9))
            .unwrap();
        writer.sync().unwrap();
        drop(writer);

        let leased =
            Container::prepare_checkpoint_open(&p, true, FileKind::Reliquary, Some([5; 16]))
                .unwrap();
        let (writer, prefix) = leased.finish(Some(checkpoint.clone())).unwrap();
        let prefix = prefix.unwrap();
        assert_eq!(prefix.covered_end(), checkpoint.covered_end);
        assert!(!writer.has_unbound_version_reservation(reserved));
        assert_eq!(writer.transaction_time_ns(reserved), Some(-3));
        assert!(writer.has_unbound_version_reservation(undated));
        assert_eq!(writer.transaction_time_ns(undated), None);
        assert_eq!(writer.latest_version(), 4);
        assert_eq!(writer.transaction_time_ns(4), Some(9));
        {
            let mut suffix = writer.scan_frames_from(prefix.covered_end()).unwrap();
            assert_eq!(suffix.next_frame().unwrap().unwrap().object, bound);
            assert_eq!(suffix.next_frame().unwrap().unwrap().payload, group);
            assert!(suffix.next_frame().unwrap().is_none());
        }
        let (refreshed, _) = writer.capture_checkpoint(Some(&prefix)).unwrap();
        drop(writer);

        let cold = Container::open_writer(&p).unwrap();
        assert_eq!(cold.capture_checkpoint(None).unwrap().0, refreshed);
        drop(cold);
        let reader =
            Container::prepare_checkpoint_open(&p, false, FileKind::Reliquary, None).unwrap();
        let (reader, prefix) = reader.finish(Some(refreshed)).unwrap();
        let mut suffix = reader
            .scan_frames_from(prefix.unwrap().covered_end())
            .unwrap();
        assert!(suffix.next_frame().unwrap().is_none());
    }

    #[test]
    fn checkpoint_refuses_a_foreign_prefix_and_preserves_explicit_repair() {
        let d = Dir::new();
        let p = d.0.join("primary.rel");
        let foreign = d.0.join("same-owner.rel");
        let mut writer = Container::create(&p, rel(), [5; 16]).unwrap();
        let body = writer.append(b"accepted").unwrap();
        writer.sync().unwrap();
        let (checkpoint, _) = writer.capture_checkpoint(None).unwrap();
        drop(writer);

        fs::copy(&p, &foreign).unwrap();
        let prepare = |path| {
            Container::prepare_checkpoint_open(path, true, FileKind::Reliquary, Some([5; 16]))
                .unwrap()
        };
        assert!(
            prepare(&foreign)
                .finish(Some(checkpoint.clone()))
                .unwrap()
                .1
                .is_none()
        );

        let mut file = OpenOptions::new().write(true).open(&p).unwrap();
        let payload_start = body.offset + FRAME_HEADER_LEN;
        file.seek(SeekFrom::Start(payload_start)).unwrap();
        file.write_all(b"A").unwrap();
        file.sync_all().unwrap();
        assert!(
            prepare(&p)
                .finish(Some(checkpoint.clone()))
                .unwrap()
                .1
                .is_none()
        );

        file.seek(SeekFrom::Start(payload_start)).unwrap();
        file.write_all(b"a").unwrap();
        file.seek(SeekFrom::End(0)).unwrap();
        file.write_all(&[1]).unwrap();
        file.sync_all().unwrap();
        drop(file);
        assert!(matches!(
            prepare(&p).finish(Some(checkpoint.clone())),
            Err(ContainerError::RepairRequired { valid_end }) if valid_end == checkpoint.covered_end
        ));

        let mut repair = Container::open_for_repair(&p, FileKind::Reliquary).unwrap();
        let plan = repair.plan_tail_repair().unwrap().unwrap();
        repair.repair_incomplete_tail(plan).unwrap();
        drop(repair);
        assert!(prepare(&p).finish(Some(checkpoint)).unwrap().1.is_some());
    }

    #[test]
    fn indexed_reads_share_the_lease_and_fence_the_writer_on_damage() {
        use sha2::{Digest, Sha256};

        let d = Dir::new();
        let p = d.0.join("indexed.rel");
        let mut writer = Container::create(&p, rel(), [5; 16]).unwrap();
        let object = writer.append(b"some old text").unwrap();
        writer.sync().unwrap();
        let reader = writer.leased_frame_reader().unwrap();
        assert_eq!(
            reader
                .read_frame_range(object, 5, 3, Sha256::digest(b"old").into())
                .unwrap(),
            b"old"
        );
        writer.append(b"new text").unwrap();
        assert_eq!(
            reader
                .read_frame(object, Sha256::digest(b"some old text").into())
                .unwrap(),
            b"some old text"
        );
        assert!(matches!(
            reader.read_frame(object, [0; 32]),
            Err(ContainerError::InvalidFrameReference)
        ));
        assert!(matches!(
            writer.append(b"fenced"),
            Err(ContainerError::WriteRecoveryRequired)
        ));
        drop(writer);
        assert!(matches!(
            reader.read_frame(object, Sha256::digest(b"some old text").into()),
            Err(ContainerError::UnstableSnapshot)
        ));
        assert!(Container::open_writer(&p).is_ok());
    }

    #[test]
    fn grouped_clock_records_advance_contiguous_range_and_reject_bad_range() {
        let d = Dir::new();
        let p = d.0.join("grouped.rel");
        let mut c = Container::create(&p, rel(), [6; 16]).unwrap();
        // RELINSC7's physical clock prefix: header + embedded range/time.
        let mut grouped = vec![0; 84];
        grouped[..8].copy_from_slice(b"RELINSC7");
        grouped[64..72].copy_from_slice(&1_u64.to_le_bytes());
        grouped[72..76].copy_from_slice(&2_u32.to_le_bytes());
        grouped[76..84].copy_from_slice(&900_i64.to_le_bytes());
        c.append(&grouped).unwrap();
        c.sync().unwrap();
        drop(c);
        let c = Container::open_read_only(&p).unwrap();
        assert_eq!(c.latest_version(), 2);
        assert_eq!(c.transaction_time_ns(1), Some(900));
        assert_eq!(c.transaction_time_ns(2), Some(900));

        let bad = d.0.join("bad-grouped.rel");
        let mut c = Container::create(&bad, rel(), [6; 16]).unwrap();
        grouped[64..72].copy_from_slice(&2_u64.to_le_bytes());
        c.append(&grouped).unwrap();
        c.sync().unwrap();
        drop(c);
        assert!(matches!(
            Container::open_read_only(&bad),
            Err(ContainerError::InvalidVersionRecord)
        ));
    }

    #[test]
    fn retired_textual_clock_tags_are_not_replayed() {
        let d = Dir::new();
        let p = d.0.join("retired-tags.rel");
        let retired: [[u8; 8]; 6] = [
            [0x43, 0x56, 0x41, 0x56, 0x45, 0x52, 0x53, 0x31],
            [0x43, 0x56, 0x41, 0x56, 0x45, 0x52, 0x53, 0x32],
            [0x43, 0x56, 0x41, 0x50, 0x55, 0x42, 0x30, 0x31],
            [0x43, 0x56, 0x41, 0x49, 0x4e, 0x53, 0x43, 0x37],
            [0x43, 0x56, 0x41, 0x49, 0x4e, 0x53, 0x43, 0x38],
            [0x43, 0x56, 0x41, 0x58, 0x4d, 0x45, 0x4d, 0x32],
        ];
        let mut c = Container::create(&p, rel(), [8; 16]).unwrap();
        for marker in retired {
            c.append(&marker).unwrap();
        }
        c.sync().unwrap();
        drop(c);
        let c = Container::open_read_only(&p).unwrap();
        assert_eq!(c.latest_version(), 0);
    }

    #[test]
    fn writes_and_reopens_exact_current_header_and_frames() {
        let d = Dir::new();
        let p = d.0.join("data.rel");
        let mut c = Container::create(&p, rel(), [7; 16]).unwrap();
        assert_eq!(c.version(), CURRENT_VERSION);
        let object = c.append(b"hello").unwrap();
        c.sync().unwrap();
        drop(c);
        let c = Container::open_read_only_as(&p, FileKind::Reliquary).unwrap();
        assert_eq!(c.owner_id(), [7; 16]);
        assert_eq!(
            c.read_frames().unwrap()[0],
            PhysicalFrame {
                object,
                payload: b"hello".to_vec()
            }
        );
        let bytes = fs::read(&p).unwrap();
        assert_eq!(&bytes[..8], &MAGIC);
        assert_eq!(&bytes[8..12], &[2, 0, 1, 0]);
        assert_eq!(&bytes[12..16], &40u32.to_le_bytes());
    }

    #[test]
    fn rejects_old_and_mixed_headers_without_mutation() {
        let d = Dir::new();
        let p = d.0.join("bad.rel");
        drop(Container::create(&p, rel(), [8; 16]).unwrap());
        let canonical = fs::read(&p).unwrap();
        let old_signature = [0x43, 0x56, 0x41, 0, 13, 10, 26, 10];
        for (magic, version) in [(old_signature, [2, 0, 0, 0]), (MAGIC, [2, 0, 0, 0])] {
            let mut bad = canonical.clone();
            bad[..8].copy_from_slice(&magic);
            bad[8..12].copy_from_slice(&version);
            bad.extend_from_slice(&[1, 2, 3]);
            fs::write(&p, &bad).unwrap();
            let before = fs::read(&p).unwrap();
            assert!(matches!(
                Container::open_read_only(&p),
                Err(ContainerError::UnsupportedFormatSignature
                    | ContainerError::UnsupportedVersion(_))
            ));
            assert!(matches!(
                Container::open_for_repair(&p, FileKind::Reliquary),
                Err(ContainerError::UnsupportedFormatSignature
                    | ContainerError::UnsupportedVersion(_))
            ));
            assert_eq!(fs::read(&p).unwrap(), before);
        }
    }

    #[test]
    fn rejects_wrong_kind_and_invalid_identity_without_mutation() {
        let d = Dir::new();
        let p = d.0.join("phy.phy");
        drop(Container::create(&p, ContainerIdentity::phylactery(), [9; 16]).unwrap());
        let before = fs::read(&p).unwrap();
        assert!(matches!(
            Container::open_read_only_as(&p, FileKind::Reliquary),
            Err(ContainerError::WrongFileKind { .. })
        ));
        assert_eq!(fs::read(&p).unwrap(), before);
        let mut bad = before.clone();
        bad[18] = 1;
        fs::write(&p, &bad).unwrap();
        assert!(matches!(
            Container::open_read_only(&p),
            Err(ContainerError::InvalidIdentity)
        ));
        assert_eq!(fs::read(&p).unwrap(), bad);
    }

    #[test]
    fn tail_repair_is_separate_and_plan_is_bound_to_observation() {
        let d = Dir::new();
        let p = d.0.join("repair.rel");
        let mut c = Container::create(&p, rel(), [3; 16]).unwrap();
        c.append(b"accepted").unwrap();
        c.sync().unwrap();
        drop(c);
        let mut file = OpenOptions::new().append(true).open(&p).unwrap();
        file.write_all(&[1, 2, 3]).unwrap();
        file.sync_all().unwrap();
        drop(file);
        let before = fs::read(&p).unwrap();
        assert!(matches!(
            Container::open_read_only(&p),
            Err(ContainerError::RepairRequired { .. })
        ));
        assert_eq!(fs::read(&p).unwrap(), before);
        let mut repair = Container::open_for_repair(&p, FileKind::Reliquary).unwrap();
        let plan = repair.plan_tail_repair().unwrap().unwrap();
        assert!(plan.valid_end() < plan.observed_len());
        repair.repair_incomplete_tail(plan).unwrap();
        drop(repair);
        assert!(
            Container::open_read_only(&p)
                .unwrap()
                .read_frames()
                .unwrap()
                .len()
                == 1
        );
    }

    #[test]
    fn reader_and_writer_leases_are_mutually_exclusive() {
        let d = Dir::new();
        let p = d.0.join("lease.rel");
        let _writer = Container::create(&p, rel(), [4; 16]).unwrap();
        assert!(matches!(
            Container::open_read_only(&p),
            Err(ContainerError::WriterConflict)
        ));
    }

    #[test]
    fn open_validation_and_cursor_stream_without_collecting_payloads() {
        let d = Dir::new();
        let p = d.0.join("stream.rel");
        let payload = vec![0xA5; 1024 * 1024];
        let mut writer = Container::create(&p, rel(), [5; 16]).unwrap();
        writer.append(&payload).unwrap();
        writer.sync().unwrap();
        drop(writer);

        let reader = Container::open_read_only(&p).unwrap();
        let mut cursor = reader.physical_cursor().unwrap();
        let mut frame = cursor.next_frame().unwrap().unwrap();
        assert_eq!(frame.object().length, payload.len() as u64);
        let mut chunk = [0; 4096];
        let mut total = 0usize;
        loop {
            let count = frame.read(&mut chunk).unwrap();
            if count == 0 {
                break;
            }
            assert!(chunk[..count].iter().all(|byte| *byte == 0xA5));
            total += count;
        }
        assert_eq!(total, payload.len());
        assert!(cursor.next_frame().unwrap().is_none());
    }

    #[test]
    fn buffered_cursors_can_interleave_and_skip_unread_payloads() {
        let d = Dir::new();
        let p = d.0.join("interleaved.rel");
        let mut writer = Container::create(&p, rel(), [5; 16]).unwrap();
        let large = vec![0xA5; SCAN_BUFFER_SIZE + 17];
        writer.append(&large).unwrap();
        let empty = writer.append(b"").unwrap();
        let tail = writer.append(b"tail").unwrap();
        writer.sync().unwrap();
        drop(writer);

        let reader = Container::open_read_only(&p).unwrap();
        let mut left = reader.physical_cursor().unwrap();
        let mut right = reader.physical_cursor().unwrap();
        {
            let mut frame = left.next_frame().unwrap().unwrap();
            let mut prefix = [0; 8];
            frame.read_exact(&mut prefix).unwrap();
            assert_eq!(prefix, [0xA5; 8]);

            let mut raw_header = [0; HEADER_LEN as usize];
            reader
                .physical_bytes()
                .unwrap()
                .read_exact(&mut raw_header)
                .unwrap();
            assert_eq!(raw_header, encode_header(rel(), [5; 16]));
            let mut complete = Vec::new();
            right
                .next_frame()
                .unwrap()
                .unwrap()
                .read_to_end(&mut complete)
                .unwrap();
            assert_eq!(complete, large);
            frame.read_exact(&mut prefix).unwrap();
            assert_eq!(prefix, [0xA5; 8]);
        }

        for cursor in [&mut left, &mut right] {
            assert_eq!(cursor.next_frame().unwrap().unwrap().object(), empty);
            let mut last = cursor.next_frame().unwrap().unwrap();
            assert_eq!(last.object(), tail);
            let mut payload = Vec::new();
            last.read_to_end(&mut payload).unwrap();
            assert_eq!(payload, b"tail");
            assert!(cursor.next_frame().unwrap().is_none());
        }
    }

    #[test]
    fn buffered_scan_rejects_a_length_change_before_completion() {
        let d = Dir::new();
        let p = d.0.join("length-changed.rel");
        let mut writer = Container::create(&p, rel(), [5; 16]).unwrap();
        writer.append(b"first").unwrap();
        writer.append(b"second").unwrap();
        let mut scan = writer.scan_frames().unwrap();
        assert_eq!(scan.next_frame().unwrap().unwrap().payload, b"first");

        let length = writer.file.metadata().unwrap().len();
        writer.file.set_len(length + 2).unwrap();
        assert_eq!(scan.next_frame().unwrap().unwrap().payload, b"second");
        assert!(matches!(
            scan.next_frame(),
            Err(ContainerError::UnstableSnapshot)
        ));
    }

    #[test]
    fn a_failed_partial_append_blocks_writes_until_explicit_recovery() {
        let d = Dir::new();
        let p = d.0.join("partial.rel");
        let mut writer = Container::create(&p, rel(), [6; 16]).unwrap();
        writer.fail_next_append_after = Some(2);
        assert!(matches!(
            writer.append(b"partial payload"),
            Err(ContainerError::Io(_))
        ));
        assert!(matches!(
            writer.append(b"must not follow"),
            Err(ContainerError::WriteRecoveryRequired)
        ));
        drop(writer);

        assert!(matches!(
            Container::open_writer(&p),
            Err(ContainerError::RepairRequired { .. })
        ));
        let mut repair = Container::open_for_repair(&p, FileKind::Reliquary).unwrap();
        let plan = repair.plan_tail_repair().unwrap().unwrap();
        repair.repair_incomplete_tail(plan).unwrap();
        drop(repair);
        let mut recovered = Container::open_writer(&p).unwrap();
        recovered.append(b"after recovery").unwrap();
        recovered.sync().unwrap();
        assert_eq!(
            recovered.read_frames().unwrap()[0].payload,
            b"after recovery"
        );
    }

    #[test]
    fn child_process_lease_holder() {
        let Ok(path) = std::env::var("REL_CONTAINER_CHILD_PATH") else {
            return;
        };
        let _writer = Container::open_writer(path).unwrap();
        println!("READY");
        std::io::stdout().flush().unwrap();
        let mut release = [0];
        std::io::stdin().read_exact(&mut release).unwrap();
    }

    #[test]
    fn exclusive_lease_blocks_other_processes() {
        let d = Dir::new();
        let p = d.0.join("cross-process.rel");
        drop(Container::create(&p, rel(), [7; 16]).unwrap());
        let hardlink = d.0.join("cross-process-hardlink.rel");
        let hardlink_created = match fs::hard_link(&p, &hardlink) {
            Ok(()) => true,
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::PermissionDenied | io::ErrorKind::Unsupported
                ) =>
            {
                eprintln!("hard links are unavailable on this test filesystem");
                false
            }
            Err(error) => panic!("failed to create hardlink fixture: {error}"),
        };
        let symlink = d.0.join("cross-process-symlink.rel");
        #[cfg(unix)]
        let symlink_created = {
            std::os::unix::fs::symlink(&p, &symlink).unwrap();
            true
        };
        #[cfg(windows)]
        let symlink_created = match std::os::windows::fs::symlink_file(&p, &symlink) {
            Ok(()) => true,
            Err(error) if error.kind() == io::ErrorKind::PermissionDenied => {
                eprintln!(
                    "symlink creation requires a Windows privilege; hardlink is still tested"
                );
                false
            }
            Err(error) => panic!("failed to create symlink fixture: {error}"),
        };

        let mut child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "tests::child_process_lease_holder",
                "--nocapture",
            ])
            .env("REL_CONTAINER_CHILD_PATH", &p)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let mut output = BufReader::new(child.stdout.take().unwrap());
        let mut line = String::new();
        let mut ready = false;
        while output.read_line(&mut line).unwrap_or(0) > 0 {
            if line.trim() == "READY" {
                ready = true;
                break;
            }
            line.clear();
        }
        let conflict = matches!(
            Container::open_read_only(&p),
            Err(ContainerError::WriterConflict)
        );
        let aliases = hardlink_created
            .then_some(&hardlink)
            .into_iter()
            .chain(symlink_created.then_some(&symlink));
        for alias in aliases {
            assert!(
                matches!(
                    Container::open_read_only(alias),
                    Err(ContainerError::WriterConflict)
                ),
                "reader entered through alias {}",
                alias.display()
            );
            assert!(
                matches!(
                    Container::open_writer(alias),
                    Err(ContainerError::WriterConflict)
                ),
                "writer entered through alias {}",
                alias.display()
            );
        }
        child.stdin.take().unwrap().write_all(b"x").unwrap();
        let status = child.wait().unwrap();
        assert!(ready, "child process failed to acquire its writer lease");
        assert!(status.success(), "child process did not exit cleanly");
        assert!(
            conflict,
            "a reader entered while another process held the writer lease"
        );
        for alias in hardlink_created
            .then_some(&hardlink)
            .into_iter()
            .chain(symlink_created.then_some(&symlink))
        {
            assert!(
                Container::open_read_only(alias).is_ok(),
                "alias did not reopen after release"
            );
        }
    }
}
