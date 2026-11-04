use crate::{
    FreshnessEvent, FreshnessEventProof, FreshnessEventProvenance, FreshnessMemoryId,
    FreshnessPolicy, FreshnessUseReceipt,
};
use std::collections::BTreeMap;

pub(super) const JOURNAL_MAGIC: &[u8; 8] = b"RELFRS02";
pub(super) const POLICY_MAGIC: &[u8; 8] = b"RELFRP01";
const JOURNAL_SCHEMA: u16 = 1;
const POLICY_SCHEMA: u16 = 1;
const MAX_STRING_BYTES: usize = 1024 * 1024;
const MAX_COLLECTION_ITEMS: usize = 1_000_000;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum JournalEntry {
    Birth {
        memory_id: FreshnessMemoryId,
        accepted_turn: u64,
        admitted_at_birth: bool,
    },
    Admission {
        memory_id: FreshnessMemoryId,
        accepted_turn: u64,
    },
    Event(FreshnessEvent),
    AcceptedUse(FreshnessUseReceipt, FreshnessEventProof),
}

pub(super) fn encode_policy(owner: [u8; 16], policy: FreshnessPolicy) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(42);
    bytes.extend_from_slice(POLICY_MAGIC);
    bytes.extend_from_slice(&owner);
    bytes.extend_from_slice(&POLICY_SCHEMA.to_le_bytes());
    bytes.extend_from_slice(&policy.decay_turns_per_point.to_le_bytes());
    for value in [
        policy.access_principal,
        policy.linkage_principal,
        policy.local_hop_cost,
        policy.outside_hop_cost,
    ] {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes
}

pub(super) fn decode_policy(bytes: &[u8]) -> Result<Option<([u8; 16], FreshnessPolicy)>, String> {
    if !bytes.starts_with(POLICY_MAGIC) {
        return Ok(None);
    }
    if bytes.len() != 42 {
        return Err("invalid Freshness policy length".into());
    }
    let mut reader = Reader::new(&bytes[8..]);
    let owner = reader.array()?;
    if reader.u16()? != POLICY_SCHEMA {
        return Err("unsupported Freshness policy schema".into());
    }
    let policy = FreshnessPolicy {
        decay_turns_per_point: reader.u64()?,
        access_principal: reader.i16()?,
        linkage_principal: reader.i16()?,
        local_hop_cost: reader.i16()?,
        outside_hop_cost: reader.i16()?,
    };
    reader.finish()?;
    policy.validate().map_err(|error| error.to_string())?;
    Ok(Some((owner, policy)))
}

pub(super) fn encode_entry(owner: [u8; 16], entry: &JournalEntry) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(JOURNAL_MAGIC);
    bytes.extend_from_slice(&owner);
    bytes.extend_from_slice(&JOURNAL_SCHEMA.to_le_bytes());
    match entry {
        JournalEntry::Birth {
            memory_id,
            accepted_turn,
            admitted_at_birth,
        } => {
            bytes.push(0);
            bytes.extend_from_slice(&memory_id.0);
            bytes.extend_from_slice(&accepted_turn.to_le_bytes());
            bytes.push(u8::from(*admitted_at_birth));
        }
        JournalEntry::Admission {
            memory_id,
            accepted_turn,
        } => {
            bytes.push(1);
            bytes.extend_from_slice(&memory_id.0);
            bytes.extend_from_slice(&accepted_turn.to_le_bytes());
        }
        JournalEntry::Event(event) => {
            bytes.push(2);
            write_event(&mut bytes, event)?;
        }
        JournalEntry::AcceptedUse(receipt, proof) => {
            bytes.push(3);
            write_string(&mut bytes, &receipt.use_id)?;
            bytes.extend_from_slice(&receipt.accepted_turn.to_le_bytes());
            bytes.extend_from_slice(&receipt.policy_version.to_le_bytes());
            write_count(&mut bytes, receipt.sources.len())?;
            for source in &receipt.sources {
                bytes.extend_from_slice(&source.0);
            }
            write_proof(&mut bytes, *proof);
        }
    }
    Ok(bytes)
}

