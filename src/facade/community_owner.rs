//! Typed product values over the sole Community persistence implementation.
use crate::graph_owner::GraphOwner;
use crate::*;
use reliquary_container::Container;
#[derive(Default)]
pub(crate) struct CommunityOwner {
    pub(crate) inner: reliquary_graph::CommunityStore,
}
impl CommunityOwner {
    pub(crate) fn latest(&self) -> Option<CommunitySnapshot> {
        self.inner.latest().cloned().map(Into::into)
    }
    pub(crate) fn latest_if_current_graph(&self, graph: &GraphOwner) -> Option<CommunitySnapshot> {
        self.inner
            .latest_if_current_graph(&graph.inner)
            .cloned()
            .map(Into::into)
    }
    pub(crate) fn latest_if_live_members(&self, graph: &GraphOwner) -> Option<CommunitySnapshot> {
        self.inner
            .latest_if_live_members(&graph.inner)
            .cloned()
            .map(Into::into)
    }
    pub(crate) fn contains_snapshot(&self, generation: u64, version: u64) -> bool {
        self.inner.contains_snapshot(generation, version)
    }
    pub(crate) fn semantic_name(&self, id: CommunityId) -> Option<CommunitySemanticName> {
        self.inner.semantic_name(id).map(Into::into)
    }
    pub(crate) fn current_semantic_names(&self, graph: &GraphOwner) -> Vec<CommunitySemanticName> {
        self.inner
            .current_semantic_names(&graph.inner)
            .into_iter()
            .map(Into::into)
            .collect()
    }
    pub(crate) fn latest_lineage(&self) -> Option<CommunityLineageTransition> {
        self.inner.latest_lineage()
    }
    pub(crate) fn next_generation(&self) -> Result<u64, CommunityError> {
        self.inner.next_generation().map_err(Into::into)
    }
    pub(crate) fn stats(&self, graph: &GraphOwner) -> CommunityStats {
        self.inner.stats(&graph.inner)
    }
    pub(crate) fn publish(
        &mut self,
        c: &mut Container,
        graph: &GraphOwner,
        owner: [u8; 16],
        snapshot: CommunitySnapshot,
    ) -> Result<(), CommunityError> {
        self.inner
            .publish(c, &graph.inner, owner, snapshot.into())
            .map_err(Into::into)
    }
    pub(crate) fn publish_semantic_name(
        &mut self,
        c: &mut Container,
        graph: &GraphOwner,
        name: CommunitySemanticName,
    ) -> Result<bool, CommunityError> {
        self.inner
            .publish_semantic_name(c, &graph.inner, name.into())
            .map_err(Into::into)
    }
}
pub(crate) struct CommunityReplay(reliquary_graph::CommunityOpenState);
impl CommunityReplay {
    pub(crate) fn new() -> Self {
        Self(reliquary_graph::CommunityOpenState::new())
    }
    pub(crate) fn ingest(&mut self, payload: &[u8]) -> Result<(), CommunityError> {
        self.0.ingest(payload).map_err(Into::into)
    }
    pub(crate) fn finish(
        self,
        graph: &GraphOwner,
        owner: Option<[u8; 16]>,
    ) -> Result<CommunityOwner, CommunityError> {
        self.0
            .finish(&graph.inner, owner)
            .map(|inner| CommunityOwner { inner })
            .map_err(Into::into)
    }
}
pub(crate) fn community_id(owner: [u8; 16], members: &[MemoryId]) -> CommunityId {
    reliquary_graph::community_store::community_id(
        owner,
        &members.iter().map(|id| id.0).collect::<Vec<_>>(),
    )
}
pub(crate) fn scan_merge_snapshot(
    graph: &GraphOwner,
    owner: [u8; 16],
    generation: u64,
) -> Result<CommunitySnapshot, CommunityError> {
    reliquary_graph::community_scan_merge::scan_merge_snapshot(&graph.inner, owner, generation)
        .map(Into::into)
        .map_err(Into::into)
}
