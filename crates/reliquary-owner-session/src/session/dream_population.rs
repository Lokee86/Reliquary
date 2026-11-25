//! Historical active birth populations, excluding the source cohort.
use super::*;
use crate::records::dream::{Population, PopulationHistory};
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
        .filter(|&id| is_active_at(memories, id, cut))
        .collect())
}

/// Project the active population before each requested cohort from typed Memory history.
pub(super) fn birth_population_history(
    memories: &MemoryStore,
    first_versions: impl IntoIterator<Item = u64>,
) -> Result<BTreeMap<u64, Population>, OwnerSessionError> {
    let first_versions: BTreeSet<_> = first_versions.into_iter().collect();
    let mut previous_cut = 0;
    let mut deltas = Vec::with_capacity(first_versions.len());
    for &first_version in &first_versions {
        if first_version > memories.memory_version() {
            return Err(OwnerSessionError::StaleDreamCut);
        }
        let cut = before_birth(first_version)?;
        let changed: BTreeSet<_> = memories
            .changes_after(previous_cut, cut, usize::MAX)
            .map(|(_, id)| id)
            .collect();
        let mut added = Vec::new();
        let mut removed = Vec::new();
        for id in changed {
            match (
                is_active_at(memories, id, previous_cut),
                is_active_at(memories, id, cut),
            ) {
                (false, true) => added.push(id),
                (true, false) => removed.push(id),
                _ => (),
            }
        }
        deltas.push((added, removed));
        previous_cut = cut;
    }
    let history = PopulationHistory::from_deltas(deltas)
        .map_err(|error| OwnerSessionError::Record(error.into()))?;
    Ok(first_versions
        .into_iter()
        .zip((1..).map(|step| history.snapshot(step)))
        .collect())
}

fn is_active_at(memories: &MemoryStore, id: MemoryId, cut: u64) -> bool {
    memories
        .record_at_cut(id, cut)
        .is_some_and(|record| !record.archived)
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
                if is_active_at(self.memories, id, version) {
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
        Ok(population
            .iter()
            .all(|&id| is_active_at(self.memories, id, cut)))
    }
}

fn before_birth(birth_version: u64) -> Result<u64, OwnerSessionError> {
    birth_version
        .checked_sub(1)
        .ok_or(OwnerSessionError::StaleDreamCut)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::records::dream::population_digest;
    use reliquary_container::{ContainerIdentity, ReliquaryScope};

    #[test]
    fn shared_birth_populations_follow_retained_memory_lifecycle() {
        let directory = tempfile::tempdir().unwrap();
        let mut container = Container::create(
            directory.path().join("birth-population.rel"),
            ContainerIdentity::reliquary(ReliquaryScope::Project),
            [62; 16],
        )
        .unwrap();
        let mut memories = MemoryStore::create(&mut container).unwrap();
        let draft = |mutation: &str, archived| MemoryDraft {
            title: "Historical Memory".into(),
            content: "Retained through lifecycle changes.".into(),
            category: "fact".into(),
            memory_type: "semantic".into(),
            authority_kind: "user".into(),
            temporal_status: "current".into(),
            scope: "local".into(),
            lifecycle_state: if archived { "archived" } else { "knowledge" }.into(),
            archived,
            parent_id: None,
            superseded_by: None,
            source_node_id: None,
            content_source_conversation_id: None,
            content_source_node_id: None,
            grounding_source_conversation_id: None,
            grounding_source_node_id: None,
            source_episode_id: None,
            source_ref: None,
            source_time_ns: None,
            temporal_inference: None,
            mutation_id: mutation.into(),
            created_at_ns: 1,
            updated_at_ns: 1,
        };
        let create = |memories: &mut MemoryStore, container: &mut Container, mutation| {
            memories
                .publish(container, None, 0, draft(mutation, false))
                .unwrap()
                .revision
                .record
        };
        let subject = create(&mut memories, &mut container, "subject");
        let before_archive = create(&mut memories, &mut container, "before-archive");
        memories
            .publish(&mut container, Some(subject.id), 1, draft("archive", true))
            .unwrap();
        let after_archive = create(&mut memories, &mut container, "after-archive");
        memories
            .publish(&mut container, Some(subject.id), 2, draft("readmit", false))
            .unwrap();
        let after_readmission = create(&mut memories, &mut container, "after-readmission");
        memories.tombstone(&mut container, subject.id, 3).unwrap();
        let after_tombstone = create(&mut memories, &mut container, "after-tombstone");

        let births = [
            (before_archive, true),
            (after_archive, false),
            (after_readmission, true),
            (after_tombstone, false),
        ];
        let cuts = births.iter().map(|(record, _)| record.memory_version);
        let populations =
            birth_population_history(&memories, cuts.clone().rev().chain(cuts)).unwrap();
        assert_eq!(populations.len(), births.len());
        for (cohort, active) in births {
            let version = cohort.memory_version;
            let population = &populations[&version];
            let expected = birth_population_at(&memories, version).unwrap();
            assert_eq!(population.contains(&subject.id), active);
            assert!(!population.contains(&cohort.id));
            assert_eq!(population.digest(), population_digest(&expected));
            assert_eq!(population, &Population::from_set(expected));
        }
        assert!(matches!(
            birth_population_history(&memories, [memories.memory_version() + 1]),
            Err(OwnerSessionError::StaleDreamCut)
        ));
    }
}
