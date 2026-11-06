use crate::{
    EgoAnchor, EgoAnchorId, EgoAnchorPriority, EgoIdentity, EgoIdentityId, EgoLifecycleState,
    EgoPersonality, EgoWebSynthesis, MAX_EGO_ANCHOR_CHARS,
};
use reliquary_container::{Container, ContainerError, ContainerIdentity, FileKind};
use std::collections::BTreeMap;

mod history;
use std::fmt;

const RECORD_MAGIC: &[u8; 8] = b"RELEGO01";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EgoOwnerKind {
    Reliquary,
    Phylactery,
}

impl EgoOwnerKind {
    fn file_kind(self) -> FileKind {
        match self {
            Self::Reliquary => FileKind::Reliquary,
            Self::Phylactery => FileKind::Phylactery,
        }
    }

    fn from_file_kind(kind: FileKind) -> Self {
        match kind {
            FileKind::Reliquary => Self::Reliquary,
            FileKind::Phylactery => Self::Phylactery,
        }
    }
}

#[derive(Debug)]
pub enum EgoError {
    Container(ContainerError),
    WrongOwnerKind,
    OwnerIdentityMismatch,
    PrivateRecordInReliquary(&'static str),
    InvalidRecord(&'static str),
    RevisionConflict { expected: u64, actual: u64 },
    NotFound(&'static str),
    TerminalRecord(&'static str),
    VersionExhausted,
}

impl fmt::Display for EgoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Container(error) => error.fmt(f),
            Self::WrongOwnerKind => f.write_str("Ego state does not match container kind"),
            Self::OwnerIdentityMismatch => f.write_str("Ego state is bound to another owner"),
            Self::PrivateRecordInReliquary(kind) => {
                write!(f, "private Ego {kind} records are Phylactery-owned")
            }
            Self::InvalidRecord(reason) => write!(f, "invalid Ego record: {reason}"),
            Self::RevisionConflict { expected, actual } => {
                write!(
                    f,
                    "Ego revision conflict: expected {expected}, found {actual}"
                )
            }
            Self::NotFound(kind) => write!(f, "Ego {kind} was not found"),
            Self::TerminalRecord(kind) => write!(f, "Ego {kind} is tombstoned"),
            Self::VersionExhausted => f.write_str("Ego version exhausted"),
        }
    }
}

impl std::error::Error for EgoError {}

impl From<ContainerError> for EgoError {
    fn from(value: ContainerError) -> Self {
        Self::Container(value)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum EgoRecord {
    Identity {
        version: u64,
        id: EgoIdentityId,
        revision: u64,
        deleted: bool,
        name: String,
        text: String,
    },
    Personality {
        version: u64,
        revision: u64,
        source_memory_version: u64,
        text: String,
    },
    Anchor {
        version: u64,
        id: EgoAnchorId,
        revision: u64,
        deleted: bool,
        priority: EgoAnchorPriority,
        text: String,
    },
    Synthesis {
        version: u64,
        revision: u64,
        source_memory_version: u64,
        text: String,
    },
}

impl EgoRecord {
    fn version(&self) -> u64 {
        match self {
            Self::Identity { version, .. }
            | Self::Personality { version, .. }
            | Self::Anchor { version, .. }
            | Self::Synthesis { version, .. } => *version,
        }
    }

    fn private_record(&self) -> Option<&'static str> {
        match self {
            Self::Identity { .. } => Some("Identity"),
            Self::Personality { .. } => Some("Personality"),
            Self::Synthesis { .. } => None,
            Self::Anchor { .. } => None,
        }
    }

    fn encode(&self) -> Result<Vec<u8>, EgoError> {
        let mut out = RECORD_MAGIC.to_vec();
        match self {
            Self::Identity {
                version,
                id,
                revision,
                deleted,
                name,
                text,
            } => {
                out.push(1);
                put_u64(&mut out, *version);
                out.extend_from_slice(&id.0);
                put_u64(&mut out, *revision);
                out.push(u8::from(*deleted));
                put_string(&mut out, name)?;
                put_string(&mut out, text)?;
            }
            Self::Personality {
                version,
                revision,
                source_memory_version,
                text,
            } => {
                out.push(2);
                put_u64(&mut out, *version);
                put_u64(&mut out, *revision);
                put_u64(&mut out, *source_memory_version);
                put_string(&mut out, text)?;
            }
            Self::Anchor {
                version,
                id,
                revision,
                deleted,
                priority,
                text,
            } => {
                out.push(3);
                put_u64(&mut out, *version);
                out.extend_from_slice(&id.0);
                put_u64(&mut out, *revision);
                out.push(u8::from(*deleted));
                out.push(match priority {
                    EgoAnchorPriority::High => 1,
                    EgoAnchorPriority::Normal => 2,
                    EgoAnchorPriority::Low => 3,
                });
                put_string(&mut out, text)?;
            }
            Self::Synthesis {
                version,
                revision,
                source_memory_version,
                text,
            } => {
                out.push(4);
                put_u64(&mut out, *version);
                put_u64(&mut out, *revision);
                put_u64(&mut out, *source_memory_version);
                put_string(&mut out, text)?;
            }
        }
        Ok(out)
    }

    fn decode(bytes: &[u8]) -> Result<Option<Self>, EgoError> {
        if bytes.get(..8) != Some(RECORD_MAGIC.as_slice()) {
            return Ok(None);
        }
        let mut at = 8;
        let kind = get_u8(bytes, &mut at)?;
        let record = match kind {
            1 => {
                let version = get_u64(bytes, &mut at)?;
                let id = EgoIdentityId(get_array::<16>(bytes, &mut at)?);
                let revision = get_u64(bytes, &mut at)?;
                let deleted = get_bool(bytes, &mut at)?;
                let name = get_string(bytes, &mut at)?;
                let text = get_string(bytes, &mut at)?;
                Self::Identity {
                    version,
                    id,
                    revision,
                    deleted,
                    name,
                    text,
                }
            }
            2 => Self::Personality {
                version: get_u64(bytes, &mut at)?,
                revision: get_u64(bytes, &mut at)?,
                source_memory_version: get_u64(bytes, &mut at)?,
                text: get_string(bytes, &mut at)?,
            },
            3 => {
                let version = get_u64(bytes, &mut at)?;
                let id = EgoAnchorId(get_array::<16>(bytes, &mut at)?);
                let revision = get_u64(bytes, &mut at)?;
                let deleted = get_bool(bytes, &mut at)?;
                let priority = match get_u8(bytes, &mut at)? {
                    1 => EgoAnchorPriority::High,
                    2 => EgoAnchorPriority::Normal,
                    3 => EgoAnchorPriority::Low,
                    _ => return Err(EgoError::InvalidRecord("unknown anchor priority")),
                };
                let text = get_string(bytes, &mut at)?;
                Self::Anchor {
                    version,
                    id,
                    revision,
                    deleted,
                    priority,
                    text,
                }
            }
            4 => Self::Synthesis {
                version: get_u64(bytes, &mut at)?,
                revision: get_u64(bytes, &mut at)?,
                source_memory_version: get_u64(bytes, &mut at)?,
                text: get_string(bytes, &mut at)?,
            },
            _ => return Err(EgoError::InvalidRecord("unknown record kind")),
        };
        if at != bytes.len() {
            return Err(EgoError::InvalidRecord("record has trailing bytes"));
        }
        Ok(Some(record))
    }
}

/// Ego's durable projection over frames in its REL or PHY container.
///
/// Identity and Personality are private PHY facts. Anchors are owner-local and
/// may be stored in either owner. Ego inference and synthesis are not performed.
pub struct EgoStore {
    owner_kind: EgoOwnerKind,
    owner_id: Option<[u8; 16]>,
    container_identity: Option<ContainerIdentity>,
    version: u64,
    identities: BTreeMap<EgoIdentityId, EgoIdentity>,
    identity_history: BTreeMap<(EgoIdentityId, u64), EgoIdentity>,
    identity_tombstones: BTreeMap<EgoIdentityId, u64>,
    personality: Option<EgoPersonality>,
    personality_history: BTreeMap<u64, EgoPersonality>,
    synthesis: Option<EgoWebSynthesis>,
    synthesis_history: BTreeMap<u64, EgoWebSynthesis>,
    anchors: BTreeMap<EgoAnchorId, EgoAnchor>,
    anchor_history: BTreeMap<(EgoAnchorId, u64), EgoAnchor>,
    anchor_tombstones: BTreeMap<EgoAnchorId, u64>,
    records: Vec<EgoRecord>,
}

impl EgoStore {
    pub fn open(container: &Container) -> Result<Self, EgoError> {
        let identity = container.identity();
        let mut store = Self::empty(EgoOwnerKind::from_file_kind(identity.file_kind));
        store.owner_id = Some(container.owner_id());
        store.container_identity = Some(identity);
        for frame in container.read_frames()? {
            let Some(record) = EgoRecord::decode(&frame.payload)? else {
                continue;
            };
            store.apply_replayed(record)?;
        }
        Ok(store)
    }

