use crate::{
    COMMUNITY_ALGORITHM_VERSION, COMMUNITY_NAMING_CONTRACT_VERSION, CommunitySemanticName,
    CommunitySemanticNameSource, CompatibilityProfileId, Cva,
    DEFAULT_MEMORY_RETRIEVAL_SUBCENTROIDS, DreamCommunityNamingError, GeneralEndpoint, Memory,
    Phylactery,
};
use reliquary_dream::{
    DreamCommunityNameEvaluationError, DreamCommunityNameEvaluator,
    community_representative_ids as representative_ids,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DreamCommunityNamingResult {
    pub attempted: usize,
    pub named: Vec<CommunitySemanticName>,
    pub remaining: usize,
}

pub struct DreamCommunityNamer<E> {
    evaluator: DreamCommunityNameEvaluator<E>,
}

impl<E: GeneralEndpoint> DreamCommunityNamer<E> {
    pub fn new(endpoint: E) -> Self {
        Self {
            evaluator: DreamCommunityNameEvaluator::new(endpoint),
        }
    }

    pub fn with_system_prompt(endpoint: E, system_prompt: impl Into<String>) -> Self {
        Self {
            evaluator: DreamCommunityNameEvaluator::with_system_prompt(endpoint, system_prompt),
        }
    }

    pub fn model(&self) -> &str {
        self.evaluator.model()
    }

    pub fn name_communities(
        &self,
        cva: &mut Cva,
        compatibility_profile_id: CompatibilityProfileId,
        limit: usize,
    ) -> Result<DreamCommunityNamingResult, DreamCommunityNamingError> {
        if limit == 0 {
            return Err(DreamCommunityNamingError::InvalidConfig("limit"));
        }
        let snapshot = current_snapshot(cva.community_snapshot(), cva.memory_graph_version())?;
        let index = cva.build_memory_retrieval_index(
            compatibility_profile_id,
            DEFAULT_MEMORY_RETRIEVAL_SUBCENTROIDS,
        )?;
        let pending: Vec<_> = snapshot
            .communities
            .iter()
            .filter(|community| needs_name(cva.community_semantic_name(community.id).as_ref()))
            .take(limit)
            .cloned()
            .collect();
        let mut named = Vec::with_capacity(pending.len());
        for community in &pending {
            let centroids: Vec<_> = index
                .subcentroids()
                .iter()
                .filter(|centroid| centroid.community_id == community.id)
                .map(|centroid| centroid.vector.clone())
                .collect();
            let vectors: std::collections::HashMap<_, _> = community
                .members
                .iter()
                .filter_map(|id| index.vector_for(*id).map(|vector| (*id, vector.to_vec())))
                .collect();
            let representatives = representative_ids(&community.members, &centroids, &vectors);
            let memories = representatives
                .iter()
                .map(|memory_id| cva.memory(*memory_id))
                .collect::<Result<Vec<_>, _>>()?;
            let record = CommunitySemanticName {
                community_id: community.id,
                baseline_community_id: community.id,
                contract_version: COMMUNITY_NAMING_CONTRACT_VERSION,
                source: CommunitySemanticNameSource::Dream,
                name: self.evaluate_name(&memories)?,
                representative_memories: representatives,
            };
            cva.publish_community_semantic_name(record.clone())?;
            named.push(record);
        }
        let remaining = snapshot
            .communities
            .iter()
            .filter(|community| needs_name(cva.community_semantic_name(community.id).as_ref()))
            .count();
        Ok(DreamCommunityNamingResult {
            attempted: pending.len(),
            named,
            remaining,
        })
    }

    pub fn name_phylactery_communities(
        &self,
        phylactery: &mut Phylactery,
        compatibility_profile_id: CompatibilityProfileId,
        limit: usize,
    ) -> Result<DreamCommunityNamingResult, DreamCommunityNamingError> {
        if limit == 0 {
            return Err(DreamCommunityNamingError::InvalidConfig("limit"));
        }
        let snapshot = current_snapshot(
            phylactery.community_snapshot(),
            phylactery.memory_graph_version(),
        )?;
        let index = phylactery.build_memory_retrieval_index(
            compatibility_profile_id,
            DEFAULT_MEMORY_RETRIEVAL_SUBCENTROIDS,
        )?;
        let pending: Vec<_> = snapshot
            .communities
            .iter()
            .filter(|community| {
                needs_name(phylactery.community_semantic_name(community.id).as_ref())
            })
            .take(limit)
            .cloned()
            .collect();
        let mut named = Vec::with_capacity(pending.len());
        for community in &pending {
            let centroids: Vec<_> = index
                .subcentroids()
                .iter()
                .filter(|centroid| centroid.community_id == community.id)
                .map(|centroid| centroid.vector.clone())
                .collect();
            let vectors: std::collections::HashMap<_, _> = community
                .members
                .iter()
                .filter_map(|id| index.vector_for(*id).map(|vector| (*id, vector.to_vec())))
                .collect();
            let representatives = representative_ids(&community.members, &centroids, &vectors);
            let memories = representatives
                .iter()
                .map(|memory_id| phylactery.memory(*memory_id))
                .collect::<Result<Vec<_>, _>>()?;
            let record = CommunitySemanticName {
                community_id: community.id,
                baseline_community_id: community.id,
                contract_version: COMMUNITY_NAMING_CONTRACT_VERSION,
                source: CommunitySemanticNameSource::Dream,
                name: self.evaluate_name(&memories)?,
                representative_memories: representatives,
            };
            phylactery.publish_community_semantic_name(record.clone())?;
            named.push(record);
        }
        let remaining = snapshot
            .communities
            .iter()
            .filter(|community| {
                needs_name(phylactery.community_semantic_name(community.id).as_ref())
            })
            .count();
        Ok(DreamCommunityNamingResult {
            attempted: pending.len(),
            named,
            remaining,
        })
    }

    fn evaluate_name(&self, memories: &[Memory]) -> Result<String, DreamCommunityNamingError> {
        self.evaluator
            .generate_name(memories)
            .map_err(|error| match error {
                DreamCommunityNameEvaluationError::Payload(message)
                | DreamCommunityNameEvaluationError::InvalidOutput(message) => {
                    DreamCommunityNamingError::InvalidOutput(message)
                }
                DreamCommunityNameEvaluationError::Inference(error) => {
                    DreamCommunityNamingError::Inference(error)
                }
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
