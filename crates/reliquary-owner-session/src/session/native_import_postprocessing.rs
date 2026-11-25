//! Owner admission for semantic work on the durable import extraction cohort.
use super::*;
use crate::session::grouped_completion::StoredGroupedCompletion;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

pub(in crate::session) struct VerifiedDreamImport {
    import_id: String,
    archive_version: u64,
    completion_count: usize,
    epoch_binding_count: usize,
    episodes: BTreeSet<WorkEpisodeId>,
}

impl VerifiedDreamImport {
    fn matches(&self, session: &RelSession, import_id: &str) -> bool {
        self.import_id == import_id
            && self.archive_version == session.archive.archive_version()
            && self.completion_count == session.grouped_completions.len()
            && self.epoch_binding_count == session.native_import_grouped_epochs.len()
    }
}

impl RelWriter {
    pub fn native_import_dream_identity(import_id: &str, source: MemoryId) -> (String, [u8; 32]) {
        let mut digest = Sha256::new();
        digest.update(b"reliquary-native-import-dream-v1");
        digest.update(import_id.as_bytes());
        digest.update(source.0);
        let digest = digest.finalize();
        (
            format!("{}{digest:x}", native_import_dream_prefix(import_id)),
            digest.into(),
        )
    }

    /// Check the open epoch and completed extraction before admitting a new stage.
    pub fn validate_native_import_postprocessing(
        &self,
        import_id: &str,
    ) -> Result<(), OwnerSessionError> {
        self.ensure_import_scope(import_id)?;
        self.session.native_import_completed_outputs(import_id)?;
        Ok(())
    }

    pub(in crate::session) fn verify_native_import_dream_cohort(
        &mut self,
        import_id: &str,
    ) -> Result<(), OwnerSessionError> {
        self.ensure_import_scope(import_id)?;
        if self
            .verified_dream_import
            .as_ref()
            .is_some_and(|verified| verified.matches(&self.session, import_id))
        {
            return Ok(());
        }

        // The verified Episodes have terminal Insomnia work and immutable
        // grouped receipts. An Archive or receipt-cohort change requires
        // rechecking the source paths under this writer lease.
        let episodes = self
            .session
            .native_import_completed_outputs(import_id)?
            .into_iter()
            .map(|completion| WorkEpisodeId(completion.episode_id))
            .collect();
        self.verified_dream_import = Some(VerifiedDreamImport {
            import_id: import_id.into(),
            archive_version: self.session.archive.archive_version(),
            completion_count: self.session.grouped_completions.len(),
            epoch_binding_count: self.session.native_import_grouped_epochs.len(),
            episodes,
        });
        Ok(())
    }

    /// Verify membership in this epoch's accepted local extraction. Source
    /// provenance alone is insufficient to authorize an import mutation.
    pub fn validate_native_import_memory(
        &self,
        import_id: &str,
        id: MemoryId,
    ) -> Result<(), OwnerSessionError> {
        self.native_import_memory_birth_version(import_id, id)
            .map(|_| ())
    }

    pub(in crate::session) fn native_import_memory_birth_version(
        &self,
        import_id: &str,
        id: MemoryId,
    ) -> Result<Option<u64>, OwnerSessionError> {
        self.ensure_import_scope(import_id)?;
        let record = self.session.common.memories.revision(id, 1)?.record;
        let episode = record
            .source_episode_id
            .ok_or_else(|| OwnerSessionError::Record("Memory lacks an imported Episode".into()))?;
        let verified_episode = self.verified_dream_import.as_ref().is_some_and(|verified| {
            verified.matches(&self.session, import_id)
                && verified.episodes.contains(&WorkEpisodeId(episode))
        });
        if !verified_episode {
            self.validate_import_episode_identity(
                import_id,
                reliquary_archive::EpisodeId(episode),
            )?;
        }
        self.session
            .grouped_import_birth(import_id, &record, episode)
    }

    /// Report current active import Memories whose body lacks an accepted
    /// vector mapping under this profile.
    pub fn native_import_missing_memory_vectors(
        &self,
        import_id: &str,
        profile: CompatibilityProfileId,
    ) -> Result<Vec<MemoryId>, OwnerSessionError> {
        self.require_native_import_profile(import_id, profile)?;
        let common = &self.session.common;
        if common.profiles.get_active(profile).is_none() {
            return Err(OwnerSessionError::Record(
                "import vector profile is unavailable".into(),
            ));
        }
        let mut missing = Vec::new();
        for id in self.native_import_memory_ids(import_id)? {
            let current = common.memories.current(id)?.record;
            if !current.archived
                && common
                    .memory_vectors
                    .location(
                        profile,
                        reliquary_vector_store::MemoryBodyId(current.body_id.0),
                    )
                    .is_none()
            {
                missing.push(id);
            }
        }
        Ok(missing)
    }

    /// A new import Memory is available to other first-cycle passes after
    /// its own pass settles. A pass may archive or supersede a candidate.
    pub fn native_import_dream_candidate_ready(&self, import_id: &str, id: MemoryId) -> bool {
        self.session.imports.open_epoch().is_some_and(|epoch| {
            epoch.import_id == import_id
                && (self
                    .session
                    .common
                    .memories
                    .first_memory_version(id)
                    .is_some_and(|version| version <= epoch.pre.memory_version)
                    || self.initial_dream_settled(
                        &Self::native_import_dream_identity(import_id, id).0,
                    ))
        })
    }

