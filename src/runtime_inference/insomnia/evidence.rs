use super::extraction::{InsomniaEvidenceResult, InsomniaEvidenceTurn, InsomniaExtractionError};
use crate::lexical_index::LexicalIndex;
use crate::lexical_search::lexical_terms;
use crate::{Archive, Cva, Episode, Fragment, Node, ResolvedTurn};
use reliquary_container::Container;
use serde_json::Value;
use std::collections::{HashMap, HashSet};

pub const MAX_INSOMNIA_EVIDENCE_REQUESTS: usize = 4;
pub const MAX_INSOMNIA_EVIDENCE_TURNS: usize = 64;
pub const MAX_INSOMNIA_EVIDENCE_BYTES: usize = 128 << 10;
const MAX_RANGE_NODES: usize = 64;
const MAX_SEARCH_RESULTS: usize = 5;
const DEFAULT_SEARCH_RESULTS: usize = 3;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct EvidenceRequest {
    pub kind: String,
    pub conversation_id: String,
    pub node_id: String,
    pub start_node_id: String,
    pub end_node_id: String,
    pub query: String,
    pub limit: usize,
}

pub(crate) fn parse_evidence_requests(
    value: &Value,
) -> Result<Vec<EvidenceRequest>, InsomniaExtractionError> {
    let Some(requests) = value.get("evidence_requests") else {
        return Ok(Vec::new());
    };
    let requests = requests.as_array().ok_or_else(|| {
        InsomniaExtractionError::InvalidOutput("evidence_requests must be an array".into())
    })?;
    if requests.len() > MAX_INSOMNIA_EVIDENCE_REQUESTS {
        return Err(InsomniaExtractionError::InvalidOutput(format!(
            "{} evidence requests exceeds maximum {}",
            requests.len(),
            MAX_INSOMNIA_EVIDENCE_REQUESTS
        )));
    }
    requests.iter().map(parse_request).collect()
}

fn parse_request(value: &Value) -> Result<EvidenceRequest, InsomniaExtractionError> {
    Ok(EvidenceRequest {
        kind: string_field(value, "kind")?.trim().to_lowercase(),
        conversation_id: optional_string(value, "conversation_id")?,
        node_id: optional_string(value, "node_id")?,
        start_node_id: optional_string(value, "start_node_id")?,
        end_node_id: optional_string(value, "end_node_id")?,
        query: optional_string(value, "query")?,
        limit: optional_usize(value, "limit")?,
    })
}

pub(crate) fn resolve_evidence(
    cva: &mut Cva,
    episode: &Episode,
    requests: &[EvidenceRequest],
) -> Result<(Vec<InsomniaEvidenceResult>, Vec<InsomniaEvidenceTurn>), InsomniaExtractionError> {
    cva.lexical_index
        .ensure_current(&cva.archive, &mut cva.container)
        .map_err(|error| {
            InsomniaExtractionError::InvalidOutput(format!(
                "Archive lexical index could not be built: {error}"
            ))
        })?;
    let plans = plan_evidence(&cva.archive, &cva.lexical_index, episode, requests);
    hydrate_evidence_parts(&cva.archive, &mut cva.container, &plans)
}

pub(crate) fn plan_evidence(
    archive: &Archive,
    lexical_index: &LexicalIndex,
    _episode: &Episode,
    requests: &[EvidenceRequest],
) -> Vec<EvidencePlan> {
    requests
        .iter()
        .map(|request| EvidencePlan {
            kind: request.kind.clone(),
            selection: match request.kind.as_str() {
                "turn" => plan_turn(archive, request),
                "conversation_range" => plan_range(archive, request),
                "archive_search" => plan_search(lexical_index, request),
                _ => Err(format!("unknown evidence request kind {:?}", request.kind)),
            },
        })
        .collect()
}

