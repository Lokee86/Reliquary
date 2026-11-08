//! Bounded Memory-to-Memory neighborhoods at an accepted owner cut.

use super::*;
use crate::management_workflow::hex;
use reliquary_owner_session::model::NodeRef;
use std::collections::{BTreeSet, VecDeque};

const MAX_SEEDS: usize = 128;
const MAX_NODES: usize = 500;
const MAX_EDGES: usize = 1000;
const MAX_INITIAL_CANDIDATES: usize = 512;
const MAX_RELATIONS_EXAMINED: usize = 4096;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryGraphQuery {
    pub seed_ids: Vec<MemoryId>,
    pub community_id: Option<[u8; 32]>,
    pub depth: u8,
    pub node_limit: usize,
    pub edge_limit: usize,
    pub expected_cut: Option<InspectionCut>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct MemoryGraphNode {
    pub id: String,
    pub revision: u64,
    pub title: String,
    pub origin: String,
    pub community_id: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct MemoryGraphEdge {
    pub id: String,
    #[serde(flatten)]
    pub edge: GraphEdge,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct MemoryGraphPage {
    pub nodes: Vec<MemoryGraphNode>,
    pub edges: Vec<MemoryGraphEdge>,
    pub truncated: bool,
}

pub(super) fn inspect(
    read: &ManagementRead<'_>,
    query: &MemoryGraphQuery,
    page: &mut PageBuilder,
) -> Result<(), ManagementError> {
    if !(1..=3).contains(&query.depth)
        || !(1..=MAX_NODES).contains(&query.node_limit)
        || !(1..=MAX_EDGES).contains(&query.edge_limit)
        || query.seed_ids.len() > MAX_SEEDS.min(query.node_limit)
        || query.seed_ids.iter().collect::<BTreeSet<_>>().len() != query.seed_ids.len()
    {
        return Err(ManagementError::InvalidInput("Memory graph bounds"));
    }

    let cut = page.page.cut;
    if query.expected_cut.is_some_and(|expected| expected != cut) {
        return Err(ManagementError::Stale);
    }
    let communities = read
        .community()
        .filter(|snapshot| snapshot.graph_version == cut.graph);
    let members = query
        .community_id
        .map(|id| {
            communities
                .ok_or(ManagementError::Unsupported)?
                .communities
                .iter()
                .find(|community| community.id.0 == id)
                .map(|community| community.members.as_slice())
                .ok_or(ManagementError::NotFound)
        })
        .transpose()?;
    let in_scope = |id: MemoryId| {
        members.is_none_or(|members| members.binary_search(&id.0).is_ok())
            && read.memory_is_readable(id)
    };
    if query.seed_ids.iter().copied().any(|id| !in_scope(id)) {
        return Err(ManagementError::NotFound);
    }
    let mut graph = MemoryGraphPage::default();
    let mut frontier = VecDeque::new();
    let mut nodes = BTreeSet::new();
    let mut edges = BTreeSet::new();
    // Reserve the item wrapper, object keys and truncation flag. Each value
    // is charged at its actual JSON wire size before inclusion.
    let mut bytes = page.remaining_bytes.saturating_sub(1024);
    let include_node =
        |graph: &mut MemoryGraphPage, nodes: &mut BTreeSet<MemoryId>, bytes: &mut usize, id| {
            if nodes.len() == query.node_limit {
                return Ok(false);
            }
            let (record, body) = read
                .memory_projection(id, None)
                .map_err(|error| ManagementError::Owner(error.to_string()))?;
            let body = body.ok_or(ManagementError::NotFound)?;
            let community_id = communities.and_then(|snapshot| {
                snapshot
                    .communities
                    .iter()
                    .find(|community| community.members.binary_search(&id.0).is_ok())
                    .map(|community| hex(&community.id.0))
            });
            let node = MemoryGraphNode {
                id: hex(&id.0),
                revision: record.revision,
                title: body.title[..body.title.floor_char_boundary(256)].to_owned(),
                origin: record.authority_kind.clone(),
                community_id,
            };
            if !take_bytes(bytes, &node)? {
                return Ok(false);
            }
            nodes.insert(id);
            graph.nodes.push(node);
            Ok::<_, ManagementError>(true)
        };

    let seeds = if !query.seed_ids.is_empty() {
        query.seed_ids.clone()
    } else if let Some(members) = members {
        members
            .iter()
            .take(MAX_INITIAL_CANDIDATES + 1)
            .map(|id| MemoryId(*id))
            .collect()
    } else {
        initial_candidates(read)
    };
    graph.truncated = seeds.len() > MAX_INITIAL_CANDIDATES;
    for seed in seeds.into_iter().take(MAX_INITIAL_CANDIDATES) {
        if !in_scope(seed) {
            continue;
        }
        if !include_node(&mut graph, &mut nodes, &mut bytes, seed)? {
            graph.truncated = true;
            break;
        }
        frontier.push_back((seed, 0));
    }

    let mut examined = 0;
    'walk: while let Some((anchor, depth)) = frontier.pop_front() {
        if depth == query.depth {
            continue;
        }
        for relation in read.incident_relations(NodeRef::Memory(anchor.0)) {
            examined += 1;
            if examined > MAX_RELATIONS_EXAMINED {
                graph.truncated = true;
                break 'walk;
            }
            let (NodeRef::Memory(source), NodeRef::Memory(target)) =
                (relation.source, relation.target)
            else {
                continue;
            };
            let neighbor = MemoryId(if source == anchor.0 { target } else { source });
            if !in_scope(neighbor) || !edges.insert((source, target, relation.kind)) {
                continue;
            }
            if graph.edges.len() == query.edge_limit {
                graph.truncated = true;
                break 'walk;
            }
            if !nodes.contains(&neighbor) {
                if !include_node(&mut graph, &mut nodes, &mut bytes, neighbor)? {
                    graph.truncated = true;
                    continue;
                }
                frontier.push_back((neighbor, depth + 1));
            }
            let edge = MemoryGraphEdge {
                id: edge_id(cut.owner_id, source, target, relation.kind),
                edge: semantic::graph_edge(relation, None),
            };
            if !take_bytes(&mut bytes, &edge)? {
                graph.truncated = true;
                break 'walk;
            }
            graph.edges.push(edge);
        }
    }

    page.single(InspectionItem::MemoryGraphPage(graph))
}

fn initial_candidates(read: &ManagementRead<'_>) -> Vec<MemoryId> {
    let mut ids = Vec::new();
    for offset in (0..=MAX_INITIAL_CANDIDATES).step_by(128) {
        let limit = (MAX_INITIAL_CANDIDATES + 1 - offset).min(128);
        let batch = read.memory_ids_from_offset(offset, limit);
        let exhausted = batch.len() < limit;
        ids.extend(batch);
        if exhausted {
            break;
        }
    }
    ids
}

fn take_bytes(bytes: &mut usize, value: &impl Serialize) -> Result<bool, ManagementError> {
    let cost = serde_json::to_vec(value)
        .map_err(|error| ManagementError::Owner(error.to_string()))?
        .len()
        + 1;
    if cost > *bytes {
        return Ok(false);
    }
    *bytes -= cost;
    Ok(true)
}

pub(super) fn edge_id(
    owner: [u8; 16],
    source: [u8; 32],
    target: [u8; 32],
    kind: RelationKind,
) -> String {
    let mut digest = Sha256::new();
    digest.update(b"management-memory-edge-v1\0");
    digest.update(owner);
    digest.update(source);
    digest.update(target);
    digest.update(format!("{kind:?}"));
    hex(&digest.finalize())
}
