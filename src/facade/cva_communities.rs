use crate::community_scan_merge::scan_merge_snapshot;
use crate::{
    COMMUNITY_ALGORITHM_VERSION, CommunityError, CommunityId, CommunityLineageTransition,
    CommunitySemanticName, CommunitySnapshot, CommunityStats, Cva,
};

impl Cva {
    pub fn community_snapshot(&self) -> Option<CommunitySnapshot> {
        self.communities
            .latest_if_live_members(&self.graph)
            .cloned()
    }

    pub fn community_stats(&self) -> CommunityStats {
        self.communities.stats(&self.graph)
    }

    pub fn community_semantic_name(
        &self,
        community_id: CommunityId,
    ) -> Option<CommunitySemanticName> {
        self.communities.latest_if_live_members(&self.graph)?;
        self.communities.semantic_name(community_id)
    }

    pub fn community_semantic_names(&self) -> Vec<CommunitySemanticName> {
        self.communities.current_semantic_names(&self.graph)
    }

    pub fn community_lineage(&self) -> Option<CommunityLineageTransition> {
        self.communities.latest_lineage()
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
        self.owner.communities.publish_semantic_name(
            &mut self.owner.container,
            &self.owner.graph,
            record,
        )
    }

    pub fn refresh_communities_leiden(&mut self) -> Result<CommunitySnapshot, CommunityError> {
        if let Some(snapshot) = self.communities.latest()
            && snapshot.derived_graph_version == self.graph.memory_graph_version()
            && self
                .communities
                .latest_if_current_graph(&self.graph)
                .is_some()
            && snapshot.algorithm_version == COMMUNITY_ALGORITHM_VERSION
        {
            return Ok(snapshot.clone());
        }
        let owner_uuid = self
            .owner_uuid()
            .ok_or(CommunityError::MissingOwnerIdentity)?;
        let generation = self.communities.next_generation()?;
        let snapshot = scan_merge_snapshot(&self.graph, owner_uuid, generation)?;
        self.owner.communities.publish(
            &mut self.owner.container,
            &self.owner.graph,
            owner_uuid,
            snapshot.clone(),
        )?;
        Ok(snapshot)
    }
}