    pub fn empty_for(file_kind: FileKind) -> Self {
        Self::empty(EgoOwnerKind::from_file_kind(file_kind))
    }

    fn empty(owner_kind: EgoOwnerKind) -> Self {
        Self {
            owner_kind,
            owner_id: None,
            container_identity: None,
            version: 0,
            identities: BTreeMap::new(),
            identity_history: BTreeMap::new(),
            identity_tombstones: BTreeMap::new(),
            personality: None,
            personality_history: BTreeMap::new(),
            synthesis: None,
            synthesis_history: BTreeMap::new(),
            anchors: BTreeMap::new(),
            anchor_history: BTreeMap::new(),
            anchor_tombstones: BTreeMap::new(),
            records: Vec::new(),
        }
    }

    pub fn owner_kind(&self) -> EgoOwnerKind {
        self.owner_kind
    }

    pub fn version(&self) -> u64 {
        self.version
    }

    pub fn identity(&self, id: EgoIdentityId) -> Option<&EgoIdentity> {
        self.identities.get(&id)
    }

    pub fn identities(&self) -> impl Iterator<Item = &EgoIdentity> {
        self.identities.values()
    }

    pub fn identity_revision(&self, id: EgoIdentityId, revision: u64) -> Option<&EgoIdentity> {
        self.identity_history.get(&(id, revision))
    }

