use crate::{EvidenceTurn, MAX_INSOMNIA_EVIDENCE_REQUESTS};
use reliquary_archive::ResolvedTurn;
use serde_json::{Map, Value, json};
use std::collections::HashSet;

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

pub(super) struct MissingClauseFields {
    pub source_node_id: String,
    pub clause_index: usize,
    source_timestamp_ns: i64,
    pub fields: Vec<&'static str>,
}

pub(super) fn sanitize_turn_keys(
    value: &mut Value,
    turns: &[ResolvedTurn],
) -> Result<Vec<String>, String> {
    let users: Vec<String> = turns
        .iter()
        .filter(|t| t.role == "user")
        .map(|t| t.node_id.clone())
        .collect();
    let allowed: HashSet<&str> = users.iter().map(String::as_str).collect();
    let object = value
        .get_mut("turns")
        .and_then(Value::as_object_mut)
        .ok_or("missing ledger turns object")?;
    object.retain(|key, _| allowed.contains(key.as_str()));
    Ok(users
        .into_iter()
        .filter(|id| !object.contains_key(id))
        .collect())
}

pub(super) fn turn_schema(
    turns: &[ResolvedTurn],
    evidence: &[EvidenceTurn],
    missing: &[String],
    allow_requests: bool,
) -> Value {
    let mut properties = Map::new();
    for turn in turns
        .iter()
        .filter(|t| t.role == "user" && missing.contains(&t.node_id))
    {
        let authority = eligible_attribution_ids(turns, evidence, turn.timestamp_ns, true);
        let grounding = eligible_attribution_ids(turns, evidence, turn.timestamp_ns, false);
        let clause = clause_schema(authority, grounding);
        properties.insert(
            turn.node_id.clone(),
            json!({"type":"array","minItems":1,"maxItems":16,"items":clause}),
        );
    }
    json!({"type":"object","additionalProperties":false,"properties":{"turns":{"type":"object","additionalProperties":false,"properties":properties,"required":missing},"evidence_requests":evidence_request_schema(allow_requests)},"required":["turns","evidence_requests"]})
}

pub(super) fn merge_turn_repair(
    base: &mut Value,
    mut repair: Value,
    missing: &[String],
    allow_requests: bool,
) -> Result<(), String> {
    let allowed: HashSet<&str> = missing.iter().map(String::as_str).collect();
    let repaired = repair
        .get_mut("turns")
        .and_then(Value::as_object_mut)
        .ok_or("turn repair is missing turns object")?;
    repaired.retain(|key, _| allowed.contains(key.as_str()));
    if missing.iter().any(|id| !repaired.contains_key(id)) {
        return Err("turn repair omitted a required user turn".into());
    }
    let base_turns = base
        .get_mut("turns")
        .and_then(Value::as_object_mut)
        .ok_or("missing base ledger turns")?;
    for id in missing {
        base_turns.insert(
            id.clone(),
            repaired
                .remove(id)
                .ok_or("required turn repair disappeared")?,
        );
    }
    let requests = repair
        .get_mut("evidence_requests")
        .and_then(Value::as_array_mut)
        .ok_or("turn repair omitted evidence_requests")?;
    if !allow_requests && !requests.is_empty() {
        return Err("final ledger repair requested archive evidence".into());
    }
    let base_requests = base
        .get_mut("evidence_requests")
        .and_then(Value::as_array_mut)
        .ok_or("ledger omitted evidence_requests")?;
    for request in requests.drain(..) {
        if !base_requests.contains(&request) {
            base_requests.push(request);
        }
    }
    if base_requests.len() > MAX_INSOMNIA_EVIDENCE_REQUESTS {
        return Err("ledger repair exceeded evidence request limit".into());
    }
    Ok(())
}

