use crate::community_scan_merge::{
    COMMUNITY_SCAN_MERGE_FAN_IN, RegionSummary, ScanMergeConfig, StructuralCommunityGraph,
};
use crate::{COMMUNITY_LEIDEN_RESOLUTION, COMMUNITY_LEIDEN_SEED, CommunityError};
use leiden_rs::{GraphDataBuilder, Leiden, LeidenConfig, QualityType};
use std::collections::BTreeMap;

pub(crate) fn reduce_graph(
    graph: &StructuralCommunityGraph,
    config: ScanMergeConfig,
) -> Result<RegionSummary, CommunityError> {
    let shard_nodes = config.shard_nodes.max(1);
    let plan = crate::community_scan_merge_plan::ScanPlan::new(graph, shard_nodes);
    let shard_count = plan.leaf_nodes.len();
    let mut current = crate::community_scan_merge_workers::parallel_collect(
        shard_count,
        config.workers,
        |shard| scan_leaf(&plan.leaf_nodes[shard], &plan.leaf_edges[shard]),
    )?;

    let mut level = 1;
    while current.len() > 1 {
        let parent_count = current.len().div_ceil(COMMUNITY_SCAN_MERGE_FAN_IN);
        let previous = &current;
        current = crate::community_scan_merge_workers::parallel_collect(
            parent_count,
            config.workers,
            |parent| {
                let start = parent * COMMUNITY_SCAN_MERGE_FAN_IN;
                let end = (start + COMMUNITY_SCAN_MERGE_FAN_IN).min(previous.len());
                merge_regions(&previous[start..end], plan.boundary_edges(level, parent))
            },
        )?;
        level += 1;
    }
    current
        .pop()
        .ok_or(CommunityError::InvalidSnapshot("empty scan reduction"))
}

fn scan_leaf(nodes: &[usize], edges: &[(usize, usize)]) -> Result<RegionSummary, CommunityError> {
    let groups = nodes.iter().map(|node| vec![*node]).collect::<Vec<_>>();
    let local_by_node = nodes
        .iter()
        .enumerate()
        .map(|(local, node)| (*node, local))
        .collect::<BTreeMap<_, _>>();
    let local_edges = edges
        .iter()
        .map(|&(source, target)| (local_by_node[&source], local_by_node[&target], 1.0))
        .collect::<Vec<_>>();
    optimize_groups(groups, local_edges)
}

fn merge_regions(
    children: &[RegionSummary],
    boundary_edges: &[(usize, usize)],
) -> Result<RegionSummary, CommunityError> {
    let mut groups = Vec::new();
    let mut edges = BTreeMap::<(usize, usize), f64>::new();
    let mut node_to_group = BTreeMap::new();

    for child in children {
        let offset = groups.len();
        for group in &child.groups {
            let index = groups.len();
            for node in group {
                node_to_group.insert(*node, index);
            }
            groups.push(group.clone());
        }
        for (&(source, target), &weight) in &child.edges {
            *edges.entry((source + offset, target + offset)).or_default() += weight;
        }
    }
    for &(source, target) in boundary_edges {
        let source = *node_to_group
            .get(&source)
            .ok_or(CommunityError::InvalidSnapshot("scan boundary source"))?;
        let target = *node_to_group
            .get(&target)
            .ok_or(CommunityError::InvalidSnapshot("scan boundary target"))?;
        add_weight(&mut edges, source, target, 1.0);
    }
    optimize_groups(
        groups,
        edges
            .into_iter()
            .map(|((source, target), weight)| (source, target, weight))
            .collect(),
    )
}

fn optimize_groups(
    groups: Vec<Vec<usize>>,
    edges: Vec<(usize, usize, f64)>,
) -> Result<RegionSummary, CommunityError> {
    if groups.is_empty() {
        return Ok(RegionSummary {
            groups,
            edges: BTreeMap::new(),
            quality: 0.0,
        });
    }
    if edges.is_empty() {
        return Ok(RegionSummary {
            groups,
            edges: BTreeMap::new(),
            quality: 0.0,
        });
    }

    let mut builder = GraphDataBuilder::new(groups.len());
    for (node, group) in groups.iter().enumerate() {
        builder
            .set_node_weight(node, group.len() as f64)
            .map_err(leiden_error)?;
    }
    for &(source, target, weight) in &edges {
        builder
            .add_edge(source, target, weight)
            .map_err(leiden_error)?;
    }
    let data = builder.build().map_err(leiden_error)?;
    let result = Leiden::new(leiden_config())
        .run(&data)
        .map_err(leiden_error)?;

    let community_count = result.partition.num_communities();
    let mut merged = vec![Vec::new(); community_count];
    for (node, group) in groups.into_iter().enumerate() {
        merged[result.partition.community_of(node)].extend(group);
    }
    for group in &mut merged {
        group.sort_unstable();
    }

    let mut aggregated = BTreeMap::new();
    for (source, target, weight) in edges {
        add_weight(
            &mut aggregated,
            result.partition.community_of(source),
            result.partition.community_of(target),
            weight,
        );
    }
    Ok(RegionSummary {
        groups: merged,
        edges: aggregated,
        quality: result.quality,
    })
}

fn add_weight(edges: &mut BTreeMap<(usize, usize), f64>, a: usize, b: usize, weight: f64) {
    let pair = if a <= b { (a, b) } else { (b, a) };
    *edges.entry(pair).or_default() += weight;
}

fn leiden_config() -> LeidenConfig {
    LeidenConfig {
        resolution: COMMUNITY_LEIDEN_RESOLUTION,
        seed: Some(COMMUNITY_LEIDEN_SEED),
        quality: QualityType::Modularity,
        parallel_local_moving_threshold: None,
        parallel_aggregation_threshold: None,
        ..LeidenConfig::default()
    }
}

fn leiden_error(error: impl ToString) -> CommunityError {
    CommunityError::Leiden(error.to_string())
}
