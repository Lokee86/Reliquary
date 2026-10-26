use crate::MemoryId;

pub const COMMUNITY_ALGORITHM_VERSION: u32 = 2;
pub const COMMUNITY_LEIDEN_SEED: u64 = 0x4c45_4944_454e_0001;
pub const COMMUNITY_LEIDEN_RESOLUTION: f64 = 1.0;
pub const COMMUNITY_NAMING_CONTRACT_VERSION: u32 = 2;
pub const DEFAULT_COMMUNITY_NAMING_REPRESENTATIVES: usize = 8;
pub const MAX_COMMUNITY_SEMANTIC_NAME_BYTES: usize = 96;
pub const DREAM_COMMUNITY_NAME_RETAIN_JACCARD_PERMILLE: u32 = 750;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CommunityId(pub [u8; 32]);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Community {
    pub id: CommunityId,
    pub members: Vec<MemoryId>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommunitySemanticNameSource {
    Dream,
    User,
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommunityLineageLink {
    pub predecessor: CommunityId,
    pub successor: CommunityId,
    pub intersection: usize,
    pub predecessor_members: usize,
    pub successor_members: usize,
    pub continuation: bool,
    pub split: bool,
    pub merge: bool,
}

impl CommunityLineageLink {
    pub fn jaccard_per_mille(&self) -> u32 {
        let union = self.predecessor_members + self.successor_members - self.intersection;
        ratio_per_mille(self.intersection, union)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommunityLineageTransition {
    pub from_generation: u64,
    pub to_generation: u64,
    pub links: Vec<CommunityLineageLink>,
    pub new_communities: Vec<CommunityId>,
    pub ended_communities: Vec<CommunityId>,
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

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CommunityStats {
    pub generations: u64,
    pub communities: usize,
    pub memberships: usize,
    pub derived_graph_version: Option<u64>,
    pub current: bool,
}

fn ratio_per_mille(numerator: usize, denominator: usize) -> u32 {
    if denominator == 0 {
        return 0;
    }
    (((numerator as u128) * 1_000) / denominator as u128) as u32
}
