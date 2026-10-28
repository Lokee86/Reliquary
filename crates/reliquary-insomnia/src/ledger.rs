use super::extraction::{InsomniaEvidenceTurn, InsomniaExtractionError};
use crate::ResolvedTurn;
use serde_json::{Map, Value, json};
use std::collections::HashSet;

pub(super) const LEDGER_SYSTEM_PROMPT: &str = r#"You are the authority-selection pass for Reliquary Insomnia.
Given one complete authoritative conversation episode, produce a clause-level ledger. This pass decides WHAT durable user-authoritative state exists. It does not write Memories.
Every user turn MUST be accounted for by at least one clause under its required turn ID. Assistant turns never receive ledger clauses. A mixed user turn may have multiple clauses when one clause is durable and another is only a question, receipt, or transient metadata.

Apply a strict durable-state test before retaining anything: after the immediate exchange/task is over, would this proposition still be useful as user/project state? Do not retain conversational scaffolding merely because it is factually true.

Authority rules:
1. direct: the user turn itself asserts durable state. direct MUST have empty authority_source_node_id.
2. correction: the user explicitly corrects/replaces earlier state. correction MUST have empty authority_source_node_id; earlier context may be grounding only when needed to identify what is being corrected.
3. adoption: the user explicitly accepts/selects/confirms a specific assistant-authored proposition. Terse assent such as "that works", "sounds good", "do it", "go with that", or equivalent CAN be durable adoption when it directly answers a single clear preceding assistant proposal. authority_source_node_id is REQUIRED and identifies that assistant turn.
4. retention: the user explicitly asks to remember/retain a proposition. If the proposition is assistant-authored, authority_source_node_id is required; otherwise it is empty.
5. grounding_source_node_id identifies an otherwise unclear referent only. It never authorizes details absent from the user turn. Do not ground merely because context exists. Do not self-ground. If the proposition is understandable independently, grounding_source_node_id MUST be empty.
6. Omitted clauses MUST use authority_kind/category/type/lifecycle = none, an empty proposition, and empty authority/grounding source IDs.

Disposition rules:
7. Questions and requests are non-authoritative by default. Do not turn a proposed choice, recalled prior plan, challenge, uncertainty, or open/alternative question into a decision/correction merely because a proposition can be inferred from it. "I thought we were going to X; now you're saying Y?" does NOT establish X. A confirmation tag on an independently asserted proposition can retain the assertion: "X has to/must/needs to be Y, right?" may retain the requirement while omitting the request for confirmation.
8. Pure execution/checkpoint receipts are omitted: prompt/phase/step numbers, "completed through N", test/commit/push completion, transient renumbering, and similar workflow position are not durable state by themselves. If the same turn separately states resulting project state, SPLIT it and retain that state without checkpoint framing. A current functional assessment such as "everything seems to be working OK so far" counts as resulting state.
9. Transient troubleshooting artifacts, pasted logs, temporary command output, immediate execution requests, and ephemeral debugging observations are omitted unless the user independently states a durable conclusion/state derived from them.
10. Preserve modality exactly. Future plans/possibilities remain future; uncertainty remains uncertainty; historical events do not become current state.
11. Prefer the FINAL user-authoritative source for overlapping semantic state. When a later authoritative turn restates, refines, or replaces an earlier proposition, mark the earlier overlapping clause superseded. Keep older state only when independently durable and not represented later.
12. Advice, explanations, examples, and assistant proposals are not user state unless explicitly adopted/retained. Do not treat assistant confirmation after a user assertion as authority for that assertion.
13. Keep propositions narrow. Do not infer importance, rationale, implementation detail, architecture, paths, causal explanation, or broader state from nearby context unless the user actually authorizes it.

Evidence rules:
14. Request read-only archive evidence only when necessary to resolve an explicit callback, adopted/retained assistant content, or a genuinely unresolved referent that is not available in the authoritative episode. Exhaust the episode first. Use at most four narrow requests.
15. Archive evidence is supporting context, never independent user authority. source authority MUST remain a user turn in the authoritative episode. Earlier assistant evidence may supply authority_source_node_id only when a current user turn explicitly adopts/retains that assistant proposition. Earlier user or assistant evidence may supply grounding_source_node_id only to resolve a referent.
16. When read_only_archive_evidence is supplied, return the FINAL ledger and an empty evidence_requests array. No second evidence round is allowed.

