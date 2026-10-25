use crate::{
    CommunityError, CommunityId, CommunityLineageTransition, CommunitySemanticName,
    CommunitySnapshot, CommunityStats, Cva,
};

impl Cva {
    pub fn community_snapshot(&self) -> Option<CommunitySnapshot> {
        self.runtime_community_snapshot()
    }

    pub fn community_stats(&self) -> CommunityStats {
        self.runtime_community_stats()
    }

    pub fn community_semantic_name(
        &self,
        community_id: CommunityId,
    ) -> Option<CommunitySemanticName> {
        self.runtime_community_semantic_name(community_id)
    }

    pub fn community_semantic_names(&self) -> Vec<CommunitySemanticName> {
        self.runtime_community_semantic_names()
    }

    pub fn community_lineage(&self) -> Option<CommunityLineageTransition> {
        self.runtime_community_lineage()
    }

    pub fn set_community_name(
        &mut self,
        community_id: CommunityId,
        name: impl Into<String>,
    ) -> Result<bool, CommunityError> {
        self.runtime_set_knowledge_community_name(community_id, name.into())
    }

    pub(crate) fn publish_community_semantic_name(
        &mut self,
        record: CommunitySemanticName,
    ) -> Result<bool, CommunityError> {
        self.runtime_publish_community_semantic_name(record)
    }

    pub fn refresh_communities_leiden(&mut self) -> Result<CommunitySnapshot, CommunityError> {
        self.runtime_refresh_communities_leiden()
    }
}