    pub fn identity_lifecycle(&self, id: EgoIdentityId) -> Option<EgoLifecycleState> {
        self.identities
            .contains_key(&id)
            .then_some(EgoLifecycleState::Active)
            .or_else(|| {
                self.identity_tombstones.get(&id).copied().map(|version| {
                    EgoLifecycleState::Tombstoned {
                        ego_version: Some(version),
                    }
                })
            })
    }

    pub fn personality(&self) -> Option<&EgoPersonality> {
        self.personality.as_ref()
    }

    pub fn personality_revision(&self, revision: u64) -> Option<&EgoPersonality> {
        self.personality_history.get(&revision)
    }

    pub fn synthesis(&self) -> Option<&EgoWebSynthesis> {
        self.synthesis.as_ref()
    }

    pub fn synthesis_revision(&self, revision: u64) -> Option<&EgoWebSynthesis> {
        self.synthesis_history.get(&revision)
    }

    pub fn anchor(&self, id: EgoAnchorId) -> Option<&EgoAnchor> {
        self.anchors.get(&id)
    }

    pub fn anchors(&self) -> impl Iterator<Item = &EgoAnchor> {
        self.anchors.values()
    }

    pub fn anchor_revision(&self, id: EgoAnchorId, revision: u64) -> Option<&EgoAnchor> {
        self.anchor_history.get(&(id, revision))
    }

