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
