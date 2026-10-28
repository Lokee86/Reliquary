use crate::{TemporalAnalysis, TemporalAssessment, TemporalIndication, TemporalIndicationKind};

/// A deterministic interpretation of one original indication.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TemporalCandidate {
    pub id: String,
    pub indication: TemporalIndication,
    pub canonical_expression: String,
    pub analysis: TemporalAnalysis,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TemporalChoiceSet {
    pub question_id: String,
    pub indication: TemporalIndication,
    pub candidates: Vec<TemporalCandidate>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TemporalChoices {
    pub assessment: TemporalAssessment,
    pub sets: Vec<TemporalChoiceSet>,
}

/// Enumerate only grammar-supported interpretations; unsupported material stays unresolved.
pub fn assess_choices(text: &str, reference_timestamp_ns: Option<i64>) -> TemporalChoices {
    let assessment = crate::chronos::assess(text, reference_timestamp_ns);
    let indications = &assessment.resolution.unresolved_indications;
    let mut sets = Vec::new();
    if indications.len() <= 32 {
        for (index, indication) in indications.iter().enumerate() {
            if indication.kind != TemporalIndicationKind::Recurrence
                || text.get(indication.start_byte..indication.end_byte)
                    != Some(indication.evidence.as_str())
            {
                continue;
            }
            let alternatives = match indication.evidence.to_ascii_lowercase().as_str() {
                "biweekly" => ["every two weeks", "twice weekly"],
                "bimonthly" => ["every two months", "twice monthly"],
                _ => continue,
            };
            let mut candidates = Vec::new();
            for (alternative, expression) in alternatives.iter().enumerate() {
                if let Ok(mut analysis) =
                    crate::chronos_inference_verify::verify_canonical_expression(
                        expression,
                        indication.kind,
                        reference_timestamp_ns,
                    )
                {
                    for pattern in &mut analysis.patterns {
                        pattern.evidence = indication.evidence.clone();
                    }
                    candidates.push(TemporalCandidate {
                        id: format!("u{index:03}c{alternative:02}"),
                        indication: indication.clone(),
                        canonical_expression: (*expression).to_owned(),
                        analysis,
                    });
                }
            }
            if candidates.len() == 2 {
                sets.push(TemporalChoiceSet {
                    question_id: format!("u{index:03}"),
                    indication: indication.clone(),
                    candidates,
                });
            }
        }
        if sets.iter().map(|s| s.candidates.len()).sum::<usize>() > 64 {
            sets.clear();
        }
    }
    TemporalChoices { assessment, sets }
}

/// Construct immutable state and choice questions for the shared decision service.
pub fn selection_request(
    text: &str,
    reference: Option<i64>,
    choices: &TemporalChoices,
) -> (serde_json::Value, serde_json::Value) {
    use serde_json::{Map, json};
    let mut questions = Map::new();
    let mut descriptions = Vec::new();
    for set in &choices.sets {
        let mut criteria = Map::new();
        criteria.insert(
            "abstain".into(),
            json!("The context does not establish either interpretation."),
        );
        for candidate in &set.candidates {
            criteria.insert(candidate.id.clone(), json!(candidate.canonical_expression));
        }
        descriptions.push(json!({"question": set.question_id, "start_byte": set.indication.start_byte, "end_byte": set.indication.end_byte, "kind": format!("{:?}", set.indication.kind), "evidence": set.indication.evidence, "candidates": criteria}));
        questions.insert(set.question_id.clone(), json!({"type":"choice", "instructions":"Select only when the context supports the interpretation; otherwise abstain.", "criteria":criteria}));
    }
    (
        json!({"protocol":"chronos-temporal-selection-v1", "semantic_text":text, "reference_timestamp_ns":reference, "resolved_evidence":format!("{:?}", choices.assessment.analysis), "indications":descriptions}),
        serde_json::Value::Object(questions),
    )
}

/// Validate every answer against freshly enumerated local candidates before persistence.
pub fn accept_selection(
    text: &str,
    reference: Option<i64>,
    model: &str,
    response: &serde_json::Value,
) -> Result<Option<crate::TemporalInference>, crate::TemporalInferenceError> {
    let invalid =
        || crate::TemporalInferenceError::InvalidOutput("invalid temporal selection".into());
    let choices = assess_choices(text, reference);
    let envelope = response.as_object().ok_or_else(invalid)?;
    if envelope.len() != 1 {
        return Err(invalid());
    }
    let answers = envelope
        .get("answers")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(invalid)?;
    if answers.len() != choices.sets.len() {
        return Err(invalid());
    }
    let mut resolutions = Vec::new();
    for set in &choices.sets {
        let answer = answers
            .get(&set.question_id)
            .and_then(serde_json::Value::as_object)
            .ok_or_else(invalid)?;
        if answer.len() != 2
            || answer.get("type").and_then(serde_json::Value::as_str) != Some("choice")
        {
            return Err(invalid());
        }
        let selected = answer
            .get("choice")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(invalid)?;
        if selected == "abstain" {
            continue;
        }
        let candidate = set
            .candidates
            .iter()
            .find(|c| c.id == selected)
            .ok_or_else(invalid)?;
        let mut verified = crate::chronos_inference_verify::verify_canonical_expression(
            &candidate.canonical_expression,
            set.indication.kind,
            reference,
        )?;
        for pattern in &mut verified.patterns {
            pattern.evidence = set.indication.evidence.clone();
        }
        if verified != candidate.analysis || candidate.indication != set.indication {
            return Err(invalid());
        }
        resolutions.push(crate::TemporalInferenceResolution {
            start_byte: set.indication.start_byte,
            end_byte: set.indication.end_byte,
            kind: set.indication.kind,
            evidence: set.indication.evidence.clone(),
            canonical_expression: candidate.canonical_expression.clone(),
        });
    }
    Ok((!resolutions.is_empty()).then(|| crate::TemporalInference {
        model: model.to_owned(),
        contract_version: crate::CHRONOS_INFERENCE_CONTRACT_VERSION.into(),
        resolutions,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn biweekly_has_two_distinct_verified_candidates_with_original_span() {
        let choices = assess_choices("é Review biweekly.", None);
        assert_eq!(choices.sets.len(), 1);
        let set = &choices.sets[0];
        assert_eq!(set.indication.evidence, "biweekly");
        assert_eq!(
            &"é Review biweekly."[set.indication.start_byte..set.indication.end_byte],
            "biweekly"
        );
        assert_eq!(set.candidates.len(), 2);
        assert_eq!(set.candidates[0].analysis.patterns[0].interval, 2);
        assert_eq!(
            set.candidates[1].analysis.patterns[0].occurrences_per_period,
            2
        );
        assert_eq!(set.candidates[1].analysis.patterns[0].weekday, None);
        assert!(
            crate::chronos::temporal_matches(
                &set.candidates[0].analysis,
                &set.candidates[1].analysis
            )
            .0
            .is_empty()
        );
    }
    #[test]
    fn unresolved_unsupported_and_over_limit_do_not_offer_choices() {
        assert!(assess_choices("northern spring", None).sets.is_empty());
        let text = vec!["biweekly"; 33].join(" ");
        assert!(assess_choices(&text, None).sets.is_empty());
    }
    #[test]
    fn semiweekly_resolves_without_selection() {
        let choices = assess_choices("Meet semiweekly.", None);
        assert!(choices.sets.is_empty());
        assert_eq!(
            choices.assessment.resolution.status,
            crate::TemporalResolutionStatus::FullyResolved
        );
        assert_eq!(
            choices.assessment.analysis.patterns[0].occurrences_per_period,
            2
        );
    }
    #[test]
    fn bimonthly_has_distinct_verified_alternatives() {
        let choices = assess_choices("Review bimonthly.", None);
        let set = &choices.sets[0];
        assert_eq!(set.candidates.len(), 2);
        assert_eq!(set.candidates[0].analysis.patterns[0].interval, 2);
        assert_eq!(
            set.candidates[1].analysis.patterns[0].occurrences_per_period,
            2
        );
        assert_eq!(set.candidates[1].analysis.patterns[0].month_day, None);
        assert!(
            crate::chronos::temporal_matches(
                &set.candidates[0].analysis,
                &set.candidates[1].analysis
            )
            .0
            .is_empty()
        );
        let exact = assess_choices("Review semimonthly.", None);
        assert!(exact.sets.is_empty());
        assert_eq!(
            exact.assessment.resolution.status,
            crate::TemporalResolutionStatus::FullyResolved
        );
        assert_eq!(
            exact.assessment.analysis.patterns[0].occurrences_per_period,
            2
        );
    }
    #[test]
    fn labeled_candidate_corpus_has_no_unsupported_alternatives() {
        let cases = [
            ("Review biweekly", 2),
            ("réunion biweekly", 2),
            ("Review semiweekly", 0),
            ("Deadline 2027-03-04", 0),
            ("next week", 0),
            ("spring 2027", 0),
        ];
        for (text, expected) in cases {
            let choices = assess_choices(text, None);
            assert_eq!(
                choices
                    .sets
                    .iter()
                    .map(|s| s.candidates.len())
                    .sum::<usize>(),
                expected,
                "{text}"
            );
        }
    }
    #[test]
    fn selection_rejects_invented_and_incomplete_answers_and_accepts_abstention() {
        use serde_json::json;
        let text = "Review biweekly.";
        assert!(
            accept_selection(
                text,
                None,
                "model",
                &json!({"answers":{"u000":{"type":"choice","choice":"invented"}}})
            )
            .is_err()
        );
        assert!(accept_selection(text, None, "model", &json!({"answers":{}})).is_err());
        assert!(
            accept_selection(
                text,
                None,
                "model",
                &json!({"answers":{"u000":{"type":"choice","choice":"abstain"}}})
            )
            .unwrap()
            .is_none()
        );
        let inference = accept_selection(
            text,
            None,
            "model",
            &json!({"answers":{"u000":{"type":"choice","choice":"u000c01"}}}),
        )
        .unwrap()
        .unwrap();
        let analysis = crate::chronos::analyze_with_inference(text, None, &inference).unwrap();
        assert_eq!(analysis.patterns[0].occurrences_per_period, 2);
        assert_eq!(analysis.patterns[0].evidence, "biweekly");
        let mut forged = inference;
        forged.resolutions[0].start_byte += 1;
        assert!(crate::chronos::analyze_with_inference(text, None, &forged).is_err());
    }
}