pub(super) fn decode_entry(bytes: &[u8]) -> Result<Option<([u8; 16], JournalEntry)>, String> {
    if !bytes.starts_with(JOURNAL_MAGIC) {
        if bytes.starts_with(b"RELFRS") {
            return Err("unsupported Freshness journal version".into());
        }
        return Ok(None);
    }
    let mut reader = Reader::new(&bytes[8..]);
    let owner = reader.array()?;
    if reader.u16()? != JOURNAL_SCHEMA {
        return Err("unsupported Freshness journal schema".into());
    }
    let entry = match reader.u8()? {
        0 => {
            let memory_id = FreshnessMemoryId(reader.array()?);
            let accepted_turn = reader.u64()?;
            let admitted_at_birth = reader.bool()?;
            JournalEntry::Birth {
                memory_id,
                accepted_turn,
                admitted_at_birth,
            }
        }
        1 => JournalEntry::Admission {
            memory_id: FreshnessMemoryId(reader.array()?),
            accepted_turn: reader.u64()?,
        },
        2 => JournalEntry::Event(read_event(&mut reader)?),
        3 => {
            let use_id = reader.string()?;
            let accepted_turn = reader.u64()?;
            let policy_version = reader.u64()?;
            let count = reader.count()?;
            let mut sources = Vec::with_capacity(count);
            for _ in 0..count {
                sources.push(FreshnessMemoryId(reader.array()?));
            }
            if sources.is_empty() || sources.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err("invalid Freshness accepted-use sources".into());
            }
            let proof = read_proof(&mut reader)?;
            JournalEntry::AcceptedUse(
                FreshnessUseReceipt {
                    use_id,
                    accepted_turn,
                    sources,
                    policy_version,
                },
                proof,
            )
        }
        _ => return Err("unknown Freshness journal record kind".into()),
    };
    reader.finish()?;
    Ok(Some((owner, entry)))
}

fn write_event(bytes: &mut Vec<u8>, event: &FreshnessEvent) -> Result<(), String> {
    write_string(bytes, &event.event_id)?;
    bytes.extend_from_slice(&event.accepted_turn.to_le_bytes());
    bytes.push(event.provenance.producer_kind);
    write_string(bytes, &event.provenance.source_id)?;
    bytes.extend_from_slice(&event.provenance.origin_owner);
    bytes.extend_from_slice(&event.provenance.accepted_turn.to_le_bytes());
    bytes.extend_from_slice(&event.provenance.policy_version.to_le_bytes());
    write_proof(bytes, event.proof);
    write_count(bytes, event.effects.len())?;
    for (id, amount) in &event.effects {
        bytes.extend_from_slice(&id.0);
        bytes.extend_from_slice(&amount.to_le_bytes());
    }
    Ok(())
}

fn read_event(reader: &mut Reader<'_>) -> Result<FreshnessEvent, String> {
    let event_id = reader.string()?;
    let accepted_turn = reader.u64()?;
    let producer_kind = reader.u8()?;
    let source_id = reader.string()?;
    let origin_owner = reader.array()?;
    let provenance_turn = reader.u64()?;
    let policy_version = reader.u64()?;
    let proof = read_proof(reader)?;
    let count = reader.count()?;
    let mut effects = BTreeMap::new();
    for _ in 0..count {
        let id = FreshnessMemoryId(reader.array()?);
        let amount = reader.i16()?;
        if effects.insert(id, amount).is_some() {
            return Err("duplicate Freshness event recipient".into());
        }
    }
    Ok(FreshnessEvent {
        event_id,
        accepted_turn,
        provenance: FreshnessEventProvenance {
            producer_kind,
            source_id,
            origin_owner,
            accepted_turn: provenance_turn,
            policy_version,
        },
        proof,
        effects,
    })
}

