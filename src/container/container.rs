#[path = "container_scan.rs"]
mod scan;

use std::collections::BTreeMap;
use std::fmt;
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use uuid::Uuid;

const MAGIC: [u8; 8] = *b"CVA\0\r\n\x1a\n";
const LEGACY_HEADER_LEN: u64 = 16;
const LEGACY_TYPED_HEADER_LEN: u64 = 24;
const IDENTITY_HEADER_LEN: u64 = 40;
const CHUNK_HEADER_LEN: u64 = 8;
const CURRENT_VERSION: FormatVersion = FormatVersion { major: 1, minor: 0 };

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

/// Storage-neutral identity for one durable object.
///
/// The legacy Container currently backs this with a physical chunk reference,
/// but callers outside the storage boundary cannot observe that representation.
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

    pub(crate) fn from_legacy_bytes(bytes: [u8; 16]) -> Self {
        Self::from_chunk(ChunkRef {
            offset: u64::from_le_bytes(bytes[..8].try_into().unwrap()),
            len: u64::from_le_bytes(bytes[8..].try_into().unwrap()),
        })
    }

    pub(crate) fn legacy_bytes(self) -> [u8; 16] {
        let chunk = self.into_chunk();
        let mut bytes = [0_u8; 16];
        bytes[..8].copy_from_slice(&chunk.offset.to_le_bytes());
        bytes[8..].copy_from_slice(&chunk.len.to_le_bytes());
        bytes
    }

    pub(crate) fn precedes(self, later: Self) -> bool {
        self.into_chunk().offset < later.into_chunk().offset
    }

    #[cfg(test)]
    pub(crate) fn legacy_offset(self) -> u64 {
        self.into_chunk().offset
    }

    #[cfg(test)]
    pub(crate) fn legacy_len(self) -> u64 {
        self.into_chunk().len
    }
}

