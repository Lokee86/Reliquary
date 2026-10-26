use crate::MemoryId;
pub use reliquary_graph::{
    COMMUNITY_ALGORITHM_VERSION, COMMUNITY_LEIDEN_RESOLUTION, COMMUNITY_LEIDEN_SEED,
    COMMUNITY_NAMING_CONTRACT_VERSION, DEFAULT_COMMUNITY_NAMING_REPRESENTATIVES,
    DREAM_COMMUNITY_NAME_RETAIN_JACCARD_PERMILLE, MAX_COMMUNITY_SEMANTIC_NAME_BYTES,
};
pub use reliquary_graph::{
    CommunityId, CommunityLineageLink, CommunityLineageTransition, CommunitySemanticNameSource,
    CommunityStats,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Community {
    pub id: CommunityId,
    pub members: Vec<MemoryId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommunitySemanticName {
    pub community_id: CommunityId,
    pub baseline_community_id: CommunityId,
    pub contract_version: u32,
    pub source: CommunitySemanticNameSource,
    pub name: String,
    pub representative_memories: Vec<MemoryId>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CommunitySnapshot {
    pub generation: u64,
    pub derived_graph_version: u64,
    pub algorithm_version: u32,
    pub seed: u64,
    pub resolution: f64,
    pub quality: f64,
    pub communities: Vec<Community>,
}

impl CommunitySnapshot {
    pub fn community_for(&self, memory_id: MemoryId) -> Option<&Community> {
        self.communities
            .iter()
            .find(|community| community.members.contains(&memory_id))
    }
}

impl From<Community> for reliquary_graph::Community {
    fn from(v: Community) -> Self {
        Self {
            id: v.id,
            members: v.members.into_iter().map(|id| id.0).collect(),
        }
    }
}

impl From<reliquary_graph::Community> for Community {
    fn from(v: reliquary_graph::Community) -> Self {
        Self {
            id: v.id,
            members: v.members.into_iter().map(MemoryId).collect(),
        }
    }
}

impl From<CommunitySemanticName> for reliquary_graph::CommunitySemanticName {
    fn from(v: CommunitySemanticName) -> Self {
        Self {
            community_id: v.community_id,
            baseline_community_id: v.baseline_community_id,
            contract_version: v.contract_version,
            source: v.source,
            name: v.name,
            representative_memories: v
                .representative_memories
                .into_iter()
                .map(|id| id.0)
                .collect(),
        }
    }
}

impl From<reliquary_graph::CommunitySemanticName> for CommunitySemanticName {
    fn from(v: reliquary_graph::CommunitySemanticName) -> Self {
        Self {
            community_id: v.community_id,
            baseline_community_id: v.baseline_community_id,
            contract_version: v.contract_version,
            source: v.source,
            name: v.name,
            representative_memories: v
                .representative_memories
                .into_iter()
                .map(MemoryId)
                .collect(),
        }
    }
}

impl From<CommunitySnapshot> for reliquary_graph::CommunitySnapshot {
    fn from(v: CommunitySnapshot) -> Self {
        Self {
            generation: v.generation,
            derived_graph_version: v.derived_graph_version,
            algorithm_version: v.algorithm_version,
            seed: v.seed,
            resolution: v.resolution,
            quality: v.quality,
            communities: v.communities.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<reliquary_graph::CommunitySnapshot> for CommunitySnapshot {
    fn from(v: reliquary_graph::CommunitySnapshot) -> Self {
        Self {
            generation: v.generation,
            derived_graph_version: v.derived_graph_version,
            algorithm_version: v.algorithm_version,
            seed: v.seed,
            resolution: v.resolution,
            quality: v.quality,
            communities: v.communities.into_iter().map(Into::into).collect(),
        }
    }
}
