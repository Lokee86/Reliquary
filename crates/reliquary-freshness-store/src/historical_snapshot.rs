use crate::{FreshnessMemoryId, FreshnessPolicy, FreshnessRecord};
use std::collections::BTreeMap;

pub(crate) const MAGIC: &[u8; 8] = b"RELHFRS1";
const MAX_IMPORT_ID_BYTES: usize = 1024;
const MAX_RECORDS: usize = 1_000_000;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HistoricalFreshnessSnapshot {
    pub owner_uuid: [u8; 16],
    pub import_id: String,
    pub base_cut: u64,
    pub settlement_cut: u64,
    pub journal_entries_included: u64,
    pub evidence_digest: [u8; 32],
    pub records: BTreeMap<FreshnessMemoryId, FreshnessRecord>,
}

impl HistoricalFreshnessSnapshot {
    pub(crate) fn validate(&self, policy: &FreshnessPolicy) -> Result<(), String> {
        if self.import_id.is_empty()
            || self.import_id.len() > MAX_IMPORT_ID_BYTES
            || self.base_cut > self.settlement_cut
            || self.records.len() > MAX_RECORDS
        {
            return Err("invalid historical Freshness snapshot identity".into());
        }
        for record in self.records.values() {
            match (record.admitted_at_turn, record.accounted_turn) {
                (None, None) if *record == FreshnessRecord::created() => {}
                (Some(admitted), Some(accounted))
                    if admitted <= accounted && accounted <= self.settlement_cut =>
                {
                    record
                        .score_at_with_policy(self.settlement_cut, policy)
                        .map_err(|error| error.to_string())?;
                }
                _ => return Err("invalid historical Freshness record anchors".into()),
            }
        }
        Ok(())
    }

    pub(crate) fn encode(&self, policy: &FreshnessPolicy) -> Result<Vec<u8>, String> {
        self.validate(policy)?;
        let mut out = Vec::new();
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&self.owner_uuid);
        put_u32(&mut out, self.import_id.len())?;
        out.extend_from_slice(self.import_id.as_bytes());
        out.extend_from_slice(&self.base_cut.to_le_bytes());
        out.extend_from_slice(&self.settlement_cut.to_le_bytes());
        out.extend_from_slice(&self.journal_entries_included.to_le_bytes());
        out.extend_from_slice(&self.evidence_digest);
        put_u32(&mut out, self.records.len())?;
        for (id, record) in &self.records {
            out.extend_from_slice(&id.0);
            out.extend_from_slice(&record.score.to_le_bytes());
            put_optional_u64(&mut out, record.admitted_at_turn);
            put_optional_u64(&mut out, record.accounted_turn);
        }
        Ok(out)
    }

    pub(crate) fn decode(bytes: &[u8], policy: &FreshnessPolicy) -> Result<Option<Self>, String> {
        if !bytes.starts_with(MAGIC) {
            return Ok(None);
        }
        let mut reader = Reader::new(&bytes[8..]);
        let owner_uuid = reader.array()?;
        let import_id = reader.string(MAX_IMPORT_ID_BYTES)?;
        let base_cut = reader.u64()?;
        let settlement_cut = reader.u64()?;
        let journal_entries_included = reader.u64()?;
        let evidence_digest = reader.array()?;
        let count = reader.u32()? as usize;
        if count > MAX_RECORDS {
            return Err("historical Freshness snapshot exceeds record limit".into());
        }
        let mut records = BTreeMap::new();
        for _ in 0..count {
            let id = FreshnessMemoryId(reader.array()?);
            let record = FreshnessRecord {
                score: reader.i16()?,
                admitted_at_turn: reader.optional_u64()?,
                accounted_turn: reader.optional_u64()?,
            };
            if records.insert(id, record).is_some() {
                return Err("duplicate historical Freshness record".into());
            }
        }
        reader.finish()?;
        let snapshot = Self {
            owner_uuid,
            import_id,
            base_cut,
            settlement_cut,
            journal_entries_included,
            evidence_digest,
            records,
        };
        snapshot.validate(policy)?;
        Ok(Some(snapshot))
    }
}

fn put_u32(out: &mut Vec<u8>, value: usize) -> Result<(), String> {
    out.extend_from_slice(
        &u32::try_from(value)
            .map_err(|_| "historical Freshness field exceeds wire limit")?
            .to_le_bytes(),
    );
    Ok(())
}

fn put_optional_u64(out: &mut Vec<u8>, value: Option<u64>) {
    out.push(u8::from(value.is_some()));
    if let Some(value) = value {
        out.extend_from_slice(&value.to_le_bytes());
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, at: 0 }
    }
    fn take(&mut self, count: usize) -> Result<&'a [u8], String> {
        let end = self
            .at
            .checked_add(count)
            .ok_or("snapshot offset overflow")?;
        let value = self
            .bytes
            .get(self.at..end)
            .ok_or("truncated historical Freshness snapshot")?;
        self.at = end;
        Ok(value)
    }
    fn array<const N: usize>(&mut self) -> Result<[u8; N], String> {
        self.take(N)?
            .try_into()
            .map_err(|_| "truncated snapshot".into())
    }
    fn u32(&mut self) -> Result<u32, String> {
        Ok(u32::from_le_bytes(self.array()?))
    }
    fn u64(&mut self) -> Result<u64, String> {
        Ok(u64::from_le_bytes(self.array()?))
    }
    fn i16(&mut self) -> Result<i16, String> {
        Ok(i16::from_le_bytes(self.array()?))
    }
    fn optional_u64(&mut self) -> Result<Option<u64>, String> {
        match self.take(1)?[0] {
            0 => Ok(None),
            1 => Ok(Some(self.u64()?)),
            _ => Err("invalid historical Freshness optional value".into()),
        }
    }
    fn string(&mut self, limit: usize) -> Result<String, String> {
        let count = self.u32()? as usize;
        if count == 0 || count > limit {
            return Err("invalid historical Freshness string".into());
        }
        std::str::from_utf8(self.take(count)?)
            .map(str::to_owned)
            .map_err(|_| "historical Freshness string is not UTF-8".into())
    }
    fn finish(self) -> Result<(), String> {
        if self.at == self.bytes.len() {
            Ok(())
        } else {
            Err("trailing historical Freshness snapshot bytes".into())
        }
    }
}