#[derive(Debug)]
pub struct Container {
    file: File,
    path: PathBuf,
    version: FormatVersion,
    identity: Option<ContainerIdentity>,
    owner_uuid: Option<[u8; 16]>,
    pub(crate) header_len: u64,
    pub(crate) next_version: u64,
    pub(crate) transaction_times: BTreeMap<u64, i64>,
    pub(crate) next_transaction_time_override: Option<Option<i64>>,
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
        file.write_all(&encode_header(CURRENT_VERSION))?;
        file.sync_all()?;
        Ok(Self {
            file,
            path: path.to_path_buf(),
            version: CURRENT_VERSION,
            identity: None,
            owner_uuid: None,
            header_len: LEGACY_HEADER_LEN,
            next_version: 1,
            transaction_times: BTreeMap::new(),
            next_transaction_time_override: None,
        })
    }

    pub fn create_with_identity(
        path: impl AsRef<Path>,
        identity: ContainerIdentity,
    ) -> Result<Self, ContainerError> {
        Self::create_with_identity_and_uuid(path, identity, *Uuid::new_v4().as_bytes())
    }

    pub(crate) fn create_with_legacy_identity(
        path: impl AsRef<Path>,
        identity: ContainerIdentity,
    ) -> Result<Self, ContainerError> {
        validate_identity(identity)?;
        let path = path.as_ref();
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(path)?;
        file.write_all(&encode_legacy_identity_header(identity))?;
        file.sync_all()?;
        Ok(Self {
            file,
            path: path.to_path_buf(),
            version: CURRENT_VERSION,
            identity: Some(identity),
            owner_uuid: None,
            header_len: LEGACY_TYPED_HEADER_LEN,
            next_version: 1,
            transaction_times: BTreeMap::new(),
            next_transaction_time_override: None,
        })
    }

    pub(crate) fn create_with_identity_and_uuid(
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
        file.write_all(&encode_identity_header(identity, owner_uuid))?;
        file.sync_all()?;
        Ok(Self {
            file,
            path: path.to_path_buf(),
            version: CURRENT_VERSION,
            identity: Some(identity),
            owner_uuid: Some(owner_uuid),
            header_len: IDENTITY_HEADER_LEN,
            next_version: 1,
            transaction_times: BTreeMap::new(),
            next_transaction_time_override: None,
        })
    }

    pub fn append(&mut self, payload: &[u8]) -> Result<ObjectRef, ContainerError> {
        let len = u64::try_from(payload.len()).map_err(|_| ContainerError::ChunkTooLarge)?;
        let offset = self.file.seek(SeekFrom::End(0))?;
        self.file.write_all(&len.to_le_bytes())?;
        self.file.write_all(payload)?;
        Ok(ObjectRef::from_chunk(ChunkRef { offset, len }))
    }

    pub fn read(&mut self, object: ObjectRef) -> Result<Vec<u8>, ContainerError> {
        let chunk = object.into_chunk();
        self.file.seek(SeekFrom::Start(chunk.offset))?;
        let stored_len = read_u64(&mut self.file, chunk.offset)?;
        if stored_len != chunk.len {
            return Err(ContainerError::InvalidChunkRef(object));
        }
        let len = usize::try_from(stored_len).map_err(|_| ContainerError::ChunkTooLarge)?;
        let mut payload = vec![0_u8; len];
        self.file
            .read_exact(&mut payload)
            .map_err(|error| truncated_or_io(error, chunk.offset))?;
        Ok(payload)
    }

    pub(crate) fn write_chunk_prefix(
        &mut self,
        object: ObjectRef,
        payload: &[u8],
    ) -> Result<(), ContainerError> {
        let chunk = object.into_chunk();
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

    pub(crate) fn split_chunk(
        &mut self,
        object: ObjectRef,
        first_len: u64,
        second_prefix: &[u8],
    ) -> Result<(ObjectRef, ObjectRef), ContainerError> {
        let chunk = object.into_chunk();
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

    pub(crate) fn merge_adjacent_chunks(
        &mut self,
        first_object: ObjectRef,
        second_object: ObjectRef,
    ) -> Result<ObjectRef, ContainerError> {
        let first = first_object.into_chunk();
        let second = second_object.into_chunk();
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

    pub(crate) fn object_capacity(&self, object: ObjectRef) -> u64 {
        object.into_chunk().len
    }

    pub(crate) fn objects_adjacent(&self, left: ObjectRef, right: ObjectRef) -> bool {
        let left = left.into_chunk();
        let right = right.into_chunk();
        left.offset
            .checked_add(CHUNK_HEADER_LEN)
            .and_then(|value| value.checked_add(left.len))
            == Some(right.offset)
    }

    /// Visits payloads sequentially without retaining an O(chunk-count) index.
    ///
    /// Unlike open-time recovery, this query path never truncates or repairs a
    /// malformed tail. The visitor sees at most one allocated payload at a time.
    pub(crate) fn visit_payloads<E>(
        &mut self,
        visitor: impl FnMut(ObjectRef, &[u8]) -> Result<(), E>,
    ) -> Result<(), E>
    where
        E: From<ContainerError>,
    {
        let file_len = self
            .file
            .metadata()
            .map_err(ContainerError::Io)
            .map_err(E::from)?
            .len();
        self.visit_payloads_through(file_len, visitor)
    }

    /// Visit only a complete accepted physical prefix without reading or repairing
    /// later bytes. The caller supplies a previously accepted chunk boundary.
    pub(crate) fn visit_payloads_through<E>(
        &mut self,
        prefix_end: u64,
        mut visitor: impl FnMut(ObjectRef, &[u8]) -> Result<(), E>,
    ) -> Result<(), E>
    where
        E: From<ContainerError>,
    {
        let actual_len = self
            .file
            .metadata()
            .map_err(ContainerError::Io)
            .map_err(E::from)?
            .len();
        if prefix_end < self.header_len || prefix_end > actual_len {
            return Err(E::from(ContainerError::TruncatedChunk(prefix_end)));
        }
        let file_len = prefix_end;
        let mut offset = self.header_len;
        while offset < file_len {
            if file_len - offset < CHUNK_HEADER_LEN {
                return Err(E::from(ContainerError::TruncatedChunk(offset)));
            }
            self.file
                .seek(SeekFrom::Start(offset))
                .map_err(ContainerError::Io)
                .map_err(E::from)?;
            let len = read_u64(&mut self.file, offset).map_err(E::from)?;
            let end = offset
                .checked_add(CHUNK_HEADER_LEN)
                .and_then(|value| value.checked_add(len))
                .ok_or_else(|| E::from(ContainerError::ChunkTooLarge))?;
            if end > file_len {
                return Err(E::from(ContainerError::TruncatedChunk(offset)));
            }
            let payload_len =
                usize::try_from(len).map_err(|_| E::from(ContainerError::ChunkTooLarge))?;
            let mut payload = vec![0; payload_len];
            self.file
                .read_exact(&mut payload)
                .map_err(|error| E::from(truncated_or_io(error, offset)))?;
            visitor(ObjectRef::from_chunk(ChunkRef { offset, len }), &payload)?;
            offset = end;
        }
        Ok(())
    }

    pub fn chunks(&mut self) -> Result<Vec<ObjectRef>, ContainerError> {
        let file_len = self.file.metadata()?.len();
        let mut offset = self.header_len;
        let mut chunks = Vec::new();
        while offset < file_len {
            if file_len - offset < CHUNK_HEADER_LEN {
                return Err(ContainerError::TruncatedChunk(offset));
            }
            self.file.seek(SeekFrom::Start(offset))?;
            let len = read_u64(&mut self.file, offset)?;
            let end = offset
                .checked_add(CHUNK_HEADER_LEN)
                .and_then(|value| value.checked_add(len))
                .ok_or(ContainerError::ChunkTooLarge)?;
            if end > file_len {
                return Err(ContainerError::TruncatedChunk(offset));
            }
            chunks.push(ObjectRef::from_chunk(ChunkRef { offset, len }));
            offset = end;
        }
        Ok(chunks)
    }

    pub(crate) fn create_empty_like(&self, path: impl AsRef<Path>) -> Result<Self, ContainerError> {
        match (self.header_len, self.identity, self.owner_uuid) {
            (LEGACY_HEADER_LEN, None, None) => Self::create(path),
            (LEGACY_TYPED_HEADER_LEN, Some(identity), None) => {
                Self::create_with_legacy_identity(path, identity)
            }
            (IDENTITY_HEADER_LEN, Some(identity), Some(owner_uuid)) => {
                Self::create_with_identity_and_uuid(path, identity, owner_uuid)
            }
            _ => Err(ContainerError::InvalidIdentity),
        }
    }

    pub(crate) fn object_payloads(&mut self) -> Result<Vec<(ObjectRef, Vec<u8>)>, ContainerError> {
        let objects = self.chunks()?;
        let mut payloads = Vec::with_capacity(objects.len());
        for object in objects {
            payloads.push((object, self.read(object)?));
        }
        Ok(payloads)
    }

    pub fn path(&self) -> &Path {
        &self.path
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

    pub(crate) fn truncate_tail_chunk(
        &mut self,
        object: ObjectRef,
    ) -> Result<bool, ContainerError> {
        let chunk = object.into_chunk();
        let end = chunk
            .offset
            .checked_add(CHUNK_HEADER_LEN)
            .and_then(|value| value.checked_add(chunk.len))
            .ok_or(ContainerError::ChunkTooLarge)?;
        if self.file.metadata()?.len() != end {
            return Ok(false);
        }
        self.file.set_len(chunk.offset)?;
        self.file.sync_data()?;
        Ok(true)
    }

    pub fn sync(&self) -> Result<(), ContainerError> {
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
}

impl From<io::Error> for ContainerError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

fn encode_header(version: FormatVersion) -> [u8; LEGACY_HEADER_LEN as usize] {
    let mut header = [0_u8; LEGACY_HEADER_LEN as usize];
    header[..8].copy_from_slice(&MAGIC);
    header[8..10].copy_from_slice(&version.major.to_le_bytes());
    header[10..12].copy_from_slice(&version.minor.to_le_bytes());
    header[12..16].copy_from_slice(&(LEGACY_HEADER_LEN as u32).to_le_bytes());
    header
}

fn encode_legacy_identity_header(
    identity: ContainerIdentity,
) -> [u8; LEGACY_TYPED_HEADER_LEN as usize] {
    let current = encode_identity_header(identity, [0; 16]);
    let mut header = [0_u8; LEGACY_TYPED_HEADER_LEN as usize];
    header.copy_from_slice(&current[..LEGACY_TYPED_HEADER_LEN as usize]);
    header[12..16].copy_from_slice(&(LEGACY_TYPED_HEADER_LEN as u32).to_le_bytes());
    header
}

fn encode_identity_header(
    identity: ContainerIdentity,
    owner_uuid: [u8; 16],
) -> [u8; IDENTITY_HEADER_LEN as usize] {
    let mut header = [0_u8; IDENTITY_HEADER_LEN as usize];
    header[..8].copy_from_slice(&MAGIC);
    header[8..10].copy_from_slice(&CURRENT_VERSION.major.to_le_bytes());
    header[10..12].copy_from_slice(&CURRENT_VERSION.minor.to_le_bytes());
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
    let mut header = [0_u8; LEGACY_HEADER_LEN as usize];
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
    if length == LEGACY_HEADER_LEN as u32 {
        // Legacy v1 has no embedded semantic discriminator. Reliquary treats
        // this physical form explicitly as legacy Project state while keeping
        // the missing identity visible so migration remains detectable.
        return Ok((version, None, None, LEGACY_HEADER_LEN));
    }
    if length != LEGACY_TYPED_HEADER_LEN as u32 && length != IDENTITY_HEADER_LEN as u32 {
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

fn truncated_or_io(error: io::Error, offset: u64) -> ContainerError {
    if error.kind() == io::ErrorKind::UnexpectedEof {
        ContainerError::TruncatedChunk(offset)
    } else {
        ContainerError::Io(error)
    }
}
