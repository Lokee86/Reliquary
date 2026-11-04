use reliquary_memory_core::{MemoryBodyId, MemoryId};
use serde_json::{Value, json};

/// Immutable facts the classifier may use for one Memory.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DreamMemory {
    pub id: MemoryId,
    pub body_id: MemoryBodyId,
    pub category: String,
    pub memory_type: String,
    pub lifecycle_state: String,
    pub title: String,
    pub content: String,
    pub source_timestamp_ns: Option<i64>,
    /// Caller-prepared temporal anchors and recurrence facts.
    pub temporal: Value,
    /// Caller-supplied graph context, serialized as JSON values.
    pub graph_relations: Vec<Value>,
}

pub trait DreamContext {
    fn memory(&self) -> &DreamMemory;
}

impl DreamContext for DreamMemory {
    fn memory(&self) -> &DreamMemory {
        self
    }
}

pub trait DreamCandidateContexts {
    type Context: DreamContext;
    fn source(&self) -> &Self::Context;
    fn candidates(&self) -> Vec<&Self::Context>;
}

pub(crate) fn canonical_pair<'a, C: DreamContext>(
    left: &'a C,
    right: &'a C,
) -> Option<(&'a C, &'a C)> {
    if left.memory().id == right.memory().id {
        return None;
    }
    Some(if left.memory().id.0 < right.memory().id.0 {
        (left, right)
    } else {
        (right, left)
    })
}

pub(crate) fn context_json(context: &impl DreamContext) -> Value {
    let m = context.memory();
    json!({
        "memory_id": hex(&m.id.0), "body_id": hex(&m.body_id.0),
        "category": m.category, "memory_type": m.memory_type,
        "lifecycle_state": m.lifecycle_state, "title": m.title, "content": m.content,
        "source_timestamp_ns": m.source_timestamp_ns,
        "temporal": m.temporal, "graph_relations": m.graph_relations,
    })
}

pub(crate) fn memory_text(context: &impl DreamContext) -> String {
    let m = context.memory();
    format!("{}\n{}", m.title, m.content)
}

fn hex(bytes: &[u8; 32]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
