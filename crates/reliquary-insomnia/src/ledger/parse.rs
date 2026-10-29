use super::*;

pub(crate) fn parse(
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

fn string(value: &Value, key: &str) -> Result<String, InsomniaExtractionError> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(|value| value.trim().to_owned())
        .ok_or_else(|| invalid(format!("ledger field {key} is missing")))
}
