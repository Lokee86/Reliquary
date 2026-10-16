//! Canonical ephemeral host delivery values. This module stores no delivery journal.
use crate::MemoryRef;
use crate::agentic_memory_retrieval_model::*;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

pub(crate) fn memory_ref_value(memory: &MemoryRef) -> Value {
    json!({"owner_id":memory.owner_id,"memory_id":hex_id(memory.memory_id)})
}
pub(crate) fn hex_id(id: crate::MemoryId) -> String {
    id.0.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn canonical(value: Value) -> Value {
    match value {
        Value::Object(fields) => Value::Object(
            fields
                .into_iter()
                .collect::<BTreeMap<_, _>>()
                .into_iter()
                .map(|(key, value)| (key, canonical(value)))
                .collect(),
        ),
        Value::Array(values) => Value::Array(values.into_iter().map(canonical).collect()),
        other => other,
    }
}

impl AgenticMemoryPayload {
    /// The exact model-visible JSON object; no receipt or completion tool is exposed.
    pub fn to_json(&self) -> Value {
        let operation = match self.operation {
            AgenticMemoryOperation::Query => "memory_query",
            AgenticMemoryOperation::Refinement => "refine_query",
            AgenticMemoryOperation::Traversal => "traverse_graph",
        };
        let outcome = match &self.outcome {
            AgenticMemoryOutcome::Memories(content) => json!({"status":"memories",
                "memories":content.iter().map(|body|json!({"memory":memory_ref_value(&body.memory),
                    "title":body.title,"content":body.content})).collect::<Vec<_>>()}),
            AgenticMemoryOutcome::AlreadyProvided(ids) => json!({"status":"already_provided",
                "memories":ids.iter().map(memory_ref_value).collect::<Vec<_>>()}),
            AgenticMemoryOutcome::NoMatches => json!({"status":"no_matches"}),
        };
        canonical(json!({"version":1,"operation":operation,"outcome":outcome,
            "anchors":self.anchors.iter().map(|report|json!({
                "anchor":memory_ref_value(&report.anchor),
                "unseen_neighbors":report.unseen_neighbors,"declined_neighbors":report.declined_neighbors,
                "relations":report.relations.iter().map(|edge|json!({
                    "neighbor":memory_ref_value(&edge.neighbor),"kind":format!("{:?}",edge.kind),
                    "direction":format!("{:?}",edge.direction)})).collect::<Vec<_>>(),
                "approved":report.approved.iter().map(memory_ref_value).collect::<Vec<_>>()
            })).collect::<Vec<_>>() }))
    }
}

impl PreparedAgenticMemoryResult {
    /// Canonical host manifest. Hosts retain this exact value through their existing replay seam.
    /// Only `payload` is model-visible. Owner cuts and delivered IDs are receipt inputs.
    pub fn canonical_manifest(&self) -> Value {
        canonical(
            json!({"version":1,"delivery_id":self.delivery_id,"payload":self.payload.to_json(),
            "delivered":self.delivered.iter().map(memory_ref_value).collect::<Vec<_>>(),
            "owner_cuts":self.owner_cuts}),
        )
    }

    /// Binds the exact payload, delivered refs, cuts and stable host invocation identity.
    pub fn manifest_digest(&self) -> String {
        let bytes =
            serde_json::to_vec(&self.canonical_manifest()).expect("manifest contains JSON values");
        Sha256::digest(bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }
}

#[cfg(test)]
#[path = "agentic_memory_retrieval_manifest_tests.rs"]
mod tests;
