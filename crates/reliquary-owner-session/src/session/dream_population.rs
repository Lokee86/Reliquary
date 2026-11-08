//! Historical active birth populations, excluding the source cohort.
use super::*;
use std::collections::BTreeSet;
// A cohort is one physical creation frame; siblings never count as older roots.
// The canonical Memory histories supply lifecycle state at the cohort's birth,
// even when earlier Memories were subsequently promoted, archived or tombstoned.
pub(super) fn birth_population(
    container: &Container,
    memories: &MemoryStore,
    source: MemoryId,
) -> Result<BTreeSet<MemoryId>, OwnerSessionError> {
    for frame in container.read_frames()? {
        let envelope = reliquary_container::decode_publication(&frame.payload)?;
        let payload = envelope
            .as_ref()
            .map_or(frame.payload.as_slice(), |e| e.body);
        let mut cohort = Vec::new();
        if let Some(record) = crate::records::user_memory::UserMemoryRecord::decode(payload)
            .map_err(OwnerSessionError::Record)?
        {
            if let crate::records::user_memory::UserMemoryEffect::Correction { creation, .. } =
                record.effect
            {
                cohort.push(reliquary_memory_core::decode_grouped_creation(&creation)?);
            }
        } else if payload.starts_with(b"RELMCRT1") {
            cohort.push(reliquary_memory_core::decode_grouped_creation(payload)?);
        } else if payload.starts_with(b"RELINSC7") {
            for record in GroupedInsomniaCompletion::decode(payload)
                .map_err(OwnerSessionError::Record)?
                .memory_records
            {
                cohort.push(reliquary_memory_core::decode_grouped_creation(&record)?);
            }
        }
        if cohort.iter().any(|r| r.id == source) {
            if cohort.iter().any(|r| r.revision != 1) {
                return Err(OwnerSessionError::StaleDreamCut);
            }
            let cut = cohort
                .iter()
                .map(|r| r.memory_version)
                .min()
                .ok_or(OwnerSessionError::StaleDreamCut)?;
            let mut population = BTreeSet::new();
            for id in memories.memory_ids() {
                let history = memories.history(id)?;
                if history.tombstone.is_some_and(|t| t.memory_version < cut) {
                    continue;
                }
                if let Some(revision) = history
                    .revisions
                    .iter()
                    .rev()
                    .find(|r| r.record.memory_version < cut)
                {
                    if !revision.record.archived {
                        population.insert(id);
                    }
                }
            }
            return Ok(population);
        }
    }
    Err(OwnerSessionError::StaleDreamCut)
}
