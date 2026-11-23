use super::*;
use reliquary_graph::{Community, CommunityGraph, community_id};

/// Versions and file binding for the owner state used in one calculation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CommunityRecomputeCut {
    owner: [u8; 16],
    history: [u8; 32],
    file: (u64, u64),
    memory: u64,
    graph: u64,
    community: Option<u64>,
}

pub struct CommunityRecomputeInput {
    pub cut: CommunityRecomputeCut,
    pub graph: CommunityGraph,
}

impl RelSession {
    /// Capture only the owner-local IDs and edges needed after the read lock is released.
    pub fn community_recompute_input(&self) -> CommunityRecomputeInput {
        let graph = &self.common.graph;
        let eligible = self
            .common
            .eligible_community_members(&graph.active_memory_nodes());
        CommunityRecomputeInput {
            cut: self.community_recompute_cut(),
            graph: CommunityGraph::from_relations(eligible, graph.active_relations()),
        }
    }

    fn community_recompute_cut(&self) -> CommunityRecomputeCut {
        CommunityRecomputeCut {
            owner: self.owner_id,
            history: self.history_generation,
            file: self.container_file_identity,
            memory: self.common.memories.memory_version(),
            graph: self.common.graph.graph_version(),
            community: self
                .common
                .communities
                .current()
                .map(|value| value.generation),
        }
    }
}

impl CommonState {
    fn eligible_community_members(&self, graph_members: &BTreeSet<[u8; 32]>) -> BTreeSet<[u8; 32]> {
        let cut = self.memories.memory_version();
        graph_members
            .iter()
            .copied()
            .filter(|&id| {
                let id = MemoryId(id);
                self.memories.record_at_cut(id, cut).is_some_and(|record| {
                    !record.archived
                        && record.superseded_by.is_none()
                        && self.memories.current_body_available(id)
                })
            })
            .collect()
    }
}

impl RelWriter {
    /// Validate the exact active membership and return the accepted generation and effect.
    pub fn publish_recomputed_communities(
        &mut self,
        expected: CommunityRecomputeCut,
        groups: Vec<Vec<[u8; 32]>>,
    ) -> Result<(u64, bool), OwnerSessionError> {
        self.ensure_write_ready()?;
        if self.session.community_recompute_cut() != expected {
            return Err(OwnerSessionError::ManagementStale);
        }

        let state = &mut self.session.common;
        let graph_members = state.graph.active_memory_nodes();
        let mut remaining = state.eligible_community_members(&graph_members);
        let partial = remaining.len() < graph_members.len();
        let mut communities = Vec::with_capacity(groups.len());
        for mut members in groups {
            if members.is_empty() || members.iter().any(|id| !remaining.remove(id)) {
                return Err(
                    GraphError::InvalidCommunity("invalid active Community membership").into(),
                );
            }
            members.sort_unstable();
            communities.push(Community {
                id: community_id(self.session.owner_id, &members),
                members,
            });
        }
        if !remaining.is_empty() {
            return Err(GraphError::InvalidCommunity("partition omits eligible Memories").into());
        }
        communities.sort_unstable_by_key(|group| group.id);

        if let Some(current) = state.communities.current()
            && current.graph_version == expected.graph
            && current.communities == communities
        {
            return Ok((current.generation, false));
        }
        let generation = expected
            .community
            .unwrap_or(0)
            .checked_add(1)
            .ok_or(GraphError::VersionExhausted)?;
        let snapshot = CommunitySnapshot {
            generation,
            graph_version: expected.graph,
            communities,
        };
        let result = if partial {
            state
                .communities
                .publish_partial_snapshot(&mut self.container, &state.graph, snapshot)
        } else {
            state
                .communities
                .publish(&mut self.container, &state.graph, snapshot)
        };
        match result {
            Ok(()) => {
                self.session.writer_lease_proof = self.container.writer_lease_proof();
                Ok((generation, true))
            }
            Err(error) => {
                if matches!(error, GraphError::Container(_)) {
                    self.session.write_poisoned = true;
                }
                Err(error.into())
            }
        }
    }
}
