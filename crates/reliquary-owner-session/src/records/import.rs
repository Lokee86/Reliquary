use super::import_settlement::{self, valid_digest};
use super::{
    ImportComplete, ImportEpochStatus, ImportFreshnessSettlement, ImportStart, ImportWatermarks,
};
use reliquary_container::Container;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

const START_MAGIC_V1: &[u8; 8] = b"RELIEPS1";
const COMPLETE_MAGIC_V1: &[u8; 8] = b"RELIEPC1";
const START_MAGIC: &[u8; 8] = b"RELIEPS2";
const COMPLETE_MAGIC: &[u8; 8] = b"RELIEPC2";

#[path = "import_processing.rs"]
mod processing;
pub(crate) use processing::{
    ImportProcessingBinding, MAGIC as IMPORT_PROCESSING_MAGIC, processing_marker_id,
};

#[derive(Default)]
pub struct ImportEpochStore {
    owner: Option<[u8; 16]>,
    starts: BTreeMap<String, ImportStart>,
    completions: BTreeMap<String, ImportComplete>,
    settlements: BTreeMap<String, ImportFreshnessSettlement>,
    processing: BTreeMap<String, ImportProcessingBinding>,
    open: Option<String>,
}

impl ImportEpochStore {
    pub(crate) fn prefix(owner: [u8; 16]) -> Self {
        Self {
            owner: Some(owner),
            ..Self::default()
        }
    }

    /// Replay one physical epoch marker into a bounded prefix projection.
    pub(crate) fn ingest_prefix(&mut self, payload: &[u8]) -> Result<(), String> {
        if payload.starts_with(import_settlement::MAGIC) {
            self.ingest_settlement(ImportFreshnessSettlement::decode(payload)?)
        } else if payload.starts_with(IMPORT_PROCESSING_MAGIC) {
            self.ingest_processing_binding(payload)
        } else if payload.starts_with(START_MAGIC) {
            self.ingest_start(decode(payload, START_MAGIC)?)
        } else if payload.starts_with(COMPLETE_MAGIC) {
            self.ingest_complete(decode(payload, COMPLETE_MAGIC)?)
        } else if payload.starts_with(START_MAGIC_V1) {
            self.ingest_start(decode(payload, START_MAGIC_V1)?)
        } else if payload.starts_with(COMPLETE_MAGIC_V1) {
            self.ingest_complete(decode(payload, COMPLETE_MAGIC_V1)?)
        } else {
            Err("unsupported import epoch marker".into())
        }
    }

    pub fn open(container: &Container) -> Result<Self, String> {
        let owner = container.owner_id();
        let mut store = Self {
            owner: Some(owner),
            ..Self::default()
        };
        let mut prefix_hash = import_settlement::prefix_hasher();
        for frame in container.read_frames().map_err(|e| e.to_string())? {
            if frame.payload.starts_with(import_settlement::MAGIC) {
                let settlement = ImportFreshnessSettlement::decode(&frame.payload)?;
                let expected = format!("{:x}", prefix_hash.clone().finalize());
                if settlement.prefix_digest != expected {
                    return Err("import settlement physical source prefix mismatch".into());
                }
                store.ingest_settlement(settlement)?;
            } else if frame.payload.starts_with(IMPORT_PROCESSING_MAGIC) {
                store.ingest_processing_binding(&frame.payload)?;
            } else if frame.payload.starts_with(START_MAGIC)
                || frame.payload.starts_with(START_MAGIC_V1)
            {
                let magic = if frame.payload.starts_with(START_MAGIC) {
                    START_MAGIC
                } else {
                    START_MAGIC_V1
                };
                let start = decode::<ImportStart>(&frame.payload, magic)?;
                if start.owner_uuid != owner {
                    return Err("import epoch owner mismatch".into());
                }
                store.ingest_start(start)?;
            } else if frame.payload.starts_with(COMPLETE_MAGIC)
                || frame.payload.starts_with(COMPLETE_MAGIC_V1)
            {
                let magic = if frame.payload.starts_with(COMPLETE_MAGIC) {
                    COMPLETE_MAGIC
                } else {
                    COMPLETE_MAGIC_V1
                };
                let complete = decode::<ImportComplete>(&frame.payload, magic)?;
                if complete.owner_uuid != owner {
                    return Err("import epoch owner mismatch".into());
                }
                store.ingest_complete(complete)?;
            } else if store
                .open
                .as_ref()
                .is_some_and(|id| store.settlements.contains_key(id))
            {
                return Err("import mutation followed settlement before completion".into());
            }
            import_settlement::push_frame(&mut prefix_hash, &frame.payload);
        }
        Ok(store)
    }

    pub fn append_start(
        &mut self,
        container: &mut Container,
        start: ImportStart,
    ) -> Result<(), String> {
        self.check_owner(container)?;
        self.validate_start(&start)?;
        if start.format_version != 2 {
            return Err("new import starts require the current clock contract".into());
        }
        if self.open.is_some() {
            return Err("another import epoch is open".into());
        }
        if self.starts.contains_key(&start.import_id) {
            return Err("import start already exists".into());
        }
        let bytes = encode(START_MAGIC, &start)?;
        container.append(&bytes).map_err(|e| e.to_string())?;
        container.sync().map_err(|e| e.to_string())?;
        self.ingest_start(start)
    }

