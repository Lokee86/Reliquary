use crate::{GraphRelation, Memory, MemoryDraft, MemoryId};
use uuid::Uuid;

/// Passive replacement intent. Owner execution applies these values in order.
pub(crate) struct KnowledgeReplacementPlan {
    pub(crate) current: Memory,
    pub(crate) replacement_draft: MemoryDraft,
    pub(crate) adjacent_relations: Vec<GraphRelation>,
}

pub(crate) fn user_creation_draft(
    title: String,
    content: String,
    category: String,
    memory_type: String,
    now_ns: i64,
) -> MemoryDraft {
    MemoryDraft {
        category,
        memory_type,
        authority_kind: "direct".into(),
        title,
        content,
        scope: "private".into(),
        lifecycle_state: "extracted".into(),
        archived: false,
        superseded_by: None,
        parent_id: None,
        source_node_id: None,
        content_source_conversation_id: None,
        content_source_node_id: None,
        grounding_source_conversation_id: None,
        grounding_source_node_id: None,
        source_episode_id: None,
        source_time_ns: Some(now_ns),
        temporal_status: "unknown".into(),
        mutation_id: format!("user_creation:{}", Uuid::new_v4()),
        created_at_ns: now_ns,
        updated_at_ns: now_ns,
    }
}

pub(crate) fn plan_knowledge_replacement(
    current: &Memory,
    relations: &[GraphRelation],
    title: String,
    content: String,
    now_ns: i64,
) -> KnowledgeReplacementPlan {
    KnowledgeReplacementPlan {
        current: current.clone(),
        replacement_draft: replacement_draft(current, title, content, now_ns),
        adjacent_relations: relations
            .iter()
            .filter(|relation| relation.source == current.id || relation.target == current.id)
            .cloned()
            .collect(),
    }
}

pub(crate) fn knowledge_retirement_draft(
    current: &Memory,
    replacement_id: MemoryId,
    now_ns: i64,
) -> MemoryDraft {
    let mut retired = replacement_draft(
        current,
        current.title.clone(),
        current.content.clone(),
        now_ns,
    );
    retired.mutation_id = format!(
        "knowledge-retire:{}:{}",
        hex(&current.id.0),
        hex(&replacement_id.0),
    );
    retired.lifecycle_state = "archived".into();
    retired.archived = true;
    retired.superseded_by = Some(replacement_id);
    retired
}

fn replacement_draft(current: &Memory, title: String, content: String, now_ns: i64) -> MemoryDraft {
    let provenance = if current.mutation_id.starts_with("user_creation:") {
        "user_creation"
    } else {
        "knowledge-replacement"
    };
    MemoryDraft {
        category: current.category.clone(),
        memory_type: current.memory_type.clone(),
        authority_kind: current.authority_kind.clone(),
        title,
        content,
        scope: current.scope.clone(),
        lifecycle_state: current.lifecycle_state.clone(),
        archived: current.archived,
        superseded_by: None,
        parent_id: current.parent_id,
        source_node_id: current.source_node_id.clone(),
        content_source_conversation_id: current.content_source_conversation_id.clone(),
        content_source_node_id: current.content_source_node_id.clone(),
        grounding_source_conversation_id: current.grounding_source_conversation_id.clone(),
        grounding_source_node_id: current.grounding_source_node_id.clone(),
        source_episode_id: current.source_episode_id,
        source_time_ns: current.source_time_ns,
        temporal_status: current.temporal_status.clone(),
        mutation_id: format!("{provenance}:{}", Uuid::new_v4()),
        created_at_ns: current.created_at_ns,
        updated_at_ns: now_ns.max(current.updated_at_ns.saturating_add(1)),
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
