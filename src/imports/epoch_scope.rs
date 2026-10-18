//! Store-derived attribution for one open native import epoch.
use super::ImportStart;
use crate::{Cva, CvaError, EpisodeOrigin, MemoryId};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

pub(crate) struct ImportEpochScope {
    pub start: ImportStart,
    pub start_offset: u64,
    pub marker_end: u64,
    pub start_digest: [u8; 32],
    pub base_cut: u64,
    pub source_nodes: BTreeSet<(String, String)>,
    pub episodes: BTreeSet<[u8; 32]>,
    pub cohort: BTreeSet<MemoryId>,
}

impl Cva {
    pub(crate) fn import_epoch_scope(&mut self) -> Result<ImportEpochScope, CvaError> {
        let start = self.open_import_epoch().ok_or_else(|| {
            CvaError::Freshness("historical reconstruction requires an open import epoch".into())
        })?;
        let receipt = self
            .archive
            .import_receipts
            .get(&start.import_key)
            .ok_or_else(|| {
                CvaError::Freshness("import Archive construction/receipt is incomplete".into())
            })?;
        let source_nodes: BTreeSet<_> = receipt
            .conversations
            .iter()
            .flat_map(|c| {
                c.messages.iter().filter_map(|m| {
                    m.node_id
                        .as_ref()
                        .map(|n| (c.conversation_id.clone(), n.clone()))
                })
            })
            .collect();
        let mut start_marker = None;
        let mut scan = crate::Container::open_read_only_for_scan(self.container.path())?;
        scan.visit_payloads::<CvaError>(|object, payload| {
            if let Some(super::epoch_codec::ImportEpochRecord::Start(candidate)) =
                super::epoch_codec::decode(payload)
                    .map_err(|e| CvaError::Freshness(e.to_string()))?
            {
                if candidate == start {
                    let location = object.legacy_bytes();
                    let offset = u64::from_le_bytes(location[..8].try_into().unwrap());
                    let len = u64::from_le_bytes(location[8..].try_into().unwrap());
                    let end = offset + 8 + len;
                    if start_marker
                        .replace((offset, end, Sha256::digest(payload).into()))
                        .is_some()
                    {
                        return Err(CvaError::Freshness("duplicate import start marker".into()));
                    }
                }
            }
            Ok(())
        })?;
        let (start_offset, marker_end, start_digest) = start_marker
            .ok_or_else(|| CvaError::Freshness("open import start marker is unavailable".into()))?;

        let mut episodes = BTreeSet::new();
        let mut scan = crate::Container::open_read_only_for_scan(self.container.path())?;
        scan.visit_payloads::<CvaError>(|object, payload| {
            let chunk = object.legacy_bytes();
            let offset = u64::from_le_bytes(chunk[..8].try_into().unwrap());
            if offset <= start_offset {
                return Ok(());
            }
            if let Some(episode) = crate::episode_codec::decode_episode(payload)? {
                if episode.origin == EpisodeOrigin::Import
                    && episode_matches_source(self, &episode, &source_nodes)?
                {
                    episodes.insert(episode.id.0);
                }
            }
            Ok(())
        })?;
        let mut cohort = BTreeSet::new();
        let mut scan = crate::Container::open_read_only_for_scan(self.container.path())?;
        scan.visit_payloads::<CvaError>(|object, payload| {
            let chunk = object.legacy_bytes();
            let offset = u64::from_le_bytes(chunk[..8].try_into().unwrap());
            if offset <= start_offset {
                return Ok(());
            }
            if let Some(completion) = crate::insomnia::completion::decode_completion(payload)
                .map_err(|error| CvaError::Freshness(error.into()))?
            {
                if episodes.contains(&completion.episode_id.0) {
                    for id in completion.freshness_birth_memory_ids {
                        let record = completion
                            .records
                            .iter()
                            .find(|r| r.id == id && r.revision == 1)
                            .ok_or_else(|| {
                                CvaError::Freshness(
                                    "import birth has no first-publication record".into(),
                                )
                            })?;
                        if record.source_episode_id != Some(completion.episode_id)
                            || self.memories.first_mutation_id(id)
                                != Some(record.mutation_id.as_str())
                            || record.global_version <= start.pre.global_version
                        {
                            return Err(CvaError::Freshness(
                                "import birth attribution conflicts".into(),
                            ));
                        }
                        cohort.insert(id);
                    }
                }
            }
            Ok(())
        })?;
        let base_cut = self
            .activity_cut_at_archive_version(start.pre.archive_version)
            .map_err(|e| CvaError::Freshness(e.to_string()))?;
        Ok(ImportEpochScope {
            start,
            start_offset,
            marker_end,
            start_digest,
            base_cut,
            source_nodes,
            episodes,
            cohort,
        })
    }
}

fn episode_matches_source(
    cva: &mut Cva,
    episode: &crate::Episode,
    source_nodes: &BTreeSet<(String, String)>,
) -> Result<bool, CvaError> {
    if !source_nodes.contains(&(
        episode.conversation_id.clone(),
        episode.start_node_id.clone(),
    )) || !source_nodes.contains(&(episode.conversation_id.clone(), episode.end_node_id.clone()))
    {
        return Ok(false);
    }
    let turns = cva.episode_turns(episode.id)?;
    Ok(!turns.is_empty()
        && turns.iter().all(|turn| {
            source_nodes.contains(&(episode.conversation_id.clone(), turn.node_id.clone()))
        }))
}

#[cfg(test)]
#[path = "epoch_scope_tests.rs"]
mod tests;