    pub fn anchor_lifecycle(&self, id: EgoAnchorId) -> Option<EgoLifecycleState> {
        self.anchors
            .contains_key(&id)
            .then_some(EgoLifecycleState::Active)
            .or_else(|| {
                self.anchor_tombstones.get(&id).copied().map(|version| {
                    EgoLifecycleState::Tombstoned {
                        ego_version: Some(version),
                    }
                })
            })
    }

    pub fn create_identity(
        &mut self,
        container: &mut Container,
        id: EgoIdentityId,
        name: String,
        text: String,
    ) -> Result<EgoIdentity, EgoError> {
        self.require_private_owner(container, "Identity")?;
        validate_text(&name, "Identity name")?;
        validate_text(&text, "Identity")?;
        if self.identities.contains_key(&id) || self.identity_tombstones.contains_key(&id) {
            return Err(EgoError::InvalidRecord("Identity ID is already claimed"));
        }
        let value = EgoIdentity {
            id,
            revision: 1,
            name,
            text,
        };
        self.append_apply(
            container,
            EgoRecord::Identity {
                version: self.next_version()?,
                id,
                revision: 1,
                deleted: false,
                name: value.name.clone(),
                text: value.text.clone(),
            },
        )?;
        Ok(value)
    }

    pub fn update_identity(
        &mut self,
        container: &mut Container,
        id: EgoIdentityId,
        expected_revision: u64,
        name: String,
        text: String,
    ) -> Result<EgoIdentity, EgoError> {
        self.require_private_owner(container, "Identity")?;
        validate_text(&name, "Identity name")?;
        validate_text(&text, "Identity")?;
        let current = self
            .identities
            .get(&id)
            .ok_or(EgoError::NotFound("Identity"))?;
        check_revision(expected_revision, current.revision)?;
        if current.name == name && current.text == text {
            return Ok(current.clone());
        }
        let value = EgoIdentity {
            id,
            revision: next_revision(current.revision)?,
            name,
            text,
        };
        self.append_apply(
            container,
            EgoRecord::Identity {
                version: self.next_version()?,
                id,
                revision: value.revision,
                deleted: false,
                name: value.name.clone(),
                text: value.text.clone(),
            },
        )?;
        Ok(value)
    }

    pub fn delete_identity(
        &mut self,
        container: &mut Container,
        id: EgoIdentityId,
        expected_revision: u64,
    ) -> Result<(), EgoError> {
        self.require_private_owner(container, "Identity")?;
        let current = self
            .identities
            .get(&id)
            .ok_or(EgoError::NotFound("Identity"))?;
        check_revision(expected_revision, current.revision)?;
        self.append_apply(
            container,
            EgoRecord::Identity {
                version: self.next_version()?,
                id,
                revision: next_revision(current.revision)?,
                deleted: true,
                name: String::new(),
                text: String::new(),
            },
        )
    }

    pub fn put_personality(
        &mut self,
        container: &mut Container,
        expected_revision: u64,
        text: String,
        source_memory_version: u64,
    ) -> Result<EgoPersonality, EgoError> {
        self.require_private_owner(container, "Personality")?;
        validate_text(&text, "Personality")?;
        let actual = self.personality.as_ref().map_or(0, |value| value.revision);
        check_revision(expected_revision, actual)?;
        if let Some(current) = &self.personality
            && current.text == text
            && current.source_memory_version == source_memory_version
        {
            return Ok(current.clone());
        }
        let value = EgoPersonality {
            revision: next_revision(actual)?,
            text,
            source_memory_version,
        };
        self.append_apply(
            container,
            EgoRecord::Personality {
                version: self.next_version()?,
                revision: value.revision,
                source_memory_version,
                text: value.text.clone(),
            },
        )?;
        Ok(value)
    }

