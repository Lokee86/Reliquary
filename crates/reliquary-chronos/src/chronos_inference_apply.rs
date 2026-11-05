use crate::{TemporalAnalysis, TemporalInference, TemporalInferenceError};

pub(crate) fn apply_inference(
    text: &str,
    mut analysis: TemporalAnalysis,
    inference: &TemporalInference,
    reference_timestamp_ns: Option<i64>,
) -> Result<TemporalAnalysis, TemporalInferenceError> {
    if inference.model.trim().is_empty()
        || inference.resolutions.len() > 64
        || inference
            .resolutions
            .iter()
            .any(|r| r.canonical_expression.len() > 1024)
        || inference.contract_version != crate::CHRONOS_INFERENCE_CONTRACT_VERSION
    {
        return Err(TemporalInferenceError::InvalidOutput(
            "unsupported persisted inference contract version".into(),
        ));
    }
    let detection = crate::detect(text);
    let mut seen = std::collections::HashSet::new();
    for resolution in &inference.resolutions {
        if text.get(resolution.start_byte..resolution.end_byte)
            != Some(resolution.evidence.as_str())
            || !detection.indications.iter().any(|indication| {
                indication.start_byte == resolution.start_byte
                    && indication.end_byte == resolution.end_byte
                    && indication.kind == resolution.kind
            })
            || !seen.insert((resolution.start_byte, resolution.end_byte, resolution.kind))
        {
            return Err(TemporalInferenceError::InvalidOutput(
                "persisted inference does not match a unique source indication".into(),
            ));
        }
        let mut inferred = crate::chronos_inference_verify::verify_canonical_expression(
            &resolution.canonical_expression,
            resolution.kind,
            reference_timestamp_ns,
        )?;
        let source_indication = detection
            .indications
            .iter()
            .find(|indication| {
                indication.start_byte == resolution.start_byte
                    && indication.end_byte == resolution.end_byte
                    && indication.kind == resolution.kind
            })
            .expect("validated source indication");
        match crate::chronos_resolution::explicit_definition_outcome(
            text,
            source_indication,
            reference_timestamp_ns,
        ) {
            crate::chronos_resolution::ExplicitDefinitionOutcome::Proven {
                canonical_expression,
                ..
            } => {
                let expected = crate::chronos_inference_verify::verify_canonical_expression(
                    &canonical_expression,
                    resolution.kind,
                    reference_timestamp_ns,
                )?;
                let same = inferred.patterns.len() == 1 && expected.patterns.len() == 1 && {
                    let a = &inferred.patterns[0];
                    let b = &expected.patterns[0];
                    a.frequency == b.frequency
                        && a.interval == b.interval
                        && a.occurrences_per_period == b.occurrences_per_period
                        && a.weekday == b.weekday
                        && a.month_day == b.month_day
                        && a.month == b.month
                };
                if !same {
                    return Err(TemporalInferenceError::InvalidOutput(
                        "persisted inference contradicts a local explicit definition".into(),
                    ));
                }
            }
            crate::chronos_resolution::ExplicitDefinitionOutcome::Blocked => {
                return Err(TemporalInferenceError::InvalidOutput(
                    "persisted inference targets a blocked explicit definition".into(),
                ));
            }
            crate::chronos_resolution::ExplicitDefinitionOutcome::None => {}
        }
        for anchor in &mut inferred.anchors {
            anchor.evidence = resolution.evidence.clone();
        }
        for duration in &mut inferred.durations {
            duration.evidence = resolution.evidence.clone();
        }
        for duration in &mut inferred.duration_ranges {
            duration.evidence = resolution.evidence.clone();
        }
        for duration in &mut inferred.approximate_durations {
            duration.evidence = resolution.evidence.clone();
        }
        for relation in &mut inferred.event_relations {
            relation.evidence = resolution.evidence.clone();
        }
        for interval in &mut inferred.intervals {
            interval.evidence = resolution.evidence.clone();
        }
        for pattern in &mut inferred.patterns {
            pattern.evidence = resolution.evidence.clone();
        }
        crate::chronos_parser::merge_temporal_analysis(&mut analysis, inferred);
    }
    Ok(analysis)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{TemporalIndicationKind, TemporalInferenceResolution};

    fn inference(canonical_expression: &str) -> TemporalInference {
        TemporalInference {
            model: "fixture".into(),
            contract_version: crate::CHRONOS_INFERENCE_CONTRACT_VERSION.into(),
            resolutions: vec![TemporalInferenceResolution {
                start_byte: 7,
                end_byte: 15,
                kind: TemporalIndicationKind::Recurrence,
                evidence: "biweekly".into(),
                canonical_expression: canonical_expression.into(),
            }],
        }
    }

    #[test]
    fn replay_accepts_compatible_explicit_definition_and_rejects_contradiction() {
        let source = "Review biweekly, meaning every two weeks.";
        let accepted = crate::analyze_with_inference(source, None, &inference("every two weeks"));
        assert!(accepted.is_ok());
        assert!(crate::analyze_with_inference(source, None, &inference("twice weekly"),).is_err());
    }
}