    pub fn append_settlement(
        &mut self,
        container: &mut Container,
        settlement: ImportFreshnessSettlement,
    ) -> Result<bool, String> {
        self.check_owner(container)?;
        if let Some(previous) = self.settlements.get(&settlement.import_id) {
            return if previous == &settlement {
                Ok(false)
            } else {
                Err("conflicting import Freshness settlement".into())
            };
        }
        self.validate_settlement(&settlement)?;
        if settlement.prefix_digest != import_settlement::source_prefix_digest(container)? {
            return Err("import settlement source prefix changed".into());
        }
        let bytes = settlement.encode()?;
        container.append(&bytes).map_err(|e| e.to_string())?;
        container.sync().map_err(|e| e.to_string())?;
        self.ingest_settlement(settlement)?;
        Ok(true)
    }

    pub fn settlement(&self, id: &str) -> Option<&ImportFreshnessSettlement> {
        self.settlements.get(id)
    }

    /// Returns false for an identical already accepted receipt.
    pub fn append_complete(
        &mut self,
        container: &mut Container,
        complete: ImportComplete,
    ) -> Result<bool, String> {
        self.check_owner(container)?;
        if let Some(previous) = self.completions.get(&complete.import_id) {
            return if previous == &complete {
                Ok(false)
            } else {
                Err("conflicting import completion".into())
            };
        }
        self.validate_complete(&complete)?;
        if complete.format_version != 2 {
            return Err("new import completions require the current clock contract".into());
        }
        let bytes = encode(COMPLETE_MAGIC, &complete)?;
        container.append(&bytes).map_err(|e| e.to_string())?;
        container.sync().map_err(|e| e.to_string())?;
        self.ingest_complete(complete)?;
        Ok(true)
    }

    pub fn check_complete(&self, complete: &ImportComplete) -> Result<bool, String> {
        if let Some(previous) = self.completions.get(&complete.import_id) {
            return if previous == complete {
                Ok(false)
            } else {
                Err("conflicting import completion".into())
            };
        }
        self.validate_complete(complete)?;
        Ok(true)
    }
    pub fn open_epoch(&self) -> Option<&ImportStart> {
        self.open.as_ref().and_then(|id| self.starts.get(id))
    }
    pub fn status(&self, id: &str) -> Option<ImportEpochStatus> {
        let start = self.starts.get(id)?.clone();
        Some(match self.completions.get(id) {
            Some(complete) => ImportEpochStatus::Completed {
                start,
                complete: complete.clone(),
            },
            None => ImportEpochStatus::Open(start),
        })
    }
    pub fn statuses(&self) -> Vec<ImportEpochStatus> {
        self.starts
            .keys()
            .filter_map(|id| self.status(id))
            .collect()
    }
    pub fn validate_receipts(&self, receipts: &BTreeMap<String, String>) -> Result<(), String> {
        for (id, complete) in &self.completions {
            let start = &self.starts[id];
            let digest = receipts
                .get(&start.import_key)
                .ok_or("completed import lacks receipt")?;
            if *digest != complete.receipt_digest {
                return Err("import receipt digest mismatch".into());
            }
        }
        Ok(())
    }
    pub fn validate_owner(&self, owner: [u8; 16]) -> Result<(), String> {
        if self.owner == Some(owner) {
            Ok(())
        } else {
            Err("import epoch owner mismatch".into())
        }
    }

    fn check_owner(&self, c: &Container) -> Result<(), String> {
        if self.owner == Some(c.owner_id()) {
            Ok(())
        } else {
            Err("import epoch owner mismatch".into())
        }
    }
    fn ingest_start(&mut self, start: ImportStart) -> Result<(), String> {
        self.validate_start(&start)?;
        if self.starts.contains_key(&start.import_id) {
            return Err("duplicate import start".into());
        }
        if self.open.is_some() {
            return Err("overlapping import epochs".into());
        }
        self.open = Some(start.import_id.clone());
        self.starts.insert(start.import_id.clone(), start);
        Ok(())
    }
    fn ingest_settlement(&mut self, settlement: ImportFreshnessSettlement) -> Result<(), String> {
        self.validate_settlement(&settlement)?;
        if self
            .settlements
            .insert(settlement.import_id.clone(), settlement)
            .is_some()
        {
            return Err("duplicate import Freshness settlement".into());
        }
        Ok(())
    }

    fn validate_settlement(&self, value: &ImportFreshnessSettlement) -> Result<(), String> {
        let start = self
            .starts
            .get(&value.import_id)
            .ok_or("settlement without import start")?;
        if self.open.as_deref() != Some(value.import_id.as_str())
            || start.owner_uuid != value.owner_uuid
            || self
                .processing_binding(&value.import_id)
                .is_some_and(|plan| plan.profile.is_none())
            || ![
                &value.start_digest,
                &value.receipt_digest,
                &value.prefix_digest,
                &value.freshness_state_digest,
            ]
            .iter()
            .all(|s| valid_digest(s))
            || value
                .replay_source_authority
                .as_ref()
                .is_some_and(|authority| !valid_digest(&authority.state_digest))
            || value.start_digest
                != format!(
                    "{:x}",
                    Sha256::digest(serde_json::to_vec(start).map_err(|e| e.to_string())?)
                )
            || value.global_version < start.pre.global_version
            || value.memory_version < start.pre.memory_version
            || value.graph_version < start.pre.graph_version
        {
            return Err("invalid epoch-scoped import Freshness settlement".into());
        }
        Ok(())
    }

