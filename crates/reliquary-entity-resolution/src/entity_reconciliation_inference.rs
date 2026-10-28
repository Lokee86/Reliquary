use crate::entity_reconciliation_schema::{
    ENTITY_RECONCILIATION_SYSTEM_PROMPT, entity_reconciliation_schema,
};
use crate::{Entity, EntityReconciliationRelation, GeneralEndpoint, Memory, ReconciliationError};
use serde_json::{Value, json};

pub fn classify_pair<E: GeneralEndpoint>(
    endpoint: &E,
    left: &Entity,
    right: &Entity,
    left_support: &[Memory],
    right_support: &[Memory],
) -> Result<EntityReconciliationRelation, ReconciliationError> {
    let payload = serde_json::to_string(&json!({
        "left": entity_json(left, left_support),
        "right": entity_json(right, right_support),
    }))
    .map_err(|error| ReconciliationError::InvalidOutput(error.to_string()))?;

    let output = endpoint.complete_json(
        ENTITY_RECONCILIATION_SYSTEM_PROMPT,
        &payload,
        "entity_reconciliation",
        &entity_reconciliation_schema(),
    )?;
    parse_relation(&output)
}

fn entity_json(entity: &Entity, support: &[Memory]) -> Value {
    json!({
        "canonical_name": entity.canonical_name,
        "aliases": entity.aliases,
        "kind": entity.kind,
        "summary": entity.summary,
        "supporting_memories": support.iter().map(memory_json).collect::<Vec<_>>(),
    })
}

fn memory_json(memory: &Memory) -> Value {
    json!({
        "title": truncate(&memory.title, 512),
        "content": truncate(&memory.content, 1600),
    })
}

fn truncate(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_owned();
    }
    value.chars().take(max_chars).collect()
}

fn parse_relation(output: &Value) -> Result<EntityReconciliationRelation, ReconciliationError> {
    match output.get("relation").and_then(Value::as_str) {
        Some("same_identity") => Ok(EntityReconciliationRelation::SameIdentity),
        Some("related_distinct") => Ok(EntityReconciliationRelation::RelatedDistinct),
        Some("uncertain") => Ok(EntityReconciliationRelation::Uncertain),
        _ => Err(ReconciliationError::InvalidOutput(
            "missing or invalid relation".into(),
        )),
    }
}
