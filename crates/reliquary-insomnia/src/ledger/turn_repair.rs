use super::*;

pub(crate) fn repair_schema(
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

pub(crate) fn sanitize_turn_keys(
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

pub(crate) fn merge_repair(
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
