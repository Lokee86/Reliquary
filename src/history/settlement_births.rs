//! Physical import-birth evidence for legacy initial Freshness settlements.

use crate::memory_store::MemoryStore;
use crate::owner_lifecycle::{HistorySource, OwnerError};
use crate::{Archive, MemoryId};
use std::collections::BTreeSet;

/// Recover native import births from the accepted physical history prefix.
/// Callers validate the checkpoint's Memory prefix before opening the source,
/// preserving the legacy preflight/error order.
pub(crate) fn legacy_import_births(
    source: &HistorySource<'_>,
    archive: &Archive,
    memories: &MemoryStore,
    global_cut: u64,
    cohort: &BTreeSet<MemoryId>,
) -> Result<BTreeSet<MemoryId>, OwnerError> {
    let mut births = BTreeSet::new();
    let mut cursor = source
        .cursor()
        .map_err(|error| OwnerError::Container(error.0))?;
    while let Some(frame) = cursor
        .next_frame()
        .map_err(|error| OwnerError::Container(error.0))?
    {
        if let Some(completion) = crate::insomnia::completion::decode_completion(&frame.payload)
            .map_err(|error| OwnerError::Freshness(error.into()))?
        {
            for id in completion.freshness_birth_memory_ids {
                if !cohort.contains(&id) {
                    continue;
                }
                let record = completion
                    .records
                    .iter()
                    .find(|record| record.id == id && record.revision == 1)
                    .ok_or_else(|| {
                        OwnerError::Freshness(
                            "import checkpoint lacks first-publication proof".into(),
                        )
                    })?;
                let episode = archive.episode(completion.episode_id).ok_or_else(|| {
                    OwnerError::Freshness("import checkpoint Episode is unavailable".into())
                })?;
                if record.global_version > global_cut
                    || record.source_episode_id != Some(episode.id)
                    || episode.origin != crate::EpisodeOrigin::Import
                    || memories.first_mutation_id(id) != Some(record.mutation_id.as_str())
                {
                    return Err(OwnerError::Freshness(
                        "import checkpoint native birth proof conflicts".into(),
                    ));
                }
                births.insert(id);
            }
        }
    }
    Ok(births)
}

/// Authenticate legacy singleton births after the physical native-birth scan.
/// The first revision is materialized before checking accepted Dream birth facts;
/// the cohort scan is deferred until the original short-circuit reaches it.
pub(crate) fn validate_legacy_singleton_births(
    source: &HistorySource<'_>,
    memories: &MemoryStore,
    dream: &crate::freshness_dream_journal::DreamFreshnessJournal,
    global_cut: u64,
    cohort: &BTreeSet<MemoryId>,
    births: &BTreeSet<MemoryId>,
) -> Result<(), OwnerError> {
    for id in cohort.difference(births) {
        let first = source.memory_revision(memories, *id, 1)?;
        if first.global_version > global_cut
            || !dream.is_publication_birth(*id)
            || super::publication_birth_cohort(source, *id)? != BTreeSet::from([*id])
            || memories.first_mutation_id(*id) != Some(first.mutation_id.as_str())
        {
            return Err(OwnerError::Freshness(
                "checkpoint lacks verified native singleton publication".into(),
            ));
        }
    }
    if !births.is_subset(cohort) {
        return Err(OwnerError::Freshness(
            "import checkpoint does not cover a proven native import".into(),
        ));
    }
    Ok(())
}