pub(crate) fn hydrate_evidence_parts(
    archive: &Archive,
    container: &mut Container,
    plans: &[EvidencePlan],
) -> Result<(Vec<InsomniaEvidenceResult>, Vec<InsomniaEvidenceTurn>), InsomniaExtractionError> {
    let mut results = Vec::with_capacity(plans.len());
    let mut all_turns = Vec::new();
    for plan in plans {
        let resolved = match &plan.selection {
            Ok(EvidenceSelection::Turn {
                conversation_id,
                node,
            }) => hydrate_turn(archive, container, conversation_id, node),
            Ok(EvidenceSelection::Range {
                conversation_id,
                nodes,
            }) => hydrate_range(archive, container, conversation_id, nodes),
            Ok(EvidenceSelection::Search { fragments }) => {
                hydrate_search(archive, container, fragments)
            }
            Err(error) => Err(error.clone()),
        };
        match resolved {
            Ok(turns) => {
                all_turns.extend(turns.iter().cloned());
                results.push(InsomniaEvidenceResult {
                    kind: plan.kind.clone(),
                    turns,
                    error: None,
                    truncated: false,
                });
            }
            Err(error) => results.push(InsomniaEvidenceResult {
                kind: plan.kind.clone(),
                turns: Vec::new(),
                error: Some(error),
                truncated: false,
            }),
        }
    }
    let bounded = bounded_turns(all_turns);
    let allowed: HashSet<_> = bounded
        .iter()
        .map(|turn| (turn.conversation_id.clone(), turn.node_id.clone()))
        .collect();
    for result in &mut results {
        let before = result.turns.len();
        result
            .turns
            .retain(|turn| allowed.contains(&(turn.conversation_id.clone(), turn.node_id.clone())));
        result.truncated = result.turns.len() < before;
    }
    Ok((results, bounded))
}

pub(crate) struct EvidencePlan {
    kind: String,
    selection: Result<EvidenceSelection, String>,
}

pub(crate) enum EvidenceSelection {
    Turn {
        conversation_id: String,
        node: Node,
    },
    Range {
        conversation_id: String,
        nodes: Vec<Node>,
    },
    Search {
        fragments: Vec<Fragment>,
    },
}

fn plan_turn(archive: &Archive, request: &EvidenceRequest) -> Result<EvidenceSelection, String> {
    let conversation_id = request.conversation_id.trim();
    let node_id = request.node_id.trim();
    if conversation_id.is_empty() || node_id.is_empty() {
        return Err("turn evidence requires conversation_id and node_id".into());
    }
    let node = archive
        .require_node(conversation_id, node_id, crate::ArchiveError::MissingNode)
        .map_err(|_| "requested turn was not found".to_owned())?
        .clone();
    Ok(EvidenceSelection::Turn {
        conversation_id: conversation_id.to_owned(),
        node,
    })
}

fn plan_range(archive: &Archive, request: &EvidenceRequest) -> Result<EvidenceSelection, String> {
    let conversation_id = request.conversation_id.trim();
    let start_node_id = request.start_node_id.trim();
    let end_node_id = request.end_node_id.trim();
    if conversation_id.is_empty() || start_node_id.is_empty() || end_node_id.is_empty() {
        return Err(
            "conversation_range requires conversation_id, start_node_id, and end_node_id".into(),
        );
    }
    let path = archive
        .branch_nodes(conversation_id, end_node_id)
        .map_err(|_| "conversation range end node was not found".to_owned())?;
    let start = path
        .iter()
        .position(|node| node.id == start_node_id)
        .ok_or_else(|| "conversation range start node is not an ancestor of end node".to_owned())?;
    let selected = &path[start..];
    if selected.is_empty() || selected.len() > MAX_RANGE_NODES {
        return Err(format!(
            "conversation_range must contain between 1 and {MAX_RANGE_NODES} nodes"
        ));
    }
    Ok(EvidenceSelection::Range {
        conversation_id: conversation_id.to_owned(),
        nodes: selected.to_vec(),
    })
}

fn plan_search(
    lexical_index: &LexicalIndex,
    request: &EvidenceRequest,
) -> Result<EvidenceSelection, String> {
    let query = request.query.trim();
    if query.is_empty() || query.len() > 512 {
        return Err("archive_search requires a query of at most 512 bytes".into());
    }
    let limit = if request.limit == 0 {
        DEFAULT_SEARCH_RESULTS
    } else {
        request.limit
    };
    if !(1..=MAX_SEARCH_RESULTS).contains(&limit) {
        return Err(format!(
            "archive_search limit must be between 1 and {MAX_SEARCH_RESULTS}"
        ));
    }
    let terms = lexical_terms(query);
    let candidates = lexical_index.search(&terms, limit.saturating_mul(4));
    let conversation_filter = request.conversation_id.trim();
    let fragments = candidates
        .into_iter()
        .filter(|hit| {
            conversation_filter.is_empty() || hit.fragment.conversation_id == conversation_filter
        })
        .take(limit)
        .map(|hit| hit.fragment)
        .collect();
    Ok(EvidenceSelection::Search { fragments })
}