    /// Persists supplied synthesis text without generating or evaluating it.
    pub fn put_synthesis(
        &mut self,
        container: &mut Container,
        expected_revision: u64,
        source_memory_version: u64,
        text: String,
    ) -> Result<EgoWebSynthesis, EgoError> {
        self.ensure_container(container)?;
        validate_text(&text, "Synthesis")?;
        let actual = self.synthesis.as_ref().map_or(0, |value| value.revision);
        check_revision(expected_revision, actual)?;
        if let Some(current) = &self.synthesis
            && current.text == text
            && current.source_memory_version == source_memory_version
        {
            return Ok(current.clone());
        }
        let value = EgoWebSynthesis {
            revision: next_revision(actual)?,
            source_memory_version,
            text,
        };
        self.append_apply(
            container,
            EgoRecord::Synthesis {
                version: self.next_version()?,
                revision: value.revision,
                source_memory_version,
                text: value.text.clone(),
            },
        )?;
        Ok(value)
    }

    pub fn create_anchor(
        &mut self,
        container: &mut Container,
        id: EgoAnchorId,
        priority: EgoAnchorPriority,
        text: String,
    ) -> Result<EgoAnchor, EgoError> {
        self.ensure_container(container)?;
        validate_anchor_text(&text)?;
        if self.anchors.contains_key(&id) || self.anchor_tombstones.contains_key(&id) {
            return Err(EgoError::InvalidRecord("Anchor ID is already claimed"));
        }
        let value = EgoAnchor {
            id,
            revision: 1,
            priority,
            text,
        };
        self.append_apply(
            container,
            EgoRecord::Anchor {
                version: self.next_version()?,
                id,
                revision: 1,
                deleted: false,
                priority,
                text: value.text.clone(),
            },
        )?;
        Ok(value)
    }

    pub fn update_anchor(
        &mut self,
        container: &mut Container,
        id: EgoAnchorId,
        expected_revision: u64,
        priority: EgoAnchorPriority,
        text: String,
    ) -> Result<EgoAnchor, EgoError> {
        self.ensure_container(container)?;
        validate_anchor_text(&text)?;
        let current = self.anchors.get(&id).ok_or_else(|| {
            if self.anchor_tombstones.contains_key(&id) {
                EgoError::TerminalRecord("Anchor")
            } else {
                EgoError::NotFound("Anchor")
            }
        })?;
        check_revision(expected_revision, current.revision)?;
        if current.priority == priority && current.text == text {
            return Ok(current.clone());
        }
        let value = EgoAnchor {
            id,
            revision: next_revision(current.revision)?,
            priority,
            text,
        };
        self.append_apply(
            container,
            EgoRecord::Anchor {
                version: self.next_version()?,
                id,
                revision: value.revision,
                deleted: false,
                priority,
                text: value.text.clone(),
            },
        )?;
        Ok(value)
    }

    pub fn delete_anchor(
        &mut self,
        container: &mut Container,
        id: EgoAnchorId,
        expected_revision: u64,
    ) -> Result<(), EgoError> {
        self.ensure_container(container)?;
        let current = self.anchors.get(&id).ok_or(EgoError::NotFound("Anchor"))?;
        check_revision(expected_revision, current.revision)?;
        self.append_apply(
            container,
            EgoRecord::Anchor {
                version: self.next_version()?,
                id,
                revision: next_revision(current.revision)?,
                deleted: true,
                priority: current.priority,
                text: String::new(),
            },
        )
    }

    fn require_private_owner(
        &mut self,
        container: &Container,
        kind: &'static str,
    ) -> Result<(), EgoError> {
        self.ensure_container(container)?;
        if self.owner_kind != EgoOwnerKind::Phylactery {
            return Err(EgoError::PrivateRecordInReliquary(kind));
        }
        Ok(())
    }