    pub fn native_import_memory_ids(
        &self,
        import_id: &str,
    ) -> Result<Vec<MemoryId>, OwnerSessionError> {
        self.session.native_import_memory_ids(import_id)
    }
}

impl RelSession {
    pub(in crate::session) fn grouped_import_birth(
        &self,
        import_id: &str,
        record: &reliquary_memory_core::MemoryRecord,
        episode: [u8; 32],
    ) -> Result<Option<u64>, OwnerSessionError> {
        let (_, completion) = self
            .grouped_completions
            .range((WorkEpisodeId(episode), 0)..=(WorkEpisodeId(episode), u32::MAX))
            .find(|(key, completion)| {
                self.native_import_grouped_epochs
                    .get(key)
                    .is_some_and(|epoch| epoch == import_id)
                    && completion.memory_ids.contains(&record.id.0)
            })
            .ok_or_else(|| {
                OwnerSessionError::Record("Memory is outside the accepted import extraction".into())
            })?;
        // A grouped receipt can reference an existing Memory. Use its
        // creation span only when it contains this Memory's first revision.
        Ok(completion
            .memory_versions
            .filter(|(first, last)| (*first..=*last).contains(&record.memory_version))
            .map(|(first, _)| first))
    }

    /// Enumerate accepted REL-local outputs in birth order. The epoch's
    /// physical epoch binding also supports sealed and completed reopens.
    pub fn native_import_memory_ids(
        &self,
        import_id: &str,
    ) -> Result<Vec<MemoryId>, OwnerSessionError> {
        let mut ids: Vec<_> = self
            .native_import_completed_outputs(import_id)?
            .into_iter()
            .flat_map(|completion| completion.memory_ids.iter().copied().map(MemoryId))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        ids.sort_by_key(|id| self.common.memories.first_memory_version(*id));
        Ok(ids)
    }

    /// Accepted references to one explicitly selected PHY. The caller must
    /// authorize both owners before requesting this projection.
    pub fn native_import_routed_memory_ids(
        &self,
        import_id: &str,
        destination_owner: [u8; 16],
    ) -> Result<Vec<MemoryId>, OwnerSessionError> {
        let owner = destination_owner
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        Ok(self
            .native_import_completed_outputs(import_id)?
            .into_iter()
            .flat_map(|completion| &completion.external_memory_refs)
            .filter(|reference| reference.owner_id == owner)
            .map(|reference| MemoryId(reference.memory_id))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect())
    }

    fn native_import_completed_outputs(
        &self,
        import_id: &str,
    ) -> Result<Vec<&StoredGroupedCompletion>, OwnerSessionError> {
        let (start, open) = match self.imports.status(import_id) {
            Some(ImportEpochStatus::Open(start)) if start.format_version == 2 => (start, true),
            Some(ImportEpochStatus::Completed { start, .. }) if start.format_version == 2 => {
                (start, false)
            }
            _ => {
                return Err(OwnerSessionError::Record(
                    "unknown current-format import epoch".into(),
                ));
            }
        };
        let key = super::decode_import_key(&start.import_key)
            .ok_or_else(|| OwnerSessionError::Record("invalid native import key".into()))?;
        let receipt = self.archive.import_receipt(&key).ok_or_else(|| {
            OwnerSessionError::Record("native import receipt is unavailable".into())
        })?;
        if open && self.imports.settlement(import_id).is_none() {
            self.validate_import_work_complete(receipt)?;
        }
        let mapped = super::work::mapped_source_nodes(receipt);
        let mut result = Vec::new();
        for (key, completion) in &self.grouped_completions {
            if self
                .native_import_grouped_epochs
                .get(key)
                .is_none_or(|epoch| epoch != import_id)
            {
                continue;
            }
            let id = reliquary_archive::EpisodeId(completion.episode_id);
            let Some(episode) = self.archive.episode(id) else {
                continue;
            };
            if episode.origin != reliquary_archive::EpisodeOrigin::Import {
                continue;
            }
            let turns = self.archive.episode_turns(id)?.ok_or_else(|| {
                OwnerSessionError::Record("import Episode path is unavailable".into())
            })?;
            if super::work::source_path_is_mapped(&mapped, &episode.conversation_id, &turns) {
                result.push(completion);
            }
        }
        Ok(result)
    }
}

pub(in crate::session) fn native_import_dream_prefix(import_id: &str) -> String {
    format!("native-import-dream:{import_id}:")
}

pub(in crate::session) fn native_dream_episode_is_mapped(
    archive: &Archive,
    id: reliquary_archive::EpisodeId,
    maps_node: impl Fn(&str, &str) -> bool,
) -> Result<bool, OwnerSessionError> {
    let Some(episode) = archive
        .episode(id)
        .filter(|episode| episode.origin == reliquary_archive::EpisodeOrigin::Import)
    else {
        return Ok(false);
    };
    Ok(archive.episode_node_ids(id)?.is_some_and(|nodes| {
        !nodes.is_empty()
            && nodes
                .iter()
                .all(|node| maps_node(&episode.conversation_id, node))
    }))
}
