//! Durable identity of the bounded candidates that must finish before settlement.
use crate::records::dream::dream_begin_codec::{self, BeginPopulation, BeginRecord};
use crate::records::dream_batch::Observation;
use crate::{DreamMemoryCut, MemoryId};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InitialDreamFrontier {
    pub source: DreamMemoryCut,
    pub candidates: Vec<DreamMemoryCut>,
    pub graph_version: u64,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FrontierRecord {
    pub owner: [u8; 16],
    pub pass_id: String,
    pub begin: Option<Vec<u8>>,
    pub source: Observation,
    pub candidates: Vec<Observation>,
    pub graph_version: u64,
    pub memory_version: u64,
}
impl FrontierRecord {
    pub fn encode(&self) -> Result<Vec<u8>, String> {
        self.validate_begin()?;
        let mut out = b"RELDRC01".to_vec();
        out.extend(serde_json::to_vec(self).map_err(|e| e.to_string())?);
        if out.len() > 256 * 1024 {
            return Err("Dream frontier byte limit".into());
        }
        Ok(out)
    }
    pub fn decode(bytes: &[u8]) -> Result<Option<Self>, String> {
        Ok(Self::decode_with_begin(bytes)?.map(|(frontier, _)| frontier))
    }

    pub fn decode_with_begin(bytes: &[u8]) -> Result<Option<(Self, Option<BeginRecord>)>, String> {
        if !bytes.starts_with(b"RELDRC") {
            return Ok(None);
        }
        if !bytes.starts_with(b"RELDRC01") || bytes.len() > 256 * 1024 {
            return Err("invalid Dream frontier".into());
        }
        let result: Self = serde_json::from_slice(&bytes[8..]).map_err(|e| e.to_string())?;
        let begin = result.validate_begin()?;
        Ok(Some((result, begin)))
    }

    fn validate_begin(&self) -> Result<Option<BeginRecord>, String> {
        let ids: std::collections::BTreeSet<_> = self.candidates.iter().map(|o| o.id).collect();
        if self.pass_id.is_empty()
            || self.pass_id.len() > 256
            || self.candidates.len() > 16
            || ids.len() != self.candidates.len()
            || ids.contains(&self.source.id)
            || self.source.revision == 0
            || self.candidates.iter().any(|o| o.revision == 0)
        {
            return Err("invalid Dream frontier cut".into());
        }
        if let Some(payload) = &self.begin {
            let begin = dream_begin_codec::decode(payload)?;
            if begin.owner_uuid != self.owner
                || begin.pass_id != self.pass_id
                || begin.source_id != MemoryId(self.source.id)
                || begin.graph_version_before != self.graph_version
                || matches!(begin.population, BeginPopulation::NativeBirth { first_memory_version, .. }
                    if first_memory_version > self.memory_version)
            {
                return Err("frontier Begin mismatch".into());
            }
            return Ok(Some(begin));
        }
        Ok(None)
    }
}
