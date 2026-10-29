use super::*;

pub(crate) fn merge_clause_field_repairs(
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