For retained/superseded clauses, proposition states only the authorized durable state. Reliquary owns byte-exact provenance; do not reproduce source quotes. When uncertain whether a clause is durable authority or conversational scaffolding, omit it."#;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct LedgerEntry {
    pub source_node_id: String,
    pub disposition: String,
    pub authority_kind: String,
    pub category: String,
    pub memory_type: String,
    pub lifecycle: String,
    pub proposition: String,
    pub authority_source_node_id: String,
    pub grounding_source_node_id: String,
}

pub(super) fn schema(
    turns: &[ResolvedTurn],
    evidence_turns: &[InsomniaEvidenceTurn],
    allow_evidence_requests: bool,
) -> Value {
    let user_ids: Vec<String> = turns
        .iter()
        .filter(|turn| turn.role == "user")
        .map(|turn| turn.node_id.clone())
        .collect();
    let mut assistant_ids: Vec<String> = turns
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
    assistant_ids.push(String::new());
    let mut support_ids: Vec<String> = turns
        .iter()
        .map(|turn| turn.node_id.clone())
        .chain(evidence_turns.iter().map(|turn| turn.node_id.clone()))
        .collect();
    support_ids.push(String::new());

    let clause = clause_schema(assistant_ids, support_ids);
    let mut turn_properties = Map::new();
    let mut required_turns = Vec::new();
    for id in user_ids {
        required_turns.push(Value::String(id.clone()));
        turn_properties.insert(
            id,
            json!({"type": "array", "minItems": 1, "maxItems": 16, "items": clause}),
        );
    }
    json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "turns": {
                "type": "object",
                "additionalProperties": false,
                "properties": turn_properties,
                "required": required_turns
            },
            "evidence_requests": evidence_request_schema(allow_evidence_requests)
        },
        "required": ["turns", "evidence_requests"]
    })
}

pub(super) fn repair_schema(
    turns: &[ResolvedTurn],
    evidence_turns: &[InsomniaEvidenceTurn],
    missing_user_ids: &[String],
    allow_evidence_requests: bool,
) -> Value {
    let mut assistant_ids: Vec<String> = turns
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
    assistant_ids.push(String::new());
    let mut support_ids: Vec<String> = turns
        .iter()
        .map(|turn| turn.node_id.clone())
        .chain(evidence_turns.iter().map(|turn| turn.node_id.clone()))
        .collect();
    support_ids.push(String::new());
    let clause = clause_schema(assistant_ids, support_ids);
    let mut properties = Map::new();
    for id in missing_user_ids {
        properties.insert(
            id.clone(),
            json!({"type": "array", "minItems": 1, "maxItems": 16, "items": clause}),
        );
    }
    json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "turns": {
                "type": "object",
                "additionalProperties": false,
                "properties": properties,
                "required": missing_user_ids
            },
            "evidence_requests": evidence_request_schema(allow_evidence_requests)
        },
        "required": ["turns", "evidence_requests"]
    })
}

pub(super) fn sanitize_turn_keys(
    value: &mut Value,
    turns: &[ResolvedTurn],
) -> Result<Vec<String>, InsomniaExtractionError> {
    let user_ids: Vec<String> = turns
        .iter()
        .filter(|turn| turn.role == "user")
        .map(|turn| turn.node_id.clone())
        .collect();
    let allowed: HashSet<&str> = user_ids.iter().map(String::as_str).collect();
    let object = value
        .get_mut("turns")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| invalid("missing ledger turns object"))?;
    object.retain(|key, _| allowed.contains(key.as_str()));
    Ok(user_ids
        .into_iter()
        .filter(|id| !object.contains_key(id))
        .collect())
}

