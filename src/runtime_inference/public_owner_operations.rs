//! Native runtime operations for the supported public REL/PHY owner bindings.
use crate::{CvaError, PhylacteryError, RelMetadata, ReliquaryScopeKind};
use std::ops::{Deref, DerefMut};

/// Holds the historical embedding write permission for one concrete runtime
/// operation. Dropping the guard restores the policy even during unwinding.
pub(crate) struct HistoricalEmbeddingWriteScope<'a> {
    execution: &'a mut crate::RelExecutionOwner,
}

impl Deref for HistoricalEmbeddingWriteScope<'_> {
    type Target = crate::RelExecutionOwner;

    fn deref(&self) -> &Self::Target {
        self.execution
    }
}

impl DerefMut for HistoricalEmbeddingWriteScope<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.execution
    }
}

impl Drop for HistoricalEmbeddingWriteScope<'_> {
    fn drop(&mut self) {
        self.execution
            .owner
            .set_historical_embedding_write_scope(false);
    }
}

impl crate::RelExecutionOwner {
    pub(crate) fn runtime_historical_completed_initial_dream_passes(
        &self,
    ) -> Result<std::collections::BTreeMap<crate::MemoryId, crate::InitialDreamPass>, CvaError>
    {
        if self.runtime_owner_uuid().is_none() {
            return Err(CvaError::Freshness(
                "historical Dream proof requires a durable REL owner".into(),
            ));
        }
        crate::owner_lifecycle::HistoryReadAccess::new(&self.container)
            .retained_completed_dream_passes(&self.archive, &self.memories)
            .map_err(Into::into)
    }

    pub(crate) fn runtime_historical_dream_representatives(
        &mut self,
        source_id: crate::MemoryId,
        roots: &std::collections::BTreeSet<crate::MemoryId>,
    ) -> Result<std::collections::BTreeMap<crate::MemoryId, crate::MemoryId>, CvaError> {
        let source = crate::owner_lifecycle::HistoryReadAccess::new(&self.container);
        crate::history::dream_representative::historical_dream_representatives(
            &source,
            &self.archive,
            &self.memories,
            &self.dream_freshness,
            &self.runtime_graph_relations(),
            source_id,
            roots,
        )
        .map_err(Into::into)
    }

    pub(crate) fn runtime_validate_pruned_descriptions(&self) -> Result<(), CvaError> {
        let source = crate::owner_lifecycle::HistorySource::new(&self.container);
        crate::history::reliquary_description_retention::validate_description_retention(
            source,
            &self.owner.entities,
            &self.owner.relationships,
            &self.owner.owner_maintenance,
        )
        .map_err(Into::into)
    }

    pub(crate) fn runtime_materialized_dream_begin(
        &mut self,
        object: crate::ObjectRef,
        payload: &[u8],
    ) -> Result<Option<crate::InitialDreamPass>, CvaError> {
        crate::owner_lifecycle::dream_begin_transport::materialized_dream_begin(
            &self.container,
            &self.archive,
            &self.memories,
            object,
            payload,
        )
    }

    pub(crate) fn runtime_append_semantic_source(
        &mut self,
        owner_uuid: [u8; 16],
        payload: &[u8],
    ) -> Result<(), CvaError> {
        crate::owner_lifecycle::freshness_operations::persist_policy_if_needed(
            &mut self.owner,
            owner_uuid,
        )
        .map_err(CvaError::Freshness)?;
        crate::owner_lifecycle::freshness_operations::append_and_sync(
            &mut self.owner.container,
            payload,
        )
        .map_err(|error| CvaError::Freshness(error.to_string()))
    }

    pub(crate) fn runtime_historical_backing_length(&self) -> Result<u64, String> {
        crate::owner_lifecycle::HistoryReadAccess::new(&self.owner.container)
            .path_length()
            .map_err(|error| error.to_string())
    }

    pub(crate) fn runtime_historical_backing_facts(&self) -> Result<(u64, [u8; 32]), String> {
        let access = crate::owner_lifecycle::HistoryReadAccess::new(&self.owner.container);
        let length = access.path_length().map_err(|error| error.to_string())?;
        let digest = access.file_digest().map_err(|error| error.to_string())?;
        Ok((length, digest))
    }

