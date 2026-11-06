//! Physical REL/PHY container framing for the canonical 2.1 format.
//!
//! This crate intentionally admits one signature/version pair. Format conversion
//! belongs in a separate offline program, never in normal container open paths.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use uuid::Uuid;

const MAGIC: [u8; 8] = *b"REL\0\r\n\x1a\n";
const HEADER_LEN: u64 = 40;
const FRAME_HEADER_LEN: u64 = 8;
const CURRENT_VERSION: FormatVersion = FormatVersion { major: 2, minor: 1 };

mod grouped_clock;
mod publication_envelope;
pub use publication_envelope::{PublicationEnvelope, decode_publication};
mod version_clock;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FormatVersion {
    pub major: u16,
    pub minor: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FileKind {
    Reliquary,
    Phylactery,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReliquaryScope {
    Unspecified,
    Organization,
    Project,
    Connection,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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
    TruncatedHeader,
    TruncatedFrame {
        offset: u64,
    },
    InvalidFrameLength {
        offset: u64,
        length: u64,
    },
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
            Self::TruncatedHeader => f.write_str("truncated container header"),
            Self::TruncatedFrame { offset } => write!(f, "truncated frame at {offset}"),
            Self::InvalidFrameLength { offset, length } => {
                write!(f, "invalid frame length {length} at {offset}")
            }
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ObjectRef {
    pub offset: u64,
    pub length: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PhysicalFrame {
    pub object: ObjectRef,
    pub payload: Vec<u8>,
}

/// Streaming physical frame cursor. It retains only frame coordinates and reads
/// each payload through a bounded `Read` implementation.
pub struct PhysicalCursor<'a> {
    _container: &'a Container,
    file: File,
    next_offset: u64,
    file_len: u64,
    done: bool,
}

pub struct FramePayloadReader<'a> {
    file: &'a mut File,
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
        let read = self.file.read(&mut output[..limit])?;
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
    write_blocked: bool,
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
            write_blocked: false,
            next_version: 1,
            transaction_times: BTreeMap::new(),
            standalone_reservations: BTreeSet::new(),
            #[cfg(test)]
            fail_next_append_after: None,
        })
    }

    pub fn open_read_only(path: impl AsRef<Path>) -> Result<Self, ContainerError> {
        Self::open_read_only_inner(path.as_ref(), None)
    }

    pub fn open_read_only_as(
        path: impl AsRef<Path>,
        expected: FileKind,
    ) -> Result<Self, ContainerError> {
        Self::open_read_only_inner(path.as_ref(), Some(expected))
    }

    fn open_read_only_inner(
        path: &Path,
        expected: Option<FileKind>,
    ) -> Result<Self, ContainerError> {
        let mut file = OpenOptions::new().read(true).open(path)?;
        acquire_shared(&file)?;
        let (identity, owner_id) = read_header(&mut file)?;
        check_kind(identity, expected)?;
        let mut container = Self {
            file,
            path: path.to_path_buf(),
            identity,
            owner_id,
            mode: Mode::Read,
            lease_token: *Uuid::new_v4().as_bytes(),
            write_epoch: 0,
            write_blocked: false,
            next_version: 1,
            transaction_times: BTreeMap::new(),
            standalone_reservations: BTreeSet::new(),
            #[cfg(test)]
            fail_next_append_after: None,
        };
        container.validate_frames()?;
        container.rebuild_clock()?;
        Ok(container)
    }

    pub fn open_writer(path: impl AsRef<Path>) -> Result<Self, ContainerError> {
        Self::open_writer_as_inner(path.as_ref(), None)
    }

    pub fn open_writer_as(
        path: impl AsRef<Path>,
        expected: FileKind,
    ) -> Result<Self, ContainerError> {
        Self::open_writer_as_inner(path.as_ref(), Some(expected))
    }

    fn open_writer_as_inner(
        path: &Path,
        expected: Option<FileKind>,
    ) -> Result<Self, ContainerError> {
        let mut file = OpenOptions::new().read(true).write(true).open(path)?;
        acquire_exclusive(&file)?;
        let (identity, owner_id) = read_header(&mut file)?;
        check_kind(identity, expected)?;
        let mut container = Self {
            file,
            path: path.to_path_buf(),
            identity,
            owner_id,
            mode: Mode::Write,
            lease_token: *Uuid::new_v4().as_bytes(),
            write_epoch: 0,
            write_blocked: false,
            next_version: 1,
            transaction_times: BTreeMap::new(),
            standalone_reservations: BTreeSet::new(),
            #[cfg(test)]
            fail_next_append_after: None,
        };
        container.validate_frames()?;
        container.rebuild_clock()?;
        Ok(container)
    }

    /// Opens an exclusive repair session. It validates only the immutable header;
    /// tail inspection and truncation remain explicit caller actions.
    pub fn open_for_repair(
        path: impl AsRef<Path>,
        expected: FileKind,
    ) -> Result<Self, ContainerError> {
        let path = path.as_ref();
        let mut file = OpenOptions::new().read(true).write(true).open(path)?;
        acquire_exclusive(&file)?;
        let (identity, owner_id) = read_header(&mut file)?;
        check_kind(identity, Some(expected))?;
        Ok(Self {
            file,
            path: path.to_path_buf(),
            identity,
            owner_id,
            mode: Mode::Repair,
            lease_token: *Uuid::new_v4().as_bytes(),
            write_epoch: 0,
            write_blocked: false,
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
        let mut file = self.file.try_clone()?;
        file.seek(SeekFrom::Start(0))?;
        Ok(file)
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
        if self.write_blocked {
            return Err(ContainerError::WriteRecoveryRequired);
        }
        Ok(())
    }

    fn rebuild_clock(&mut self) -> Result<(), ContainerError> {
        let mut next_version = 1;
        let mut transaction_times = BTreeMap::new();
        let mut standalone_reservations = BTreeSet::new();
        let mut cursor = self.physical_cursor()?;
        while let Some(mut frame) = cursor.next_frame()? {
            let mut bytes = Vec::new();
            frame.read_to_end(&mut bytes).map_err(ContainerError::Io)?;
            version_clock::observe_payload(
                &mut next_version,
                &mut transaction_times,
                &mut standalone_reservations,
                &bytes,
            )?;
        }
        drop(cursor);
        self.next_version = next_version;
        self.transaction_times = transaction_times;
        self.standalone_reservations = standalone_reservations;
        Ok(())
    }

    pub fn append(&mut self, payload: &[u8]) -> Result<ObjectRef, ContainerError> {
        if self.mode != Mode::Write {
            return Err(ContainerError::ReadOnly);
        }
        if self.write_blocked {
            return Err(ContainerError::WriteRecoveryRequired);
        }
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
        if self.mode != Mode::Write {
            return Err(ContainerError::ReadOnly);
        }
        if self.write_blocked {
            return Err(ContainerError::WriteRecoveryRequired);
        }
        if let Err(error) = self.file.sync_all() {
            self.write_blocked = true;
            return Err(ContainerError::Io(error));
        }
        Ok(())
    }

    pub fn read_frames(&self) -> Result<Vec<PhysicalFrame>, ContainerError> {
        let mut cursor = self.physical_cursor()?;
        let mut frames = Vec::new();
        while let Some(mut payload_reader) = cursor.next_frame()? {
            let object = payload_reader.object();
            let mut payload = Vec::new();
            payload_reader
                .read_to_end(&mut payload)
                .map_err(ContainerError::Io)?;
            frames.push(PhysicalFrame { object, payload });
        }
        Ok(frames)
    }

    pub fn physical_cursor(&self) -> Result<PhysicalCursor<'_>, ContainerError> {
        let file = self.file.try_clone()?;
        let file_len = file.metadata()?.len();
        Ok(PhysicalCursor {
            _container: self,
            file,
            next_offset: HEADER_LEN,
            file_len,
            done: false,
        })
    }

    fn validate_frames(&self) -> Result<(), ContainerError> {
        let mut cursor = self.physical_cursor()?;
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
    /// Advances to the next frame. Dropping a payload reader skips its unread
    /// suffix when the cursor advances again.
    pub fn next_frame(&mut self) -> Result<Option<FramePayloadReader<'_>>, ContainerError> {
        if self.done {
            return Ok(None);
        }
        let actual_len = self.file.metadata()?.len();
        if actual_len != self.file_len {
            return Err(ContainerError::UnstableSnapshot);
        }
        if self.next_offset == self.file_len {
            self.done = true;
            return Ok(None);
        }
        if self.next_offset > self.file_len || self.file_len - self.next_offset < FRAME_HEADER_LEN {
            return Err(ContainerError::RepairRequired {
                valid_end: self.next_offset,
            });
        }
        let offset = self.next_offset;
        self.file.seek(SeekFrom::Start(offset))?;
        let mut length_bytes = [0; FRAME_HEADER_LEN as usize];
        self.file.read_exact(&mut length_bytes).map_err(|error| {
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
            return Err(ContainerError::RepairRequired { valid_end: offset });
        }
        self.file.seek(SeekFrom::Start(payload_start))?;
        self.next_offset = frame_end;
        Ok(Some(FramePayloadReader {
            file: &mut self.file,
            object: ObjectRef { offset, length },
            remaining: length,
        }))
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
