use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MissingClauseFields {
    pub source_node_id: String,
    pub clause_index: usize,
    pub fields: Vec<&'static str>,
}

pub(crate) fn missing_clause_fields(
    value: &Value,
    turns: &[ResolvedTurn],
) -> Result<Vec<MissingClauseFields>, InsomniaExtractionError> {
    let object = value
        .get("turns")
        .and_then(Value::as_object)
        .ok_or_else(|| invalid("missing ledger turns object"))?;
    let mut missing = Vec::new();
    for turn in turns.iter().filter(|turn| turn.role == "user") {
        let clauses = object
            .get(&turn.node_id)
            .and_then(Value::as_array)
            .ok_or_else(|| invalid(format!("ledger omitted user turn {}", turn.node_id)))?;
        for (clause_index, clause) in clauses.iter().enumerate() {
            let mut fields = Vec::new();
            let disposition = trimmed_string(clause, "disposition");
            if !matches!(disposition, Some("retain" | "omit" | "superseded")) {
                fields.push("disposition");
            }
            if disposition != Some("omit") {
                let authority_kind = trimmed_string(clause, "authority_kind");
                if !matches!(
                    authority_kind,
                    Some("direct" | "correction" | "adoption" | "retention")
                ) {
                    fields.push("authority_kind");
                }
                if !trimmed_string(clause, "category")
                    .is_some_and(|value| CATEGORIES.contains(&value))
                {
                    fields.push("category");
                }
                if !trimmed_string(clause, "type").is_some_and(|value| TYPES.contains(&value)) {
                    fields.push("type");
                }
                if !matches!(
                    trimmed_string(clause, "lifecycle"),
                    Some("current" | "future" | "historical" | "superseded")
                ) {
                    fields.push("lifecycle");
                }
                if !trimmed_string(clause, "proposition").is_some_and(|value| !value.is_empty()) {
                    fields.push("proposition");
                }

                if trimmed_string(clause, "authority_source_node_id").is_none() {
                    fields.push("authority_source_node_id");
                }
                if trimmed_string(clause, "grounding_source_node_id").is_none() {
                    fields.push("grounding_source_node_id");
                }
            }
            if !fields.is_empty() {
                fields.sort_unstable();
                fields.dedup();
                missing.push(MissingClauseFields {
                    source_node_id: turn.node_id.clone(),
                    clause_index,
                    fields,
                });
            }
        }
    }
    Ok(missing)
}

pub(crate) fn clause_field_repair_schema(
    turns: &[ResolvedTurn],
    evidence_turns: &[InsomniaEvidenceTurn],
    missing: &[MissingClauseFields],
) -> Value {
    let mut repair_properties = Map::new();
    let mut required_repairs = Vec::new();
    for (index, target) in missing.iter().enumerate() {
        let key = format!("r{index:03}");
        required_repairs.push(Value::String(key.clone()));
        let mut fields = Map::new();
        for field in &target.fields {
            fields.insert(
                (*field).to_owned(),
                field_schema(turns, evidence_turns, field),
            );
        }
        repair_properties.insert(
            key,
            json!({
                "type": "object",
                "additionalProperties": false,
                "properties": fields,
                "required": target.fields
            }),
        );
    }
    json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "repairs": {
                "type": "object",
                "additionalProperties": false,
                "properties": repair_properties,
                "required": required_repairs
            }
        },
        "required": ["repairs"]
    })
}

fn field_schema(
    turns: &[ResolvedTurn],
    evidence_turns: &[InsomniaEvidenceTurn],
    field: &str,
) -> Value {
    match field {
        "disposition" => json!({"type": "string", "enum": ["retain", "omit", "superseded"]}),
        "authority_kind" => {
            json!({"type": "string", "enum": ["direct", "correction", "adoption", "retention", "none"]})
        }
        "category" => {
            json!({"type": "string", "enum": ["fact", "preference", "decision", "instruction", "relationship", "constraint", "correction", "commitment", "none"]})
        }
        "type" => {
            json!({"type": "string", "enum": ["identity", "education", "employment", "location", "possession", "health", "finance", "schedule", "communication", "project", "process", "product", "relationship", "other", "none"]})
        }
        "lifecycle" => {
            json!({"type": "string", "enum": ["current", "future", "historical", "superseded", "none"]})
        }
        "proposition" => json!({"type": "string"}),
        "authority_source_node_id" => {
            let mut ids: Vec<String> = turns
                .iter()
                .filter(|turn| turn.role == "assistant")
                .map(|turn| turn.node_id.clone())
                .chain(
                    evidence_turns
                        .iter()
                        .filter(|turn| turn.role == "assistant")
                        .map(|turn| turn.node_id.clone()),
                )
                .collect();
            ids.push(String::new());
            json!({"type": "string", "enum": ids})
        }
        "grounding_source_node_id" => {
            let mut ids: Vec<String> = turns
                .iter()
                .map(|turn| turn.node_id.clone())
                .chain(evidence_turns.iter().map(|turn| turn.node_id.clone()))
                .collect();
            ids.push(String::new());
            json!({"type": "string", "enum": ids})
        }
        _ => unreachable!("known missing ledger field"),
    }
}

fn trimmed_string<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str).map(str::trim)
}