    fn ensure_container(&mut self, container: &Container) -> Result<(), EgoError> {
        if container.identity().file_kind != self.owner_kind.file_kind() {
            return Err(EgoError::WrongOwnerKind);
        }
        match self.owner_id {
            Some(owner_id)
                if owner_id != container.owner_id()
                    || self.container_identity != Some(container.identity()) =>
            {
                Err(EgoError::OwnerIdentityMismatch)
            }
            Some(_) => Ok(()),
            None => {
                *self = Self::open(container)?;
                Ok(())
            }
        }
    }

    fn next_version(&self) -> Result<u64, EgoError> {
        self.version
            .checked_add(1)
            .ok_or(EgoError::VersionExhausted)
    }

    fn append_apply(
        &mut self,
        container: &mut Container,
        record: EgoRecord,
    ) -> Result<(), EgoError> {
        self.ensure_container(container)?;
        let encoded = record.encode()?;
        container.append(&encoded)?;
        container.sync()?;
        self.apply(record, false)
    }

    fn apply_replayed(&mut self, record: EgoRecord) -> Result<(), EgoError> {
        if self.owner_kind == EgoOwnerKind::Reliquary
            && let Some(kind) = record.private_record()
        {
            return Err(EgoError::PrivateRecordInReliquary(kind));
        }
        if record.version() != self.next_version()? {
            return Err(EgoError::InvalidRecord(
                "Ego version sequence is not contiguous",
            ));
        }
        self.apply(record, true)
    }

