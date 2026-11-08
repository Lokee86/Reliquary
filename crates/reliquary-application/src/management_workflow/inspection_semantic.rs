use super::*;
use reliquary_owner_session::model::{NodeRef, Relation};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", content = "id", rename_all = "snake_case")]
pub enum GraphNode {
    Memory([u8; 32]),
    Entity([u8; 32]),
}

impl From<GraphNode> for NodeRef {
    fn from(node: GraphNode) -> Self {
        match node {
            GraphNode::Memory(id) => Self::Memory(id),
            GraphNode::Entity(id) => Self::Entity(id),
        }
    }
}

impl From<NodeRef> for GraphNode {
    fn from(node: NodeRef) -> Self {
        match node {
            NodeRef::Memory(id) => Self::Memory(id),
            NodeRef::Entity(id) => Self::Entity(id),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphDirection {
    Incoming,
    Outgoing,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct GraphEdge {
    pub source: GraphNode,
    pub target: GraphNode,
    #[serde(serialize_with = "serialize_enum")]
    pub kind: RelationKind,
    #[serde(serialize_with = "serialize_enum")]
    pub origin: RelationOrigin,
    pub direction: Option<GraphDirection>,
    pub active: bool,
    pub graph_version: u64,
}

fn serialize_enum<S: serde::Serializer>(
    value: &impl std::fmt::Debug,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    serializer.collect_str(&format_args!("{value:?}"))
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct InspectedEntity {
    pub id: EntityId,
    pub revision: u64,
    pub canonical_name: String,
    pub aliases: Vec<String>,
    pub kind: String,
    pub summary: String,
    pub created_at_ns: i64,
    pub updated_at_ns: i64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CommunityMetadata {
    pub id: [u8; 32],
    pub generation: u64,
    pub graph_version: u64,
    pub member_count: usize,
    pub name: Option<String>,
    pub name_origin: CommunityNameOrigin,
    pub current: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CommunityNameOrigin {
    User,
    Unnamed,
}

pub(super) fn inspect(
    read: &ManagementRead<'_>,
    query: &InspectionQuery,
    page: &mut PageBuilder,
) -> Result<(), ManagementError> {
    match *query {
        InspectionQuery::Neighbors { anchor } => {
            require_node(read, anchor)?;
            let anchor = NodeRef::from(anchor);
            let edges = read.incident_relations(anchor).filter(|edge| {
                node_available(read, edge.source) && node_available(read, edge.target)
            });
            page.extend(edges.skip(page.offset), |edge| {
                let direction = if edge.source == anchor {
                    GraphDirection::Outgoing
                } else {
                    GraphDirection::Incoming
                };
                Ok(InspectionItem::Edge(graph_edge(edge, Some(direction))))
            })
        }
        InspectionQuery::Edge {
            source,
            target,
            kind,
        } => {
            let edge = read
                .relation(source.into(), target.into(), kind)
                .ok_or(ManagementError::NotFound)?;
            page.single(InspectionItem::Edge(graph_edge(edge, None)))
        }
        InspectionQuery::Entities => page.extend(read.entities().skip(page.offset), |entity| {
            Ok(project_entity(entity))
        }),
        InspectionQuery::Entity(id) => {
            let entity = read
                .entity(id)
                .ok_or_else(|| absent(read.entity_terminal(id)))?;
            page.single(project_entity(entity))
        }
        InspectionQuery::Relationships => page
            .extend(read.relationships().skip(page.offset), |relationship| {
                Ok(InspectionItem::Relationship(relationship.clone()))
            }),
        InspectionQuery::Relationship(id) => {
            let relationship = read
                .relationship(id)
                .ok_or_else(|| absent(read.relationship_terminal(id)))?;
            page.single(InspectionItem::Relationship(relationship.clone()))
        }
        InspectionQuery::Communities => {
            let snapshot = read.community().ok_or(ManagementError::NotFound)?;
            let current = snapshot.graph_version == page.page.cut.graph;
            page.extend(snapshot.communities.iter().skip(page.offset), |community| {
                let name = read.community_name(community.id).map(str::to_owned);
                let name_origin = if name.is_some() {
                    CommunityNameOrigin::User
                } else {
                    CommunityNameOrigin::Unnamed
                };
                Ok(InspectionItem::Community(CommunityMetadata {
                    id: community.id.0,
                    generation: snapshot.generation,
                    graph_version: snapshot.graph_version,
                    member_count: community.members.len(),
                    name,
                    name_origin,
                    current,
                }))
            })
        }
        InspectionQuery::CommunityMembers {
            id,
            expected_generation,
        } => {
            let snapshot = read.community().ok_or(ManagementError::NotFound)?;
            if snapshot.graph_version != page.page.cut.graph
                || snapshot.generation != expected_generation
            {
                return Err(ManagementError::Stale);
            }
            let index = snapshot
                .communities
                .binary_search_by_key(&id, |community| community.id.0)
                .map_err(|_| ManagementError::NotFound)?;
            page.extend(
                snapshot.communities[index]
                    .members
                    .iter()
                    .copied()
                    .filter(|id| read.memory_is_readable(MemoryId(*id)))
                    .skip(page.offset),
                |id| Ok(InspectionItem::CommunityMember { id }),
            )
        }
        _ => unreachable!("semantic inspection query"),
    }
}

pub(super) fn graph_edge(edge: &Relation, direction: Option<GraphDirection>) -> GraphEdge {
    GraphEdge {
        source: edge.source.into(),
        target: edge.target.into(),
        kind: edge.kind,
        origin: edge.origin,
        direction,
        active: edge.active,
        graph_version: edge.graph_version,
    }
}

fn node_available(read: &ManagementRead<'_>, node: NodeRef) -> bool {
    match node {
        NodeRef::Memory(id) => read.memory_is_readable(MemoryId(id)),
        NodeRef::Entity(id) => read.entity(EntityId(id)).is_some(),
    }
}

fn require_node(read: &ManagementRead<'_>, node: GraphNode) -> Result<(), ManagementError> {
    if node_available(read, node.into()) {
        return Ok(());
    }
    Err(absent(match node {
        GraphNode::Memory(id) => read
            .memory_terminal(MemoryId(id), read.versions().0)
            .is_some(),
        GraphNode::Entity(id) => read.entity_terminal(EntityId(id)),
    }))
}

fn absent(terminal: bool) -> ManagementError {
    if terminal {
        ManagementError::Terminal
    } else {
        ManagementError::NotFound
    }
}

fn project_entity(entity: &reliquary_owner_session::model::Entity) -> InspectionItem {
    InspectionItem::Entity(InspectedEntity {
        id: entity.id,
        revision: entity.revision,
        canonical_name: entity.canonical_name.clone(),
        aliases: entity.aliases.clone(),
        kind: entity.kind.clone(),
        summary: entity.summary.clone(),
        created_at_ns: entity.created_at_ns,
        updated_at_ns: entity.updated_at_ns,
    })
}