fn hydrate_turn(
    archive: &Archive,
    container: &mut Container,
    conversation_id: &str,
    node: &Node,
) -> Result<Vec<InsomniaEvidenceTurn>, String> {
    let content = archive
        .content(container, node.content_id)
        .map_err(|error| error.to_string())?;
    Ok(vec![to_evidence_turn(
        conversation_id,
        ResolvedTurn {
            node_id: node.id.clone(),
            role: node.role.clone(),
            principal_id: node.principal_id.clone(),
            timestamp_ns: node.timestamp_ns,
            content,
        },
    )])
}

fn hydrate_range(
    archive: &Archive,
    container: &mut Container,
    conversation_id: &str,
    nodes: &[Node],
) -> Result<Vec<InsomniaEvidenceTurn>, String> {
    nodes
        .iter()
        .map(|node| {
            let content = archive
                .content(container, node.content_id)
                .map_err(|error| error.to_string())?;
            Ok(InsomniaEvidenceTurn {
                conversation_id: conversation_id.to_owned(),
                node_id: node.id.clone(),
                role: node.role.clone(),
                timestamp_ns: node.timestamp_ns,
                content,
            })
        })
        .collect()
}

fn hydrate_search(
    archive: &Archive,
    container: &mut Container,
    fragments: &[Fragment],
) -> Result<Vec<InsomniaEvidenceTurn>, String> {
    let mut turns = Vec::new();
    for fragment in fragments {
        let conversation_id = fragment.conversation_id.clone();
        let fragment_turns = archive
            .fragment_turns(container, fragment.id)
            .map_err(|error| error.to_string())?;
        turns.extend(
            fragment_turns
                .into_iter()
                .map(|turn| to_evidence_turn(&conversation_id, turn)),
        );
    }
    Ok(turns)
}

fn string_field(value: &Value, field: &str) -> Result<String, InsomniaExtractionError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| {
            InsomniaExtractionError::InvalidOutput(format!("missing string field {field}"))
        })
}

fn optional_string(value: &Value, field: &str) -> Result<String, InsomniaExtractionError> {
    match value.get(field) {
        None | Some(Value::Null) => Ok(String::new()),
        Some(value) => value.as_str().map(str::to_owned).ok_or_else(|| {
            InsomniaExtractionError::InvalidOutput(format!("{field} must be a string"))
        }),
    }
}

fn optional_usize(value: &Value, field: &str) -> Result<usize, InsomniaExtractionError> {
    match value.get(field) {
        None | Some(Value::Null) => Ok(0),
        Some(value) => value
            .as_u64()
            .and_then(|value| usize::try_from(value).ok())
            .ok_or_else(|| {
                InsomniaExtractionError::InvalidOutput(format!(
                    "{field} must be a non-negative integer"
                ))
            }),
    }
}

fn to_evidence_turn(conversation_id: &str, turn: ResolvedTurn) -> InsomniaEvidenceTurn {
    InsomniaEvidenceTurn {
        conversation_id: conversation_id.to_owned(),
        node_id: turn.node_id,
        role: turn.role,
        timestamp_ns: turn.timestamp_ns,
        content: turn.content,
    }
}

fn bounded_turns(turns: Vec<InsomniaEvidenceTurn>) -> Vec<InsomniaEvidenceTurn> {
    let mut unique = HashMap::new();
    for turn in turns {
        unique.insert((turn.conversation_id.clone(), turn.node_id.clone()), turn);
    }
    let mut turns: Vec<_> = unique.into_values().collect();
    turns.sort_by(|left, right| {
        left.timestamp_ns
            .cmp(&right.timestamp_ns)
            .then_with(|| left.conversation_id.cmp(&right.conversation_id))
            .then_with(|| left.node_id.cmp(&right.node_id))
    });
    let mut bounded = Vec::new();
    let mut bytes = 0usize;
    for turn in turns {
        if bounded.len() == MAX_INSOMNIA_EVIDENCE_TURNS {
            break;
        }
        let next = bytes.saturating_add(turn.content.len());
        if next > MAX_INSOMNIA_EVIDENCE_BYTES {
            continue;
        }
        bytes = next;
        bounded.push(turn);
    }
    bounded
}