    fn apply(&mut self, record: EgoRecord, replay: bool) -> Result<(), EgoError> {
        let accepted = record.clone();
        let version = record.version();
        match record {
            EgoRecord::Identity {
                id,
                revision,
                deleted,
                name,
                text,
                ..
            } => {
                if self.owner_kind != EgoOwnerKind::Phylactery {
                    return Err(EgoError::PrivateRecordInReliquary("Identity"));
                }
                if self.identity_tombstones.contains_key(&id) {
                    return Err(EgoError::TerminalRecord("Identity"));
                }
                let previous = self.identities.get(&id).map(|value| value.revision);
                if deleted && !self.identities.contains_key(&id) {
                    return Err(EgoError::InvalidRecord("cannot tombstone absent Identity"));
                }
                validate_revision(previous, revision)?;
                if deleted {
                    self.identities.remove(&id);
                    self.identity_tombstones.insert(id, version);
                } else {
                    let value = EgoIdentity {
                        id,
                        revision,
                        name,
                        text,
                    };
                    validate_text(&value.name, "Identity name")?;
                    validate_text(&value.text, "Identity")?;
                    self.identity_history.insert((id, revision), value.clone());
                    self.identities.insert(id, value);
                }
            }
            EgoRecord::Personality {
                revision,
                source_memory_version,
                text,
                ..
            } => {
                if self.owner_kind != EgoOwnerKind::Phylactery {
                    return Err(EgoError::PrivateRecordInReliquary("Personality"));
                }
                validate_revision(
                    self.personality.as_ref().map(|value| value.revision),
                    revision,
                )?;
                validate_text(&text, "Personality")?;
                let value = EgoPersonality {
                    revision,
                    text,
                    source_memory_version,
                };
                self.personality_history.insert(revision, value.clone());
                self.personality = Some(value);
            }
            EgoRecord::Synthesis {
                revision,
                source_memory_version,
                text,
                ..
            } => {
                validate_revision(
                    self.synthesis.as_ref().map(|value| value.revision),
                    revision,
                )?;
                validate_text(&text, "Synthesis")?;
                let value = EgoWebSynthesis {
                    revision,
                    source_memory_version,
                    text,
                };
                self.synthesis_history.insert(revision, value.clone());
                self.synthesis = Some(value);
            }
            EgoRecord::Anchor {
                id,
                revision,
                deleted,
                priority,
                text,
                ..
            } => {
                if self.anchor_tombstones.contains_key(&id) {
                    return Err(EgoError::TerminalRecord("Anchor"));
                }
                if deleted && !self.anchors.contains_key(&id) {
                    return Err(EgoError::InvalidRecord("cannot tombstone absent Anchor"));
                }
                validate_revision(self.anchors.get(&id).map(|value| value.revision), revision)?;
                if deleted {
                    self.anchors.remove(&id);
                    self.anchor_tombstones.insert(id, version);
                } else {
                    validate_anchor_text(&text)?;
                    let value = EgoAnchor {
                        id,
                        revision,
                        priority,
                        text,
                    };
                    self.anchor_history.insert((id, revision), value.clone());
                    self.anchors.insert(id, value);
                }
            }
        }
        if replay || version == self.next_version()? {
            self.version = version;
        }
        self.records.push(accepted);
        Ok(())
    }
}

fn validate_text(text: &str, _field: &'static str) -> Result<(), EgoError> {
    if text.trim().is_empty() {
        Err(EgoError::InvalidRecord("text must not be empty"))
    } else {
        Ok(())
    }
}

fn validate_anchor_text(text: &str) -> Result<(), EgoError> {
    validate_text(text, "Anchor")?;
    if text.chars().count() > MAX_EGO_ANCHOR_CHARS {
        return Err(EgoError::InvalidRecord("Anchor exceeds character limit"));
    }
    Ok(())
}

fn validate_revision(previous: Option<u64>, revision: u64) -> Result<(), EgoError> {
    let expected = previous
        .map(|value| value.checked_add(1).ok_or(EgoError::VersionExhausted))
        .transpose()?
        .unwrap_or(1);
    if revision != expected {
        return Err(EgoError::InvalidRecord("record revision is not contiguous"));
    }
    Ok(())
}

fn check_revision(expected: u64, actual: u64) -> Result<(), EgoError> {
    if expected == actual {
        Ok(())
    } else {
        Err(EgoError::RevisionConflict { expected, actual })
    }
}

fn next_revision(revision: u64) -> Result<u64, EgoError> {
    revision.checked_add(1).ok_or(EgoError::VersionExhausted)
}

fn put_u64(out: &mut Vec<u8>, value: u64) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn put_string(out: &mut Vec<u8>, value: &str) -> Result<(), EgoError> {
    let len =
        u32::try_from(value.len()).map_err(|_| EgoError::InvalidRecord("text is too large"))?;
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(value.as_bytes());
    Ok(())
}

fn get_u8(bytes: &[u8], cursor: &mut usize) -> Result<u8, EgoError> {
    let value = *bytes
        .get(*cursor)
        .ok_or(EgoError::InvalidRecord("record is truncated"))?;
    *cursor += 1;
    Ok(value)
}

fn get_bool(bytes: &[u8], cursor: &mut usize) -> Result<bool, EgoError> {
    match get_u8(bytes, cursor)? {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(EgoError::InvalidRecord("boolean tag is invalid")),
    }
}

fn get_array<const N: usize>(bytes: &[u8], cursor: &mut usize) -> Result<[u8; N], EgoError> {
    let end = cursor
        .checked_add(N)
        .ok_or(EgoError::InvalidRecord("record length overflow"))?;
    let raw = bytes
        .get(*cursor..end)
        .ok_or(EgoError::InvalidRecord("record is truncated"))?;
    *cursor = end;
    Ok(raw.try_into().expect("slice length checked"))
}

fn get_u64(bytes: &[u8], cursor: &mut usize) -> Result<u64, EgoError> {
    Ok(u64::from_le_bytes(get_array::<8>(bytes, cursor)?))
}

fn get_string(bytes: &[u8], cursor: &mut usize) -> Result<String, EgoError> {
    let len = u32::from_le_bytes(get_array::<4>(bytes, cursor)?) as usize;
    let end = cursor
        .checked_add(len)
        .ok_or(EgoError::InvalidRecord("string length overflow"))?;
    let raw = bytes
        .get(*cursor..end)
        .ok_or(EgoError::InvalidRecord("string is truncated"))?;
    *cursor = end;
    String::from_utf8(raw.to_vec()).map_err(|_| EgoError::InvalidRecord("text is not UTF-8"))
}
