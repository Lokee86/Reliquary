//! Historical active birth populations, excluding the source cohort.
use super::*;
use std::collections::BTreeSet;
// A cohort is one physical creation frame; siblings never count as older roots.
// The canonical Memory records supply lifecycle state at the cohort's birth,
// even when earlier Memories were subsequently promoted, archived or tombstoned.
pub(super) fn birth_population(
    container: &Container,
    memories: &MemoryStore,
    source: MemoryId,
) -> Result<BTreeSet<MemoryId>, OwnerSessionError> {
    let mut frames = container
        .scan_matching_frames(crate::records::frame_selection::without_archive_or_packed)?;
    while let Some(frame) = frames.next_frame()? {
        let envelope = reliquary_container::decode_publication(frame.payload)?;
        let payload = envelope.as_ref().map_or(frame.payload, |e| e.body);
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
        } else if crate::records::insomnia_completion::is_grouped_completion(payload) {
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
            return birth_population_at(memories, cut);
        }
    }
    Err(OwnerSessionError::StaleDreamCut)
}

pub(super) fn birth_population_at(
    memories: &MemoryStore,
    first_birth_version: u64,
) -> Result<BTreeSet<MemoryId>, OwnerSessionError> {
    let cut = first_birth_version
        .checked_sub(1)
        .ok_or(OwnerSessionError::StaleDreamCut)?;
    Ok(memories
        .memory_ids()
        .filter(|&id| {
            memories
                .record_at_cut(id, cut)
                .is_some_and(|record| !record.archived)
        })
        .collect())
}
