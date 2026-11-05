//! Storage-owned REL grouped Insomnia/Memory/Freshness carrier.
//!
//! This module validates bytes and cross-record structural invariants only.
//! Episode admission, source authorization and candidate policy remain above it.
use reliquary_memory_core::{
    MemoryBodyId, MemoryRecord, decode_grouped_creation, encode_grouped_creation,
    memory_body_bytes, memory_body_id,
};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

const MAGIC: &[u8; 8] = b"RELINSC7";
const MAX_ITEMS: usize = 100_000;
const MAX_CARRIER_BYTES: usize = 128 * 1024 * 1024;
const MAX_ITEM_BYTES: usize = 64 * 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroupedMemoryBody {
    pub id: MemoryBodyId,
    /// Memory Core body identity preimage: title length/title/content length/content.
    pub bytes: Vec<u8>,
}

impl GroupedMemoryBody {
    /// Construct the canonical durable body payload without exposing the
    /// storage byte preimage to an Application workflow.
    pub fn from_content(title: &str, content: &str) -> Self {
        Self {
            id: memory_body_id(title, content),
            bytes: memory_body_bytes(title, content),
        }
    }
}

/// Encode a validated grouped-creation record at the owner durability boundary.
pub fn encode_insomnia_memory_creation(record: &MemoryRecord) -> Result<Vec<u8>, String> {
    encode_grouped_creation(record).map_err(|error| error.to_string())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroupedExternalMemoryRef {
    pub owner_id: String,
    pub memory_id: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroupedInsomniaCompletion {
    pub episode_id: [u8; 32],
    pub attempt: u32,
    pub started_at_ns: i64,
    pub completed_at_ns: i64,
    pub transaction_time_ns: i64,
    pub extractor_model: String,
    pub extractor_version: String,
    pub rejected_count: u32,
    /// Memory IDs local to this REL owner. Routed PHY IDs live only in
    /// `external_memory_refs` and are never folded into this list.
    pub memory_ids: Vec<[u8; 32]>,
    pub external_memory_refs: Vec<GroupedExternalMemoryRef>,
    pub global_version_start: u64,
    pub bodies: Vec<GroupedMemoryBody>,
    /// RELMCRT1 creation records in physical/global version order.
    pub memory_records: Vec<Vec<u8>>,
    /// Opaque owner-routing codec records; semantic validation is Perception-owned.
    pub routing_metadata: Vec<Vec<u8>>,
    pub freshness_birth_turn: Option<u64>,
    pub freshness_policy_version: Option<u64>,
    pub freshness_birth_memory_ids: Vec<[u8; 32]>,
}

impl GroupedInsomniaCompletion {
    pub fn encode(&self) -> Result<Vec<u8>, String> {
        self.validate_structure()?;
        let count = u32::try_from(self.memory_records.len())
            .map_err(|_| "too many grouped Memory records")?;
        let has_birth = self.freshness_birth_turn.is_some();
        let mut out = Vec::new();
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&self.episode_id);
        out.extend_from_slice(&self.attempt.to_le_bytes());
        out.extend_from_slice(&self.started_at_ns.to_le_bytes());
        out.extend_from_slice(&self.completed_at_ns.to_le_bytes());
        out.extend_from_slice(&self.rejected_count.to_le_bytes());
        out.extend_from_slice(&self.global_version_start.to_le_bytes());
        out.extend_from_slice(&count.to_le_bytes());
        out.extend_from_slice(&self.transaction_time_ns.to_le_bytes());
        out.push(u8::from(has_birth));
        put_string(&mut out, &self.extractor_model)?;
        put_string(&mut out, &self.extractor_version)?;
        put_ids(&mut out, &self.memory_ids)?;
        let external_count = u32::try_from(self.external_memory_refs.len())
            .map_err(|_| "too many external Memory references")?;
        out.extend_from_slice(&external_count.to_le_bytes());
        for reference in &self.external_memory_refs {
            put_string(&mut out, &reference.owner_id)?;
            out.extend_from_slice(&reference.memory_id);
        }
        let body_count = u32::try_from(self.bodies.len()).map_err(|_| "too many grouped bodies")?;
        out.extend_from_slice(&body_count.to_le_bytes());
        for body in &self.bodies {
            out.extend_from_slice(&body.id.0);
            put_bytes(&mut out, &body.bytes)?;
        }
        let records_count = u32::try_from(self.memory_records.len())
            .map_err(|_| "too many grouped Memory records")?;
        out.extend_from_slice(&records_count.to_le_bytes());
        for record in &self.memory_records {
            put_bytes(&mut out, record)?;
        }
        let routing_count =
            u32::try_from(self.routing_metadata.len()).map_err(|_| "too many routing records")?;
        out.extend_from_slice(&routing_count.to_le_bytes());
        for record in &self.routing_metadata {
            put_bytes(&mut out, record)?;
        }
        if has_birth {
            out.extend_from_slice(&self.freshness_birth_turn.unwrap().to_le_bytes());
            out.extend_from_slice(&self.freshness_policy_version.unwrap().to_le_bytes());
            put_ids(&mut out, &self.freshness_birth_memory_ids)?;
        }
        if out.len() > MAX_CARRIER_BYTES {
            return Err("grouped completion is too large".into());
        }
        Ok(out)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > MAX_CARRIER_BYTES {
            return Err("grouped completion is too large".into());
        }
        if bytes.get(..8) != Some(MAGIC.as_slice()) {
            return Err("unsupported grouped completion marker".into());
        }
        let mut r = Reader { bytes, at: 8 };
        let episode_id = r.array()?;
        let attempt = r.u32()?;
        let started_at_ns = r.i64()?;
        let completed_at_ns = r.i64()?;
        let rejected_count = r.u32()?;
        let global_version_start = r.u64()?;
        let global_count = r.u32()? as usize;
        let transaction_time_ns = r.i64()?;
        let has_birth = match r.u8()? {
            0 => false,
            1 => true,
            _ => return Err("invalid Freshness proof flag".into()),
        };
        let extractor_model = r.string()?;
        let extractor_version = r.string()?;
        let memory_ids = r.ids()?;
        let external_count = r.count()?;
        let mut external_memory_refs = Vec::with_capacity(external_count);
        for _ in 0..external_count {
            external_memory_refs.push(GroupedExternalMemoryRef {
                owner_id: r.string()?,
                memory_id: r.array()?,
            });
        }
        let body_count = r.count()?;
        let mut bodies = Vec::with_capacity(body_count);
        for _ in 0..body_count {
            bodies.push(GroupedMemoryBody {
                id: MemoryBodyId(r.array()?),
                bytes: r.bytes()?,
            });
        }
        let record_count = r.count()?;
        let mut memory_records = Vec::with_capacity(record_count);
        for _ in 0..record_count {
            memory_records.push(r.bytes()?);
        }
        let routing_count = r.count()?;
        let mut routing_metadata = Vec::with_capacity(routing_count);
        for _ in 0..routing_count {
            routing_metadata.push(r.bytes()?);
        }
        let (freshness_birth_turn, freshness_policy_version, freshness_birth_memory_ids) =
            if has_birth {
                (Some(r.u64()?), Some(r.u64()?), r.ids()?)
            } else {
                (None, None, Vec::new())
            };
        if r.at != bytes.len() {
            return Err("trailing grouped completion bytes".into());
        }
        let value = Self {
            episode_id,
            attempt,
            started_at_ns,
            completed_at_ns,
            transaction_time_ns,
            extractor_model,
            extractor_version,
            rejected_count,
            memory_ids,
            external_memory_refs,
            global_version_start,
            bodies,
            memory_records,
            routing_metadata,
            freshness_birth_turn,
            freshness_policy_version,
            freshness_birth_memory_ids,
        };
        if global_count != value.memory_records.len() {
            return Err("grouped physical range count mismatch".into());
        }
        value.validate_structure()?;
        Ok(value)
    }

    /// Validate replay-visible structural proofs against an upper owner's Episode fact.
    /// This does not authenticate the supplied Episode or external owner at
    /// write time; routed publication receipts are checked by RelSession.
    pub fn validate_replay_episode(&self, accepted_episode: [u8; 32]) -> Result<(), String> {
        self.validate_structure()?;
        if self.episode_id != accepted_episode {
            return Err("grouped completion Episode mismatch".into());
        }
        if !self.routing_metadata.is_empty() {
            return Err("routed completion metadata requires separate owner authentication".into());
        }
        let births = self
            .memory_records
            .iter()
            .map(|bytes| {
                decode_grouped_creation(bytes)
                    .map(|r| r.id.0)
                    .map_err(|e| e.to_string())
            })
            .collect::<Result<BTreeSet<_>, _>>()?;
        if births.iter().any(|id| !self.memory_ids.contains(id)) {
            return Err("completion Memory IDs omit a grouped birth".into());
        }
        Ok(())
    }

    fn validate_structure(&self) -> Result<(), String> {
        if self.attempt == 0 || self.completed_at_ns < self.started_at_ns {
            return Err("invalid grouped completion attempt or timestamps".into());
        }
        if self.extractor_model.is_empty() || self.extractor_version.is_empty() {
            return Err("missing grouped extractor identity".into());
        }
        let count = self.memory_records.len();
        if count > MAX_ITEMS
            || self.memory_ids.len() > MAX_ITEMS
            || self.bodies.len() > MAX_ITEMS
            || self.routing_metadata.len() > MAX_ITEMS
            || self.external_memory_refs.len() > MAX_ITEMS
        {
            return Err("too many grouped completion items".into());
        }
        if count > 0 && self.global_version_start == 0 {
            return Err("invalid grouped global version start".into());
        }
        self.global_version_start
            .checked_add(u64::try_from(count).map_err(|_| "grouped range overflow")?)
            .ok_or("grouped range overflow")?;
        let mut memory_ids = BTreeSet::new();
        if self.memory_ids.iter().any(|id| !memory_ids.insert(*id)) {
            return Err("duplicate grouped Memory ID".into());
        }
        let mut bodies = BTreeSet::new();
        for body in &self.bodies {
            if !bodies.insert(body.id) {
                return Err("duplicate grouped body identity".into());
            }
            if body.bytes.len() > MAX_ITEM_BYTES
                || Sha256::digest(&body.bytes).as_slice() != body.id.0
            {
                return Err("invalid grouped Memory body identity".into());
            }
        }
        let mut records = BTreeSet::new();
        for bytes in &self.memory_records {
            let record = decode_grouped_creation(bytes).map_err(|e| e.to_string())?;
            if record.source_episode_id != Some(self.episode_id) {
                return Err("grouped Memory birth Episode mismatch".into());
            }
            if !memory_ids.contains(&record.id.0) {
                return Err("completion Memory IDs omit a grouped birth".into());
            }
            if !records.insert(record.id.0) {
                return Err("duplicate grouped Memory creation".into());
            }
        }
        if self.freshness_birth_turn.is_some() != self.freshness_policy_version.is_some()
            || (self.freshness_birth_turn.is_none() && !self.freshness_birth_memory_ids.is_empty())
        {
            return Err("incomplete Freshness birth proof".into());
        }
        if let Some(policy) = self.freshness_policy_version {
            if policy != u64::from(reliquary_freshness_store::FRESHNESS_POLICY_VERSION) {
                return Err("unsupported Freshness birth policy".into());
            }
            let exact = records.iter().copied().collect::<BTreeSet<_>>();
            let admitted = self
                .freshness_birth_memory_ids
                .iter()
                .copied()
                .collect::<BTreeSet<_>>();
            if admitted.len() != self.freshness_birth_memory_ids.len() || admitted != exact {
                return Err("Freshness birth cohort differs from grouped creations".into());
            }
        }
        let mut external_refs = BTreeSet::new();
        for reference in &self.external_memory_refs {
            if !is_owner_id(&reference.owner_id) {
                return Err("invalid external Memory owner ID".into());
            }
            if memory_ids.contains(&reference.memory_id) {
                return Err("external Memory reference duplicates a local Memory ID".into());
            }
            if !external_refs.insert((reference.owner_id.as_str(), reference.memory_id)) {
                return Err("duplicate external Memory reference".into());
            }
        }
        Ok(())
    }
}

fn is_owner_id(owner_id: &str) -> bool {
    owner_id.len() == 32
        && owner_id
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn put_string(out: &mut Vec<u8>, value: &str) -> Result<(), String> {
    let len = u32::try_from(value.len()).map_err(|_| "grouped text too large")?;
    if value.len() > 16 * 1024 * 1024 {
        return Err("grouped text too large".into());
    }
    check_growth(
        out,
        value.len().checked_add(4).ok_or("grouped size overflow")?,
    )?;
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(value.as_bytes());
    Ok(())
}
fn put_bytes(out: &mut Vec<u8>, value: &[u8]) -> Result<(), String> {
    if value.len() > MAX_ITEM_BYTES {
        return Err("grouped item too large".into());
    }
    let len = u32::try_from(value.len()).map_err(|_| "grouped item too large")?;
    check_growth(
        out,
        value.len().checked_add(4).ok_or("grouped size overflow")?,
    )?;
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(value);
    Ok(())
}
fn put_ids(out: &mut Vec<u8>, ids: &[[u8; 32]]) -> Result<(), String> {
    let count = u32::try_from(ids.len()).map_err(|_| "too many grouped IDs")?;
    if ids.len() > MAX_ITEMS {
        return Err("too many grouped IDs".into());
    }
    let extra = ids
        .len()
        .checked_mul(32)
        .and_then(|n| n.checked_add(4))
        .ok_or("grouped size overflow")?;
    check_growth(out, extra)?;
    out.extend_from_slice(&count.to_le_bytes());
    for id in ids {
        out.extend_from_slice(id);
    }
    Ok(())
}

fn check_growth(out: &[u8], extra: usize) -> Result<(), String> {
    if out
        .len()
        .checked_add(extra)
        .is_none_or(|size| size > MAX_CARRIER_BYTES)
    {
        return Err("grouped completion is too large".into());
    }
    Ok(())
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}
impl Reader<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8], String> {
        let end = self.at.checked_add(n).ok_or("grouped offset overflow")?;
        let value = self
            .bytes
            .get(self.at..end)
            .ok_or("truncated grouped completion")?;
        self.at = end;
        Ok(value)
    }
    fn u8(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }
    fn u32(&mut self) -> Result<u32, String> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn u64(&mut self) -> Result<u64, String> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    fn i64(&mut self) -> Result<i64, String> {
        Ok(i64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    fn array<const N: usize>(&mut self) -> Result<[u8; N], String> {
        Ok(self.take(N)?.try_into().unwrap())
    }
    fn count(&mut self) -> Result<usize, String> {
        let n = self.u32()? as usize;
        if n > MAX_ITEMS {
            return Err("too many grouped items".into());
        }
        Ok(n)
    }
    fn bytes(&mut self) -> Result<Vec<u8>, String> {
        let n = self.u32()? as usize;
        if n > MAX_ITEM_BYTES {
            return Err("grouped item too large".into());
        }
        Ok(self.take(n)?.to_vec())
    }
    fn string(&mut self) -> Result<String, String> {
        let n = self.u32()? as usize;
        if n > 16 * 1024 * 1024 {
            return Err("grouped text too large".into());
        }
        String::from_utf8(self.take(n)?.to_vec()).map_err(|_| "invalid grouped UTF-8".into())
    }
    fn ids(&mut self) -> Result<Vec<[u8; 32]>, String> {
        let n = self.count()?;
        let mut out = Vec::with_capacity(n);
        for _ in 0..n {
            out.push(self.array()?);
        }
        Ok(out)
    }
}

#[cfg(test)]
#[path = "insomnia_completion_tests.rs"]
mod tests;
