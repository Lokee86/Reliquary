//! Stream one raw ChatGPT conversation at a time. Unknown fields are skipped.

use super::ancestry::ParentGraph;
use super::graph::{SourceConversation, SourceTurn};
use super::*;
use serde::{
    Deserialize, Deserializer,
    de::{self, IgnoredAny, MapAccess, SeqAccess, Visitor},
};
use serde_json::value::RawValue;

#[derive(Deserialize)]
struct Conversation {
    id: Option<String>,
    conversation_id: Option<String>,
    title: Option<String>,
    current_node: Option<String>,
    #[serde(default)]
    is_do_not_remember: bool,
    #[serde(deserialize_with = "read_mapping")]
    mapping: Vec<(String, Node)>,
}

#[derive(Deserialize)]
struct Node {
    id: Option<String>,
    parent: Option<String>,
    children: Option<Vec<String>>,
    message: Option<Message>,
}

#[derive(Deserialize)]
struct Message {
    id: String,
    author: Author,
    create_time: Option<Box<RawValue>>,
    content: Content,
    metadata: Option<Metadata>,
    channel: Option<String>,
    recipient: Option<String>,
}

#[derive(Deserialize)]
struct Author {
    role: String,
}

#[derive(Deserialize)]
struct Content {
    content_type: String,
    #[serde(default)]
    parts: Vec<Part>,
}

#[derive(Deserialize, Default)]
struct Metadata {
    is_visually_hidden_from_conversation: Option<bool>,
    attachments: Option<Vec<IgnoredAny>>,
}

pub(super) fn read(
    reader: impl Read,
    plan: &mut PlanBuilder,
) -> Result<(), NativeImportSourceError> {
    struct Conversations<'a> {
        plan: &'a mut PlanBuilder,
        failure: &'a mut Option<NativeImportSourceError>,
    }
    impl<'de> Visitor<'de> for Conversations<'_> {
        type Value = ();

        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("a ChatGPT conversations array")
        }

        fn visit_seq<S: SeqAccess<'de>>(self, mut rows: S) -> Result<(), S::Error> {
            while let Some(conversation) = rows.next_element::<Conversation>()? {
                if let Err(error) = normalize(conversation, self.plan) {
                    *self.failure = Some(error);
                    return Err(de::Error::custom("invalid ChatGPT conversation"));
                }
            }
            Ok(())
        }
    }
    let mut decoder = serde_json::Deserializer::from_reader(reader);
    let mut failure = None;
    decoder
        .deserialize_seq(Conversations {
            plan,
            failure: &mut failure,
        })
        .and_then(|()| decoder.end())
        .map_err(|error| failure.unwrap_or_else(|| json_error(error)))
}

fn read_mapping<'de, D: Deserializer<'de>>(decoder: D) -> Result<Vec<(String, Node)>, D::Error> {
    struct Mapping;
    impl<'de> Visitor<'de> for Mapping {
        type Value = Vec<(String, Node)>;

        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("a bounded ChatGPT node mapping")
        }

        fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<Self::Value, M::Error> {
            let mut nodes = Vec::new();
            while let Some(node) = map.next_entry()? {
                if nodes.len() == MAX_NODES {
                    return Err(de::Error::custom("ChatGPT node limit exceeded"));
                }
                // Retain duplicate keys for the graph validator to reject.
                nodes.push(node);
            }
            Ok(nodes)
        }
    }
    decoder.deserialize_map(Mapping)
}

fn normalize(
    mut source: Conversation,
    plan: &mut PlanBuilder,
) -> Result<(), NativeImportSourceError> {
    if let (Some(id), Some(alternate)) = (&source.id, &source.conversation_id)
        && id != alternate
    {
        return Err(invalid("ChatGPT conversation IDs disagree"));
    }
    let id = source
        .id
        .or(source.conversation_id)
        .ok_or_else(|| invalid("missing ChatGPT conversation ID"))?;
    plan.register(&id, source.mapping.len())?;
    if source.is_do_not_remember {
        plan.coverage.omitted_hidden_messages += source
            .mapping
            .iter()
            .filter(|(_, node)| node.message.is_some())
            .count();
        return Ok(());
    }
    let graph = ParentGraph::new(
        source
            .mapping
            .iter()
            .map(|(id, node)| (id.as_str(), node.parent.as_deref())),
    )?;
    let current = source
        .current_node
        .as_ref()
        .map(|current| {
            graph
                .by_id
                .get(current)
                .copied()
                .ok_or_else(|| invalid("ChatGPT current node is missing"))
        })
        .transpose()?;
    let mut message_ids = BTreeSet::new();
    let mut nearest: Vec<Option<usize>> = vec![None; graph.order.len()];
    let mut turns: Vec<SourceTurn> = Vec::new();
    for &index in &graph.order {
        let (node_id, node) = &mut source.mapping[index];
        if node.id.as_ref().is_some_and(|embedded| embedded != node_id) {
            return Err(invalid("ChatGPT mapping key and node ID disagree"));
        }
        if let Some(children) = &node.children {
            let mut listed = BTreeSet::new();
            for child in children {
                if !listed.insert(child)
                    || graph
                        .by_id
                        .get(child)
                        .is_none_or(|&child| graph.parents[child] != Some(index))
                {
                    return Err(invalid("ChatGPT child list disagrees with source ancestry"));
                }
            }
            if listed.len() != graph.child_count(index) {
                return Err(invalid("ChatGPT child list omits a source child"));
            }
        }
        let retained_parent = graph.parents[index].and_then(|parent| nearest[parent]);
        nearest[index] = retained_parent;
        let Some(message) = node.message.take() else {
            continue;
        };
        validate_id(&message.id)?;
        if !message_ids.insert(message.id.clone()) {
            return Err(invalid("duplicate ChatGPT message ID"));
        }
        if let Some(visible) = message.visible_text(&mut plan.coverage)? {
            nearest[index] = Some(turns.len());
            turns.push(SourceTurn {
                id: node_id.clone(),
                source_id: visible.id,
                parent: retained_parent.map(|parent| turns[parent].id.clone()),
                original_parent: node.parent.clone(),
                role: visible.role,
                timestamp_ns: visible.timestamp_ns,
                content: visible.text,
                asset_references: visible.assets,
                source_branch: None,
            });
        }
    }

    let mut branches = Vec::new();
    for tip in current
        .into_iter()
        .chain(graph.leaves.into_iter().filter(|tip| Some(*tip) != current))
    {
        if let Some(leaf) = nearest[tip] {
            branches.push(Branch {
                id: source.mapping[tip].0.clone(),
                conversation_id: id.clone(),
                leaf_node_id: turns[leaf].id.clone(),
                canonical: Some(tip) == current,
            });
        }
    }
    plan.add_conversation(SourceConversation {
        id,
        title: source.title,
        turns,
        branches,
        episodes: None,
    })
}

#[path = "source_chatgpt_text.rs"]
mod visible_text;
use visible_text::Part;
