use crate::{GraphError, Id, NodeRef, Relation};
use std::collections::{BTreeMap, BTreeSet, HashMap};

pub(crate) const COMMUNITY_SCAN_SHARD_NODES: usize = 2_048;
pub(crate) const COMMUNITY_SCAN_MERGE_FAN_IN: usize = 2;
pub(crate) const COMMUNITY_LEIDEN_SEED: u64 = 0x4c45_4944_454e_0001;
pub(crate) const COMMUNITY_LEIDEN_RESOLUTION: f64 = 1.0;

#[derive(Debug)]
pub struct CommunityGraph {
    pub(crate) nodes: Vec<Id>,
    pub(crate) edges: Vec<(usize, usize)>,
}

#[derive(Debug, PartialEq)]
pub struct CommunityPartition {
    pub groups: Vec<Vec<Id>>,
    pub modularity: f64,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ScanMergeConfig {
    pub(crate) shard_nodes: usize,
    pub(crate) workers: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct RegionSummary {
    pub(crate) groups: Vec<Vec<usize>>,
    pub(crate) edges: BTreeMap<(usize, usize), f64>,
    pub(crate) quality: f64,
}

impl CommunityGraph {
    /// Project eligible Memory IDs into sorted dense indices and unique undirected links.
    pub fn from_relations<'a>(
        eligible: BTreeSet<Id>,
        relations: impl IntoIterator<Item = &'a Relation>,
    ) -> Self {
        let nodes: Vec<_> = eligible.into_iter().collect();
        let indices: HashMap<_, _> = nodes.iter().copied().zip(0..).collect();
        let mut edges = BTreeSet::new();
        for relation in relations {
            if relation.active
                && let (NodeRef::Memory(source), NodeRef::Memory(target)) =
                    (relation.source, relation.target)
                && let (Some(&a), Some(&b)) = (indices.get(&source), indices.get(&target))
            {
                edges.insert((a.min(b), a.max(b)));
            }
        }
        Self {
            nodes,
            edges: edges.into_iter().collect(),
        }
    }

    pub fn eligible_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn solve(&self) -> Result<CommunityPartition, GraphError> {
        scan_merge_structural(
            self,
            ScanMergeConfig {
                shard_nodes: COMMUNITY_SCAN_SHARD_NODES,
                workers: std::thread::available_parallelism().map_or(1, usize::from),
            },
        )
    }
}

pub(crate) fn scan_merge_structural(
    graph: &CommunityGraph,
    config: ScanMergeConfig,
) -> Result<CommunityPartition, GraphError> {
    if graph.nodes.is_empty() {
        return Ok(CommunityPartition {
            groups: Vec::new(),
            modularity: 0.0,
        });
    }

    let summary = crate::community_scan_merge_reduce::reduce_graph(graph, config)?;
    let mut groups = summary
        .groups
        .into_iter()
        .map(|group| {
            let mut members: Vec<_> = group.into_iter().map(|node| graph.nodes[node]).collect();
            members.sort_unstable();
            members
        })
        .collect::<Vec<_>>();
    groups.sort_unstable();
    Ok(CommunityPartition {
        groups,
        modularity: summary.quality,
    })
}

#[cfg(test)]
#[path = "community_scan_merge_tests.rs"]
mod tests;
