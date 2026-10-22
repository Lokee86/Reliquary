use crate::community_scan_merge::scan_merge_snapshot;
use crate::{
    COMMUNITY_ALGORITHM_VERSION, CommunityError, CommunityId, CommunityLineageTransition,
    CommunitySemanticName, CommunitySemanticNameSource, CommunitySnapshot, CommunityStats, Cva,
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
        let name = name.into().trim().to_owned();
        self.publish_community_semantic_name(CommunitySemanticName {
            community_id,
            baseline_community_id: community_id,
            contract_version: 0,
            source: CommunitySemanticNameSource::User,
            name,
            representative_memories: Vec::new(),
        })
    }

    pub(crate) fn publish_community_semantic_name(
        &mut self,
        record: CommunitySemanticName,
    ) -> Result<bool, CommunityError> {
        self.communities
            .publish_semantic_name(&mut self.container, &self.graph, record)
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
        self.communities.publish(
            &mut self.container,
            &self.graph,
            owner_uuid,
            snapshot.clone(),
        )?;
        Ok(snapshot)
    }
}