pub(super) fn missing_clause_fields(
    value: &Value,
    turns: &[ResolvedTurn],
) -> Result<Vec<MissingClauseFields>, String> {
    let object = value
        .get("turns")
        .and_then(Value::as_object)
        .ok_or("missing ledger turns object")?;
    let mut missing = Vec::new();
    for turn in turns.iter().filter(|t| t.role == "user") {
        let clauses = object
            .get(&turn.node_id)
            .and_then(Value::as_array)
            .ok_or_else(|| format!("ledger omitted user turn {}", turn.node_id))?;
        for (clause_index, clause) in clauses.iter().enumerate() {
            let disposition = trimmed(clause, "disposition");
            let mut fields = Vec::new();
            if !matches!(disposition, Some("retain" | "omit" | "superseded")) {
                fields.push("disposition");
            }
            if disposition != Some("omit") {
                if !matches!(
                    trimmed(clause, "authority_kind"),
                    Some("direct" | "correction" | "adoption" | "retention")
                ) {
                    fields.push("authority_kind");
                }
                if !trimmed(clause, "category").is_some_and(|x| CATEGORIES.contains(&x)) {
                    fields.push("category");
                }
                if !trimmed(clause, "type").is_some_and(|x| TYPES.contains(&x)) {
                    fields.push("type");
                }
                if !matches!(
                    trimmed(clause, "lifecycle"),
                    Some("current" | "future" | "historical" | "superseded")
                ) {
                    fields.push("lifecycle");
                }
                if !trimmed(clause, "proposition").is_some_and(|x| !x.is_empty()) {
                    fields.push("proposition");
                }
                if trimmed(clause, "authority_source_node_id").is_none() {
                    fields.push("authority_source_node_id");
                }
                if trimmed(clause, "grounding_source_node_id").is_none() {
                    fields.push("grounding_source_node_id");
                }
            }
            if !fields.is_empty() {
                fields.sort_unstable();
                fields.dedup();
                missing.push(MissingClauseFields {
                    source_node_id: turn.node_id.clone(),
                    clause_index,
                    source_timestamp_ns: turn.timestamp_ns,
                    fields,
                });
            }
        }
    }
    Ok(missing)
}

pub(super) fn clause_field_schema(
    turns: &[ResolvedTurn],
    evidence: &[EvidenceTurn],
    missing: &[MissingClauseFields],
) -> Value {
    let mut repair_properties = Map::new();
    let mut required = Vec::new();
    for (i, target) in missing.iter().enumerate() {
        let key = format!("r{i:03}");
        required.push(Value::String(key.clone()));
        let mut properties = Map::new();
        for field in &target.fields {
            properties.insert(
                (*field).into(),
                field_schema(turns, evidence, target.source_timestamp_ns, field),
            );
        }
        repair_properties.insert(key, json!({"type":"object","additionalProperties":false,"properties":properties,"required":target.fields}));
    }
    json!({"type":"object","additionalProperties":false,"properties":{"repairs":{"type":"object","additionalProperties":false,"properties":repair_properties,"required":required}},"required":["repairs"]})
}

pub(super) fn merge_clause_field_repairs(
    value: &mut Value,
    missing: &[MissingClauseFields],
    repair: Value,
) -> Result<(), String> {
    let repairs = repair
        .get("repairs")
        .and_then(Value::as_object)
        .ok_or("clause-field repair is missing repairs object")?;
    if repairs.len() != missing.len() {
        return Err("clause-field repair count mismatch".into());
    }
    for (index, target) in missing.iter().enumerate() {
        let key = format!("r{index:03}");
        let fields = repairs
            .get(&key)
            .and_then(Value::as_object)
            .ok_or_else(|| format!("clause-field repair omitted {key}"))?;
        if fields.len() != target.fields.len()
            || target.fields.iter().any(|f| !fields.contains_key(*f))
        {
            return Err(format!(
                "clause-field repair {key} omitted a requested field"
            ));
        }
        let clause = value
            .get_mut("turns")
            .and_then(Value::as_object_mut)
            .and_then(|m| m.get_mut(&target.source_node_id))
            .and_then(Value::as_array_mut)
            .and_then(|a| a.get_mut(target.clause_index))
            .and_then(Value::as_object_mut)
            .ok_or("clause-field repair target unavailable")?;
        for field in &target.fields {
            clause.insert(
                (*field).into(),
                fields
                    .get(*field)
                    .ok_or("requested repair field missing")?
                    .clone(),
            );
        }
    }
    Ok(())
}

