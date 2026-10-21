use crate::MemoryId;
use std::collections::BTreeSet;

const MAGIC: &[u8; 8] = b"CVDREAM2";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum BeginPopulation {
    CompactCut { memory_cut: u64, digest: [u8; 32] },
    ExplicitSet(BTreeSet<MemoryId>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct BeginRecord {
    pub(crate) owner_uuid: [u8; 16],
    pub(crate) pass_id: String,
    pub(crate) source_id: MemoryId,
    pub(crate) turn: u64,
    pub(crate) graph_version_before: u64,
    pub(crate) population: BeginPopulation,
}

pub(crate) fn encode(record: &BeginRecord) -> Result<Vec<u8>, String> {
    if record.pass_id.is_empty() {
        return Err("empty Dream pass ID".into());
    }
    let pass_id_len = u32::try_from(record.pass_id.len())
        .map_err(|_| "Dream pass ID exceeds u32 length".to_owned())?;
    let population_len = match &record.population {
        BeginPopulation::CompactCut { memory_cut, .. } => {
            if *memory_cut == 0 {
                return Err("zero Dream Memory prefix cut".into());
            }
            8_usize + 32
        }
        BeginPopulation::ExplicitSet(ids) => {
            if ids.contains(&record.source_id) {
                return Err("Dream pass source cannot be preexisting".into());
            }
            u32::try_from(ids.len()).map_err(|_| "too many Dream population IDs".to_owned())?;
            4_usize
                .checked_add(
                    ids.len()
                        .checked_mul(32)
                        .ok_or_else(|| "Dream population size overflow".to_owned())?,
                )
                .ok_or_else(|| "Dream population size overflow".to_owned())?
        }
    };
    let capacity = 8_usize
        .checked_add(16)
        .and_then(|n| n.checked_add(1))
        .and_then(|n| n.checked_add(4))
        .and_then(|n| n.checked_add(record.pass_id.len()))
        .and_then(|n| n.checked_add(32))
        .and_then(|n| n.checked_add(8))
        .and_then(|n| n.checked_add(8))
        .and_then(|n| n.checked_add(population_len))
        .ok_or_else(|| "Dream begin record size overflow".to_owned())?;
    let mut out = Vec::new();
    out.try_reserve_exact(capacity)
        .map_err(|_| "Dream begin record allocation failed".to_owned())?;
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&record.owner_uuid);
    match &record.population {
        BeginPopulation::CompactCut { memory_cut, digest } => {
            out.push(0);
            out.extend_from_slice(&pass_id_len.to_le_bytes());
            out.extend_from_slice(record.pass_id.as_bytes());
            out.extend_from_slice(&record.source_id.0);
            out.extend_from_slice(&record.turn.to_le_bytes());
            out.extend_from_slice(&record.graph_version_before.to_le_bytes());
            out.extend_from_slice(&memory_cut.to_le_bytes());
            out.extend_from_slice(digest);
        }
        BeginPopulation::ExplicitSet(ids) => {
            let count =
                u32::try_from(ids.len()).map_err(|_| "too many Dream population IDs".to_owned())?;
            out.push(1);
            out.extend_from_slice(&pass_id_len.to_le_bytes());
            out.extend_from_slice(record.pass_id.as_bytes());
            out.extend_from_slice(&record.source_id.0);
            out.extend_from_slice(&record.turn.to_le_bytes());
            out.extend_from_slice(&record.graph_version_before.to_le_bytes());
            out.extend_from_slice(&count.to_le_bytes());
            for id in ids {
                out.extend_from_slice(&id.0);
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
#[path = "dream_begin_codec_tests.rs"]
mod tests;
