//! Normalized transient extraction input; only native callers carry Episode IDs.
use crate::{Episode, ResolvedTurn};
use serde_json::{Value, json};
pub(super) struct InsomniaSourceInput {
    pub key: [u8; 32],
    pub conversation_id: String,
}
impl InsomniaSourceInput {
    pub fn native(episode: &Episode) -> Self {
        Self {
            key: episode.id.0,
            conversation_id: episode.conversation_id.clone(),
        }
    }
    pub fn external_payload(&self, namespace: &str, turns: &[ResolvedTurn]) -> Value {
        json!({
            "source_mode":"external-derived",
            "source_namespace":namespace,
            "processing_window_id":super::candidate::hex(&self.key),
            "conversation_id":self.conversation_id,
            "start_message_id":turns.first().map(|t| &t.node_id),
            "end_message_id":turns.last().map(|t| &t.node_id),
            "turns":turns.iter().map(|t| json!({
                "id":t.node_id, "conversation_id":self.conversation_id, "role":t.role,
                "timestamp_ns":t.timestamp_ns, "content":t.content
            })).collect::<Vec<_>>()
        })
    }
}
