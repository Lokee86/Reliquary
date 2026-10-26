#[path = "publication_envelope.rs"]
pub mod publication_envelope;

#[path = "access.rs"]
mod access;
#[path = "grouped_clock.rs"]
pub(crate) mod grouped_clock;
#[path = "lease.rs"]
mod lease;
#[path = "scan.rs"]
mod scan;
pub(super) use access::GateGuard;
pub use access::PhysicalReadSource;
#[path = "repair.rs"]
mod repair;
use lease::{Lease, file_identity};
pub use repair::TailRepairPlan;
pub use scan::{PhysicalCursor, PhysicalFrame, ScanClockState};

use std::collections::BTreeMap;
use std::fmt;
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, atomic::AtomicUsize};
use uuid::Uuid;

const MAGIC: [u8; 8] = *b"CVA\0\r\n\x1a\n";
const BASE_HEADER_LEN: u64 = 16;
const IDENTITY_HEADER_LEN: u64 = 40;
const CHUNK_HEADER_LEN: u64 = 8;
const CURRENT_VERSION: FormatVersion = COMPACT_VERSION;
pub(super) const COMPACT_VERSION: FormatVersion = FormatVersion { major: 2, minor: 0 };

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

/// Legacy typed-REL discriminator retained for opening and migrating older RELs.
/// Current Reliquaries are homogeneous and store organizational type as metadata.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReliquaryScopeKind {
    Organization,
    Project,
    Connection,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ContainerIdentity {
    pub file_kind: FileKind,
    pub scope: Option<ReliquaryScopeKind>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct ChunkRef {
    offset: u64,
    len: u64,
}

/// Opaque identity of one physical object. The fields and validated file handle
/// remain private. Byte/offset accessors are intentionally public for the existing
/// durable owner codecs and historical-cut calculations; they do not authorize
/// I/O, and forged references are rejected by Container extent validation.
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ObjectRef {
    physical: ChunkRef,
}

impl fmt::Debug for ObjectRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ObjectRef(..)")
    }
}

impl ObjectRef {
    fn from_chunk(chunk: ChunkRef) -> Self {
        Self { physical: chunk }
    }

    fn into_chunk(self) -> ChunkRef {
        self.physical
    }

    pub fn from_legacy_bytes(bytes: [u8; 16]) -> Self {
        Self::from_chunk(ChunkRef {
            offset: u64::from_le_bytes(bytes[..8].try_into().unwrap()),
            len: u64::from_le_bytes(bytes[8..].try_into().unwrap()),
        })
    }

    pub fn legacy_bytes(self) -> [u8; 16] {
        let chunk = self.into_chunk();
        let mut bytes = [0_u8; 16];
        bytes[..8].copy_from_slice(&chunk.offset.to_le_bytes());
        bytes[8..].copy_from_slice(&chunk.len.to_le_bytes());
        bytes
    }

    pub fn precedes(self, later: Self) -> bool {
        self.into_chunk().offset < later.into_chunk().offset
    }

    pub fn offset(self) -> u64 {
        self.into_chunk().offset
    }

    pub fn legacy_offset(self) -> u64 {
        self.into_chunk().offset
    }

