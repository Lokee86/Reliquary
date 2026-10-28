use crate::{TemporalAnalysis, TemporalInference, TemporalInferenceError};

pub(crate) fn apply_inference(
    text: &str,
    mut analysis: TemporalAnalysis,
    inference: &TemporalInference,
    reference_timestamp_ns: Option<i64>,
) -> Result<TemporalAnalysis, TemporalInferenceError> {
    if inference.contract_version != crate::CHRONOS_INFERENCE_CONTRACT_VERSION {
        return Err(TemporalInferenceError::InvalidOutput(
            "unsupported persisted inference contract version".into(),
        ));
    }
    let detection = crate::chronos::detect(text);
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
