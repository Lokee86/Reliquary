use serde_json::{Value, json};

pub const DREAM_VERIFIER_SYSTEM_PROMPT: &str = r#"You are Dream's independent relationship verifier.
You receive two Memories in canonical MemoryId order as A and B plus a proposed relationship from a separate classification pass.
Evaluate the proposal independently. Do not defer to the classifier, its model identity, or any confidence value.
Check three things separately: whether the proposed relationship is semantically supported, whether its proposed direction/undirected form is correct, and whether the classifier's quoted evidence actually supports that proposal.
Existing Graph relationships are context, not proof. Source timestamps are chronology context, not proof of causality, duplication, or supersession. Memory creation timestamps are not semantic chronology.
Return only the required structured fields. Use uncertain when the provided Memories do not support a deterministic yes/no judgment."#;

pub fn dream_verifier_schema() -> Value {
    let signal = json!({
        "type": "string",
        "enum": ["yes", "no", "uncertain"]
    });
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": [
            "relation_supported",
            "direction_supported",
            "evidence_supported"
        ],
        "properties": {
            "relation_supported": signal,
            "direction_supported": {
                "type": "string",
                "enum": ["yes", "no", "uncertain"]
            },
            "evidence_supported": {
                "type": "string",
                "enum": ["yes", "no", "uncertain"]
            }
        }
    })
}