    fn ingest_complete(&mut self, complete: ImportComplete) -> Result<(), String> {
        self.validate_complete(&complete)?;
        self.completions
            .insert(complete.import_id.clone(), complete.clone());
        self.open = None;
        Ok(())
    }
    fn validate_start(&self, start: &ImportStart) -> Result<(), String> {
        if !matches!(start.format_version, 1 | 2) {
            return Err("unsupported import epoch clock contract".into());
        }
        if self.owner.is_some_and(|owner| owner != start.owner_uuid) {
            return Err("import epoch owner mismatch".into());
        }
        if start.import_id != import_id(start.owner_uuid, &start.import_key) {
            return Err("invalid deterministic import ID".into());
        }
        if !canonical_import_key(&start.import_key)
            || !valid_source_text(&start.source_namespace)
            || !valid_source_text(&start.source_format)
        {
            return Err("incomplete import start identity".into());
        }
        Ok(())
    }
    fn validate_complete(&self, c: &ImportComplete) -> Result<(), String> {
        let start = self
            .starts
            .get(&c.import_id)
            .ok_or("completion without start")?;
        if self.open.as_deref() != Some(c.import_id.as_str()) || start.owner_uuid != c.owner_uuid {
            return Err("invalid import completion ordering".into());
        }
        if c.format_version != start.format_version {
            return Err("import completion clock contract mismatch".into());
        }
        if c.start_digest
            != format!(
                "{:x}",
                Sha256::digest(serde_json::to_vec(start).map_err(|e| e.to_string())?)
            )
        {
            return Err("import start digest mismatch".into());
        }
        let settlement = self
            .settlements
            .get(&c.import_id)
            .ok_or("import completion lacks preceding Freshness settlement")?;
        if c.receipt_digest != settlement.receipt_digest
            || c.historical_freshness_settlement_digest != settlement.digest()?
            || c.post.global_version < settlement.global_version
            || c.post.memory_version < settlement.memory_version
            || c.post.graph_version < settlement.graph_version
        {
            return Err("import completion Freshness settlement evidence mismatch".into());
        }
        if !watermarks_monotone(start.pre, c.post) || c.completed_at_ns < start.started_at_ns {
            return Err("import completion cut is not monotone".into());
        }
        Ok(())
    }
}

pub fn import_id(owner: [u8; 16], key: &str) -> String {
    format!(
        "{:x}",
        Sha256::digest([owner.as_slice(), key.as_bytes()].concat())
    )
}
fn canonical_import_key(v: &str) -> bool {
    v.len() == 64
        && v.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn valid_source_text(v: &str) -> bool {
    !v.is_empty() && v.len() <= 4096 && !v.contains('\0')
}
fn watermarks_monotone(a: ImportWatermarks, b: ImportWatermarks) -> bool {
    b.global_version >= a.global_version
        && optional_watermark_monotone(a.archive_version, b.archive_version)
        && b.memory_version >= a.memory_version
        && optional_watermark_monotone(a.entity_version, b.entity_version)
        && optional_watermark_monotone(a.relationship_version, b.relationship_version)
        && b.graph_version >= a.graph_version
        && optional_watermark_monotone(a.memory_graph_version, b.memory_graph_version)
        && optional_watermark_monotone(a.vector_version, b.vector_version)
}
fn optional_watermark_monotone(a: Option<u64>, b: Option<u64>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => b >= a,
        (None, None) => true,
        _ => false,
    }
}
fn encode<T: serde::Serialize>(magic: &[u8; 8], value: &T) -> Result<Vec<u8>, String> {
    let body = serde_json::to_vec(value).map_err(|e| e.to_string())?;
    let len = u32::try_from(body.len()).map_err(|_| "import epoch record too large")?;
    let mut bytes = Vec::with_capacity(12 + body.len());
    bytes.extend_from_slice(magic);
    bytes.extend_from_slice(&len.to_le_bytes());
    bytes.extend_from_slice(&body);
    Ok(bytes)
}
fn decode<T: for<'de> serde::Deserialize<'de>>(bytes: &[u8], magic: &[u8; 8]) -> Result<T, String> {
    if bytes.get(..8) != Some(magic.as_slice()) {
        return Err("invalid import epoch marker".into());
    }
    let raw = bytes.get(8..12).ok_or("truncated import epoch length")?;
    let len = u32::from_le_bytes(raw.try_into().unwrap()) as usize;
    let body = bytes
        .get(12..)
        .filter(|b| b.len() == len)
        .ok_or("invalid import epoch length")?;
    serde_json::from_slice(body).map_err(|e| e.to_string())
}
