use super::*;
use crate::local_host::{LocalHost, RelAccess};
use reliquary_memory::{
    GraphNode, InspectionCursor, InspectionQuery, MemoryBodyId, MemoryRetirementRequest, OwnerKind,
    RelationKind, WorkspaceManagement,
};

const PAGE: &[(&str, bool)] = &[("--cursor", false), ("--limit", false)];

pub(crate) fn graph(args: &[String]) -> Result<Value, ProductError> {
    let query = match args.first().map(String::as_str) {
        Some("neighbors") => {
            validate_options(args, 3, PAGE)?;
            InspectionQuery::Neighbors {
                anchor: graph_node(&args[2])?,
            }
        }
        Some("edge") => {
            validate_options(args, 5, PAGE)?;
            InspectionQuery::Edge {
                source: graph_node(&args[2])?,
                target: graph_node(&args[3])?,
                kind: relation_kind(&args[4])?,
            }
        }
        _ => return Err(invalid("graph requires neighbors or edge")),
    };
    rel_page(&args[1], args, query, "graph")
}

pub(crate) fn semantic(family: &str, args: &[String]) -> Result<Value, ProductError> {
    let query = match (family, args.first().map(String::as_str)) {
        ("entity", Some("list")) => {
            validate_options(args, 2, PAGE)?;
            InspectionQuery::Entities
        }
        ("relationship", Some("list")) => {
            validate_options(args, 2, PAGE)?;
            InspectionQuery::Relationships
        }
        ("community", Some("list")) => {
            validate_options(args, 2, PAGE)?;
            InspectionQuery::Communities
        }
        ("entity", Some("get")) => {
            validate_options(args, 3, PAGE)?;
            InspectionQuery::Entity(EntityId(parse_hex(&args[2], "Entity ID")?))
        }
        ("relationship", Some("get")) => {
            validate_options(args, 3, PAGE)?;
            InspectionQuery::Relationship(RelationshipId(parse_hex(&args[2], "Relationship ID")?))
        }
        _ => return Err(invalid("unsupported semantic inspection operation")),
    };
    rel_page(&args[1], args, query, family)
}

pub(crate) fn branches(args: &[String]) -> Result<Value, ProductError> {
    validate_options(args, 3, PAGE)?;
    rel_page(
        &args[1],
        args,
        InspectionQuery::Branches {
            conversation: args[2].clone(),
        },
        "archive_branches",
    )
}

pub(crate) fn retire(args: &[String]) -> Result<Value, ProductError> {
    validate_options(
        args,
        4,
        &[
            ("--revision", false),
            ("--body", false),
            ("--successor-revision", false),
            ("--successor-body", false),
        ],
    )?;
    let memory_id = inspection::memory_id(&args[2])?;
    let superseded_by = inspection::memory_id(&args[3])?;
    let memory_revision =
        number_option(args, "--revision")?.ok_or_else(|| invalid("--revision is required"))?;
    let successor_revision = number_option(args, "--successor-revision")?
        .ok_or_else(|| invalid("--successor-revision is required"))?;
    let body_id = MemoryBodyId(parse_hex(
        required_option(args, "--body")?,
        "Memory body ID",
    )?);
    let successor_body_id = MemoryBodyId(parse_hex(
        required_option(args, "--successor-body")?,
        "successor body ID",
    )?);
    let host = LocalHost::open(&args[1], None, RelAccess::Manage)?;
    let tombstone = host
        .instance
        .workspace_management(host.rel)?
        .retire_memory(MemoryRetirementRequest {
            owner_id: host.rel.as_bytes(),
            memory_id,
            memory_revision,
            body_id,
            superseded_by,
            successor_revision,
            successor_body_id,
        })?;
    Ok(json!({"operation":"memory_retire","tombstone":{
        "revision":tombstone.revision,"memory_version":tombstone.memory_version
    }}))
}