pub(super) fn merge_repair(
    base: &mut Value,
    mut repair: Value,
    missing_user_ids: &[String],
    allow_evidence_requests: bool,
) -> Result<(), InsomniaExtractionError> {
    let expected: HashSet<&str> = missing_user_ids.iter().map(String::as_str).collect();
    let repair_turns = repair
        .get_mut("turns")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| invalid("repair is missing ledger turns object"))?;
    repair_turns.retain(|key, _| expected.contains(key.as_str()));
    if missing_user_ids
        .iter()
        .any(|id| !repair_turns.contains_key(id))
    {
        return Err(invalid("ledger repair omitted a required user turn"));
    }
    let base_turns = base
        .get_mut("turns")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| invalid("missing ledger turns object"))?;
    for id in missing_user_ids {
        base_turns.insert(
            id.clone(),
            repair_turns.remove(id).expect("required repair turn"),
        );
    }

    let repair_requests = repair
        .get_mut("evidence_requests")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| invalid("repair is missing evidence_requests"))?;
    if !allow_evidence_requests && !repair_requests.is_empty() {
        return Err(invalid("final ledger repair requested archive evidence"));
    }
    let base_requests = base
        .get_mut("evidence_requests")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| invalid("ledger is missing evidence_requests"))?;
    for request in repair_requests.drain(..) {
        if !base_requests.contains(&request) {
            base_requests.push(request);
        }
    }
    if base_requests.len() > 4 {
        return Err(invalid(
            "ledger repair exceeded archive evidence request limit",
        ));
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct MissingClauseFields {
    pub source_node_id: String,
    pub clause_index: usize,
    pub fields: Vec<&'static str>,
}

pub(super) fn missing_clause_fields(
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

pub(super) fn clause_field_repair_schema(
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

pub(super) fn merge_clause_field_repairs(
    value: &mut Value,
    missing: &[MissingClauseFields],
    repair: Value,
) -> Result<(), InsomniaExtractionError> {
    let repairs = repair
        .get("repairs")
        .and_then(Value::as_object)
        .ok_or_else(|| invalid("clause-field repair is missing repairs object"))?;
    if repairs.len() != missing.len() {
        return Err(invalid("clause-field repair count mismatch"));
    }
    for (index, target) in missing.iter().enumerate() {
        let key = format!("r{index:03}");
        let fields = repairs
            .get(&key)
            .and_then(Value::as_object)
            .ok_or_else(|| invalid(format!("clause-field repair omitted {key}")))?;
        if fields.len() != target.fields.len()
            || target
                .fields
                .iter()
                .any(|field| !fields.contains_key(*field))
        {
            return Err(invalid(format!(
                "clause-field repair {key} omitted a required field"
            )));
        }
        let clause = value
            .get_mut("turns")
            .and_then(Value::as_object_mut)
            .and_then(|turns| turns.get_mut(&target.source_node_id))
            .and_then(Value::as_array_mut)
            .and_then(|clauses| clauses.get_mut(target.clause_index))
            .and_then(Value::as_object_mut)
            .ok_or_else(|| invalid("clause-field repair target is unavailable"))?;
        for field in &target.fields {
            clause.insert(
                (*field).to_owned(),
                fields.get(*field).expect("validated repair field").clone(),
            );
        }
    }
    Ok(())
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

pub(super) fn generic_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "turns": {"type": "object"},
            "evidence_requests": evidence_request_schema(true)
        },
        "required": ["turns", "evidence_requests"]
    })
}

pub(super) fn parse(
    value: &Value,
    turns: &[ResolvedTurn],
    evidence_turns: &[InsomniaEvidenceTurn],
) -> Result<Vec<LedgerEntry>, InsomniaExtractionError> {
    let object = value
        .get("turns")
        .and_then(Value::as_object)
        .ok_or_else(|| invalid("missing ledger turns object"))?;
    let user_ids: Vec<&str> = turns
        .iter()
        .filter(|turn| turn.role == "user")
        .map(|turn| turn.node_id.as_str())
        .collect();
    let allowed_users: HashSet<&str> = user_ids.iter().copied().collect();
    if object.len() != allowed_users.len()
        || object
            .keys()
            .any(|key| !allowed_users.contains(key.as_str()))
    {
        return Err(invalid(
            "ledger turns object contains a missing or non-user source key",
        ));
    }
    let allowed_assistants: HashSet<&str> = turns
        .iter()
        .filter(|turn| turn.role == "assistant")
        .map(|turn| turn.node_id.as_str())
        .chain(
            evidence_turns
                .iter()
                .filter(|turn| turn.role == "assistant")
                .map(|turn| turn.node_id.as_str()),
        )
        .collect();
    let allowed_support: HashSet<&str> = turns
        .iter()
        .map(|turn| turn.node_id.as_str())
        .chain(evidence_turns.iter().map(|turn| turn.node_id.as_str()))
        .collect();
    let mut entries = Vec::new();
    for source_node_id in user_ids {
        let clauses = object
            .get(source_node_id)
            .and_then(Value::as_array)
            .ok_or_else(|| invalid(format!("ledger omitted user turn {source_node_id}")))?;
        if clauses.is_empty() {
            return Err(invalid(format!(
                "ledger emitted no clauses for {source_node_id}"
            )));
        }
        for clause in clauses {
            entries.push(parse_clause(
                source_node_id,
                clause,
                &allowed_assistants,
                &allowed_support,
            )?);
        }
    }
    Ok(entries)
}

