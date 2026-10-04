//! RQOP0001 Gateway-owned, REL-scoped operational journal.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};
use uuid::Uuid;

const MAGIC: &[u8; 8] = b"RQOP0001";
const VERSION: u32 = 1;
const HEADER_LEN: u64 = 28;
const PREFIX_BODY_LEN: usize = 13; // payload length, sequence, record tag
const PREFIX_LEN: usize = PREFIX_BODY_LEN + 32; // prefix checksum protects framing
const RECORD_CHECKSUM_LEN: usize = 32;
const MAX_FRAME_LEN: usize = 16 * 1024 * 1024;
const MAX_FILE_LEN: u64 = 256 * 1024 * 1024;
const MAX_RECORDS: usize = 100_000;
const TAG_SNAPSHOT: u8 = 0;
const TAG_ATTRIBUTION: u8 = 1;
const TAG_EPISODE_PLAN: u8 = 2;
const TAG_RECEIPT_OBSERVED: u8 = 3;
const TAG_HELD: u8 = 4;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ActorClass {
    User,
    Agent,
    Service,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PersonalAdmission {
    pub mount_epoch: u64,
    pub grant_generation: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AdmissionIdentity {
    pub instance_id: String,
    pub principal_id: Option<String>,
    pub phy_owner_id: Option<String>,
    #[serde(default)]
    pub personal: Option<PersonalAdmission>,
    pub mount_epoch: u64,
    pub grant_generation: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Attribution {
    pub generation_id: String,
    pub conversation_id: String,
    pub submission_id: String,
    pub actor_class: ActorClass,
    pub actor_principal: Option<String>,
    pub generation_initiator: Option<String>,
    /// Explicit admitted User generation for an Agent checkpoint/result. This
    /// is operational identity, never an Archive parent-node relationship.
    #[serde(default)]
    pub initiating_generation_id: Option<String>,
    pub admission: AdmissionIdentity,
}

/// Exact concrete PreparedMemory projection. The parent supplies a JSON codec for
/// MemoryDraft, TemporalInference, and routing metadata; assessment may be rederived.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FrozenPreparedMemory {
    pub prepared_memory: serde_json::Value,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FrozenPersonalDraft {
    pub prepared_memory: FrozenPreparedMemory,
    pub source_ref: serde_json::Value,
    pub mutation_id: String,
    pub destination_principal: String,
    pub destination_phy_owner_id: String,
    /// Exact durable User admissions authorizing all material source references.
    #[serde(default)]
    pub authorizing_generations: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FrozenRejection {
    pub candidate_key: Option<String>,
    pub reason: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FrozenPreparedApplication {
    pub project_drafts: Vec<FrozenPreparedMemory>,
    pub personal_drafts: Vec<FrozenPersonalDraft>,
    pub rejected: Vec<FrozenRejection>,
    pub model: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EpisodePlan {
    pub episode_id: String,
    pub attempt: u32,
    pub source_fingerprint: String,
    pub expected_receipt_id: String,
    pub prepared: FrozenPreparedApplication,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct OwnerMemoryRef {
    pub owner_id: String,
    pub memory_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EpisodeReceiptObserved {
    pub episode_id: String,
    pub receipt_id: String,
    pub verified_external_refs: Vec<OwnerMemoryRef>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Held {
    pub operation_id: String,
    pub reason: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum OwnerOperationRecord {
    Attribution(Attribution),
    EpisodePlan(EpisodePlan),
    EpisodeReceiptObserved(EpisodeReceiptObserved),
    Held(Held),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct Snapshot {
    records: Vec<SequencedRecord>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SequencedRecord {
    pub sequence: u64,
    pub record: OwnerOperationRecord,
}

#[derive(Debug)]
pub enum JournalError {
    Io(std::io::Error),
    Json(serde_json::Error),
    InvalidRelUuid,
    Corrupt(&'static str),
    SequenceOverflow,
    JournalFull,
    InvalidRecord(&'static str),
    WrongRel { expected: Uuid, actual: Uuid },
}
impl fmt::Display for JournalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "owner-operation journal I/O: {e}"),
            Self::Json(e) => write!(f, "owner-operation journal encoding: {e}"),
            Self::InvalidRelUuid => write!(f, "invalid REL UUID"),
            Self::Corrupt(m) => write!(f, "owner-operation journal corrupt: {m}"),
            Self::SequenceOverflow => write!(f, "owner-operation sequence overflow"),
            Self::JournalFull => write!(f, "owner-operation journal size limit exceeded"),
            Self::InvalidRecord(m) => write!(f, "invalid owner-operation record: {m}"),
            Self::WrongRel { expected, actual } => write!(
                f,
                "journal REL mismatch: expected {expected}, found {actual}"
            ),
        }
    }
}
impl std::error::Error for JournalError {}
impl From<std::io::Error> for JournalError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}
impl From<serde_json::Error> for JournalError {
    fn from(e: serde_json::Error) -> Self {
        Self::Json(e)
    }
}
type Result<T> = std::result::Result<T, JournalError>;

#[derive(Default)]
struct Seen {
    attributions: BTreeMap<String, Attribution>,
    plans: BTreeMap<String, EpisodePlan>,
    receipts: BTreeMap<String, EpisodeReceiptObserved>,
}

pub struct OwnerOperationJournal {
    path: PathBuf,
    rel_uuid: Uuid,
    records: Vec<SequencedRecord>,
    last_sequence: u64,
}

impl OwnerOperationJournal {
    /// Opens state_dir/owner-operations/<rel-uuid>.roj, creating/syncing the header.
    pub fn open(state_dir: impl AsRef<Path>, rel_uuid: &str) -> Result<Self> {
        let rel = Uuid::parse_str(rel_uuid).map_err(|_| JournalError::InvalidRelUuid)?;
        if rel.is_nil() || rel.to_string() != rel_uuid {
            return Err(JournalError::InvalidRelUuid);
        }
        let root = state_dir.as_ref().join("owner-operations");
        let existed = root.exists();
        fs::create_dir_all(&root)?;
        if !existed {
            sync_dir(state_dir.as_ref())?;
        }
        let path = root.join(format!("{rel}.roj"));
        if !path.exists() {
            match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(mut f) => {
                    write_header(&mut f, rel)?;
                    f.sync_all()?;
                    sync_dir(&root)?;
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(e) => return Err(e.into()),
            }
        }
        let (records, last_sequence) = read_file(&path, rel, true)?;
        Ok(Self {
            path,
            rel_uuid: rel,
            records,
            last_sequence,
        })
    }

    pub fn rel_uuid(&self) -> Uuid {
        self.rel_uuid
    }
    pub fn last_sequence(&self) -> u64 {
        self.last_sequence
    }
    pub fn records(&self) -> &[SequencedRecord] {
        &self.records
    }

    pub fn attribution(&self, generation_id: &str) -> Option<&Attribution> {
        self.records.iter().rev().find_map(|r| match &r.record {
            OwnerOperationRecord::Attribution(a) if a.generation_id == generation_id => Some(a),
            _ => None,
        })
    }

    /// Resolve only explicit durable source identity. Archive ancestry and its
    /// legacy principal projection never supply routing authority.
    pub fn source_attribution(
        &self,
        conversation_id: &str,
        node_id: &str,
        actor: ActorClass,
    ) -> Option<&Attribution> {
        let mut matches = self
            .records
            .iter()
            .filter_map(|record| match &record.record {
                OwnerOperationRecord::Attribution(value)
                    if value.conversation_id == conversation_id
                        && value.submission_id == node_id =>
                {
                    Some(value)
                }
                _ => None,
            });
        let value = matches.next()?;
        if matches.next().is_some() || value.actor_class != actor {
            return None;
        }
        Some(value)
    }

    pub fn authority_revoked(&self, attribution: &Attribution) -> bool {
        let origin = attribution
            .initiating_generation_id
            .as_deref()
            .unwrap_or(&attribution.generation_id);
        self.records.iter().any(|record| {
            matches!(&record.record,
                OwnerOperationRecord::Held(held) if held.operation_id == origin
                    && held.reason.starts_with("authorization revoked")
            )
        })
    }

    /// Called under the gateway publication-policy write gate, before grants change.
    pub fn revoke_personal_authority(
        &mut self,
        instance_id: &str,
        rel_submit_revoked: bool,
        revoked_phy: Option<&str>,
    ) -> Result<()> {
        let ids: Vec<String> = self
            .records
            .iter()
            .filter_map(|record| match &record.record {
                OwnerOperationRecord::Attribution(a)
                    if a.actor_class == ActorClass::User
                        && a.admission.instance_id == instance_id
                        && (rel_submit_revoked
                            || revoked_phy.is_some_and(|phy| {
                                a.admission.phy_owner_id.as_deref() == Some(phy)
                            }))
                        && !self.authority_revoked(a) =>
                {
                    Some(a.generation_id.clone())
                }
                _ => None,
            })
            .collect();
        for operation_id in ids {
            self.append_held(Held {
                operation_id,
                reason: "authorization revoked: captured personal admission".to_owned(),
            })?;
        }
        Ok(())
    }

    pub fn episode_plan(&self, episode_id: &str) -> Option<&EpisodePlan> {
        self.records.iter().rev().find_map(|r| match &r.record {
            OwnerOperationRecord::EpisodePlan(p) if p.episode_id == episode_id => Some(p),
            _ => None,
        })
    }

    pub fn append_attribution(&mut self, value: Attribution) -> Result<u64> {
        validate_attribution(&value)?;
        if let Some(old) = self.attribution(&value.generation_id) {
            if old == &value {
                return Ok(self.sequence_for_attribution(&value.generation_id).unwrap());
            }
            return Err(JournalError::InvalidRecord(
                "generation attribution is immutable",
            ));
        }
        validate_generation_binding(&value, |id| self.attribution(id))?;
        self.append(OwnerOperationRecord::Attribution(value))
    }

    /// Bind a generated source before persisting its first checkpoint/result.
    /// Identity is copied from the original durable User admission; callers do
    /// not provide a principal, PHY, attachment, or Archive parent.
    pub fn bind_agent_source(
        &mut self,
        initiating_generation_id: &str,
        generation_id: &str,
        source_node_id: &str,
    ) -> Result<u64> {
        let origin =
            self.attribution(initiating_generation_id)
                .ok_or(JournalError::InvalidRecord(
                    "initiating generation is unknown",
                ))?;
        if origin.actor_class != ActorClass::User {
            return Err(JournalError::InvalidRecord(
                "initiating generation is not User",
            ));
        }
        let value = Attribution {
            generation_id: generation_id.to_owned(),
            conversation_id: origin.conversation_id.clone(),
            submission_id: source_node_id.to_owned(),
            actor_class: ActorClass::Agent,
            actor_principal: None,
            generation_initiator: origin.generation_initiator.clone(),
            initiating_generation_id: Some(initiating_generation_id.to_owned()),
            admission: origin.admission.clone(),
        };
        self.append_attribution(value)
    }

    pub fn append_episode_plan(&mut self, value: EpisodePlan) -> Result<u64> {
        validate_plan(&value)?;
        if self.receipt(&value.episode_id).is_some() {
            return Err(JournalError::InvalidRecord(
                "receipt already observed for episode",
            ));
        }
        if let Some(old) = self.episode_plan(&value.episode_id) {
            if old == &value {
                return Ok(self.sequence_for_plan(&value.episode_id).unwrap());
            }
            return Err(JournalError::InvalidRecord(
                "frozen episode plan is immutable",
            ));
        }
        self.append(OwnerOperationRecord::EpisodePlan(value))
    }

    pub fn append_receipt_observed(&mut self, value: EpisodeReceiptObserved) -> Result<u64> {
        validate_receipt(&value)?;
        // Receipt check comes first so replay remains idempotent after compaction prunes
        // a fully receipted plan.
        if let Some(old) = self.receipt(&value.episode_id) {
            if old == &value {
                return Ok(self.sequence_for_receipt(&value.episode_id).unwrap());
            }
            return Err(JournalError::InvalidRecord("receipt observation conflicts"));
        }
        let plan = self
            .episode_plan(&value.episode_id)
            .ok_or(JournalError::InvalidRecord(
                "receipt requires a frozen plan",
            ))?;
        if plan.expected_receipt_id != value.receipt_id {
            return Err(JournalError::InvalidRecord(
                "receipt ID differs from frozen plan",
            ));
        }
        validate_receipt_for_plan(plan, &value)?;
        self.append(OwnerOperationRecord::EpisodeReceiptObserved(value))
    }

    pub fn append_held(&mut self, value: Held) -> Result<u64> {
        validate_held(&value)?;
        self.append(OwnerOperationRecord::Held(value))
    }

    fn receipt(&self, episode_id: &str) -> Option<&EpisodeReceiptObserved> {
        self.records.iter().rev().find_map(|r| match &r.record {
            OwnerOperationRecord::EpisodeReceiptObserved(x) if x.episode_id == episode_id => {
                Some(x)
            }
            _ => None,
        })
    }
    fn sequence_for_attribution(&self, id: &str) -> Option<u64> {
        self.records.iter().rev().find(|r| matches!(&r.record, OwnerOperationRecord::Attribution(x) if x.generation_id == id)).map(|r| r.sequence)
    }
    fn sequence_for_plan(&self, id: &str) -> Option<u64> {
        self.records
            .iter()
            .rev()
            .find(
                |r| matches!(&r.record, OwnerOperationRecord::EpisodePlan(x) if x.episode_id == id),
            )
            .map(|r| r.sequence)
    }
    fn sequence_for_receipt(&self, id: &str) -> Option<u64> {
        self.records.iter().rev().find(|r| matches!(&r.record, OwnerOperationRecord::EpisodeReceiptObserved(x) if x.episode_id == id)).map(|r| r.sequence)
    }

    fn append(&mut self, record: OwnerOperationRecord) -> Result<u64> {
        let sequence = self
            .last_sequence
            .checked_add(1)
            .ok_or(JournalError::SequenceOverflow)?;
        let bytes = encode_frame(sequence, &record)?;
        if fs::metadata(&self.path)?
            .len()
            .checked_add(bytes.len() as u64)
            .ok_or(JournalError::JournalFull)?
            > MAX_FILE_LEN
        {
            self.compact()?;
        }
        if fs::metadata(&self.path)?
            .len()
            .checked_add(bytes.len() as u64)
            .ok_or(JournalError::JournalFull)?
            > MAX_FILE_LEN
        {
            return Err(JournalError::JournalFull);
        }
        let mut f = OpenOptions::new().append(true).open(&self.path)?;
        f.write_all(&bytes)?;
        f.sync_all()?;
        self.records.push(SequencedRecord { sequence, record });
        self.last_sequence = sequence;
        Ok(sequence)
    }

    /// Atomic snapshot. Completed EpisodePlans are pruned only after their observed
    /// receipt is durable; source attributions and unresolved plans are retained.
    pub fn compact(&mut self) -> Result<()> {
        if self.last_sequence == 0 {
            return Ok(());
        }
        let snapshot = Snapshot {
            records: compact_records(&self.records),
        };
        let payload = serde_json::to_vec(&snapshot)?;
        let frame = encode_payload(self.last_sequence, TAG_SNAPSHOT, &payload)?;
        if HEADER_LEN + frame.len() as u64 > MAX_FILE_LEN {
            return Err(JournalError::JournalFull);
        }
        atomic_replace(&self.path, self.rel_uuid, &frame)?;
        self.records = snapshot.records;
        Ok(())
    }
}

fn compact_records(records: &[SequencedRecord]) -> Vec<SequencedRecord> {
    let completed: BTreeSet<&str> = records
        .iter()
        .filter_map(|r| match &r.record {
            OwnerOperationRecord::EpisodeReceiptObserved(x) => Some(x.episode_id.as_str()),
            _ => None,
        })
        .collect();
    let mut latest_held = BTreeMap::<&str, usize>::new();
    for (i, r) in records.iter().enumerate() {
        if let OwnerOperationRecord::Held(h) = &r.record {
            latest_held.insert(&h.operation_id, i);
        }
    }
    records
        .iter()
        .enumerate()
        .filter_map(|(i, r)| match &r.record {
            OwnerOperationRecord::EpisodePlan(p) if completed.contains(p.episode_id.as_str()) => {
                None
            }
            OwnerOperationRecord::Held(h)
                if !h.reason.starts_with("authorization revoked")
                    && latest_held.get(h.operation_id.as_str()) != Some(&i) =>
            {
                None
            }
            _ => Some(r.clone()),
        })
        .collect()
}

fn write_header(f: &mut File, rel: Uuid) -> Result<()> {
    f.write_all(MAGIC)?;
    f.write_all(&VERSION.to_le_bytes())?;
    f.write_all(rel.as_bytes())?;
    Ok(())
}
fn encode_frame(sequence: u64, record: &OwnerOperationRecord) -> Result<Vec<u8>> {
    let (tag, payload) = match record {
        OwnerOperationRecord::Attribution(v) => (TAG_ATTRIBUTION, serde_json::to_vec(v)?),
        OwnerOperationRecord::EpisodePlan(v) => (TAG_EPISODE_PLAN, serde_json::to_vec(v)?),
        OwnerOperationRecord::EpisodeReceiptObserved(v) => {
            (TAG_RECEIPT_OBSERVED, serde_json::to_vec(v)?)
        }
        OwnerOperationRecord::Held(v) => (TAG_HELD, serde_json::to_vec(v)?),
    };
    encode_payload(sequence, tag, &payload)
}
fn encode_payload(sequence: u64, tag: u8, payload: &[u8]) -> Result<Vec<u8>> {
    let payload_len = u32::try_from(payload.len()).map_err(|_| JournalError::JournalFull)?;
    let body_len = 1usize
        .checked_add(payload.len())
        .ok_or(JournalError::JournalFull)?;
    if body_len > MAX_FRAME_LEN {
        return Err(JournalError::JournalFull);
    }
    let mut prefix_body = Vec::with_capacity(PREFIX_BODY_LEN);
    prefix_body.extend_from_slice(&payload_len.to_le_bytes());
    prefix_body.extend_from_slice(&sequence.to_le_bytes());
    prefix_body.push(tag);
    let prefix_hash = Sha256::digest(&prefix_body);
    let mut record_hash_input = prefix_body.clone();
    record_hash_input.extend_from_slice(payload);
    let record_hash = Sha256::digest(&record_hash_input);
    let mut out = Vec::with_capacity(PREFIX_LEN + payload.len() + RECORD_CHECKSUM_LEN);
    out.extend_from_slice(&prefix_body);
    out.extend_from_slice(&prefix_hash);
    out.extend_from_slice(payload);
    out.extend_from_slice(&record_hash);
    Ok(out)
}

fn read_file(
    path: &Path,
    expected: Uuid,
    repair_tail: bool,
) -> Result<(Vec<SequencedRecord>, u64)> {
    let mut f = OpenOptions::new()
        .read(true)
        .write(repair_tail)
        .open(path)?;
    let size = f.metadata()?.len();
    if size > MAX_FILE_LEN {
        return Err(JournalError::JournalFull);
    }
    let mut header = [0u8; HEADER_LEN as usize];
    f.read_exact(&mut header)
        .map_err(|_| JournalError::Corrupt("short header"))?;
    if &header[..8] != MAGIC {
        return Err(JournalError::Corrupt("bad magic"));
    }
    if u32::from_le_bytes(header[8..12].try_into().unwrap()) != VERSION {
        return Err(JournalError::Corrupt("unsupported version"));
    }
    let actual =
        Uuid::from_slice(&header[12..28]).map_err(|_| JournalError::Corrupt("invalid REL UUID"))?;
    if actual.is_nil() {
        return Err(JournalError::Corrupt("nil REL UUID"));
    }
    if actual != expected {
        return Err(JournalError::WrongRel { expected, actual });
    }

    let mut offset = HEADER_LEN;
    let mut last = 0u64;
    let mut records = Vec::new();
    let mut seen = Seen::default();
    let mut snapshot_seen = false;
    while offset < size {
        let start = offset;
        let remaining = (size - offset) as usize;
        if remaining < PREFIX_LEN {
            if repair_tail {
                f.set_len(start)?;
                f.sync_all()?;
                break;
            }
            return Err(JournalError::Corrupt("truncated final frame prefix"));
        }
        f.seek(SeekFrom::Start(offset))?;
        let mut prefix = [0u8; PREFIX_LEN];
        f.read_exact(&mut prefix)?;
        let mut prefix_body = prefix[..PREFIX_BODY_LEN].to_vec();
        if Sha256::digest(&prefix_body).as_slice() != &prefix[PREFIX_BODY_LEN..] {
            return Err(JournalError::Corrupt("frame prefix checksum mismatch"));
        }
        let payload_len = u32::from_le_bytes(prefix[..4].try_into().unwrap()) as usize;
        let sequence = u64::from_le_bytes(prefix[4..12].try_into().unwrap());
        let tag = prefix[12];
        let body_len = 1usize
            .checked_add(payload_len)
            .ok_or(JournalError::Corrupt("frame length overflow"))?;
        if body_len > MAX_FRAME_LEN {
            return Err(JournalError::Corrupt("frame exceeds bound"));
        }
        let total = PREFIX_LEN
            .checked_add(payload_len)
            .and_then(|x| x.checked_add(RECORD_CHECKSUM_LEN))
            .ok_or(JournalError::Corrupt("frame length overflow"))?;
        if remaining < total {
            if repair_tail {
                f.set_len(start)?;
                f.sync_all()?;
                break;
            }
            return Err(JournalError::Corrupt("truncated final frame payload"));
        }
        let mut payload = vec![0u8; payload_len];
        f.read_exact(&mut payload)?;
        let mut got = [0u8; RECORD_CHECKSUM_LEN];
        f.read_exact(&mut got)?;
        prefix_body.extend_from_slice(&payload);
        if Sha256::digest(&prefix_body).as_slice() != got {
            return Err(JournalError::Corrupt("frame checksum mismatch"));
        }
        if sequence <= last {
            return Err(JournalError::Corrupt("non-monotonic sequence"));
        }

        if tag == TAG_SNAPSHOT {
            if offset != HEADER_LEN || snapshot_seen || sequence == 0 {
                return Err(JournalError::Corrupt(
                    "invalid snapshot position or watermark",
                ));
            }
            let snapshot: Snapshot = serde_json::from_slice(&payload)?;
            validate_snapshot(&snapshot, sequence)?;
            records = snapshot.records;
            seen = seen_from_snapshot(&records)?;
            last = sequence;
            snapshot_seen = true;
        } else {
            if sequence != last.checked_add(1).ok_or(JournalError::SequenceOverflow)? {
                return Err(JournalError::Corrupt("sequence gap"));
            }
            let record = decode_record(tag, &payload)?;
            validate_record(&record)
                .map_err(|_| JournalError::Corrupt("invalid record payload"))?;
            apply_seen(&mut seen, &record, false)
                .map_err(|_| JournalError::Corrupt("invalid record transition"))?;
            records.push(SequencedRecord { sequence, record });
            if records.len() > MAX_RECORDS {
                return Err(JournalError::JournalFull);
            }
            last = sequence;
        }
        offset += total as u64;
    }
    Ok((records, last))
}

fn validate_snapshot(snapshot: &Snapshot, watermark: u64) -> Result<()> {
    if snapshot.records.is_empty() || snapshot.records.len() > MAX_RECORDS {
        return Err(JournalError::Corrupt("invalid snapshot record count"));
    }
    let mut previous = 0;
    let mut attribution_ids = BTreeSet::new();
    let mut plan_ids = BTreeSet::new();
    let mut receipt_ids = BTreeSet::new();
    for item in &snapshot.records {
        if item.sequence == 0 || item.sequence <= previous || item.sequence > watermark {
            return Err(JournalError::Corrupt("snapshot sequence order invalid"));
        }
        validate_record(&item.record)
            .map_err(|_| JournalError::Corrupt("invalid snapshot record"))?;
        let unique = match &item.record {
            OwnerOperationRecord::Attribution(x) => {
                attribution_ids.insert(x.generation_id.as_str())
            }
            OwnerOperationRecord::EpisodePlan(x) => plan_ids.insert(x.episode_id.as_str()),
            OwnerOperationRecord::EpisodeReceiptObserved(x) => {
                receipt_ids.insert(x.episode_id.as_str())
            }
            OwnerOperationRecord::Held(_) => true,
        };
        if !unique {
            return Err(JournalError::Corrupt("duplicate snapshot identity"));
        }
        previous = item.sequence;
    }
    let _ = seen_from_snapshot(&snapshot.records)?;
    Ok(())
}
fn seen_from_snapshot(records: &[SequencedRecord]) -> Result<Seen> {
    let mut seen = Seen::default();
    for item in records {
        apply_seen(&mut seen, &item.record, true)
            .map_err(|_| JournalError::Corrupt("snapshot semantic conflict"))?;
    }
    Ok(seen)
}
fn apply_seen(
    seen: &mut Seen,
    record: &OwnerOperationRecord,
    compacted_snapshot: bool,
) -> Result<()> {
    match record {
        OwnerOperationRecord::Attribution(a) => {
            if seen.attributions.contains_key(&a.generation_id) {
                return Err(JournalError::InvalidRecord(
                    "duplicate immutable attribution",
                ));
            }
            validate_generation_binding(a, |id| seen.attributions.get(id))?;
            seen.attributions.insert(a.generation_id.clone(), a.clone());
        }
        OwnerOperationRecord::EpisodePlan(p) => {
            if seen.plans.contains_key(&p.episode_id) || seen.receipts.contains_key(&p.episode_id) {
                return Err(JournalError::InvalidRecord(
                    "duplicate or late immutable plan",
                ));
            }
            seen.plans.insert(p.episode_id.clone(), p.clone());
        }
        OwnerOperationRecord::EpisodeReceiptObserved(r) => {
            if seen.receipts.contains_key(&r.episode_id) {
                return Err(JournalError::InvalidRecord("duplicate receipt"));
            }
            if let Some(plan) = seen.plans.get(&r.episode_id) {
                validate_receipt_for_plan(plan, r)?;
            } else if !compacted_snapshot {
                return Err(JournalError::InvalidRecord("receipt lacks preceding plan"));
            }
            seen.receipts.insert(r.episode_id.clone(), r.clone());
        }
        OwnerOperationRecord::Held(_) => {}
    }
    Ok(())
}
fn decode_record(tag: u8, bytes: &[u8]) -> Result<OwnerOperationRecord> {
    Ok(match tag {
        TAG_ATTRIBUTION => OwnerOperationRecord::Attribution(serde_json::from_slice(bytes)?),
        TAG_EPISODE_PLAN => OwnerOperationRecord::EpisodePlan(serde_json::from_slice(bytes)?),
        TAG_RECEIPT_OBSERVED => {
            OwnerOperationRecord::EpisodeReceiptObserved(serde_json::from_slice(bytes)?)
        }
        TAG_HELD => OwnerOperationRecord::Held(serde_json::from_slice(bytes)?),
        _ => return Err(JournalError::Corrupt("unknown frame kind")),
    })
}
fn validate_record(record: &OwnerOperationRecord) -> Result<()> {
    match record {
        OwnerOperationRecord::Attribution(x) => validate_attribution(x),
        OwnerOperationRecord::EpisodePlan(x) => validate_plan(x),
        OwnerOperationRecord::EpisodeReceiptObserved(x) => validate_receipt(x),
        OwnerOperationRecord::Held(x) => validate_held(x),
    }
}
fn validate_attribution(x: &Attribution) -> Result<()> {
    if let Some(personal) = &x.admission.personal {
        if x.admission.phy_owner_id.is_none()
            || personal.mount_epoch == 0
            || personal.grant_generation == 0
        {
            return Err(JournalError::InvalidRecord(
                "personal admission lacks exact PHY authority",
            ));
        }
    }
    for s in [
        &x.generation_id,
        &x.conversation_id,
        &x.submission_id,
        &x.admission.instance_id,
    ] {
        validate_id(s)?;
    }
    if x.actor_class == ActorClass::User && x.actor_principal.as_deref().map_or(true, str::is_empty)
    {
        return Err(JournalError::InvalidRecord("User actor principal missing"));
    }
    if x.actor_class != ActorClass::User && x.actor_principal.is_some() {
        return Err(JournalError::InvalidRecord("non-User actor has principal"));
    }
    for s in [
        &x.actor_principal,
        &x.generation_initiator,
        &x.initiating_generation_id,
        &x.admission.principal_id,
        &x.admission.phy_owner_id,
    ]
    .into_iter()
    .flatten()
    {
        validate_id(s)?;
    }
    match x.actor_class {
        ActorClass::User => {
            if x.initiating_generation_id.is_some()
                || x.actor_principal != x.generation_initiator
                || x.actor_principal != x.admission.principal_id
            {
                return Err(JournalError::InvalidRecord(
                    "User admission identity conflicts",
                ));
            }
        }
        ActorClass::Agent => {
            if x.initiating_generation_id.is_none() || x.generation_initiator.is_none() {
                return Err(JournalError::InvalidRecord(
                    "Agent generation binding missing",
                ));
            }
        }
        ActorClass::Service => {
            if x.initiating_generation_id.is_some() {
                return Err(JournalError::InvalidRecord(
                    "Service cannot bind a User generation",
                ));
            }
        }
    }
    Ok(())
}

fn validate_generation_binding<'a>(
    value: &Attribution,
    lookup: impl FnOnce(&str) -> Option<&'a Attribution>,
) -> Result<()> {
    let Some(id) = value.initiating_generation_id.as_deref() else {
        return Ok(());
    };
    let origin = lookup(id).ok_or(JournalError::InvalidRecord(
        "Agent lacks preceding initiating generation",
    ))?;
    if origin.actor_class != ActorClass::User
        || origin.conversation_id != value.conversation_id
        || origin.generation_initiator != value.generation_initiator
        || origin.admission != value.admission
    {
        return Err(JournalError::InvalidRecord(
            "Agent initiating admission conflicts",
        ));
    }
    Ok(())
}

fn validate_plan(x: &EpisodePlan) -> Result<()> {
    for s in [&x.episode_id, &x.source_fingerprint, &x.expected_receipt_id] {
        validate_id(s)?;
    }
    if x.prepared.project_drafts.len()
        + x.prepared.personal_drafts.len()
        + x.prepared.rejected.len()
        > MAX_RECORDS
    {
        return Err(JournalError::InvalidRecord("too many prepared plan items"));
    }
    for m in &x.prepared.project_drafts {
        validate_prepared(m)?;
    }
    for p in &x.prepared.personal_drafts {
        if p.authorizing_generations.len() > 64 {
            return Err(JournalError::InvalidRecord(
                "too many authorizing generations",
            ));
        }
        let mut generations = BTreeSet::new();
        for id in &p.authorizing_generations {
            validate_id(id)?;
            if !generations.insert(id) {
                return Err(JournalError::InvalidRecord(
                    "duplicate authorizing generation",
                ));
            }
        }
        validate_prepared(&p.prepared_memory)?;
        validate_id(&p.mutation_id)?;
        validate_id(&p.destination_principal)?;
        validate_id(&p.destination_phy_owner_id)?;
        if !p.source_ref.is_object() {
            return Err(JournalError::InvalidRecord(
                "personal source_ref must be an object",
            ));
        }
        if serde_json::to_vec(&p.source_ref)?.len() > 64 * 1024 {
            return Err(JournalError::InvalidRecord("source_ref exceeds bound"));
        }
    }
    for r in &x.prepared.rejected {
        if let Some(k) = &r.candidate_key {
            validate_id(k)?;
        }
        validate_text(&r.reason)?;
    }
    validate_text(&x.prepared.model)
}
fn validate_prepared(x: &FrozenPreparedMemory) -> Result<()> {
    if !x.prepared_memory.is_object() {
        return Err(JournalError::InvalidRecord(
            "prepared memory must be an object",
        ));
    }
    if serde_json::to_vec(&x.prepared_memory)?.len() > MAX_FRAME_LEN / 2 {
        return Err(JournalError::InvalidRecord("prepared memory exceeds bound"));
    }
    Ok(())
}
fn validate_receipt(x: &EpisodeReceiptObserved) -> Result<()> {
    validate_id(&x.episode_id)?;
    validate_id(&x.receipt_id)?;
    if x.verified_external_refs.len() > 4096 {
        return Err(JournalError::InvalidRecord("too many external refs"));
    }
    let mut refs = BTreeSet::new();
    for r in &x.verified_external_refs {
        validate_id(&r.owner_id)?;
        validate_id(&r.memory_id)?;
        if !refs.insert((&r.owner_id, &r.memory_id)) {
            return Err(JournalError::InvalidRecord("duplicate external Memory ref"));
        }
    }
    Ok(())
}
fn validate_receipt_for_plan(plan: &EpisodePlan, receipt: &EpisodeReceiptObserved) -> Result<()> {
    if plan.episode_id != receipt.episode_id || plan.expected_receipt_id != receipt.receipt_id {
        return Err(JournalError::InvalidRecord(
            "receipt identity differs from frozen plan",
        ));
    }
    let mut expected = BTreeMap::<&str, usize>::new();
    for item in &plan.prepared.personal_drafts {
        *expected
            .entry(item.destination_phy_owner_id.as_str())
            .or_default() += 1;
    }
    let mut actual = BTreeMap::<&str, usize>::new();
    for item in &receipt.verified_external_refs {
        *actual.entry(item.owner_id.as_str()).or_default() += 1;
    }
    if expected != actual {
        return Err(JournalError::InvalidRecord(
            "receipt refs do not match frozen personal destinations",
        ));
    }
    Ok(())
}
fn validate_held(x: &Held) -> Result<()> {
    validate_id(&x.operation_id)?;
    validate_text(&x.reason)
}
fn validate_id(x: &str) -> Result<()> {
    if x.is_empty() || x.trim() != x || x.len() > 4096 || x.chars().any(char::is_control) {
        return Err(JournalError::InvalidRecord("invalid identifier"));
    }
    Ok(())
}
fn validate_text(x: &str) -> Result<()> {
    if x.len() > 8 * 1024 * 1024 || x.contains('\0') {
        return Err(JournalError::InvalidRecord("text exceeds bound"));
    }
    Ok(())
}

fn atomic_replace(path: &Path, rel: Uuid, bytes: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .ok_or(JournalError::Corrupt("journal has no parent"))?;
    let temp = parent.join(format!(".{rel}-{}.tmp", Uuid::new_v4()));
    let mut f = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)?;
    write_header(&mut f, rel)?;
    f.write_all(bytes)?;
    f.sync_all()?;
    drop(f);
    let _ = read_file(&temp, rel, false)?;
    #[cfg(windows)]
    {
        use std::{ffi::OsStr, os::windows::ffi::OsStrExt};
        #[link(name = "Kernel32")]
        unsafe extern "system" {
            fn MoveFileExW(existing: *const u16, new: *const u16, flags: u32) -> i32;
        }
        let a: Vec<u16> = OsStr::new(&temp).encode_wide().chain(Some(0)).collect();
        let b: Vec<u16> = OsStr::new(path).encode_wide().chain(Some(0)).collect();
        if unsafe { MoveFileExW(a.as_ptr(), b.as_ptr(), 0x1 | 0x8) } == 0 {
            let e = std::io::Error::last_os_error();
            let _ = fs::remove_file(&temp);
            return Err(e.into());
        }
    }
    #[cfg(not(windows))]
    if let Err(e) = fs::rename(&temp, path) {
        let _ = fs::remove_file(&temp);
        return Err(e.into());
    }
    sync_dir(parent)
}
#[cfg(test)]
#[path = "runtime_host_owner_operation_journal_tests.rs"]
mod tests;

fn sync_dir(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        File::open(path)?.sync_all()?;
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
    Ok(())
}
