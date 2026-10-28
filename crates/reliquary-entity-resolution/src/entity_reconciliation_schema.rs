use serde_json::{Value, json};

pub const ENTITY_RECONCILIATION_SYSTEM_PROMPT: &str = r#"Decide whether two existing durable Entities represent the SAME continuing identity.

Return same_identity only when the two records are interchangeable identifiers for one referent. Different wording, qualification, abbreviation, or contextual description may still be the same identity.

Return related_distinct when they are related but refer to different things. File vs type, repository vs application, owner vs owned component, person vs organization, and two different same-named referents are distinct. When evidence is insufficient, return uncertain.

A false merge is worse than leaving two identities separate. Use the supplied names, aliases, stable summaries, and bounded supporting Memories. Do not invent facts."#;

pub fn entity_reconciliation_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["relation"],
        "properties": {
            "relation": {
                "type": "string",
                "enum": ["same_identity", "related_distinct", "uncertain"]
            }
        }
    })
}
