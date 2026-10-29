use super::*;

impl<E: GeneralEndpoint> InsomniaExtractor<E> {
    pub(super) fn complete_ledger(
        &self,
        payload: &Value,
        turns: &[ResolvedTurn],
        evidence_turns: &[InsomniaEvidenceTurn],
        allow_evidence_requests: bool,
        stage_reporter: &dyn Fn(InsomniaSemanticStage, usize),
    ) -> Result<Value, InsomniaExtractionError> {
        let encoded = serde_json::to_string(payload)
            .map_err(|error| InsomniaExtractionError::InvalidOutput(error.to_string()))?;
        stage_reporter(InsomniaSemanticStage::Ledger, 0);
        let mut result = self.endpoint.complete_json(
            ledger::LEDGER_SYSTEM_PROMPT,
            &encoded,
            "insomnia_authority_disposition_ledger",
            &ledger::schema(turns, evidence_turns, allow_evidence_requests),
        )?;
        let missing = ledger::sanitize_turn_keys(&mut result, turns)?;
        if missing.is_empty() {
            self.repair_missing_clause_fields(
                payload,
                turns,
                evidence_turns,
                &mut result,
                stage_reporter,
            )?;
            return Ok(result);
        }

        let repair_payload = json!({
            "authoritative_input": payload,
            "partial_ledger": result,
            "missing_user_turn_ids": missing,
            "instruction": "Repair only the missing user-turn ledger entries listed in missing_user_turn_ids. Return exactly those turn keys and no others. Preserve the original authority rules. Do not revise clauses already present in partial_ledger."
        });
        let repair_payload = serde_json::to_string(&repair_payload)
            .map_err(|error| InsomniaExtractionError::InvalidOutput(error.to_string()))?;
        let repair_prompt = format!(
            "{}\n\nREPAIR MODE: The previous structured result omitted required user-turn keys. Return clauses only for the explicitly listed missing user turns. Do not repeat or modify already-present turns.",
            ledger::LEDGER_SYSTEM_PROMPT
        );
        stage_reporter(InsomniaSemanticStage::LedgerTurnRepair, missing.len());
        let repair = self.endpoint.complete_json(
            &repair_prompt,
            &repair_payload,
            "insomnia_authority_disposition_ledger_repair",
            &ledger::repair_schema(turns, evidence_turns, &missing, allow_evidence_requests),
        )?;
        ledger::merge_repair(&mut result, repair, &missing, allow_evidence_requests)?;
        self.repair_missing_clause_fields(
            payload,
            turns,
            evidence_turns,
            &mut result,
            stage_reporter,
        )?;
        Ok(result)
    }

    pub(super) fn repair_missing_clause_fields(
        &self,
        payload: &Value,
        turns: &[ResolvedTurn],
        evidence_turns: &[InsomniaEvidenceTurn],
        result: &mut Value,
        stage_reporter: &dyn Fn(InsomniaSemanticStage, usize),
    ) -> Result<(), InsomniaExtractionError> {
        const MAX_REPAIR_CLAUSES: usize = 64;
        let missing = ledger::missing_clause_fields(result, turns)?;
        if missing.is_empty() {
            return Ok(());
        }
        if missing.len() > MAX_REPAIR_CLAUSES {
            return Err(InsomniaExtractionError::InvalidOutput(format!(
                "ledger has too many incomplete clauses to repair: {}",
                missing.len()
            )));
        }
        let targets = missing
            .iter()
            .enumerate()
            .map(|(index, target)| {
                let partial_clause = result
                    .get("turns")
                    .and_then(Value::as_object)
                    .and_then(|items| items.get(&target.source_node_id))
                    .and_then(Value::as_array)
                    .and_then(|clauses| clauses.get(target.clause_index))
                    .cloned()
                    .ok_or_else(|| {
                        InsomniaExtractionError::InvalidOutput(
                            "ledger clause repair target disappeared".into(),
                        )
                    })?;
                Ok(json!({
                    "repair_id": format!("r{index:03}"),
                    "source_node_id": target.source_node_id,
                    "clause_index": target.clause_index,
                    "partial_clause": partial_clause,
                    "missing_fields": target.fields,
                }))
            })
            .collect::<Result<Vec<_>, InsomniaExtractionError>>()?;
        let repair_payload = json!({
            "authoritative_input": payload,
            "repair_targets": targets,
            "instruction": "For every repair target, replace only its listed repair fields. Those fields are missing, empty where non-empty is required, or invalid under the ledger contract. Preserve every other field and its meaning. Return exactly one repair object per repair_id and only the requested fields within each repair."
        });
        let repair_payload = serde_json::to_string(&repair_payload)
            .map_err(|error| InsomniaExtractionError::InvalidOutput(error.to_string()))?;
        let repair_prompt = format!(
            "{}\n\nCLAUSE-FIELD REPAIR MODE: Structured ledger clauses contain missing or malformed fields. Infer only the explicitly listed repair fields for every target from the authoritative input and partial clauses. Do not revise any field that is not listed for repair.",
            ledger::LEDGER_SYSTEM_PROMPT
        );
        stage_reporter(InsomniaSemanticStage::LedgerFieldRepair, missing.len());
        let repair = self.endpoint.complete_json(
            &repair_prompt,
            &repair_payload,
            "insomnia_authority_disposition_clause_repair",
            &ledger::clause_field_repair_schema(turns, evidence_turns, &missing),
        )?;
        ledger::merge_clause_field_repairs(result, &missing, repair)?;
        if !ledger::missing_clause_fields(result, turns)?.is_empty() {
            return Err(InsomniaExtractionError::InvalidOutput(
                "ledger clause-field repair remained incomplete".into(),
            ));
        }
        Ok(())
    }
}
