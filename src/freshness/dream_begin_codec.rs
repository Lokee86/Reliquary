use crate::MemoryId;
use reliquary_dream_store::dream_begin_codec as store;
use std::collections::BTreeSet;

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

pub(crate) fn decode(bytes: &[u8]) -> Result<BeginRecord, String> {
    let record = store::decode_with_id(bytes, MemoryId)?;
    Ok(BeginRecord {
        owner_uuid: record.owner_uuid,
        pass_id: record.pass_id,
        source_id: record.source_id,
        turn: record.turn,
        graph_version_before: record.graph_version_before,
        population: match record.population {
            store::BeginPopulation::CompactCut { memory_cut, digest } => {
                BeginPopulation::CompactCut { memory_cut, digest }
            }
            store::BeginPopulation::ExplicitSet(ids) => BeginPopulation::ExplicitSet(ids),
        },
    })
}

pub(crate) fn encode(record: &BeginRecord) -> Result<Vec<u8>, String> {
    store::encode_fields(
        record.owner_uuid,
        &record.pass_id,
        record.source_id,
        record.turn,
        record.graph_version_before,
        |id| &id.0,
        match &record.population {
            BeginPopulation::CompactCut { memory_cut, digest } => {
                store::BeginPopulationRef::CompactCut {
                    memory_cut: *memory_cut,
                    digest,
                }
            }
            BeginPopulation::ExplicitSet(ids) => store::BeginPopulationRef::ExplicitSet(ids),
        },
    )
}

#[cfg(test)]
#[path = "dream_begin_codec_tests.rs"]
mod tests;