    pub fn legacy_len(self) -> u64 {
        self.into_chunk().len
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SessionMode {
    Writer,
    Read,
    Replay,
    Repair,
}

#[derive(Debug)]
pub struct Container {
    file: File,
    // The lock is attached to this exact open file handle, never inferred from
    // the pathname. It remains alive for the complete Container lifetime.
    lease: Option<Lease>,
    mutation_gate: Arc<AtomicUsize>,
    path: PathBuf,
    version: FormatVersion,
    identity: Option<ContainerIdentity>,
    owner_uuid: Option<[u8; 16]>,
    pub(crate) header_len: u64,
    pub(crate) next_version: u64,
    pub(crate) transaction_times: BTreeMap<u64, i64>,
    pub(crate) next_transaction_time_override: Option<Option<i64>>,
    write_admission_closed: bool,
    scope_active: Arc<std::sync::atomic::AtomicUsize>,
    scope_id: [u8; 16],
    mode: SessionMode,
}

impl Container {
    pub fn create(path: impl AsRef<Path>) -> Result<Self, ContainerError> {
        let path = path.as_ref();
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(ContainerError::Io)?;
        let lease = match Lease::exclusive(&file) {
            Ok(lease) => lease,
            Err(error) => {
                cleanup_created_empty(path, &file);
                return Err(error);
            }
        };
        lease.ensure_path_identity(path)?;
        file.write_all(&encode_header(CURRENT_VERSION))?;
        file.sync_all()?;
        Ok(Self {
            file,
            lease: Some(lease),
            mutation_gate: Arc::new(AtomicUsize::new(0)),
            path: path.to_path_buf(),
            version: CURRENT_VERSION,
            identity: None,
            owner_uuid: None,
            header_len: BASE_HEADER_LEN,
            next_version: 1,
            transaction_times: BTreeMap::new(),
            next_transaction_time_override: None,
            write_admission_closed: false,
            scope_active: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            scope_id: *Uuid::new_v4().as_bytes(),
            mode: SessionMode::Writer,
        })
    }

    pub fn create_with_identity(
        path: impl AsRef<Path>,
        identity: ContainerIdentity,
    ) -> Result<Self, ContainerError> {
        Self::create_with_identity_and_uuid(path, identity, *Uuid::new_v4().as_bytes())
    }

    pub fn create_with_identity_and_uuid(
        path: impl AsRef<Path>,
        identity: ContainerIdentity,
        owner_uuid: [u8; 16],
    ) -> Result<Self, ContainerError> {
        validate_identity(identity)?;
        let path = path.as_ref();
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(path)?;
        let lease = match Lease::exclusive(&file) {
            Ok(lease) => lease,
            Err(error) => {
                cleanup_created_empty(path, &file);
                return Err(error);
            }
        };
        lease.ensure_path_identity(path)?;
        file.write_all(&encode_identity_header(identity, owner_uuid))?;
        file.sync_all()?;
        Ok(Self {
            file,
            lease: Some(lease),
            mutation_gate: Arc::new(AtomicUsize::new(0)),
            path: path.to_path_buf(),
            version: CURRENT_VERSION,
            identity: Some(identity),
            owner_uuid: Some(owner_uuid),
            header_len: IDENTITY_HEADER_LEN,
            next_version: 1,
            transaction_times: BTreeMap::new(),
            next_transaction_time_override: None,
            write_admission_closed: false,
            scope_active: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            scope_id: *Uuid::new_v4().as_bytes(),
            mode: SessionMode::Writer,
        })
    }

    pub fn append(&mut self, payload: &[u8]) -> Result<ObjectRef, ContainerError> {
        self.require_write_admission()?;
        self.require_writer_lease()?;
        let _mutation = self.mutation_gate()?;
        self.validate_path_identity()?;
        let len = u64::try_from(payload.len()).map_err(|_| ContainerError::ChunkTooLarge)?;
        let offset = self.file.seek(SeekFrom::End(0))?;
        self.file.write_all(&len.to_le_bytes())?;
        self.file.write_all(payload)?;
        Ok(ObjectRef::from_chunk(ChunkRef { offset, len }))
    }

    pub fn close_write_admission(&mut self) -> Result<WriteAdmission, ContainerError> {
        if self.mode != SessionMode::Writer
            || self.write_admission_closed
            || self.scope_active.load(std::sync::atomic::Ordering::Acquire) != 0
        {
            return Err(ContainerError::WriteAdmissionClosed);
        }
        self.write_admission_closed = true;
        self.scope_id = *Uuid::new_v4().as_bytes();
        Ok(WriteAdmission {
            scope_id: self.scope_id,
            active: Arc::clone(&self.scope_active),
        })
    }

    pub fn require_write_admission(&self) -> Result<(), ContainerError> {
        self.require_writer_lease()?;
        if self.mode != SessionMode::Writer
            || (self.write_admission_closed
                && self.scope_active.load(std::sync::atomic::Ordering::Acquire) == 0)
        {
            Err(ContainerError::WriteAdmissionClosed)
        } else {
            Ok(())
        }
    }

    fn require_writer_lease(&self) -> Result<(), ContainerError> {
        if self.lease.as_ref().is_some_and(Lease::is_exclusive) {
            self.validate_path_identity()
        } else {
            Err(ContainerError::ReadOnly)
        }
    }

    fn check_object_extent(&self, object: ObjectRef) -> Result<(ChunkRef, u64), ContainerError> {
        let chunk = object.into_chunk();
        if chunk.offset < self.header_len {
            return Err(ContainerError::InvalidChunkRef(object));
        }
        let end = chunk
            .offset
            .checked_add(CHUNK_HEADER_LEN)
            .and_then(|value| value.checked_add(chunk.len))
            .ok_or(ContainerError::ChunkTooLarge)?;
        if end > self.physical_length()? {
            return Err(ContainerError::InvalidChunkRef(object));
        }
        Ok((chunk, end))
    }
    /// Physical allocation guard; semantic owners choose the byte budget.
    pub fn read_bounded(
        &self,
        object: ObjectRef,
        max_bytes: usize,
    ) -> Result<Vec<u8>, ContainerError> {
        if object.into_chunk().len > max_bytes as u64 {
            return Err(ContainerError::ChunkTooLarge);
        }
        self.read(object)
    }

    pub fn read(&self, object: ObjectRef) -> Result<Vec<u8>, ContainerError> {
        let _read = self.read_gate()?;
        self.validate_path_identity()?;
        let (chunk, _) = self.check_object_extent(object)?;
        let mut length_bytes = [0_u8; 8];
        self.read_exact_at(&mut length_bytes, chunk.offset)
            .map_err(|error| map_chunk_read_error(error, chunk.offset))?;
        let stored_len = u64::from_le_bytes(length_bytes);
        if stored_len != chunk.len {
            return Err(ContainerError::InvalidChunkRef(object));
        }
        let len = usize::try_from(stored_len).map_err(|_| ContainerError::ChunkTooLarge)?;
        let mut payload = vec![0_u8; len];
        let payload_offset = chunk
            .offset
            .checked_add(CHUNK_HEADER_LEN)
            .ok_or(ContainerError::ChunkTooLarge)?;
        self.read_exact_at(&mut payload, payload_offset)
            .map_err(|error| map_chunk_read_error(error, chunk.offset))?;
        Ok(payload)
    }

    /// Read an object only when its complete physical extent is inside an accepted prefix.
    pub fn read_through(
        &self,
        object: ObjectRef,
        prefix_end: u64,
    ) -> Result<Vec<u8>, ContainerError> {
        let chunk = object.into_chunk();
        let end = chunk
            .offset
            .checked_add(CHUNK_HEADER_LEN)
            .and_then(|value| value.checked_add(chunk.len))
            .ok_or(ContainerError::ChunkTooLarge)?;
        if chunk.offset < self.header_len || end > prefix_end {
            return Err(ContainerError::InvalidChunkRef(object));
        }
        if prefix_end > self.physical_length()? {
            return Err(ContainerError::TruncatedChunk(prefix_end));
        }
        self.read(object)
    }

    pub fn write_chunk_prefix(
        &mut self,
        object: ObjectRef,
        payload: &[u8],
    ) -> Result<(), ContainerError> {
        self.require_write_admission()?;
        let _mutation = self.mutation_gate()?;
        self.validate_path_identity()?;
        let (chunk, _) = self.check_object_extent(object)?;
        let payload_len =
            u64::try_from(payload.len()).map_err(|_| ContainerError::ChunkTooLarge)?;
        if payload_len > chunk.len {
            return Err(ContainerError::InvalidChunkRef(object));
        }
        self.file.seek(SeekFrom::Start(chunk.offset))?;
        if read_u64(&mut self.file, chunk.offset)? != chunk.len {
            return Err(ContainerError::InvalidChunkRef(object));
        }
        self.file.write_all(payload)?;
        Ok(())
    }

    pub fn split_chunk(
        &mut self,
        object: ObjectRef,
        first_len: u64,
        second_prefix: &[u8],
    ) -> Result<(ObjectRef, ObjectRef), ContainerError> {
        self.require_write_admission()?;
        let _mutation = self.mutation_gate()?;
        self.validate_path_identity()?;
        let (chunk, _) = self.check_object_extent(object)?;
        let second_len = chunk
            .len
            .checked_sub(first_len)
            .and_then(|remaining| remaining.checked_sub(CHUNK_HEADER_LEN))
            .ok_or(ContainerError::InvalidChunkRef(object))?;
        if u64::try_from(second_prefix.len()).map_err(|_| ContainerError::ChunkTooLarge)?
            > second_len
        {
            return Err(ContainerError::InvalidChunkRef(object));
        }
        self.file.seek(SeekFrom::Start(chunk.offset))?;
        if read_u64(&mut self.file, chunk.offset)? != chunk.len {
            return Err(ContainerError::InvalidChunkRef(object));
        }
        let second_offset = chunk
            .offset
            .checked_add(CHUNK_HEADER_LEN)
            .and_then(|value| value.checked_add(first_len))
            .ok_or(ContainerError::ChunkTooLarge)?;
        self.file.seek(SeekFrom::Start(second_offset))?;
        self.file.write_all(&second_len.to_le_bytes())?;
        self.file.write_all(second_prefix)?;
        self.file.sync_data()?;
        self.file.seek(SeekFrom::Start(chunk.offset))?;
        self.file.write_all(&first_len.to_le_bytes())?;
        self.file.sync_data()?;
        Ok((
            ObjectRef::from_chunk(ChunkRef {
                offset: chunk.offset,
                len: first_len,
            }),
            ObjectRef::from_chunk(ChunkRef {
                offset: second_offset,
                len: second_len,
            }),
        ))
    }

    pub fn merge_adjacent_chunks(
        &mut self,
        first_object: ObjectRef,
        second_object: ObjectRef,
    ) -> Result<ObjectRef, ContainerError> {
        self.require_write_admission()?;
        let _mutation = self.mutation_gate()?;
        self.validate_path_identity()?;
        let (first, _) = self.check_object_extent(first_object)?;
        let (second, _) = self.check_object_extent(second_object)?;
        let expected_second = first
            .offset
            .checked_add(CHUNK_HEADER_LEN)
            .and_then(|value| value.checked_add(first.len))
            .ok_or(ContainerError::ChunkTooLarge)?;
        if second.offset != expected_second {
            return Err(ContainerError::InvalidChunkRef(second_object));
        }
        self.file.seek(SeekFrom::Start(first.offset))?;
        if read_u64(&mut self.file, first.offset)? != first.len {
            return Err(ContainerError::InvalidChunkRef(first_object));
        }
        self.file.seek(SeekFrom::Start(second.offset))?;
        if read_u64(&mut self.file, second.offset)? != second.len {
            return Err(ContainerError::InvalidChunkRef(second_object));
        }
        let len = first
            .len
            .checked_add(CHUNK_HEADER_LEN)
            .and_then(|value| value.checked_add(second.len))
            .ok_or(ContainerError::ChunkTooLarge)?;
        self.file.seek(SeekFrom::Start(first.offset))?;
        self.file.write_all(&len.to_le_bytes())?;
        self.file.sync_data()?;
        Ok(ObjectRef::from_chunk(ChunkRef {
            offset: first.offset,
            len,
        }))
    }

    pub fn object_capacity(&self, object: ObjectRef) -> u64 {
        object.into_chunk().len
    }

    pub fn objects_adjacent(&self, left: ObjectRef, right: ObjectRef) -> bool {
        let left = left.into_chunk();
        let right = right.into_chunk();
        left.offset
            .checked_add(CHUNK_HEADER_LEN)
            .and_then(|value| value.checked_add(left.len))
            == Some(right.offset)
    }

    pub fn chunks(&mut self) -> Result<Vec<ObjectRef>, ContainerError> {
        self.chunk_refs()
    }

    /// Immutable physical enumeration shared by read-only owner operations.
    pub fn chunk_refs(&self) -> Result<Vec<ObjectRef>, ContainerError> {
        let _read = self.read_gate()?;
        let mut chunks = Vec::new();
        let mut cursor = self.physical_cursor()?;
        while let Some(frame) = cursor.next_frame()? {
            chunks.push(frame.object);
        }
        Ok(chunks)
    }

    pub fn create_empty_like(&self, path: impl AsRef<Path>) -> Result<Self, ContainerError> {
        match (self.header_len, self.identity, self.owner_uuid) {
            (BASE_HEADER_LEN, None, None) => Self::create(path),
            (IDENTITY_HEADER_LEN, Some(identity), Some(owner_uuid)) => {
                Self::create_with_identity_and_uuid(path, identity, owner_uuid)
            }
            _ => Err(ContainerError::InvalidIdentity),
        }
    }

    pub fn object_payloads(&mut self) -> Result<Vec<(ObjectRef, Vec<u8>)>, ContainerError> {
        let objects = self.chunks()?;
        let mut payloads = Vec::with_capacity(objects.len());
        for object in objects {
            payloads.push((object, self.read(object)?));
        }
        Ok(payloads)
    }

    /// Inspect the existing validated header without scanning or recovering file tails.
    pub fn inspect_header(
        path: impl AsRef<Path>,
    ) -> Result<
        (
            Option<ContainerIdentity>,
            Option<String>,
            Option<(u64, u64)>,
        ),
        ContainerError,
    > {
        let mut file = OpenOptions::new().read(true).open(path)?;
        let physical_identity = file_identity(&file)?.as_pair();
        let (_, identity, uuid, _) = read_header(&mut file)?;
        let owner_id = identity.zip(uuid).map(|(identity, uuid)| {
            format!("{}-{}", owner_prefix(identity), Uuid::from_bytes(uuid))
        });
        Ok((identity, owner_id, physical_identity))
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn header_len(&self) -> u64 {
        self.header_len
    }

    pub fn version(&self) -> FormatVersion {
        self.version
    }

    pub fn identity(&self) -> Option<ContainerIdentity> {
        self.identity
    }

    pub fn owner_uuid(&self) -> Option<[u8; 16]> {
        self.owner_uuid
    }

    pub fn owner_id(&self) -> Option<String> {
        let identity = self.identity?;
        let uuid = Uuid::from_bytes(self.owner_uuid?);
        Some(format!("{}-{uuid}", owner_prefix(identity)))
    }

    pub fn truncate_tail_chunk(&mut self, object: ObjectRef) -> Result<bool, ContainerError> {
        self.require_write_admission()?;
        let _mutation = self.mutation_gate()?;
        self.validate_path_identity()?;
        let (chunk, end) = self.check_object_extent(object)?;
        self.file.seek(SeekFrom::Start(chunk.offset))?;
        if read_u64(&mut self.file, chunk.offset)? != chunk.len {
            return Err(ContainerError::InvalidChunkRef(object));
        }
        if self.file.metadata()?.len() != end {
            return Ok(false);
        }
        self.file.set_len(chunk.offset)?;
        self.file.sync_data()?;
        Ok(true)
    }

    pub fn sync(&self) -> Result<(), ContainerError> {
        self.require_writer_lease()?;
        self.file.sync_all().map_err(ContainerError::Io)
    }
}

#[derive(Debug)]
pub enum ContainerError {
    Io(io::Error),
    InvalidMagic,
    TruncatedHeader,
    TruncatedChunk(u64),
    UnsupportedVersion(FormatVersion),
    InvalidHeaderLength(u32),
    InvalidIdentity,
    InvalidChunkRef(ObjectRef),
    InvalidVersionRecord,
    InvalidTransactionTime,
    VersionExhausted,
    ChunkTooLarge,
    WriteAdmissionClosed,
    WriterConflict,
    LockingUnsupported,
    ReadOnly,
    RepairRequired { valid_end: u64 },
    UnstableSnapshot,
    StaleIdentity,
    InvalidPrefixBoundary,
}

impl From<io::Error> for ContainerError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

fn encode_header(version: FormatVersion) -> [u8; BASE_HEADER_LEN as usize] {
    let mut header = [0_u8; BASE_HEADER_LEN as usize];
    header[..8].copy_from_slice(&MAGIC);
    header[8..10].copy_from_slice(&version.major.to_le_bytes());
    header[10..12].copy_from_slice(&version.minor.to_le_bytes());
    header[12..16].copy_from_slice(&(BASE_HEADER_LEN as u32).to_le_bytes());
    header
}

fn encode_identity_header(
    identity: ContainerIdentity,
    owner_uuid: [u8; 16],
) -> [u8; IDENTITY_HEADER_LEN as usize] {
    encode_identity_header_for_version(identity, owner_uuid, CURRENT_VERSION)
}

fn encode_identity_header_for_version(
    identity: ContainerIdentity,
    owner_uuid: [u8; 16],
    version: FormatVersion,
) -> [u8; IDENTITY_HEADER_LEN as usize] {
    let mut header = [0_u8; IDENTITY_HEADER_LEN as usize];
    header[..8].copy_from_slice(&MAGIC);
    header[8..10].copy_from_slice(&version.major.to_le_bytes());
    header[10..12].copy_from_slice(&version.minor.to_le_bytes());
    header[12..16].copy_from_slice(&(IDENTITY_HEADER_LEN as u32).to_le_bytes());
    header[16] = match identity.file_kind {
        FileKind::Reliquary => 1,
        FileKind::Phylactery => 2,
    };
    header[17] = match identity.scope {
        None => 0,
        Some(ReliquaryScopeKind::Organization) => 1,
        Some(ReliquaryScopeKind::Project) => 2,
        Some(ReliquaryScopeKind::Connection) => 3,
    };
    header[24..40].copy_from_slice(&owner_uuid);
    header
}

fn read_header(
    file: &mut File,
) -> Result<
    (
        FormatVersion,
        Option<ContainerIdentity>,
        Option<[u8; 16]>,
        u64,
    ),
    ContainerError,
> {
    let mut header = [0_u8; BASE_HEADER_LEN as usize];
    file.read_exact(&mut header).map_err(|error| {
        if error.kind() == io::ErrorKind::UnexpectedEof {
            ContainerError::TruncatedHeader
        } else {
            ContainerError::Io(error)
        }
    })?;
    if header[..8] != MAGIC {
        return Err(ContainerError::InvalidMagic);
    }
    let version = FormatVersion {
        major: u16::from_le_bytes([header[8], header[9]]),
        minor: u16::from_le_bytes([header[10], header[11]]),
    };
    if version != CURRENT_VERSION {
        return Err(ContainerError::UnsupportedVersion(version));
    }
    let length = u32::from_le_bytes([header[12], header[13], header[14], header[15]]);
    if length == BASE_HEADER_LEN as u32 {
        // Bare containers have no semantic owner identity.
        return Ok((version, None, None, BASE_HEADER_LEN));
    }
    if length != IDENTITY_HEADER_LEN as u32 {
        return Err(ContainerError::InvalidHeaderLength(length));
    }
    let mut identity_bytes = [0_u8; 8];
    file.read_exact(&mut identity_bytes)
        .map_err(|_| ContainerError::TruncatedHeader)?;
    if identity_bytes[2..].iter().any(|byte| *byte != 0) {
        return Err(ContainerError::InvalidIdentity);
    }
    let file_kind = match identity_bytes[0] {
        1 => FileKind::Reliquary,
        2 => FileKind::Phylactery,
        _ => return Err(ContainerError::InvalidIdentity),
    };
    let scope = match identity_bytes[1] {
        0 => None,
        1 => Some(ReliquaryScopeKind::Organization),
        2 => Some(ReliquaryScopeKind::Project),
        3 => Some(ReliquaryScopeKind::Connection),
        _ => return Err(ContainerError::InvalidIdentity),
    };
    let identity = ContainerIdentity { file_kind, scope };
    validate_identity(identity)?;
    let owner_uuid = if length == IDENTITY_HEADER_LEN as u32 {
        let mut bytes = [0_u8; 16];
        file.read_exact(&mut bytes)
            .map_err(|_| ContainerError::TruncatedHeader)?;
        Some(bytes)
    } else {
        None
    };
    Ok((version, Some(identity), owner_uuid, u64::from(length)))
}

fn validate_identity(identity: ContainerIdentity) -> Result<(), ContainerError> {
    match (identity.file_kind, identity.scope) {
        (FileKind::Reliquary, _) | (FileKind::Phylactery, None) => Ok(()),
        _ => Err(ContainerError::InvalidIdentity),
    }
}

fn owner_prefix(identity: ContainerIdentity) -> &'static str {
    match (identity.file_kind, identity.scope) {
        (FileKind::Phylactery, None) => "phy",
        (FileKind::Reliquary, None) => "rel",
        // Typed Reliquary prefixes are retained only so existing pre-generic
        // REL identities remain stable while they are opened or migrated.
        (FileKind::Reliquary, Some(ReliquaryScopeKind::Project)) => "proj",
        (FileKind::Reliquary, Some(ReliquaryScopeKind::Organization)) => "org",
        (FileKind::Reliquary, Some(ReliquaryScopeKind::Connection)) => "con",
        _ => unreachable!("validated container identity"),
    }
}

fn read_u64(file: &mut File, offset: u64) -> Result<u64, ContainerError> {
    let mut bytes = [0_u8; 8];
    file.read_exact(&mut bytes)
        .map_err(|error| truncated_or_io(error, offset))?;
    Ok(u64::from_le_bytes(bytes))
}

/// Remove only an empty path that still names the file this create call made.
/// This handles the rare case where OS lease acquisition fails immediately
/// after `create_new` without deleting a replacement or a populated file.
fn cleanup_created_empty(path: &Path, created: &File) {
    let Ok(created_identity) = file_identity(created) else {
        return;
    };
    let Ok(path_file) = OpenOptions::new().read(true).open(path) else {
        return;
    };
    if file_identity(&path_file).ok() != Some(created_identity)
        || path_file.metadata().map(|metadata| metadata.len()).ok() != Some(0)
    {
        return;
    }
    drop(path_file);
    let _ = std::fs::remove_file(path);
}

fn truncated_or_io(error: io::Error, offset: u64) -> ContainerError {
    if error.kind() == io::ErrorKind::UnexpectedEof {
        ContainerError::TruncatedChunk(offset)
    } else {
        ContainerError::Io(error)
    }
}

fn map_chunk_read_error(error: ContainerError, offset: u64) -> ContainerError {
    match error {
        ContainerError::Io(error) => truncated_or_io(error, offset),
        other => other,
    }
}

#[cfg(test)]
#[path = "container_format_boundary_tests.rs"]
mod format_boundary_tests;

#[cfg(test)]
#[path = "file_lease_process_tests.rs"]
mod file_lease_process_tests;

#[cfg(test)]
#[path = "container_tests.rs"]
mod container_tests;

/// Owner-issued physical admission token for a scoped write interval.
pub struct WriteAdmission {
    scope_id: [u8; 16],
    active: Arc<std::sync::atomic::AtomicUsize>,
}

impl WriteAdmission {
    /// Activates this held scope only on its originating container session.
    pub fn enter(&self, container: &Container) -> Result<WriteScope, ContainerError> {
        if container.mode != SessionMode::Writer
            || !container.write_admission_closed
            || container.scope_id != self.scope_id
        {
            return Err(ContainerError::WriteAdmissionClosed);
        }
        container.require_writer_lease()?;
        self.active
            .fetch_update(
                std::sync::atomic::Ordering::AcqRel,
                std::sync::atomic::Ordering::Acquire,
                |active| active.checked_add(1),
            )
            .map_err(|_| ContainerError::UnstableSnapshot)?;
        Ok(WriteScope {
            active: Arc::clone(&self.active),
        })
    }

    /// Reopens ordinary writes after the owner has committed its completion marker.
    pub fn reopen(&self, container: &mut Container) -> Result<(), ContainerError> {
        if container.mode != SessionMode::Writer
            || !container.write_admission_closed
            || container.scope_id != self.scope_id
        {
            return Err(ContainerError::WriteAdmissionClosed);
        }
        container.require_writer_lease()?;
        if self.active.load(std::sync::atomic::Ordering::Acquire) != 0 {
            return Err(ContainerError::UnstableSnapshot);
        }
        container.write_admission_closed = false;
        container.scope_id = *Uuid::new_v4().as_bytes();
        Ok(())
    }
}

/// Owned guard for one active scoped write interval.
pub struct WriteScope {
    active: Arc<std::sync::atomic::AtomicUsize>,
}

impl Drop for WriteScope {
    fn drop(&mut self) {
        self.active
            .fetch_sub(1, std::sync::atomic::Ordering::AcqRel);
    }
}

#[cfg(test)]
mod physical_tests {
    use super::{Container, ContainerError};
    use std::fs;
    use std::io::Write;
    use std::path::PathBuf;
    fn path(name: &str) -> PathBuf {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".test-tmp");
        fs::create_dir_all(&dir).unwrap();
        dir.join(format!("{name}-{}.cva", uuid::Uuid::new_v4()))
    }
    #[test]
    fn ordinary_open_validates_without_repairing_incomplete_tail() {
        let path = path("tail");
        let mut c = Container::create(&path).unwrap();
        c.append(b"opaque").unwrap();
        c.sync().unwrap();
        drop(c);
        let original = fs::metadata(&path).unwrap().len();
        fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap()
            .write_all(&[1, 2, 3])
            .unwrap();
        let observed = fs::metadata(&path).unwrap().len();
        assert!(matches!(
            Container::open(&path),
            Err(ContainerError::RepairRequired { .. })
        ));
        assert_eq!(fs::metadata(&path).unwrap().len(), observed);
        let mut repair = Container::open_for_repair(&path).unwrap();
        assert!(matches!(
            repair.append(b"blocked"),
            Err(ContainerError::WriteAdmissionClosed)
        ));
        let plan = repair.plan_tail_repair().unwrap().unwrap();
        repair.repair_incomplete_tail(plan).unwrap();
        assert_eq!(fs::metadata(&path).unwrap().len(), original);
        drop(repair);
        assert_eq!(Container::open(&path).unwrap().latest_version(), 0);
        let _ = fs::remove_file(path);
    }
    #[test]
    fn writer_validation_replays_current_global_clocks() {
        let path = path("clocks");
        let mut c = Container::create(&path).unwrap();
        c.allocate_version_with_transaction_time(Some(10)).unwrap();
        c.sync().unwrap();
        drop(c);
        let opened = Container::open(&path).unwrap();
        assert_eq!(opened.latest_version(), 1);
        assert_eq!(opened.transaction_time_ns(1), Some(10));
        let _ = fs::remove_file(path);
    }
}

#[cfg(test)]
mod additional_physical_tests {
    use super::{Container, ContainerError, ObjectRef};
    use std::{fs, io::Write, path::PathBuf};
    fn path(label: &str) -> PathBuf {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".test-tmp");
        fs::create_dir_all(&root).unwrap();
        root.join(format!("{label}-{}.cva", uuid::Uuid::new_v4()))
    }
    fn reference(offset: u64, len: u64) -> ObjectRef {
        let mut bytes = [0; 16];
        bytes[..8].copy_from_slice(&offset.to_le_bytes());
        bytes[8..].copy_from_slice(&len.to_le_bytes());
        ObjectRef::from_legacy_bytes(bytes)
    }
    #[test]
    fn owned_scope_token_is_session_bound_and_reopens_after_completion() {
        let path = path("scope");
        let mut c = Container::create(&path).unwrap();
        let token = c.close_write_admission().unwrap();
        assert!(matches!(
            c.append(b"blocked"),
            Err(ContainerError::WriteAdmissionClosed)
        ));
        let scope = token.enter(&c).unwrap();
        assert!(matches!(
            token.reopen(&mut c),
            Err(ContainerError::UnstableSnapshot)
        ));
        c.append(b"scoped").unwrap();
        let nested = token.enter(&c).unwrap();
        drop(scope);
        c.append(b"still scoped").unwrap();
        drop(nested);
        assert!(matches!(
            c.append(b"blocked"),
            Err(ContainerError::WriteAdmissionClosed)
        ));
        token.reopen(&mut c).unwrap();
        c.append(b"open").unwrap();
        drop(c);
        let _ = fs::remove_file(path);
    }
    #[test]
    fn repair_session_cannot_append_or_allocate_before_or_after_plan() {
        let path = path("repair-mode");
        let mut c = Container::create(&path).unwrap();
        c.allocate_version_with_transaction_time(Some(20)).unwrap();
        c.sync().unwrap();
        drop(c);
        let mut repair = Container::open_for_repair(&path).unwrap();
        assert!(matches!(
            repair.append(b"blocked"),
            Err(ContainerError::WriteAdmissionClosed)
        ));
        assert!(matches!(
            repair.allocate_version(),
            Err(ContainerError::WriteAdmissionClosed)
        ));
        assert!(repair.plan_tail_repair().unwrap().is_none());
        drop(repair);
        let reopened = Container::open(&path).unwrap();
        assert_eq!(reopened.latest_version(), 1);
        assert_eq!(reopened.transaction_time_ns(1), Some(20));
        let _ = fs::remove_file(path);
    }
    #[test]
    fn forged_object_refs_reject_headers_overflow_and_past_eof_before_allocation() {
        let path = path("object-bounds");
        let c = Container::create(&path).unwrap();
        assert!(matches!(
            c.read(reference(0, 1)),
            Err(ContainerError::InvalidChunkRef(_))
        ));
        assert!(matches!(
            c.read(reference(u64::MAX - 3, 8)),
            Err(ContainerError::ChunkTooLarge)
        ));
        assert!(matches!(
            c.read(reference(c.physical_length().unwrap() + 1, 1)),
            Err(ContainerError::InvalidChunkRef(_))
        ));
        assert!(matches!(
            c.read(reference(c.header_len(), u64::MAX)),
            Err(ContainerError::ChunkTooLarge)
        ));
        drop(c);
        let _ = fs::remove_file(path);
    }
    #[test]
    fn malformed_complete_grouped_clock_headers_fail_closed() {
        let mut external = vec![0; 68];
        external[..8].copy_from_slice(b"CVAXMEM2");
        let bad_frames = [b"CVAINSC7".to_vec(), b"CVAINSC8\x02".to_vec(), external];
        for payload in bad_frames {
            let path = path("bad-clock");
            let mut c = Container::create(&path).unwrap();
            c.append(&payload).unwrap();
            c.sync().unwrap();
            drop(c);
            assert!(matches!(
                Container::open(&path),
                Err(ContainerError::InvalidVersionRecord)
            ));
            let _ = fs::remove_file(path);
        }
    }
    #[test]
    fn active_cursor_blocks_local_mutation_and_source_holds_read_lease() {
        let path = path("read-source");
        let mut writer = Container::create(&path).unwrap();
        writer.append(b"payload").unwrap();
        let source = writer.read_source().unwrap();
        let mut cursor = source.physical_cursor().unwrap();
        assert!(cursor.next_frame().unwrap().is_some());
        assert!(matches!(
            writer.append(b"race"),
            Err(ContainerError::UnstableSnapshot)
        ));
        drop(cursor);
        drop(writer);
        assert!(matches!(
            Container::open(&path),
            Err(ContainerError::WriterConflict)
        ));
        drop(source);
        assert!(Container::open(&path).is_ok());
        let _ = fs::remove_file(path);
    }
}

