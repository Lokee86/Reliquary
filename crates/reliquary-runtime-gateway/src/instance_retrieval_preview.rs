use super::*;
use reliquary_owner_session::RelRetrievalSnapshot;
use reliquary_owner_session::model::PayloadState;
use std::collections::BTreeMap;

pub(crate) const MAX_RETRIEVAL_RESULTS: usize = 128;

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct RetrievalCut {
    pub owner_id: String,
    pub owner_kind: &'static str,
    pub rel_turn: u64,
    pub memory_version: u64,
    pub graph_version: u64,
    pub community_generation: Option<u64>,
    pub community_graph_version: Option<u64>,
    pub profile_id: Option<String>,
}

impl RetrievalCut {
    pub(crate) fn capture(
        snapshot: &RelRetrievalSnapshot,
        profile: Option<CompatibilityProfileId>,
    ) -> Self {
        Self {
            owner_id: hex(&snapshot.owner_id),
            owner_kind: "rel",
            rel_turn: snapshot.rel_turn,
            memory_version: snapshot.memory_version,
            graph_version: snapshot.graph_version,
            community_generation: snapshot.community.as_ref().map(|c| c.generation),
            community_graph_version: snapshot.community.as_ref().map(|c| c.graph_version),
            profile_id: profile.map(|id| hex(&id.0)),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct RetrievalSource {
    pub episode_id: Option<String>,
    pub owner_id: Option<String>,
    pub principal_id: Option<String>,
    pub node_id: Option<String>,
    pub content_conversation_id: Option<String>,
    pub content_node_id: Option<String>,
    pub grounding_conversation_id: Option<String>,
    pub grounding_node_id: Option<String>,
    pub time_ns: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct RetrievalMemory {
    pub memory_id: String,
    pub revision: u64,
    pub body_id: String,
    pub title: String,
    pub content: String,
    pub lifecycle_state: String,
    pub availability: &'static str,
    pub source: RetrievalSource,
    pub truncated: bool,
    pub lexical_score: Option<f64>,
    pub semantic_score: Option<f64>,
    pub combined_score: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct RetrievalPreview {
    pub cut: RetrievalCut,
    pub query: String,
    pub memories: Vec<RetrievalMemory>,
    pub truncated: bool,
}

impl RetrievalPreview {
    pub(crate) fn from_evaluation(
        snapshot: &RelRetrievalSnapshot,
        profile: Option<CompatibilityProfileId>,
        evaluation: &RetrievalEvaluation,
    ) -> Result<Self, GatewayError> {
        let memories =
            project_memories(snapshot, evaluation.memories.iter().map(|memory| memory.id))?;
        Ok(Self {
            cut: RetrievalCut::capture(snapshot, profile),
            query: evaluation.next.query().unwrap_or_default().to_owned(),
            truncated: evaluation.memories.len() > memories.len()
                || memories.iter().any(|m| m.truncated),
            memories,
        })
    }
}

pub(crate) fn project_memories(
    snapshot: &RelRetrievalSnapshot,
    ids: impl IntoIterator<Item = MemoryId>,
) -> Result<Vec<RetrievalMemory>, GatewayError> {
    let records = snapshot
        .memories
        .iter()
        .map(|m| (m.revision.record.id, &m.revision))
        .collect::<BTreeMap<_, _>>();
    ids.into_iter()
        .take(MAX_RETRIEVAL_RESULTS)
        .map(|id| {
            let revision = records.get(&id).ok_or(GatewayError::StaleRetrieval)?;
            let record = &revision.record;
            let PayloadState::Available(body) = &revision.payload else {
                return Err(GatewayError::StaleRetrieval);
            };
            let mut truncated = false;
            let mut bounded = |value: &str, limit| {
                let end = value.floor_char_boundary(limit);
                truncated |= end < value.len();
                value[..end].to_owned()
            };
            let title = bounded(&body.title, 256);
            let content = bounded(&body.content, 1024);
            let lifecycle_state = bounded(&record.lifecycle_state, 128);
            let mut reference = |value: Option<&String>| {
                value.and_then(|value| {
                    if value.len() > 256 {
                        truncated = true;
                        None
                    } else {
                        Some(value.clone())
                    }
                })
            };
            let source = record.source_ref.as_ref();
            let source = RetrievalSource {
                episode_id: source
                    .map(|source| source.source_episode_id)
                    .or(record.source_episode_id)
                    .map(|id| hex(&id)),
                owner_id: reference(source.map(|source| &source.owner_id)),
                principal_id: reference(source.and_then(|source| source.principal_id.as_ref())),
                node_id: reference(
                    source
                        .map(|source| &source.source_node_id)
                        .or(record.source_node_id.as_ref()),
                ),
                content_conversation_id: reference(
                    source
                        .and_then(|source| source.content_source_conversation_id.as_ref())
                        .or(record.content_source_conversation_id.as_ref()),
                ),
                content_node_id: reference(
                    source
                        .and_then(|source| source.content_source_node_id.as_ref())
                        .or(record.content_source_node_id.as_ref()),
                ),
                grounding_conversation_id: reference(
                    source
                        .and_then(|source| source.grounding_source_conversation_id.as_ref())
                        .or(record.grounding_source_conversation_id.as_ref()),
                ),
                grounding_node_id: reference(
                    source
                        .and_then(|source| source.grounding_source_node_id.as_ref())
                        .or(record.grounding_source_node_id.as_ref()),
                ),
                time_ns: record.source_time_ns,
            };
            Ok(RetrievalMemory {
                memory_id: hex(&id.0),
                revision: record.revision,
                body_id: hex(&record.body_id.0),
                title,
                content,
                lifecycle_state,
                availability: "available",
                source,
                truncated,
                lexical_score: None,
                semantic_score: None,
                combined_score: None,
            })
        })
        .collect()
}