fn parse_clause(
    source_node_id: &str,
    value: &Value,
    allowed_assistants: &HashSet<&str>,
    allowed_support: &HashSet<&str>,
) -> Result<LedgerEntry, InsomniaExtractionError> {
    let disposition = string(value, "disposition")?;
    if !["retain", "omit", "superseded"].contains(&disposition.as_str()) {
        return Err(invalid("ledger disposition is not recognized"));
    }
    if disposition == "omit" {
        return Ok(LedgerEntry {
            source_node_id: source_node_id.to_owned(),
            disposition,
            authority_kind: "none".into(),
            category: "none".into(),
            memory_type: "none".into(),
            lifecycle: "none".into(),
            proposition: String::new(),
            authority_source_node_id: String::new(),
            grounding_source_node_id: String::new(),
        });
    }
    let authority_kind = string(value, "authority_kind")?;
    let category = string(value, "category")?;
    let memory_type = string(value, "type")?;
    let lifecycle = string(value, "lifecycle")?;
    let proposition = string(value, "proposition")?;
    let mut authority_source_node_id = string(value, "authority_source_node_id")?;
    let mut grounding_source_node_id = string(value, "grounding_source_node_id")?;
    if !["direct", "correction", "adoption", "retention"].contains(&authority_kind.as_str()) {
        return Err(invalid("ledger authority kind is not recognized"));
    }
    if !CATEGORIES.contains(&category.as_str()) || !TYPES.contains(&memory_type.as_str()) {
        return Err(invalid("ledger classification is not recognized"));
    }
    if !["current", "future", "historical", "superseded"].contains(&lifecycle.as_str()) {
        return Err(invalid("ledger lifecycle is not recognized"));
    }
    if proposition.trim().is_empty() {
        return Err(invalid("retained ledger proposition is empty"));
    }
    if matches!(authority_kind.as_str(), "direct" | "correction") {
        authority_source_node_id.clear();
    }
    if authority_kind == "adoption" && authority_source_node_id.is_empty() {
        return Err(invalid("ledger adoption lacks assistant authority source"));
    }
    if !authority_source_node_id.is_empty()
        && !allowed_assistants.contains(authority_source_node_id.as_str())
    {
        return Err(invalid(
            "ledger authority source is not an available assistant turn",
        ));
    }
    if grounding_source_node_id == source_node_id {
        grounding_source_node_id.clear();
    }
    if !grounding_source_node_id.is_empty()
        && !allowed_support.contains(grounding_source_node_id.as_str())
    {
        return Err(invalid("ledger grounding source is not available evidence"));
    }
    Ok(LedgerEntry {
        source_node_id: source_node_id.to_owned(),
        disposition,
        authority_kind,
        category,
        memory_type,
        lifecycle,
        proposition: proposition.trim().to_owned(),
        authority_source_node_id,
        grounding_source_node_id,
    })
}

fn clause_schema(assistant_ids: Vec<String>, support_ids: Vec<String>) -> Value {
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

fn evidence_request_schema(allow: bool) -> Value {
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

fn string(value: &Value, key: &str) -> Result<String, InsomniaExtractionError> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(|value| value.trim().to_owned())
        .ok_or_else(|| invalid(format!("ledger field {key} is missing")))
}

fn invalid(message: impl Into<String>) -> InsomniaExtractionError {
    InsomniaExtractionError::InvalidOutput(message.into())
}

const CATEGORIES: &[&str] = &[
    "fact",
    "preference",
    "decision",
    "instruction",
    "relationship",
    "constraint",
    "correction",
    "commitment",
];
const TYPES: &[&str] = &[
    "identity",
    "education",
    "employment",
    "location",
    "possession",
    "health",
    "finance",
    "schedule",
    "communication",
    "project",
    "process",
    "product",
    "relationship",
    "other",
];
