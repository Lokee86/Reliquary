use super::*;

pub(crate) fn clause_schema(assistant_ids: Vec<String>, support_ids: Vec<String>) -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "disposition": {"type": "string", "enum": ["retain", "omit", "superseded"]},
            "authority_kind": {"type": "string", "enum": ["direct", "correction", "adoption", "retention", "none"]},
            "category": {"type": "string", "enum": ["fact", "preference", "decision", "instruction", "relationship", "constraint", "correction", "commitment", "none"]},
            "type": {"type": "string", "enum": ["identity", "education", "employment", "location", "possession", "health", "finance", "schedule", "communication", "project", "process", "product", "relationship", "other", "none"]},
            "lifecycle": {"type": "string", "enum": ["current", "future", "historical", "superseded", "none"]},
            "proposition": {"type": "string"},
            "authority_source_node_id": {"type": "string", "enum": assistant_ids},
            "grounding_source_node_id": {"type": "string", "enum": support_ids},
            "reason": {"type": "string"}
        },
        "required": ["disposition", "authority_kind", "category", "type", "lifecycle", "proposition", "authority_source_node_id", "grounding_source_node_id", "reason"]
    })
}

pub(crate) fn evidence_request_schema(allow: bool) -> Value {
    json!({
        "type": "array",
        "maxItems": if allow { 4 } else { 0 },
        "items": {
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "kind": {"type": "string", "enum": ["turn", "conversation_range", "archive_search"]},
                "conversation_id": {"type": "string"},
                "node_id": {"type": "string"},
                "start_node_id": {"type": "string"},
                "end_node_id": {"type": "string"},
                "query": {"type": "string"},
                "limit": {"type": "integer", "minimum": 0, "maximum": 5}
            },
            "required": ["kind", "conversation_id", "node_id", "start_node_id", "end_node_id", "query", "limit"]
        }
    })
}
