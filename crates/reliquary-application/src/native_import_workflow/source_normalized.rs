//! Decode the visible-turn manifest emitted by the two-day corpus preparation.

use super::graph::{SourceConversation, SourceEpisode, SourceTurn};
use super::*;
use serde::{Deserialize, de::IgnoredAny};
use std::collections::BTreeMap;

#[derive(Deserialize)]
struct Manifest {
    schema: String,
    window_start: Option<String>,
    window_end_exclusive: Option<String>,
    conversations: Vec<Conversation>,
    turns: Vec<Turn>,
    branches: Vec<NormalizedBranch>,
    episodes: Vec<Episode>,
    #[serde(default)]
    omitted_missing_timestamps: usize,
}

#[derive(Deserialize)]
struct Conversation {
    id: String,
    title: Option<String>,
}

#[derive(Deserialize)]
struct Turn {
    conversation_id: String,
    id: String,
    parent_id: Option<String>,
    original_parent_id: Option<String>,
    message_id: Option<String>,
    role: String,
    timestamp_ns: i64,
    content: String,
    branch_id: String,
    canonical: bool,
    #[serde(default)]
    asset_refs: Vec<IgnoredAny>,
}

#[derive(Deserialize)]
struct NormalizedBranch {
    conversation_id: String,
    id: String,
    leaf_node_id: String,
    canonical: bool,
}

#[derive(Deserialize)]
struct Episode {
    conversation_id: String,
    start_node_id: String,
    end_node_id: String,
    turn_count: Option<usize>,
}

pub(super) fn read(
    reader: impl Read,
    plan: &mut PlanBuilder,
) -> Result<(), NativeImportSourceError> {
    let manifest: Manifest = serde_json::from_reader(reader).map_err(json_error)?;
    if manifest.schema != "reliquary-native-live-2day-v1" {
        return Err(invalid("unsupported normalized source schema"));
    }
    let declared = match (manifest.window_start, manifest.window_end_exclusive) {
        (Some(start), Some(end)) => Some(NativeImportWindow::from_rfc3339(&start, &end)?),
        (None, None) => None,
        _ => return Err(invalid("normalized source window requires both endpoints")),
    };
    if plan.coverage.window.is_some() && plan.coverage.window != declared {
        return Err(invalid(
            "requested window differs from the normalized source selection",
        ));
    }
    plan.coverage.window = declared;
    plan.coverage.omitted_missing_timestamps = manifest.omitted_missing_timestamps;
    if manifest.turns.len() > MAX_NODES
        || manifest.branches.len() > MAX_NODES
        || manifest.episodes.len() > MAX_NODES
    {
        return Err(NativeImportSourceError::Limit(
            "normalized source exceeds the turn, branch or episode limit",
        ));
    }

    let mut index = BTreeMap::new();
    let mut conversations = Vec::new();
    for conversation in manifest.conversations {
        plan.register(&conversation.id, 0)?;
        index.insert(conversation.id.clone(), conversations.len());
        conversations.push(SourceConversation {
            id: conversation.id,
            title: conversation.title,
            turns: Vec::new(),
            branches: Vec::new(),
            episodes: Some(Vec::new()),
        });
    }
    let find = |id: &str| {
        index
            .get(id)
            .copied()
            .ok_or_else(|| invalid("normalized row names an unknown conversation"))
    };
    let mut source_order = BTreeMap::new();
    for (position, row) in manifest.turns.into_iter().enumerate() {
        if declared.is_some_and(|window| !window.includes(row.timestamp_ns)) {
            return Err(invalid(
                "normalized source turn is outside its declared window",
            ));
        }
        let conversation = &mut conversations[find(&row.conversation_id)?];
        if row.parent_id.as_ref().is_some_and(|parent| {
            !source_order.contains_key(&(row.conversation_id.clone(), parent.clone()))
        }) {
            return Err(invalid(
                "normalized source must list retained parents before their children",
            ));
        }
        if source_order
            .insert((row.conversation_id.clone(), row.id.clone()), position)
            .is_some()
        {
            return Err(invalid("duplicate normalized source node ID"));
        }
        conversation.turns.push(SourceTurn {
            source_id: row.message_id.unwrap_or_else(|| row.id.clone()),
            id: row.id,
            original_parent: row.original_parent_id.or_else(|| row.parent_id.clone()),
            parent: row.parent_id,
            role: row.role,
            timestamp_ns: row.timestamp_ns,
            content: row.content,
            asset_references: row.asset_refs.len(),
            source_branch: Some((row.branch_id, row.canonical)),
        });
    }
    for row in manifest.branches {
        conversations[find(&row.conversation_id)?]
            .branches
            .push(Branch {
                id: row.id,
                conversation_id: row.conversation_id,
                leaf_node_id: row.leaf_node_id,
                canonical: row.canonical,
            });
    }
    for row in manifest.episodes {
        conversations[find(&row.conversation_id)?]
            .episodes
            .as_mut()
            .expect("manifest episodes")
            .push(SourceEpisode {
                start: row.start_node_id,
                end: row.end_node_id,
                turn_count: row.turn_count,
            });
    }
    for conversation in conversations {
        plan.add_conversation(conversation)?;
    }
    // Keep the manifest's accepted activity order across conversations.
    plan.turns
        .sort_by_key(|(turn, _)| source_order[&(turn.conversation_id.clone(), turn.id.clone())]);
    Ok(())
}