fn write_proof(bytes: &mut Vec<u8>, proof: FreshnessEventProof) {
    bytes.extend_from_slice(&proof.graph_version.to_le_bytes());
    write_optional_u64(bytes, proof.community_generation);
    write_optional_u64(bytes, proof.community_graph_version);
}

fn read_proof(reader: &mut Reader<'_>) -> Result<FreshnessEventProof, String> {
    Ok(FreshnessEventProof {
        graph_version: reader.u64()?,
        community_generation: reader.option_u64()?,
        community_graph_version: reader.option_u64()?,
    })
}

fn write_optional_u64(bytes: &mut Vec<u8>, value: Option<u64>) {
    match value {
        Some(value) => {
            bytes.push(1);
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        None => bytes.push(0),
    }
}

fn write_count(bytes: &mut Vec<u8>, count: usize) -> Result<(), String> {
    if count > MAX_COLLECTION_ITEMS {
        return Err("Freshness collection exceeds limit".into());
    }
    bytes.extend_from_slice(
        &u32::try_from(count)
            .map_err(|_| "Freshness collection exceeds wire limit")?
            .to_le_bytes(),
    );
    Ok(())
}

fn write_string(bytes: &mut Vec<u8>, value: &str) -> Result<(), String> {
    if value.is_empty() || value.len() > MAX_STRING_BYTES {
        return Err("invalid Freshness string field".into());
    }
    bytes.extend_from_slice(
        &u32::try_from(value.len())
            .map_err(|_| "Freshness string exceeds wire limit")?
            .to_le_bytes(),
    );
    bytes.extend_from_slice(value.as_bytes());
    Ok(())
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}
impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }
    fn take(&mut self, count: usize) -> Result<&'a [u8], String> {
        let end = self
            .offset
            .checked_add(count)
            .ok_or("Freshness record offset overflow")?;
        let result = self
            .bytes
            .get(self.offset..end)
            .ok_or("truncated Freshness record")?;
        self.offset = end;
        Ok(result)
    }
    fn array<const N: usize>(&mut self) -> Result<[u8; N], String> {
        Ok(self.take(N)?.try_into().expect("fixed width"))
    }
    fn u8(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }
    fn bool(&mut self) -> Result<bool, String> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err("invalid Freshness boolean".into()),
        }
    }
    fn u16(&mut self) -> Result<u16, String> {
        Ok(u16::from_le_bytes(self.array()?))
    }
    fn i16(&mut self) -> Result<i16, String> {
        Ok(i16::from_le_bytes(self.array()?))
    }
    fn u32(&mut self) -> Result<u32, String> {
        Ok(u32::from_le_bytes(self.array()?))
    }
    fn u64(&mut self) -> Result<u64, String> {
        Ok(u64::from_le_bytes(self.array()?))
    }
    fn string(&mut self) -> Result<String, String> {
        let len = usize::try_from(self.u32()?).map_err(|_| "Freshness string too large")?;
        if len == 0 || len > MAX_STRING_BYTES {
            return Err("invalid Freshness string length".into());
        }
        String::from_utf8(self.take(len)?.to_vec()).map_err(|_| "invalid Freshness UTF-8".into())
    }
    fn count(&mut self) -> Result<usize, String> {
        let count = usize::try_from(self.u32()?).map_err(|_| "Freshness count too large")?;
        if count > MAX_COLLECTION_ITEMS {
            return Err("Freshness collection exceeds limit".into());
        }
        Ok(count)
    }
    fn option_u64(&mut self) -> Result<Option<u64>, String> {
        match self.u8()? {
            0 => Ok(None),
            1 => Ok(Some(self.u64()?)),
            _ => Err("invalid Freshness optional integer tag".into()),
        }
    }
    fn finish(&self) -> Result<(), String> {
        if self.offset == self.bytes.len() {
            Ok(())
        } else {
            Err("trailing Freshness record bytes".into())
        }
    }
}
