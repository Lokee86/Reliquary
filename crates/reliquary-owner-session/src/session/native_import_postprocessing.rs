//! Owner admission for semantic work on the durable import extraction cohort.
use super::*;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

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

    /// Verify membership in this epoch's accepted local extraction. Source
    /// provenance alone is insufficient to authorize an import mutation.
    pub fn validate_native_import_memory(
        &self,
        import_id: &str,
        id: MemoryId,
    ) -> Result<(), OwnerSessionError> {
        self.ensure_import_scope(import_id)?;
        let record = self.session.common.memories.revision(id, 1)?.record;
        let episode = record
            .source_episode_id
            .ok_or_else(|| OwnerSessionError::Record("Memory lacks an imported Episode".into()))?;
        self.validate_import_episode_identity(import_id, reliquary_archive::EpisodeId(episode))?;
        if !self
            .session
            .grouped_completions
            .range((WorkEpisodeId(episode), 0)..=(WorkEpisodeId(episode), u32::MAX))
            .any(|(key, completion)| {
                self.session
                    .native_import_grouped_epochs
                    .get(key)
                    .is_some_and(|epoch| epoch == import_id)
                    && completion.memory_ids.contains(&id.0)
            })
        {
            return Err(OwnerSessionError::Record(
                "Memory is outside the accepted import extraction".into(),
            ));
        }
        Ok(())
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
    ) -> Result<Vec<&GroupedInsomniaCompletion>, OwnerSessionError> {
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
