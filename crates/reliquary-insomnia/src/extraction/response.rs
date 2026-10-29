use super::*;

pub(super) fn validate_evidence_response(
    round: &InsomniaEvidenceRound,
    results: &[InsomniaEvidenceResult],
    evidence_turns: &[InsomniaEvidenceTurn],
) -> Result<(), InsomniaExtractionError> {
    let invalid = |message: &str| {
        InsomniaExtractionError::InvalidOutput(format!(
            "invalid archive evidence response: {message}"
        ))
    };
    if results.len() != round.requests.len() {
        return Err(invalid("result count does not match request count"));
    }
    if evidence_turns.len() > crate::evidence_request::MAX_INSOMNIA_EVIDENCE_TURNS {
        return Err(invalid("evidence turn limit exceeded"));
    }

    let mut authorized = HashMap::with_capacity(evidence_turns.len());
    let mut authorized_bytes = 0usize;
    for turn in evidence_turns {
        if turn.conversation_id.trim().is_empty()
            || turn.node_id.trim().is_empty()
            || turn.role.trim().is_empty()
        {
            return Err(invalid(
                "evidence turn is missing a conversation, node, or role",
            ));
        }
        let key = (turn.conversation_id.as_str(), turn.node_id.as_str());
        if authorized.insert(key, turn).is_some() {
            return Err(invalid("duplicate evidence turn identity"));
        }
        authorized_bytes = authorized_bytes
            .checked_add(turn.content.len())
            .ok_or_else(|| invalid("evidence byte count overflow"))?;
    }
    if authorized_bytes > crate::evidence_request::MAX_INSOMNIA_EVIDENCE_BYTES {
        return Err(invalid("evidence byte limit exceeded"));
    }

    let mut referenced = HashSet::new();
    let mut result_turn_count = 0usize;
    for (request, result) in round.requests.iter().zip(results) {
        if result.kind != request.kind {
            return Err(invalid("result kind does not match its request"));
        }
        if result.error.is_some() && !result.turns.is_empty() {
            return Err(invalid("failed result contains evidence turns"));
        }
        for turn in &result.turns {
            result_turn_count = result_turn_count
                .checked_add(1)
                .ok_or_else(|| invalid("result turn count overflow"))?;
            if result_turn_count
                > crate::evidence_request::MAX_INSOMNIA_EVIDENCE_TURNS
                    * crate::evidence_request::MAX_INSOMNIA_EVIDENCE_REQUESTS
            {
                return Err(invalid("result turn reference limit exceeded"));
            }
            let key = (turn.conversation_id.as_str(), turn.node_id.as_str());
            let Some(authorized_turn) = authorized.get(&key) else {
                return Err(invalid(
                    "result contains a turn absent from the authorized evidence set",
                ));
            };
            if *authorized_turn != turn {
                return Err(invalid(
                    "result turn differs from its authorized evidence value",
                ));
            }
            referenced.insert(key);
        }
    }
    if referenced.len() != authorized.len() {
        return Err(invalid(
            "authorized turns and per-request results are inconsistent",
        ));
    }
    Ok(())
}