    pub(crate) fn runtime_historical_embedding_write_scope(
        &mut self,
    ) -> HistoricalEmbeddingWriteScope<'_> {
        self.owner.set_historical_embedding_write_scope(true);
        HistoricalEmbeddingWriteScope { execution: self }
    }

    pub(crate) fn runtime_complete_insomnia_episode(
        &mut self,
        episode_id: crate::EpisodeId,
        token: crate::InsomniaLeaseToken,
        started_at_ns: i64,
        completed_at_ns: i64,
        extractor_model: String,
        extractor_version: String,
        memory_ids: Vec<crate::MemoryId>,
        rejected_count: u32,
    ) -> Result<crate::InsomniaWork, crate::InsomniaError> {
        self.owner.complete_insomnia_episode(
            episode_id,
            token,
            started_at_ns,
            completed_at_ns,
            extractor_model,
            extractor_version,
            memory_ids,
            rejected_count,
        )
    }

    pub(crate) fn runtime_legacy_scope_kind(&self) -> Option<ReliquaryScopeKind> {
        self.owner.legacy_scope_kind()
    }

    pub(crate) fn runtime_owner_uuid(&self) -> Option<[u8; 16]> {
        self.owner.owner_uuid()
    }

    pub(crate) fn runtime_latest_global_version(&self) -> u64 {
        self.owner.latest_global_version()
    }

    pub(crate) fn runtime_transaction_time_ns(&self, version: u64) -> Option<i64> {
        self.owner.transaction_time_ns(version)
    }

    pub(crate) fn runtime_version_at_or_before(&self, transaction_time_ns: i64) -> Option<u64> {
        self.owner.version_at_or_before(transaction_time_ns)
    }

    pub(crate) fn native_put_rel_metadata(
        &mut self,
        type_label: Option<String>,
        dependencies: Vec<String>,
    ) -> Result<bool, CvaError> {
        let type_label = type_label.and_then(|value| {
            let trimmed = value.trim();
            (!trimmed.is_empty()).then(|| trimmed.to_owned())
        });
        let mut dependencies: Vec<_> = dependencies
            .into_iter()
            .map(|value| value.trim().to_owned())
            .collect();
        dependencies.sort();
        dependencies.dedup();
        if let Some(owner_id) = self.runtime_owner_id()
            && dependencies
                .iter()
                .any(|dependency| dependency == &owner_id)
        {
            return Err(CvaError::RelMetadata(
                "a REL cannot depend on itself".into(),
            ));
        }
        self.owner
            .put_rel_metadata(RelMetadata {
                type_label,
                dependencies,
            })
            .map_err(CvaError::RelMetadata)
    }

    pub(crate) fn runtime_sync_public_owner(&self) -> Result<(), CvaError> {
        self.sync_owner()
    }
}

impl crate::phy_execution_owner::PhyExecutionOwner {
    pub(crate) fn runtime_validate_prune_evidence(&mut self) -> Result<(), PhylacteryError> {
        let source = crate::owner_lifecycle::HistorySource::new(&self.owner.container)
            .map_err(|error| PhylacteryError::Repack(error.to_string()))?;
        crate::history::validate_phylactery_prune_evidence(
            &self.owner.owner_maintenance,
            &source,
            &self.owner.graph,
            &self.owner.memories,
            &self.owner.entities,
            &self.owner.relationships,
            &self.owner.compatibility_profiles,
            &self.owner.memory_vectors,
            &self.owner.packed_vectors,
        )
        .map_err(Into::into)
    }

    pub(crate) fn runtime_reclaim_storage(
        &mut self,
        output_path: &std::path::Path,
    ) -> Result<crate::StorageReclamationReport, PhylacteryError> {
        let source =
            crate::owner_lifecycle::PhylacteryReclamationSource::prepare(&mut self.owner.container)
                .map_err(PhylacteryError::Repack)?;
        crate::maintenance::phylactery_reclamation::reclaim(
            crate::maintenance::phylactery_reclamation::PhylacteryReclamationParts {
                source,
                memories: &self.owner.memories,
                entities: &self.owner.entities,
                relationships: &self.owner.relationships,
                graph: &self.owner.graph,
            },
            output_path,
        )
        .map_err(Into::into)
    }

    pub(crate) fn runtime_owner_id(&self) -> Option<String> {
        self.owner.owner_id()
    }

    pub(crate) fn runtime_owner_uuid(&self) -> Option<[u8; 16]> {
        self.owner.owner_uuid()
    }

    pub(crate) fn runtime_latest_global_version(&self) -> u64 {
        self.owner.latest_global_version()
    }

    pub(crate) fn runtime_transaction_time_ns(&self, version: u64) -> Option<i64> {
        self.owner.transaction_time_ns(version)
    }

    pub(crate) fn runtime_version_at_or_before(&self, transaction_time_ns: i64) -> Option<u64> {
        self.owner.version_at_or_before(transaction_time_ns)
    }

    pub(crate) fn runtime_profile(&self) -> crate::PhylacteryProfile {
        self.owner.profile.current()
    }

    pub(crate) fn native_set_profile(
        &mut self,
        display_name: Option<String>,
    ) -> Result<bool, PhylacteryError> {
        self.owner
            .set_profile(display_name)
            .map_err(PhylacteryError::Profile)
    }
}
