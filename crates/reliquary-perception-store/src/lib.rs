//! Durable owner-local Entity records and exact Memory mention resolutions.
//!
//! Memory IDs and byte spans are foreign references. This crate records their
//! coordinates and caller-supplied decisions; it does not read Memory payloads
//! or synthesize Observations.

use reliquary_container::{Container, ContainerError, ContainerIdentity};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fmt;

const MAGIC: &[u8; 8] = b"RELPER01";
pub const MAX_ENTITY_ALIASES: usize = 32;
pub const MAX_ENTITY_NAME_BYTES: usize = 512;
pub const MAX_ENTITY_KIND_BYTES: usize = 128;
pub const MAX_ENTITY_SUMMARY_BYTES: usize = 4096;
pub const MAX_MENTION_CANDIDATES: usize = 8;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EntityId(pub [u8; 32]);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EntityDraft {
    pub canonical_name: String,
    pub aliases: Vec<String>,
    pub kind: String,
    pub summary: String,
    pub mutation_id: String,
    pub created_at_ns: i64,
    pub updated_at_ns: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Entity {
    pub id: EntityId,
    pub revision: u64,
    pub canonical_name: String,
    pub aliases: Vec<String>,
    pub kind: String,
    pub summary: String,
    pub mutation_id: String,
    pub created_at_ns: i64,
    pub updated_at_ns: i64,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum MemoryTextField {
    Title,
    Content,
}

impl MemoryTextField {
    fn tag(self) -> u8 {
        match self {
            Self::Title => 1,
            Self::Content => 2,
        }
    }
}

/// Exact foreign Memory mention coordinates. Offsets are byte offsets.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MemoryEntityMentionKey {
    pub memory_id: [u8; 32],
    pub field: MemoryTextField,
    pub start_byte: u32,
    pub end_byte: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EntityResolutionReason {
    ContextMatch,
    ContextConflictNewIdentity,
    NamedReferent,
    PersistentArtifact,
    DescriptiveIdentity,
    FirstSeenIdentity,
    InsufficientEvidence,
    GenericRole,
    AbstractProcess,
    TransientValue,
    SentenceLocal,
    WrapperCategory,
    Ambiguous,
    RecurrenceRequired,
    PrincipalIdentity,
}

impl EntityResolutionReason {
    fn tag(self) -> u8 {
        match self {
            Self::ContextMatch => 1,
            Self::ContextConflictNewIdentity => 2,
            Self::NamedReferent => 3,
            Self::PersistentArtifact => 4,
            Self::DescriptiveIdentity => 5,
            Self::FirstSeenIdentity => 6,
            Self::InsufficientEvidence => 7,
            Self::GenericRole => 8,
            Self::AbstractProcess => 9,
            Self::TransientValue => 10,
            Self::SentenceLocal => 11,
            Self::WrapperCategory => 12,
            Self::Ambiguous => 13,
            Self::RecurrenceRequired => 14,
            Self::PrincipalIdentity => 15,
        }
    }

    fn from_tag(tag: u8) -> Result<Self, PerceptionError> {
        match tag {
            1 => Ok(Self::ContextMatch),
            2 => Ok(Self::ContextConflictNewIdentity),
            3 => Ok(Self::NamedReferent),
            4 => Ok(Self::PersistentArtifact),
            5 => Ok(Self::DescriptiveIdentity),
            6 => Ok(Self::FirstSeenIdentity),
            7 => Ok(Self::InsufficientEvidence),
            8 => Ok(Self::GenericRole),
            9 => Ok(Self::AbstractProcess),
            10 => Ok(Self::TransientValue),
            11 => Ok(Self::SentenceLocal),
            12 => Ok(Self::WrapperCategory),
            13 => Ok(Self::Ambiguous),
            14 => Ok(Self::RecurrenceRequired),
            15 => Ok(Self::PrincipalIdentity),
            _ => Err(PerceptionError::CorruptRecord("unknown resolution reason")),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MemoryEntityResolutionStatus {
    Resolved {
        entity_id: EntityId,
        reason: EntityResolutionReason,
    },
    Ambiguous {
        candidates: Vec<EntityId>,
    },
    Unresolved {
        reason: EntityResolutionReason,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryEntityResolution {
    pub key: MemoryEntityMentionKey,
    pub revision: u32,
    pub updated_at_ns: i64,
    pub status: MemoryEntityResolutionStatus,
}

#[derive(Debug)]
pub enum PerceptionError {
    Container(ContainerError),
    OwnerMismatch,
    InvalidField(&'static str),
    CorruptRecord(&'static str),
    RevisionConflict { expected: u32, actual: u32 },
    EntityRevisionConflict { expected: u64, actual: u64 },
    MissingEntity(EntityId),
    VersionExhausted,
}

impl fmt::Display for PerceptionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Container(error) => error.fmt(f),
            Self::OwnerMismatch => f.write_str("perception record belongs to another owner"),
            Self::InvalidField(field) => write!(f, "invalid perception field: {field}"),
            Self::CorruptRecord(reason) => write!(f, "corrupt perception record: {reason}"),
            Self::RevisionConflict { expected, actual } => {
                write!(
                    f,
                    "mention revision conflict: expected {expected}, found {actual}"
                )
            }
            Self::EntityRevisionConflict { expected, actual } => write!(
                f,
                "Entity revision conflict: expected {expected}, found {actual}"
            ),
            Self::MissingEntity(id) => write!(f, "Entity {:?} does not exist", id.0),
            Self::VersionExhausted => f.write_str("perception version exhausted"),
        }
    }
}

impl std::error::Error for PerceptionError {}

impl From<ContainerError> for PerceptionError {
    fn from(value: ContainerError) -> Self {
        Self::Container(value)
    }
}

#[derive(Clone, Debug)]
enum Record {
    Entity {
        version: u64,
        owner: [u8; 16],
        value: Entity,
    },
    Mention {
        version: u64,
        owner: [u8; 16],
        value: MemoryEntityResolution,
    },
}

impl Record {
    fn version(&self) -> u64 {
        match self {
            Self::Entity { version, .. } | Self::Mention { version, .. } => *version,
        }
    }

    fn owner(&self) -> [u8; 16] {
        match self {
            Self::Entity { owner, .. } | Self::Mention { owner, .. } => *owner,
        }
    }

    fn encode(&self) -> Result<Vec<u8>, PerceptionError> {
        let mut out = MAGIC.to_vec();
        match self {
            Self::Entity {
                version,
                owner,
                value,
            } => {
                out.push(1);
                put_u64(&mut out, *version);
                out.extend_from_slice(owner);
                out.extend_from_slice(&value.id.0);
                put_u64(&mut out, value.revision);
                put_string(&mut out, &value.canonical_name)?;
                put_strings(&mut out, &value.aliases)?;
                put_string(&mut out, &value.kind)?;
                put_string(&mut out, &value.summary)?;
                put_string(&mut out, &value.mutation_id)?;
                out.extend_from_slice(&value.created_at_ns.to_le_bytes());
                out.extend_from_slice(&value.updated_at_ns.to_le_bytes());
            }
            Self::Mention {
                version,
                owner,
                value,
            } => {
                out.push(2);
                put_u64(&mut out, *version);
                out.extend_from_slice(owner);
                out.extend_from_slice(&value.key.memory_id);
                out.push(value.key.field.tag());
                out.extend_from_slice(&value.key.start_byte.to_le_bytes());
                out.extend_from_slice(&value.key.end_byte.to_le_bytes());
                out.extend_from_slice(&value.revision.to_le_bytes());
                out.extend_from_slice(&value.updated_at_ns.to_le_bytes());
                match &value.status {
                    MemoryEntityResolutionStatus::Resolved { entity_id, reason } => {
                        out.push(1);
                        out.extend_from_slice(&entity_id.0);
                        out.push(reason.tag());
                    }
                    MemoryEntityResolutionStatus::Ambiguous { candidates } => {
                        out.push(2);
                        out.push(
                            u8::try_from(candidates.len()).map_err(|_| {
                                PerceptionError::InvalidField("ambiguity candidates")
                            })?,
                        );
                        for candidate in candidates {
                            out.extend_from_slice(&candidate.0);
                        }
                    }
                    MemoryEntityResolutionStatus::Unresolved { reason } => {
                        out.push(3);
                        out.push(reason.tag());
                    }
                }
            }
        }
        Ok(out)
    }

    fn decode(bytes: &[u8]) -> Result<Option<Self>, PerceptionError> {
        if bytes.get(..8) != Some(MAGIC.as_slice()) {
            return Ok(None);
        }
        let mut at = 8;
        let tag = get_u8(bytes, &mut at)?;
        let version = get_u64(bytes, &mut at)?;
        let owner = get_array::<16>(bytes, &mut at)?;
        let record = match tag {
            1 => {
                let id = EntityId(get_array::<32>(bytes, &mut at)?);
                let revision = get_u64(bytes, &mut at)?;
                let canonical_name = get_string(bytes, &mut at)?;
                let aliases = get_strings(bytes, &mut at)?;
                let kind = get_string(bytes, &mut at)?;
                let summary = get_string(bytes, &mut at)?;
                let mutation_id = get_string(bytes, &mut at)?;
                let created_at_ns = get_i64(bytes, &mut at)?;
                let updated_at_ns = get_i64(bytes, &mut at)?;
                Self::Entity {
                    version,
                    owner,
                    value: Entity {
                        id,
                        revision,
                        canonical_name,
                        aliases,
                        kind,
                        summary,
                        mutation_id,
                        created_at_ns,
                        updated_at_ns,
                    },
                }
            }
            2 => {
                let memory_id = get_array::<32>(bytes, &mut at)?;
                let field = match get_u8(bytes, &mut at)? {
                    1 => MemoryTextField::Title,
                    2 => MemoryTextField::Content,
                    _ => return Err(PerceptionError::CorruptRecord("unknown Memory field")),
                };
                let start_byte = get_u32(bytes, &mut at)?;
                let end_byte = get_u32(bytes, &mut at)?;
                let revision = get_u32(bytes, &mut at)?;
                let updated_at_ns = get_i64(bytes, &mut at)?;
                let status = match get_u8(bytes, &mut at)? {
                    1 => MemoryEntityResolutionStatus::Resolved {
                        entity_id: EntityId(get_array::<32>(bytes, &mut at)?),
                        reason: EntityResolutionReason::from_tag(get_u8(bytes, &mut at)?)?,
                    },
                    2 => {
                        let count = get_u8(bytes, &mut at)? as usize;
                        let mut candidates = Vec::with_capacity(count);
                        for _ in 0..count {
                            candidates.push(EntityId(get_array::<32>(bytes, &mut at)?));
                        }
                        MemoryEntityResolutionStatus::Ambiguous { candidates }
                    }
                    3 => MemoryEntityResolutionStatus::Unresolved {
                        reason: EntityResolutionReason::from_tag(get_u8(bytes, &mut at)?)?,
                    },
                    _ => return Err(PerceptionError::CorruptRecord("unknown resolution status")),
                };
                Self::Mention {
                    version,
                    owner,
                    value: MemoryEntityResolution {
                        key: MemoryEntityMentionKey {
                            memory_id,
                            field,
                            start_byte,
                            end_byte,
                        },
                        revision,
                        updated_at_ns,
                        status,
                    },
                }
            }
            _ => return Err(PerceptionError::CorruptRecord("unknown record kind")),
        };
        if at != bytes.len() {
            return Err(PerceptionError::CorruptRecord("trailing bytes"));
        }
        Ok(Some(record))
    }
}

/// Per-container canonical entities and exact source-span resolution facts.
pub struct PerceptionStore {
    owner_id: [u8; 16],
    identity: ContainerIdentity,
    version: u64,
    entities: BTreeMap<EntityId, Entity>,
    entity_history: BTreeMap<(EntityId, u64), Entity>,
    mutations: BTreeMap<String, EntityId>,
    mentions: BTreeMap<MemoryEntityMentionKey, MemoryEntityResolution>,
    mention_history: BTreeMap<(MemoryEntityMentionKey, u32), MemoryEntityResolution>,
}

impl PerceptionStore {
    pub fn open(container: &Container) -> Result<Self, PerceptionError> {
        let mut store = Self {
            owner_id: container.owner_id(),
            identity: container.identity(),
            version: 0,
            entities: BTreeMap::new(),
            entity_history: BTreeMap::new(),
            mutations: BTreeMap::new(),
            mentions: BTreeMap::new(),
            mention_history: BTreeMap::new(),
        };
        for frame in container.read_frames()? {
            let Some(record) = Record::decode(&frame.payload)? else {
                continue;
            };
            if record.owner() != store.owner_id {
                return Err(PerceptionError::OwnerMismatch);
            }
            store.apply(record)?;
        }
        Ok(store)
    }

    pub fn version(&self) -> u64 {
        self.version
    }
    pub fn entity(&self, id: EntityId) -> Option<&Entity> {
        self.entities.get(&id)
    }
    pub fn entity_revision(&self, id: EntityId, revision: u64) -> Option<&Entity> {
        self.entity_history.get(&(id, revision))
    }
    pub fn mention(&self, key: MemoryEntityMentionKey) -> Option<&MemoryEntityResolution> {
        self.mentions.get(&key)
    }
    pub fn mention_revision(
        &self,
        key: MemoryEntityMentionKey,
        revision: u32,
    ) -> Option<&MemoryEntityResolution> {
        self.mention_history.get(&(key, revision))
    }
    pub fn entities(&self) -> impl Iterator<Item = &Entity> {
        self.entities.values()
    }
    pub fn mentions(&self) -> impl Iterator<Item = &MemoryEntityResolution> {
        self.mentions.values()
    }

    pub fn publish_entity(
        &mut self,
        container: &mut Container,
        draft: EntityDraft,
    ) -> Result<Entity, PerceptionError> {
        self.check_owner(container)?;
        validate_draft(&draft)?;
        let id = entity_id(&draft.mutation_id);
        if let Some(existing_id) = self.mutations.get(&draft.mutation_id) {
            let existing = self
                .entities
                .get(existing_id)
                .expect("mutation index is consistent");
            if existing.id == id
                && existing.canonical_name == draft.canonical_name
                && existing.aliases == draft.aliases
                && existing.kind == draft.kind
                && existing.summary == draft.summary
                && existing.created_at_ns == draft.created_at_ns
            {
                return Ok(existing.clone());
            }
            return Err(PerceptionError::InvalidField("conflicting mutation ID"));
        }
        let previous = self.entities.get(&id);
        if previous.is_some_and(|value| value.created_at_ns != draft.created_at_ns) {
            return Err(PerceptionError::InvalidField("created_at_ns is immutable"));
        }
        let revision = previous.map_or(1, |value| value.revision.checked_add(1).unwrap_or(0));
        if revision == 0 {
            return Err(PerceptionError::VersionExhausted);
        }
        let value = Entity {
            id,
            revision,
            canonical_name: draft.canonical_name,
            aliases: draft.aliases,
            kind: draft.kind,
            summary: draft.summary,
            mutation_id: draft.mutation_id,
            created_at_ns: previous.map_or(draft.created_at_ns, |v| v.created_at_ns),
            updated_at_ns: draft.updated_at_ns,
        };
        let record = Record::Entity {
            version: self.next_version()?,
            owner: self.owner_id,
            value: value.clone(),
        };
        self.append(container, record)?;
        Ok(value)
    }

    pub fn update_entity(
        &mut self,
        container: &mut Container,
        id: EntityId,
        expected_revision: u64,
        draft: EntityDraft,
    ) -> Result<Entity, PerceptionError> {
        self.check_owner(container)?;
        validate_draft(&draft)?;
        if self.mutations.contains_key(&draft.mutation_id) {
            return Err(PerceptionError::InvalidField("mutation ID is already used"));
        }
        let current = self
            .entities
            .get(&id)
            .ok_or(PerceptionError::MissingEntity(id))?;
        if current.revision != expected_revision {
            return Err(PerceptionError::EntityRevisionConflict {
                expected: expected_revision,
                actual: current.revision,
            });
        }
        if draft.created_at_ns != current.created_at_ns
            || draft.updated_at_ns < current.updated_at_ns
        {
            return Err(PerceptionError::InvalidField("Entity timestamps"));
        }
        let value = Entity {
            id,
            revision: current
                .revision
                .checked_add(1)
                .ok_or(PerceptionError::VersionExhausted)?,
            canonical_name: draft.canonical_name,
            aliases: draft.aliases,
            kind: draft.kind,
            summary: draft.summary,
            mutation_id: draft.mutation_id,
            created_at_ns: current.created_at_ns,
            updated_at_ns: draft.updated_at_ns,
        };
        self.append(
            container,
            Record::Entity {
                version: self.next_version()?,
                owner: self.owner_id,
                value: value.clone(),
            },
        )?;
        Ok(value)
    }

    pub fn put_mention_resolution(
        &mut self,
        container: &mut Container,
        key: MemoryEntityMentionKey,
        expected_revision: u32,
        updated_at_ns: i64,
        status: MemoryEntityResolutionStatus,
    ) -> Result<MemoryEntityResolution, PerceptionError> {
        self.check_owner(container)?;
        validate_key(key)?;
        let actual = self.mentions.get(&key).map_or(0, |value| value.revision);
        if actual != expected_revision {
            return Err(PerceptionError::RevisionConflict {
                expected: expected_revision,
                actual,
            });
        }
        validate_status(&status, &self.entities)?;
        let revision = actual
            .checked_add(1)
            .ok_or(PerceptionError::VersionExhausted)?;
        let value = MemoryEntityResolution {
            key,
            revision,
            updated_at_ns,
            status,
        };
        let record = Record::Mention {
            version: self.next_version()?,
            owner: self.owner_id,
            value: value.clone(),
        };
        self.append(container, record)?;
        Ok(value)
    }

    fn check_owner(&self, container: &Container) -> Result<(), PerceptionError> {
        if container.owner_id() == self.owner_id && container.identity() == self.identity {
            Ok(())
        } else {
            Err(PerceptionError::OwnerMismatch)
        }
    }
    fn next_version(&self) -> Result<u64, PerceptionError> {
        self.version
            .checked_add(1)
            .ok_or(PerceptionError::VersionExhausted)
    }
    fn append(&mut self, container: &mut Container, record: Record) -> Result<(), PerceptionError> {
        self.check_owner(container)?;
        let bytes = record.encode()?;
        container.append(&bytes)?;
        container.sync()?;
        self.apply(record)
    }
    fn apply(&mut self, record: Record) -> Result<(), PerceptionError> {
        let expected = self.next_version()?;
        if record.version() != expected {
            return Err(PerceptionError::CorruptRecord(
                "noncontiguous owner version",
            ));
        }
        match &record {
            Record::Entity { value, .. } => {
                validate_entity(value)?;
                if self.mutations.contains_key(&value.mutation_id) {
                    return Err(PerceptionError::CorruptRecord("duplicate mutation ID"));
                }
                let expected_revision = self
                    .entities
                    .get(&value.id)
                    .map_or(0, |e| e.revision)
                    .checked_add(1)
                    .ok_or(PerceptionError::VersionExhausted)?;
                if value.revision != expected_revision {
                    return Err(PerceptionError::CorruptRecord("Entity revision sequence"));
                }
                self.mutations.insert(value.mutation_id.clone(), value.id);
                self.entity_history
                    .insert((value.id, value.revision), value.clone());
                self.entities.insert(value.id, value.clone());
            }
            Record::Mention { value, .. } => {
                validate_key(value.key)?;
                let actual = self.mentions.get(&value.key).map_or(0, |v| v.revision);
                if value.revision
                    != actual
                        .checked_add(1)
                        .ok_or(PerceptionError::VersionExhausted)?
                {
                    return Err(PerceptionError::CorruptRecord("mention revision sequence"));
                }
                validate_status(&value.status, &self.entities)?;
                self.mention_history
                    .insert((value.key, value.revision), value.clone());
                self.mentions.insert(value.key, value.clone());
            }
        }
        self.version = expected;
        Ok(())
    }
}

fn entity_id(mutation_id: &str) -> EntityId {
    let mut hash = Sha256::new();
    hash.update(b"reliquary-entity-id\0");
    hash.update((mutation_id.len() as u64).to_le_bytes());
    hash.update(mutation_id.as_bytes());
    EntityId(hash.finalize().into())
}
fn validate_draft(value: &EntityDraft) -> Result<(), PerceptionError> {
    if value.mutation_id.trim().is_empty()
        || value.canonical_name.trim().is_empty()
        || value.canonical_name.len() > MAX_ENTITY_NAME_BYTES
        || value.kind.trim().is_empty()
        || value.kind.len() > MAX_ENTITY_KIND_BYTES
        || value.summary.len() > MAX_ENTITY_SUMMARY_BYTES
        || value.aliases.len() > MAX_ENTITY_ALIASES
        || value
            .aliases
            .iter()
            .any(|s| s.trim().is_empty() || s.len() > MAX_ENTITY_NAME_BYTES)
        || value.updated_at_ns < value.created_at_ns
    {
        return Err(PerceptionError::InvalidField("Entity draft"));
    }
    Ok(())
}
fn validate_entity(value: &Entity) -> Result<(), PerceptionError> {
    validate_draft(&EntityDraft {
        canonical_name: value.canonical_name.clone(),
        aliases: value.aliases.clone(),
        kind: value.kind.clone(),
        summary: value.summary.clone(),
        mutation_id: value.mutation_id.clone(),
        created_at_ns: value.created_at_ns,
        updated_at_ns: value.updated_at_ns,
    })
}
fn validate_key(key: MemoryEntityMentionKey) -> Result<(), PerceptionError> {
    if key.start_byte >= key.end_byte {
        Err(PerceptionError::InvalidField("source span"))
    } else {
        Ok(())
    }
}
fn validate_status(
    status: &MemoryEntityResolutionStatus,
    entities: &BTreeMap<EntityId, Entity>,
) -> Result<(), PerceptionError> {
    match status {
        MemoryEntityResolutionStatus::Resolved { entity_id, .. } => {
            if !entities.contains_key(entity_id) {
                Err(PerceptionError::MissingEntity(*entity_id))
            } else {
                Ok(())
            }
        }
        MemoryEntityResolutionStatus::Ambiguous { candidates } => {
            if !(2..=MAX_MENTION_CANDIDATES).contains(&candidates.len())
                || candidates.windows(2).any(|pair| pair[0] >= pair[1])
            {
                return Err(PerceptionError::InvalidField(
                    "ambiguity candidates must be sorted, unique, and bounded",
                ));
            }
            for id in candidates {
                if !entities.contains_key(id) {
                    return Err(PerceptionError::MissingEntity(*id));
                }
            }
            Ok(())
        }
        MemoryEntityResolutionStatus::Unresolved { .. } => Ok(()),
    }
}
fn put_u64(out: &mut Vec<u8>, value: u64) {
    out.extend_from_slice(&value.to_le_bytes());
}
fn put_string(out: &mut Vec<u8>, value: &str) -> Result<(), PerceptionError> {
    let len =
        u32::try_from(value.len()).map_err(|_| PerceptionError::InvalidField("text length"))?;
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(value.as_bytes());
    Ok(())
}
fn put_strings(out: &mut Vec<u8>, values: &[String]) -> Result<(), PerceptionError> {
    out.push(u8::try_from(values.len()).map_err(|_| PerceptionError::InvalidField("aliases"))?);
    for value in values {
        put_string(out, value)?;
    }
    Ok(())
}
fn get_array<const N: usize>(bytes: &[u8], at: &mut usize) -> Result<[u8; N], PerceptionError> {
    let end = at
        .checked_add(N)
        .ok_or(PerceptionError::CorruptRecord("length overflow"))?;
    let raw = bytes
        .get(*at..end)
        .ok_or(PerceptionError::CorruptRecord("truncated record"))?;
    *at = end;
    Ok(raw.try_into().expect("checked length"))
}
fn get_u8(bytes: &[u8], at: &mut usize) -> Result<u8, PerceptionError> {
    Ok(get_array::<1>(bytes, at)?[0])
}
fn get_u32(bytes: &[u8], at: &mut usize) -> Result<u32, PerceptionError> {
    Ok(u32::from_le_bytes(get_array::<4>(bytes, at)?))
}
fn get_u64(bytes: &[u8], at: &mut usize) -> Result<u64, PerceptionError> {
    Ok(u64::from_le_bytes(get_array::<8>(bytes, at)?))
}
fn get_i64(bytes: &[u8], at: &mut usize) -> Result<i64, PerceptionError> {
    Ok(i64::from_le_bytes(get_array::<8>(bytes, at)?))
}
fn get_string(bytes: &[u8], at: &mut usize) -> Result<String, PerceptionError> {
    let len = get_u32(bytes, at)? as usize;
    let end = at
        .checked_add(len)
        .ok_or(PerceptionError::CorruptRecord("length overflow"))?;
    let raw = bytes
        .get(*at..end)
        .ok_or(PerceptionError::CorruptRecord("truncated string"))?;
    *at = end;
    String::from_utf8(raw.to_vec()).map_err(|_| PerceptionError::CorruptRecord("invalid UTF-8"))
}
fn get_strings(bytes: &[u8], at: &mut usize) -> Result<Vec<String>, PerceptionError> {
    let count = get_u8(bytes, at)? as usize;
    if count > MAX_ENTITY_ALIASES {
        return Err(PerceptionError::CorruptRecord("too many aliases"));
    }
    (0..count).map(|_| get_string(bytes, at)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use reliquary_container::{ContainerIdentity, ReliquaryScope};
    use std::{
        fs,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };

    static NEXT: AtomicU64 = AtomicU64::new(1);
    struct TempFile(PathBuf);
    impl TempFile {
        fn new() -> Self {
            let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".perception-test-tmp");
            fs::create_dir_all(&dir).unwrap();
            Self(dir.join(format!(
                "perception-{}-{}.rel",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            )))
        }
    }
    impl Drop for TempFile {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.0);
            if let Some(parent) = self.0.parent() {
                let _ = fs::remove_dir(parent);
            }
        }
    }
    fn create(path: &PathBuf, owner: u8) -> Container {
        let mut id = [0; 16];
        id[0] = owner;
        Container::create(
            path,
            ContainerIdentity::reliquary(ReliquaryScope::Project),
            id,
        )
        .unwrap()
    }
    fn draft(mutation_id: &str, name: &str) -> EntityDraft {
        EntityDraft {
            canonical_name: name.into(),
            aliases: vec![format!("{name} alias")],
            kind: "person".into(),
            summary: "fixture evidence only".into(),
            mutation_id: mutation_id.into(),
            created_at_ns: 10,
            updated_at_ns: 10,
        }
    }
    fn key() -> MemoryEntityMentionKey {
        MemoryEntityMentionKey {
            memory_id: [0xAB; 32],
            field: MemoryTextField::Content,
            start_byte: 17,
            end_byte: 25,
        }
    }

    #[test]
    fn entity_id_matches_reference_golden() {
        assert_eq!(
            entity_id("test").0,
            [
                0x57, 0xb8, 0xfd, 0x9a, 0xdf, 0xe3, 0x07, 0xfc, 0x2d, 0xe2, 0x9e, 0x68, 0x4d, 0x4e,
                0x57, 0xe2, 0xaf, 0x87, 0x56, 0x14, 0xa8, 0x1d, 0x19, 0xe9, 0xf3, 0x21, 0x60, 0x29,
                0xde, 0x81, 0xf3, 0x48,
            ]
        );
    }

    #[test]
    fn entities_and_ambiguous_source_spans_survive_reopen() {
        let path = TempFile::new();
        let alpha;
        let beta;
        {
            let mut container = create(&path.0, 1);
            let mut store = PerceptionStore::open(&container).unwrap();
            alpha = store
                .publish_entity(&mut container, draft("create-alpha", "Alpha"))
                .unwrap()
                .id;
            let mut revised = draft("revise-alpha", "Alpha Prime");
            revised.updated_at_ns = 11;
            assert_eq!(
                store
                    .update_entity(&mut container, alpha, 1, revised)
                    .unwrap()
                    .revision,
                2
            );
            beta = store
                .publish_entity(&mut container, draft("create-beta", "Beta"))
                .unwrap()
                .id;
            let candidates = if alpha < beta {
                vec![alpha, beta]
            } else {
                vec![beta, alpha]
            };
            store
                .put_mention_resolution(
                    &mut container,
                    key(),
                    0,
                    20,
                    MemoryEntityResolutionStatus::Ambiguous { candidates },
                )
                .unwrap();
            store
                .put_mention_resolution(
                    &mut container,
                    key(),
                    1,
                    21,
                    MemoryEntityResolutionStatus::Resolved {
                        entity_id: alpha,
                        reason: EntityResolutionReason::NamedReferent,
                    },
                )
                .unwrap();
        }
        let container = Container::open_read_only(&path.0).unwrap();
        let store = PerceptionStore::open(&container).unwrap();
        assert_eq!(store.entities().count(), 2);
        let mention = store.mention(key()).unwrap();
        assert_eq!(store.entity(alpha).unwrap().revision, 2);
        assert_eq!(store.entity(alpha).unwrap().canonical_name, "Alpha Prime");
        assert_eq!((mention.key.start_byte, mention.key.end_byte), (17, 25));
        assert_eq!(mention.revision, 2);
        assert_eq!(
            store.entity_revision(alpha, 1).unwrap().canonical_name,
            "Alpha"
        );
        assert!(store.entity_revision(alpha, 3).is_none());
        assert!(
            matches!(&store.mention_revision(key(), 1).unwrap().status, MemoryEntityResolutionStatus::Ambiguous { candidates } if candidates.len() == 2)
        );
        assert!(store.mention_revision(key(), 3).is_none());
    }

    #[test]
    fn resolution_requires_known_entity_and_owner() {
        let first_path = TempFile::new();
        let mut first = create(&first_path.0, 2);
        let mut store = PerceptionStore::open(&first).unwrap();
        let entity = store
            .publish_entity(&mut first, draft("known", "Known"))
            .unwrap()
            .id;
        store
            .put_mention_resolution(
                &mut first,
                key(),
                0,
                1,
                MemoryEntityResolutionStatus::Resolved {
                    entity_id: entity,
                    reason: EntityResolutionReason::NamedReferent,
                },
            )
            .unwrap();

        let other_path = TempFile::new();
        let mut other = create(&other_path.0, 3);
        assert!(matches!(
            store.put_mention_resolution(
                &mut other,
                key(),
                0,
                2,
                MemoryEntityResolutionStatus::Unresolved {
                    reason: EntityResolutionReason::InsufficientEvidence
                }
            ),
            Err(PerceptionError::OwnerMismatch)
        ));
        let phy_path = TempFile::new();
        let mut phy = Container::create(
            &phy_path.0,
            ContainerIdentity::phylactery(),
            first.owner_id(),
        )
        .unwrap();
        assert!(matches!(
            store.publish_entity(&mut phy, draft("foreign", "Foreign")),
            Err(PerceptionError::OwnerMismatch)
        ));
        assert!(phy.read_frames().unwrap().is_empty());
        assert!(matches!(
            store.put_mention_resolution(
                &mut first,
                MemoryEntityMentionKey {
                    start_byte: 5,
                    end_byte: 5,
                    ..key()
                },
                0,
                2,
                MemoryEntityResolutionStatus::Unresolved {
                    reason: EntityResolutionReason::InsufficientEvidence
                }
            ),
            Err(PerceptionError::InvalidField("source span"))
        ));
    }
}
