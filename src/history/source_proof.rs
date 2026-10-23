//! Canonical source identities and owner-local corpus proof encoding.
use crate::{
    Node, archive::Archive, history::manifest_encoding::Value, owner_lifecycle::OwnerError,
};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) fn source_key(conversation: &str, node: &str) -> Value {
    Value::map([
        ("conversation_id", Value::text(conversation)),
        ("node_id", Value::text(node)),
    ])
}

pub(crate) fn node_binding(node: &Node) -> Value {
    Value::map([
        ("conversation_id", Value::text(&node.conversation_id)),
        ("node_id", Value::text(&node.id)),
        (
            "parent_id",
            node.parent_id.as_ref().map_or(Value::Null, Value::text),
        ),
        ("role", Value::text(&node.role)),
        (
            "principal_id",
            node.principal_id.as_ref().map_or(Value::Null, Value::text),
        ),
        ("timestamp_ns", Value::Signed(node.timestamp_ns)),
        ("content_id", Value::bytes(node.content_id.0)),
    ])
}

/// Selects all Nodes active by the accepted activity cut and their ancestry.
/// Iteration and parent traversal order match the historical facade algorithm.
pub(crate) fn required_nodes(
    archive: &Archive,
    through: u64,
) -> Result<BTreeMap<(String, String), Node>, OwnerError> {
    let mut required = BTreeMap::new();
    for node in archive.nodes.iter().filter(|node| {
        archive
            .activity_position_for_turn(&node.conversation_id, &node.id)
            .is_some_and(|position| position <= through)
    }) {
        let mut cursor = Some(node.clone());
        let mut seen = BTreeSet::new();
        while let Some(node) = cursor {
            let key = (node.conversation_id.clone(), node.id.clone());
            if !seen.insert(key.clone()) {
                return Err(OwnerError::Freshness(
                    "historical source ancestry cycle".into(),
                ));
            }
            if required.contains_key(&key) {
                break;
            }
            required.insert(key, node.clone());
            cursor = match node.parent_id.as_deref() {
                Some(parent) => Some(
                    archive
                        .nodes
                        .get(&node.conversation_id, parent)
                        .cloned()
                        .ok_or_else(|| {
                            OwnerError::Freshness("missing owner-local historical parent".into())
                        })?,
                ),
                None => None,
            };
        }
    }
    Ok(required)
}

pub(crate) fn scoped_node(
    archive: &Archive,
    conversation: &str,
    id: &str,
) -> Result<Node, OwnerError> {
    archive
        .nodes
        .get(conversation, id)
        .cloned()
        .ok_or_else(|| OwnerError::Freshness("missing import-scoped source Node".into()))
}

pub(crate) fn corpus_digest(bindings: Vec<Value>) -> Result<[u8; 32], OwnerError> {
    Value::Array(bindings)
        .digest("reliquary:historical-corpus:v1")
        .map_err(|error| OwnerError::Freshness(error.to_string()))
}

/// Owner-accepted source/activity facts for the higher Freshness planner.
/// No scoring, timestamp normalization, or publication happens here.
pub(crate) struct TimelineFacts {
    pub(crate) sources: Vec<Node>,
    pub(crate) positions: BTreeMap<(String, String), u64>,
    pub(crate) activity_count: u64,
}

pub(crate) fn timeline_facts(archive: &Archive) -> TimelineFacts {
    let sources: Vec<_> = archive.nodes.iter().cloned().collect();
    let positions = archive
        .nodes
        .iter()
        .filter(|node| matches!(node.role.as_str(), "user" | "assistant"))
        .filter_map(|node| {
            archive
                .activity_position_for_turn(&node.conversation_id, &node.id)
                .map(|position| ((node.conversation_id.clone(), node.id.clone()), position))
        })
        .collect();
    TimelineFacts {
        sources,
        positions,
        activity_count: archive.rel_turn_count(),
    }
}

/// Authenticate Archive content for already-selected source Nodes, then produce
/// the canonical corpus digest. Selection stays with the caller so its logical
/// refusal order precedes physical source validation.
pub(crate) fn corpus_digest_with_content(
    access: &crate::owner_lifecycle::HistoryReadAccess<'_>,
    archive: &Archive,
    nodes: Vec<Node>,
    mismatch_message: &'static str,
) -> Result<[u8; 32], OwnerError> {
    let mut bindings = Vec::with_capacity(nodes.len());
    let mut source = None;
    for node in nodes {
        archive.require_content(node.content_id)?;
        if source.is_none() {
            source = Some(
                access
                    .snapshot()
                    .map_err(|error| OwnerError::Container(error.0))?,
            );
        }
        bindings.push(node_binding_with_content(
            source.as_ref().expect("source was initialized"),
            archive,
            node,
            mismatch_message,
        )?);
    }
    corpus_digest(bindings)
}

