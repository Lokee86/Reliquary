use super::{ImportComplete, ImportEpochStatus, ImportStart, ImportWatermarks};
use reliquary_container::Container;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

const START_MAGIC: &[u8; 8] = b"RELIEPS1";
const COMPLETE_MAGIC: &[u8; 8] = b"RELIEPC1";

#[derive(Default)]
pub struct ImportEpochStore {
    owner: Option<[u8; 16]>,
    starts: BTreeMap<String, ImportStart>,
    completions: BTreeMap<String, ImportComplete>,
    open: Option<String>,
}

impl ImportEpochStore {
    pub fn open(container: &Container) -> Result<Self, String> {
        let owner = container.owner_id();
        let mut store = Self {
            owner: Some(owner),
            ..Self::default()
        };
        for frame in container.read_frames().map_err(|e| e.to_string())? {
            if frame.payload.starts_with(START_MAGIC) {
                let start = decode::<ImportStart>(&frame.payload, START_MAGIC)?;
                if start.owner_uuid != owner {
                    return Err("import epoch owner mismatch".into());
                }
                store.ingest_start(start)?;
            } else if frame.payload.starts_with(COMPLETE_MAGIC) {
                let complete = decode::<ImportComplete>(&frame.payload, COMPLETE_MAGIC)?;
                if complete.owner_uuid != owner {
                    return Err("import epoch owner mismatch".into());
                }
                store.ingest_complete(complete)?;
            }
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
    fn ingest_complete(&mut self, complete: ImportComplete) -> Result<(), String> {
        self.validate_complete(&complete)?;
        self.completions
            .insert(complete.import_id.clone(), complete.clone());
        self.open = None;
        Ok(())
    }
    fn validate_start(&self, start: &ImportStart) -> Result<(), String> {
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
        if c.start_digest
            != format!(
                "{:x}",
                Sha256::digest(serde_json::to_vec(start).map_err(|e| e.to_string())?)
            )
        {
            return Err("import start digest mismatch".into());
        }
        if c.receipt_digest.is_empty() || c.historical_freshness_settlement_digest.is_empty() {
            return Err("import completion evidence is incomplete".into());
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
        && b.archive_version >= a.archive_version
        && b.memory_version >= a.memory_version
        && b.entity_version >= a.entity_version
        && b.relationship_version >= a.relationship_version
        && b.graph_version >= a.graph_version
        && b.memory_graph_version >= a.memory_graph_version
        && b.vector_version >= a.vector_version
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
