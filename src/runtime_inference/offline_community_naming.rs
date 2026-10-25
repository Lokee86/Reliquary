//! Supported offline community naming publication adapters.
use crate::dream_community_naming_context::representative_ids;
use crate::{
    COMMUNITY_ALGORITHM_VERSION, COMMUNITY_NAMING_CONTRACT_VERSION, CommunitySemanticName,
    CommunitySemanticNameSource, CompatibilityProfileId, DEFAULT_MEMORY_RETRIEVAL_SUBCENTROIDS,
    DreamCommunityNamer, DreamCommunityNamingError, DreamCommunityNamingResult, GeneralEndpoint,
    RelExecutionOwner, phy_execution_owner::PhyExecutionOwner,
};
impl<E: GeneralEndpoint> DreamCommunityNamer<E> {
    pub(crate) fn runtime_name_communities(
        &self,
        cva: &mut RelExecutionOwner,
        compatibility_profile_id: CompatibilityProfileId,
        limit: usize,
    ) -> Result<DreamCommunityNamingResult, DreamCommunityNamingError> {
        if limit == 0 {
            return Err(DreamCommunityNamingError::InvalidConfig("limit"));
        }
        let snapshot = current_snapshot(
            cva.runtime_community_snapshot(),
            cva.runtime_memory_graph_version(),
        )?;
        let index = cva.native_build_memory_retrieval_index(
            compatibility_profile_id,
            DEFAULT_MEMORY_RETRIEVAL_SUBCENTROIDS,
        )?;
        let pending: Vec<_> = snapshot
            .communities
            .iter()
            .filter(|community| {
                needs_name(cva.runtime_community_semantic_name(community.id).as_ref())
            })
            .take(limit)
            .cloned()
            .collect();
        let mut named = Vec::with_capacity(pending.len());
        for community in &pending {
            let representatives = representative_ids(community, &index);
            let memories = representatives
                .iter()
                .map(|memory_id| cva.runtime_memory(*memory_id))
                .collect::<Result<Vec<_>, _>>()?;
            let record = CommunitySemanticName {
                community_id: community.id,
                baseline_community_id: community.id,
                contract_version: COMMUNITY_NAMING_CONTRACT_VERSION,
                source: CommunitySemanticNameSource::Dream,
                name: self.generate_name(&memories)?,
                representative_memories: representatives,
            };
            cva.runtime_publish_community_semantic_name(record.clone())?;
            named.push(record);
        }
        let remaining = snapshot
            .communities
            .iter()
            .filter(|community| {
                needs_name(cva.runtime_community_semantic_name(community.id).as_ref())
            })
            .count();
        Ok(DreamCommunityNamingResult {
            attempted: pending.len(),
            named,
            remaining,
        })
    }

    pub(crate) fn runtime_name_phylactery_communities(
        &self,
        phylactery: &mut PhyExecutionOwner,
        compatibility_profile_id: CompatibilityProfileId,
        limit: usize,
    ) -> Result<DreamCommunityNamingResult, DreamCommunityNamingError> {
        if limit == 0 {
            return Err(DreamCommunityNamingError::InvalidConfig("limit"));
        }
        let snapshot = current_snapshot(
            phylactery.runtime_community_snapshot(),
            phylactery.native_graph_memory_graph_version(),
        )?;
        let index = phylactery.native_build_memory_retrieval_index(
            compatibility_profile_id,
            DEFAULT_MEMORY_RETRIEVAL_SUBCENTROIDS,
        )?;
        let pending: Vec<_> = snapshot
            .communities
            .iter()
            .filter(|community| {
                needs_name(
                    phylactery
                        .runtime_community_semantic_name(community.id)
                        .as_ref(),
                )
            })
            .take(limit)
            .cloned()
            .collect();
        let mut named = Vec::with_capacity(pending.len());
        for community in &pending {
            let representatives = representative_ids(community, &index);
            let memories = representatives
                .iter()
                .map(|memory_id| phylactery.memory(*memory_id))
                .collect::<Result<Vec<_>, _>>()?;
            let record = CommunitySemanticName {
                community_id: community.id,
                baseline_community_id: community.id,
                contract_version: COMMUNITY_NAMING_CONTRACT_VERSION,
                source: CommunitySemanticNameSource::Dream,
                name: self.generate_name(&memories)?,
                representative_memories: representatives,
            };
            phylactery.runtime_publish_community_semantic_name(record.clone())?;
            named.push(record);
        }
        let remaining = snapshot
            .communities
            .iter()
            .filter(|community| {
                needs_name(
                    phylactery
                        .runtime_community_semantic_name(community.id)
                        .as_ref(),
                )
            })
            .count();
        Ok(DreamCommunityNamingResult {
            attempted: pending.len(),
            named,
            remaining,
        })
    }
}

fn current_snapshot(
    snapshot: Option<crate::CommunitySnapshot>,
    graph_version: u64,
) -> Result<crate::CommunitySnapshot, DreamCommunityNamingError> {
    snapshot
        .filter(|snapshot| {
            snapshot.derived_graph_version == graph_version
                && snapshot.algorithm_version == COMMUNITY_ALGORITHM_VERSION
        })
        .ok_or(DreamCommunityNamingError::MissingCurrentSnapshot)
}

fn needs_name(name: Option<&CommunitySemanticName>) -> bool {
    match name {
        None => true,
        Some(name) if name.source == CommunitySemanticNameSource::User => false,
        Some(name) => name.contract_version < COMMUNITY_NAMING_CONTRACT_VERSION,
    }
}