#[cfg(test)]
mod grouped_clock_tests {
    use super::{Container, ContainerError};
    use sha2::{Digest, Sha256};
    use std::{fs, path::PathBuf};
    fn path() -> PathBuf {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".test-tmp");
        fs::create_dir_all(&root).unwrap();
        root.join(format!("grouped-clock-{}.cva", uuid::Uuid::new_v4()))
    }
    fn check(path: PathBuf, payload: Vec<u8>, time: i64) {
        let mut writer = Container::create(&path).unwrap();
        writer.append(&payload).unwrap();
        writer.sync().unwrap();
        drop(writer);
        let reader = Container::open(&path).unwrap();
        assert_eq!(reader.latest_version(), 2);
        assert_eq!(reader.transaction_time_ns(1), Some(time));
        assert_eq!(reader.transaction_time_ns(2), Some(time));
        drop(reader);
        let _ = fs::remove_file(path);
    }
    #[test]
    fn validated_grouped_clock_prefixes_preserve_layout_and_times() {
        let mut c7 = vec![0; 84];
        c7[..8].copy_from_slice(b"CVAINSC7");
        c7[52..60].copy_from_slice(&777_i64.to_le_bytes());
        c7[64..72].copy_from_slice(&1_u64.to_le_bytes());
        c7[72..76].copy_from_slice(&2_u32.to_le_bytes());
        c7[76..84].copy_from_slice(&888_i64.to_le_bytes());
        check(path(), c7, 888);

        let mut c8 = vec![0; 85];
        c8[..8].copy_from_slice(b"CVAINSC8");
        c8[8] = 4;
        c8[65..73].copy_from_slice(&1_u64.to_le_bytes());
        c8[73..77].copy_from_slice(&2_u32.to_le_bytes());
        c8[77..85].copy_from_slice(&999_i64.to_le_bytes());
        check(path(), c8, 999);

        let mut external = vec![0; 68];
        external[..8].copy_from_slice(b"CVAXMEM2");
        external[16..24].copy_from_slice(&1_u64.to_le_bytes());
        external[24..28].copy_from_slice(&2_u32.to_le_bytes());
        external[28..36].copy_from_slice(&111_i64.to_le_bytes());
        let digest = Sha256::digest(&external[..36]);
        external[36..68].copy_from_slice(&digest);
        check(path(), external, 111);
    }
}

#[cfg(test)]
mod forged_write_tests {
    use super::{Container, ContainerError, ObjectRef};
    use std::{fs, path::PathBuf};
    #[test]
    fn invalid_object_refs_cannot_rewrite_or_truncate_header() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".test-tmp");
        fs::create_dir_all(&root).unwrap();
        let path = root.join(format!("forged-write-{}.cva", uuid::Uuid::new_v4()));
        let mut writer = Container::create(&path).unwrap();
        writer.append(b"payload").unwrap();
        let bytes = fs::read(&path).unwrap();
        let mut encoded = [0; 16];
        encoded[8..].copy_from_slice(&16_u64.to_le_bytes());
        let header_ref = ObjectRef::from_legacy_bytes(encoded);
        assert!(matches!(
            writer.write_chunk_prefix(header_ref, b"bad"),
            Err(ContainerError::InvalidChunkRef(_))
        ));
        assert!(matches!(
            writer.truncate_tail_chunk(header_ref),
            Err(ContainerError::InvalidChunkRef(_))
        ));
        assert_eq!(fs::read(&path).unwrap(), bytes);
        drop(writer);
        let _ = fs::remove_file(path);
    }
}
