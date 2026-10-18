//! Store-derived attribution for one open native import epoch.
use super::ImportStart;
use crate::{Cva, CvaError, EpisodeOrigin, MemoryId};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ImportedInsomniaOutput {
    pub episode_id: [u8; 32],
    pub memory_ids: BTreeSet<MemoryId>,
    pub external_memory_refs: BTreeSet<crate::MemoryRef>,
    pub records: BTreeMap<MemoryId, crate::memory_model::MemoryRecord>,
}

pub(crate) struct ImportEpochScope {
    pub start: ImportStart,
    pub start_offset: u64,
    pub marker_end: u64,
    pub start_digest: [u8; 32],
    pub base_cut: u64,
    pub source_nodes: BTreeSet<(String, String)>,
    pub episodes: BTreeSet<[u8; 32]>,
    pub cohort: BTreeSet<MemoryId>,
    pub outputs: BTreeMap<[u8; 32], ImportedInsomniaOutput>,
}

impl Cva {
    pub(crate) fn validate_import_settlement_scope(
        &mut self,
        replay: &crate::freshness_storage::ImportEventScope,
    ) -> Result<(), CvaError> {
        let mut start = None;
        let mut scan = crate::Container::open_read_only_for_scan(self.container.path())?;
        scan.visit_payloads_through::<CvaError>(replay.freeze_prefix, |object, payload| {
            let location = object.legacy_bytes();
            let offset = u64::from_le_bytes(location[..8].try_into().unwrap());
            if offset == replay.start_prefix {
                if Sha256::digest(payload).as_slice() != replay.start_evidence_digest {
                    return Err(CvaError::Freshness(
                        "import settlement start digest mismatch".into(),
                    ));
                }
                if let Some(super::epoch_codec::ImportEpochRecord::Start(value)) =
                    super::epoch_codec::decode(payload)
                        .map_err(|e| CvaError::Freshness(e.to_string()))?
                {
                    start = Some(value);
                }
            }
            Ok(())
        })?;
        let scope = self.import_epoch_scope_through(
            start
                .ok_or_else(|| CvaError::Freshness("import settlement start unavailable".into()))?,
            replay.freeze_prefix,
        )?;
        let import_id = scope.start.import_id;
        let expected_id: String = replay
            .import_id
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        if scope.start.owner_uuid != replay.owner
            || import_id != expected_id
            || scope.start_offset != replay.start_prefix
            || scope.start_digest != replay.start_evidence_digest
            || scope.base_cut != replay.base_cut
            || scope.source_nodes != replay.source_nodes
            || scope.cohort != replay.born_memories
        {
            return Err(CvaError::Freshness(
                "import settlement source/cohort evidence mismatch".into(),
            ));
        }
        Ok(())
    }

    pub(crate) fn import_epoch_scope(&mut self) -> Result<ImportEpochScope, CvaError> {
        let start = self.open_import_epoch().ok_or_else(|| {
            CvaError::Freshness("historical reconstruction requires an open import epoch".into())
        })?;
        self.import_epoch_scope_for(start)
    }

    fn import_epoch_scope_for(&mut self, start: ImportStart) -> Result<ImportEpochScope, CvaError> {
        let through = std::fs::metadata(self.container.path())
            .map_err(|e| CvaError::Freshness(e.to_string()))?
            .len();
        self.import_epoch_scope_through(start, through)
    }

    fn import_epoch_scope_through(
        &mut self,
        start: ImportStart,
        through: u64,
    ) -> Result<ImportEpochScope, CvaError> {
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
        scan.visit_payloads_through::<CvaError>(through, |object, payload| {
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
        scan.visit_payloads_through::<CvaError>(through, |object, payload| {
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
        let mut outputs = BTreeMap::new();
        let mut scan = crate::Container::open_read_only_for_scan(self.container.path())?;
        scan.visit_payloads_through::<CvaError>(through, |object, payload| {
            let chunk = object.legacy_bytes();
            let offset = u64::from_le_bytes(chunk[..8].try_into().unwrap());
            if offset <= start_offset {
                return Ok(());
            }
            if let Some(completion) = crate::insomnia::completion::decode_completion(payload)
                .map_err(|error| CvaError::Freshness(error.into()))?
            {
                if episodes.contains(&completion.episode_id.0) {
                    let episode_key = completion.episode_id.0;
                    let mut memory_ids = BTreeSet::new();
                    for id in &completion.memory_ids {
                        if !memory_ids.insert(*id) {
                            return Err(CvaError::Freshness(
                                "duplicate Memory in imported Insomnia completion".into(),
                            ));
                        }
                    }
                    let external_memory_refs = completion
                        .external_memory_refs
                        .iter()
                        .cloned()
                        .collect::<BTreeSet<_>>();
                    let mut records = BTreeMap::new();
                    for record in completion.records.iter().cloned() {
                        if !memory_ids.contains(&record.id)
                            || records.insert(record.id, record).is_some()
                        {
                            return Err(CvaError::Freshness(
                                "import completion Memory output evidence conflicts".into(),
                            ));
                        }
                    }
                    for id in &completion.freshness_birth_memory_ids {
                        let record = records.get(id).ok_or_else(|| {
                            CvaError::Freshness(
                                "import birth has no first-publication record".into(),
                            )
                        })?;
                        if record.revision != 1
                            || record.source_episode_id != Some(completion.episode_id)
                            || self.memories.first_mutation_id(*id)
                                != Some(record.mutation_id.as_str())
                            || record.global_version <= start.pre.global_version
                        {
                            return Err(CvaError::Freshness(
                                "import birth attribution conflicts".into(),
                            ));
                        }
                        cohort.insert(*id);
                    }
                    let evidence = super::epoch_scope::ImportedInsomniaOutput {
                        episode_id: episode_key,
                        memory_ids,
                        external_memory_refs,
                        records,
                    };
                    if let Some(previous) = outputs.get(&episode_key) {
                        if previous != &evidence {
                            return Err(CvaError::Freshness(
                                "conflicting imported Insomnia completion receipts".into(),
                            ));
                        }
                    } else {
                        outputs.insert(episode_key, evidence);
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
            outputs,
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
    // Reopen authenticates retained evidence even after the conversation becomes
    // terminal. Current-only Episode hydration would discard that authority.
    let mut cursor = Some(episode.end_node_id.clone());
    let mut seen = BTreeSet::new();
    while let Some(id) = cursor {
        if !seen.insert(id.clone()) {
            return Err(CvaError::Freshness("cyclic import Episode path".into()));
        }
        if !source_nodes.contains(&(episode.conversation_id.clone(), id.clone())) {
            return Ok(false);
        }
        let node = cva
            .archive
            .nodes
            .get(&episode.conversation_id, &id)
            .ok_or_else(|| CvaError::Freshness("missing retained import Episode Node".into()))?;
        if id == episode.start_node_id {
            return Ok(true);
        }
        cursor = node.parent_id.clone();
    }
    Ok(false)
}

#[cfg(test)]
#[path = "epoch_scope_tests.rs"]
mod tests;
