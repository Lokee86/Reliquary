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
    let cut = before_birth(first_birth_version)?;
    Ok(memories
        .memory_ids()
        .filter(|&id| {
            memories
                .record_at_cut(id, cut)
                .is_some_and(|record| !record.archived)
        })
        .collect())
}

/// Advances one active population through successive cohort cuts. A later
/// Dream pass may refer to an older cohort, so also support backward cuts.
pub(super) struct BirthPopulationReplay<'a> {
    memories: &'a MemoryStore,
    cut: u64,
    exact: BTreeSet<MemoryId>,
}

impl<'a> BirthPopulationReplay<'a> {
    pub(super) fn new(memories: &'a MemoryStore) -> Self {
        Self {
            memories,
            cut: 0,
            exact: BTreeSet::new(),
        }
    }

    pub(super) fn at(
        &mut self,
        birth_version: u64,
    ) -> Result<&BTreeSet<MemoryId>, OwnerSessionError> {
        let cut = before_birth(birth_version)?.min(self.memories.memory_version());
        if cut < self.cut {
            self.exact = birth_population_at(self.memories, birth_version)?;
        } else {
            for (version, id) in self.memories.changes_after(self.cut, cut, usize::MAX) {
                if self
                    .memories
                    .record_at_cut(id, version)
                    .is_some_and(|record| !record.archived)
                {
                    self.exact.insert(id);
                } else {
                    self.exact.remove(&id);
                }
            }
        }
        self.cut = cut;
        Ok(&self.exact)
    }

    pub(super) fn contains(
        &self,
        birth_version: u64,
        population: &BTreeSet<MemoryId>,
    ) -> Result<bool, OwnerSessionError> {
        let cut = before_birth(birth_version)?;
        Ok(population.iter().all(|&id| {
            self.memories
                .record_at_cut(id, cut)
                .is_some_and(|record| !record.archived)
        }))
    }
}

fn before_birth(birth_version: u64) -> Result<u64, OwnerSessionError> {
    birth_version
        .checked_sub(1)
        .ok_or(OwnerSessionError::StaleDreamCut)
}
