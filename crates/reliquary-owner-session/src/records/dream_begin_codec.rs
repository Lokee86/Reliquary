use crate::MemoryId;
use std::collections::BTreeSet;

const MAGIC: &[u8; 8] = b"CVDREAM2";

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BeginPopulation<I = MemoryId> {
    CompactCut { memory_cut: u64, digest: [u8; 32] },
    ExplicitSet(BTreeSet<I>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BeginRecord<I = MemoryId> {
    pub owner_uuid: [u8; 16],
    pub pass_id: String,
    pub source_id: I,
    pub turn: u64,
    pub graph_version_before: u64,
    pub population: BeginPopulation<I>,
}

pub enum BeginPopulationRef<'a, I> {
    CompactCut {
        memory_cut: u64,
        digest: &'a [u8; 32],
    },
    ExplicitSet(&'a BTreeSet<I>),
}

pub fn decode(bytes: &[u8]) -> Result<BeginRecord, String> {
    decode_with_id(bytes, MemoryId)
}

pub fn decode_with_id<I: Copy + Ord>(
    bytes: &[u8],
    from_bytes: impl Fn([u8; 32]) -> I,
) -> Result<BeginRecord<I>, String> {
    let mut reader = Reader { bytes, at: 0 };
    if reader.take(8)? != MAGIC {
        return Err("unsupported Dream begin version".into());
    }
    let owner_uuid = reader.array()?;
    let variant = reader.u8()?;
    if variant > 1 {
        return Err("unknown Dream begin variant".into());
    }
    let pass_id_len = reader.u32()? as usize;
    if pass_id_len == 0 {
        return Err("empty Dream pass ID".into());
    }
    let pass_id = String::from_utf8(reader.take(pass_id_len)?.to_vec())
        .map_err(|_| "invalid Dream pass ID UTF-8")?;
    let source_bytes = reader.array()?;
    let source_id = from_bytes(source_bytes);
    let turn = reader.u64()?;
    let graph_version_before = reader.u64()?;
    let population = match variant {
        0 => {
            let memory_cut = reader.u64()?;
            if memory_cut == 0 {
                return Err("zero Dream Memory prefix cut".into());
            }
            BeginPopulation::CompactCut {
                memory_cut,
                digest: reader.array()?,
            }
        }
        1 => {
            let count = reader.u32()? as usize;
            let expected_bytes = count
                .checked_mul(32)
                .ok_or_else(|| "Dream population size overflow".to_owned())?;
            if bytes.len().saturating_sub(reader.at) != expected_bytes {
                return Err("invalid Dream explicit population length".into());
            }
            let mut ids = BTreeSet::new();
            let mut previous = None;
            for _ in 0..count {
                let raw = reader.array()?;
                if raw == source_bytes {
                    return Err("Dream pass source cannot be preexisting".into());
                }
                if previous.is_some_and(|value| value >= raw) {
                    return Err("Dream explicit population is not strictly sorted".into());
                }
                previous = Some(raw);
                if !ids.insert(from_bytes(raw)) {
                    return Err("Dream explicit population has duplicate identities".into());
                }
            }
            BeginPopulation::ExplicitSet(ids)
        }
        _ => return Err("unknown Dream begin variant".into()),
    };
    if reader.at != bytes.len() {
        return Err("trailing Dream begin bytes".into());
    }
    Ok(BeginRecord {
        owner_uuid,
        pass_id,
        source_id,
        turn,
        graph_version_before,
        population,
    })
}

pub fn encode(record: &BeginRecord) -> Result<Vec<u8>, String> {
    encode_with_id(record, |id| &id.0)
}

pub fn encode_with_id<I: Copy + Ord>(
    record: &BeginRecord<I>,
    as_bytes: impl for<'a> Fn(&'a I) -> &'a [u8; 32],
) -> Result<Vec<u8>, String> {
    encode_fields(
        record.owner_uuid,
        &record.pass_id,
        record.source_id,
        record.turn,
        record.graph_version_before,
        as_bytes,
        match &record.population {
            BeginPopulation::CompactCut { memory_cut, digest } => BeginPopulationRef::CompactCut {
                memory_cut: *memory_cut,
                digest,
            },
            BeginPopulation::ExplicitSet(ids) => BeginPopulationRef::ExplicitSet(ids),
        },
    )
}

pub fn encode_fields<I: Copy + Ord>(
    owner_uuid: [u8; 16],
    pass_id: &str,
    source_id: I,
    turn: u64,
    graph_version_before: u64,
    as_bytes: impl for<'a> Fn(&'a I) -> &'a [u8; 32],
    population: BeginPopulationRef<'_, I>,
) -> Result<Vec<u8>, String> {
    if pass_id.is_empty() {
        return Err("empty Dream pass ID".into());
    }
    let pass_id_len =
        u32::try_from(pass_id.len()).map_err(|_| "Dream pass ID exceeds u32 length".to_owned())?;
    let population_len = match &population {
        BeginPopulationRef::CompactCut { memory_cut, .. } => {
            if *memory_cut == 0 {
                return Err("zero Dream Memory prefix cut".into());
            }
            8_usize + 32
        }
        BeginPopulationRef::ExplicitSet(ids) => {
            let source = as_bytes(&source_id);
            let mut previous = None;
            for id in *ids {
                let raw = as_bytes(id);
                if raw == source {
                    return Err("Dream pass source cannot be preexisting".into());
                }
                if previous.is_some_and(|value| value >= raw) {
                    return Err("Dream explicit population is not strictly sorted".into());
                }
                previous = Some(raw);
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
        .and_then(|n| n.checked_add(pass_id.len()))
        .and_then(|n| n.checked_add(32))
        .and_then(|n| n.checked_add(8))
        .and_then(|n| n.checked_add(8))
        .and_then(|n| n.checked_add(population_len))
        .ok_or_else(|| "Dream begin record size overflow".to_owned())?;
    let mut out = Vec::new();
    out.try_reserve_exact(capacity)
        .map_err(|_| "Dream begin record allocation failed".to_owned())?;
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&owner_uuid);
    match population {
        BeginPopulationRef::CompactCut { memory_cut, digest } => {
            out.push(0);
            out.extend_from_slice(&pass_id_len.to_le_bytes());
            out.extend_from_slice(pass_id.as_bytes());
            out.extend_from_slice(as_bytes(&source_id));
            out.extend_from_slice(&turn.to_le_bytes());
            out.extend_from_slice(&graph_version_before.to_le_bytes());
            out.extend_from_slice(&memory_cut.to_le_bytes());
            out.extend_from_slice(digest);
        }
        BeginPopulationRef::ExplicitSet(ids) => {
            let count =
                u32::try_from(ids.len()).map_err(|_| "too many Dream population IDs".to_owned())?;
            out.push(1);
            out.extend_from_slice(&pass_id_len.to_le_bytes());
            out.extend_from_slice(pass_id.as_bytes());
            out.extend_from_slice(as_bytes(&source_id));
            out.extend_from_slice(&turn.to_le_bytes());
            out.extend_from_slice(&graph_version_before.to_le_bytes());
            out.extend_from_slice(&count.to_le_bytes());
            for id in ids {
                out.extend_from_slice(as_bytes(id));
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
#[path = "dream_begin_codec_tests.rs"]
mod tests;

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn take(&mut self, len: usize) -> Result<&[u8], String> {
        let end = self
            .at
            .checked_add(len)
            .ok_or_else(|| "Dream begin record overflow".to_owned())?;
        let value = self
            .bytes
            .get(self.at..end)
            .ok_or_else(|| "truncated Dream begin record".to_owned())?;
        self.at = end;
        Ok(value)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], String> {
        Ok(self.take(N)?.try_into().expect("length checked"))
    }

    fn u8(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }

    fn u32(&mut self) -> Result<u32, String> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    fn u64(&mut self) -> Result<u64, String> {
        Ok(u64::from_le_bytes(self.array()?))
    }
}