/// Resolve and authenticate scoped source Nodes one at a time. Empty scopes do
/// not capture or validate a physical source.
pub(crate) fn scoped_corpus_digest_with_content(
    access: &crate::owner_lifecycle::HistoryReadAccess<'_>,
    archive: &Archive,
    source_nodes: &BTreeSet<(String, String)>,
    mismatch_message: &'static str,
) -> Result<[u8; 32], OwnerError> {
    let mut bindings = Vec::with_capacity(source_nodes.len());
    let mut source = None;
    for (conversation, id) in source_nodes {
        let node = scoped_node(archive, conversation, id)?;
        archive.require_content(node.content_id)?;
        if source.is_none() {
            source = Some(
                access
                    .snapshot()
                    .map_err(|error| OwnerError::Container(error.0))?,
            );
        }
        bindings.push(node_binding_with_content(
            source.as_ref().expect("source was initialized"),
            archive,
            node,
            mismatch_message,
        )?);
    }
    corpus_digest(bindings)
}

/// Authenticate one selected source Node's content and return its canonical
/// binding. Keeping this operation per-node preserves scoped lookup/read order.
pub(crate) fn node_binding_with_content(
    source: &crate::owner_lifecycle::HistorySource<'_>,
    archive: &Archive,
    node: Node,
    mismatch_message: &'static str,
) -> Result<Value, OwnerError> {
    let bytes = source.archive_content_bytes(archive, node.content_id)?;
    if crate::archive_store::hash_content(&bytes) != node.content_id {
        return Err(OwnerError::Freshness(mismatch_message.into()));
    }
    Ok(node_binding(&node))
}

/// Build and validate the accepted Archive activity clock for one source cut.
/// The timeline algorithm is a pure value operation; History owns selecting the
/// Archive facts that belong to this exact cut.
pub(crate) fn accepted_archive_timeline(
    archive: &Archive,
    through: u64,
) -> Result<crate::historical_timeline::HistoricalTimeline, OwnerError> {
    use crate::historical_timeline::{HistoricalSource, HistoricalTimeline, SourceKey};

    let nodes = required_nodes(archive, through)?;
    let sources = nodes
        .values()
        .map(|node| HistoricalSource {
            key: SourceKey {
                conversation_id: node.conversation_id.clone(),
                node_id: node.id.clone(),
            },
            parent_id: node.parent_id.clone(),
            role: node.role.clone(),
            timestamp_ns: node.timestamp_ns,
        })
        .collect();
    let positions = nodes
        .values()
        .filter_map(|node| {
            archive
                .activity_position_for_turn(&node.conversation_id, &node.id)
                .map(|position| {
                    (
                        SourceKey {
                            conversation_id: node.conversation_id.clone(),
                            node_id: node.id.clone(),
                        },
                        position,
                    )
                })
        })
        .collect();
    HistoricalTimeline::from_accepted_archive(sources, &positions, through)
        .map_err(|error| OwnerError::Freshness(error.to_string()))
}

/// Compose the accepted Archive timeline without taking a writer or publishing a score.
pub(crate) fn historical_freshness_timeline(
    archive: &Archive,
) -> Result<
    crate::freshness::historical_planner::HistoricalTimeline,
    crate::freshness::historical_planner::PlannerError,
> {
    use crate::freshness::historical_planner::{HistoricalSource, HistoricalTimeline, SourceKey};
    let facts = crate::history::source_proof::timeline_facts(archive);
    let sources = facts
        .sources
        .into_iter()
        .map(|node| HistoricalSource {
            key: SourceKey {
                conversation_id: node.conversation_id,
                node_id: node.id,
            },
            parent_id: node.parent_id,
            role: node.role,
            timestamp_ns: node.timestamp_ns,
        })
        .collect();
    let positions = facts
        .positions
        .into_iter()
        .map(|((conversation_id, node_id), position)| {
            (
                SourceKey {
                    conversation_id,
                    node_id,
                },
                position,
            )
        })
        .collect();
    HistoricalTimeline::from_accepted_archive(sources, &positions, facts.activity_count)
}