fn field_schema(
    turns: &[ResolvedTurn],
    evidence: &[EvidenceTurn],
    source_timestamp_ns: i64,
    field: &str,
) -> Value {
    match field {
        "disposition" => json!({"type":"string","enum":["retain","omit","superseded"]}),
        "authority_kind" => {
            json!({"type":"string","enum":["direct","correction","adoption","retention","none"]})
        }
        "category" => {
            json!({"type":"string","enum":["fact","preference","decision","instruction","relationship","constraint","correction","commitment","none"]})
        }
        "type" => {
            json!({"type":"string","enum":["identity","education","employment","location","possession","health","finance","schedule","communication","project","process","product","relationship","other","none"]})
        }
        "lifecycle" => {
            json!({"type":"string","enum":["current","future","historical","superseded","none"]})
        }
        "proposition" => json!({"type":"string"}),
        "authority_source_node_id" => {
            json!({"type":"string","enum":eligible_attribution_ids(turns, evidence, source_timestamp_ns, true)})
        }
        "grounding_source_node_id" => {
            json!({"type":"string","enum":eligible_attribution_ids(turns, evidence, source_timestamp_ns, false)})
        }
        _ => unreachable!("known repair field"),
    }
}

pub(super) fn eligible_attribution_ids<'a>(
    turns: &'a [ResolvedTurn],
    evidence: &'a [EvidenceTurn],
    source_timestamp_ns: i64,
    assistant_only: bool,
) -> Vec<&'a str> {
    turns
        .iter()
        .map(|t| (t.node_id.as_str(), t.role.as_str(), t.timestamp_ns))
        .chain(
            evidence
                .iter()
                .map(|t| (t.node_id.as_str(), t.role.as_str(), t.timestamp_ns)),
        )
        .filter(|(_, role, timestamp)| {
            *timestamp <= source_timestamp_ns && (!assistant_only || *role == "assistant")
        })
        .map(|(id, _, _)| id)
        .chain(std::iter::once(""))
        .collect()
}

fn clause_schema(assistants: Vec<&str>, support: Vec<&str>) -> Value {
    json!({"type":"object","additionalProperties":false,"properties":{"disposition":{"type":"string","enum":["retain","omit","superseded"]},"authority_kind":{"type":"string","enum":["direct","correction","adoption","retention","none"]},"category":{"type":"string","enum":["fact","preference","decision","instruction","relationship","constraint","correction","commitment","none"]},"type":{"type":"string","enum":["identity","education","employment","location","possession","health","finance","schedule","communication","project","process","product","relationship","other","none"]},"lifecycle":{"type":"string","enum":["current","future","historical","superseded","none"]},"proposition":{"type":"string"},"authority_source_node_id":{"type":"string","enum":assistants},"grounding_source_node_id":{"type":"string","enum":support},"reason":{"type":"string"}},"required":["disposition","authority_kind","category","type","lifecycle","proposition","authority_source_node_id","grounding_source_node_id","reason"]})
}

fn evidence_request_schema(allow: bool) -> Value {
    json!({"type":"array","maxItems":if allow {4}else{0},"items":{"type":"object","additionalProperties":false,"properties":{"kind":{"type":"string","enum":["turn","conversation_range","archive_search"]},"conversation_id":{"type":"string"},"node_id":{"type":"string"},"start_node_id":{"type":"string"},"end_node_id":{"type":"string"},"query":{"type":"string"},"limit":{"type":"integer","minimum":0,"maximum":5}},"required":["kind","conversation_id","node_id","start_node_id","end_node_id","query","limit"]}})
}

fn trimmed<'a>(value: &'a Value, field: &str) -> Option<&'a str> {
    value.get(field).and_then(Value::as_str).map(str::trim)
}
