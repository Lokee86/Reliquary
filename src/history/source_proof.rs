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