pub(crate) fn phy_memory(args: &[String]) -> Result<Value, ProductError> {
    const REL: (&str, bool) = ("--rel", false);
    let (positional, query) = match args.first().map(String::as_str) {
        Some("list") => (2, InspectionQuery::Memories),
        Some(command @ ("get" | "provenance")) if args.len() >= 3 => {
            let id = inspection::memory_id(&args[2])?;
            let revision = number_option(args, "--revision")?;
            if command == "get" {
                (3, InspectionQuery::Memory { id, revision })
            } else {
                (3, InspectionQuery::Provenance { id, revision })
            }
        }
        Some("history") if args.len() >= 3 => (
            3,
            InspectionQuery::MemoryHistory {
                id: inspection::memory_id(&args[2])?,
                memory_cut: number_option(args, "--cut")?,
            },
        ),
        _ => {
            return Err(invalid(
                "phy memory requires list, get, provenance, or history",
            ));
        }
    };
    let flags = match &query {
        InspectionQuery::Memory { .. } => [PAGE, &[("--revision", false)]].concat(),
        InspectionQuery::Provenance { .. } => [PAGE, &[("--revision", false), REL]].concat(),
        InspectionQuery::MemoryHistory { .. } => [PAGE, &[("--cut", false)]].concat(),
        _ => PAGE.to_vec(),
    };
    validate_options(args, positional, &flags)?;
    let (cursor, limit) = continuation(args)?;
    let (instance, owner) = if let Some(rel) = option(args, "--rel") {
        let host = LocalHost::open(rel, Some(&args[1]), RelAccess::View)?;
        (host.instance, host.phy.expect("explicitly mounted PHY"))
    } else {
        LocalHost::open_phy(&args[1])?
    };
    let page =
        instance.inspect_owner(owner, OwnerKind::Phylactery, &query, cursor.as_ref(), limit)?;
    Ok(json!({"operation":"phy_memory","page":page}))
}

pub(crate) fn rel_page(
    path: &str,
    args: &[String],
    query: InspectionQuery,
    operation: &str,
) -> Result<Value, ProductError> {
    let (cursor, limit) = continuation(args)?;
    let host = LocalHost::open(path, None, RelAccess::View)?;
    let management = host.instance.workspace_management(host.rel)?;
    inspect(&management, &query, cursor.as_ref(), limit, operation)
}

pub(crate) fn inspect(
    management: &WorkspaceManagement,
    query: &InspectionQuery,
    cursor: Option<&InspectionCursor>,
    limit: usize,
    operation: &str,
) -> Result<Value, ProductError> {
    Ok(json!({"operation":operation,"page":management.inspect(query, cursor, limit)?}))
}

fn continuation(args: &[String]) -> Result<(Option<InspectionCursor>, usize), ProductError> {
    let cursor = option(args, "--cursor")
        .map(|value| {
            if value.len() > 4096 {
                return Err(invalid("inspection cursor exceeds its bound"));
            }
            serde_json::from_str(value).map_err(|_| invalid("invalid inspection cursor"))
        })
        .transpose()?;
    let limit =
        number_option(args, "--limit")?.unwrap_or(reliquary_memory::MAX_MANAGEMENT_PAGE_ITEMS);
    Ok((cursor, limit))
}

fn graph_node(value: &str) -> Result<GraphNode, ProductError> {
    match value.split_once(':') {
        Some(("entity", id)) => Ok(GraphNode::Entity(parse_hex(id, "Entity ID")?)),
        Some(("memory", id)) => Ok(GraphNode::Memory(parse_hex(id, "Memory ID")?)),
        None => Ok(GraphNode::Memory(parse_hex(value, "Memory ID")?)),
        _ => Err(invalid("graph node must be a Memory or Entity ID")),
    }
}

fn relation_kind(value: &str) -> Result<RelationKind, ProductError> {
    match value {
        "topical" => Ok(RelationKind::Topical),
        "factual" => Ok(RelationKind::Factual),
        "causal" => Ok(RelationKind::Causal),
        "recurrent" => Ok(RelationKind::Recurrent),
        "references" => Ok(RelationKind::References),
        "duplicate_of" => Ok(RelationKind::DuplicateOf),
        "supersedes" => Ok(RelationKind::Supersedes),
        "structural_parent" => Ok(RelationKind::StructuralParent),
        "entity_association" => Ok(RelationKind::EntityAssociation),
        "principal_association" => Ok(RelationKind::PrincipalAssociation),
        _ => Err(invalid("unknown graph relation kind")),
    }
}
