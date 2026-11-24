//! Validate stored body/time-bound inference through pure Chronos.

use reliquary_owner_session::model::{MemoryBody, MemoryRecord};
pub(crate) fn analyze_memory(
    record: &MemoryRecord,
    body: &MemoryBody,
) -> Result<reliquary_chronos::TemporalAnalysis, ()> {
    if reliquary_owner_session::model::memory_body_id(&body.title, &body.content) != record.body_id
    {
        return Err(());
    }
    let text = format!("{}\n{}", body.title, body.content);
    analyze_validated_memory(record, &text)
}

fn analyze_validated_memory(
    record: &MemoryRecord,
    text: &str,
) -> Result<reliquary_chronos::TemporalAnalysis, ()> {
    let Some(value) = &record.temporal_inference else {
        return Ok(reliquary_chronos::analyze(text, record.source_time_ns));
    };
    if value.body_id != record.body_id || value.source_time_ns != record.source_time_ns {
        return Err(());
    }
    use reliquary_chronos::TemporalIndicationKind as Kind;
    use reliquary_owner_session::model::TemporalIndicationKind as Stored;
    let inference = reliquary_chronos::TemporalInference {
        model: value.inference.model.clone(),
        contract_version: value.inference.contract_version.clone(),
        resolutions: value
            .inference
            .resolutions
            .iter()
            .map(|r| reliquary_chronos::TemporalInferenceResolution {
                start_byte: r.start_byte,
                end_byte: r.end_byte,
                evidence: r.evidence.clone(),
                canonical_expression: r.canonical_expression.clone(),
                kind: match r.kind {
                    Stored::Explicit => Kind::Explicit,
                    Stored::Calendar => Kind::Calendar,
                    Stored::Relative => Kind::Relative,
                    Stored::Boundary => Kind::Boundary,
                    Stored::Recurrence => Kind::Recurrence,
                    Stored::Duration => Kind::Duration,
                },
            })
            .collect(),
    };
    reliquary_chronos::analyze_with_inference(text, record.source_time_ns, &inference)
        .map_err(|_| ())
}

use reliquary_retrieval::{TemporalBasis, TemporalEvidence, TemporalFilter};

/// Grounded temporal evidence from an accepted Memory body and its source timestamp.
/// A stale proof is ignored and deterministic analysis is used instead.
pub fn memory_temporal_evidence(
    record: &MemoryRecord,
    body: &MemoryBody,
    filter: &TemporalFilter,
) -> Vec<TemporalEvidence> {
    match filter.basis {
        TemporalBasis::Source => source_evidence(record.source_time_ns, filter),
        TemporalBasis::Mentioned => {
            if reliquary_owner_session::model::memory_body_id(&body.title, &body.content)
                != record.body_id
            {
                return Vec::new();
            }
            let text = format!("{}\n{}", body.title, body.content);
            let analysis = analyze_validated_memory(record, &text)
                .unwrap_or_else(|_| reliquary_chronos::analyze(&text, record.source_time_ns));
            mentioned_evidence(&text, analysis, filter)
        }
    }
}

/// Grounded temporal evidence from an exact Archive turn and its original source time.
pub fn text_temporal_evidence(
    text: &str,
    source_time_ns: Option<i64>,
    filter: &TemporalFilter,
) -> Vec<TemporalEvidence> {
    match filter.basis {
        TemporalBasis::Source => source_evidence(source_time_ns, filter),
        TemporalBasis::Mentioned => mentioned_evidence(
            text,
            reliquary_chronos::analyze(text, source_time_ns),
            filter,
        ),
    }
}

fn source_evidence(source_time_ns: Option<i64>, filter: &TemporalFilter) -> Vec<TemporalEvidence> {
    source_time_ns
        .filter(|&time| filter.contains(time))
        .map(|source_time_ns| vec![TemporalEvidence::Source { source_time_ns }])
        .unwrap_or_default()
}

fn mentioned_evidence(
    text: &str,
    analysis: reliquary_chronos::TemporalAnalysis,
    filter: &TemporalFilter,
) -> Vec<TemporalEvidence> {
    const MAX_EVIDENCE: usize = 16;
    let mut result = Vec::new();
    for anchor in analysis.anchors {
        if !anchor.evidence.is_empty()
            && text.contains(&anchor.evidence)
            && filter.overlaps(anchor.start_ns, anchor.end_ns)
        {
            let evidence = TemporalEvidence::Mentioned {
                phrase: anchor.evidence,
                start_ns: anchor.start_ns,
                end_exclusive_ns: anchor.end_ns,
                precision: format!("{:?}", anchor.granularity).to_lowercase(),
            };
            if !result.contains(&evidence) {
                result.push(evidence);
            }
        }
        if result.len() == MAX_EVIDENCE {
            return result;
        }
    }
    for interval in analysis.intervals {
        if let (Some(start_ns), Some(end_exclusive_ns)) = (interval.start_ns, interval.end_ns) {
            if !interval.evidence.is_empty()
                && text.contains(&interval.evidence)
                && filter.overlaps(start_ns, end_exclusive_ns)
            {
                let evidence = TemporalEvidence::Mentioned {
                    phrase: interval.evidence,
                    start_ns,
                    end_exclusive_ns,
                    precision: "range".into(),
                };
                if !result.contains(&evidence) {
                    result.push(evidence);
                }
            }
        }
        if result.len() == MAX_EVIDENCE {
            break;
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stale_proof_falls_back_but_mismatched_body_cannot_ground_mentions() {
        use reliquary_owner_session::model::{
            MemoryId, MemoryTemporalInference, TemporalInference, memory_body_id,
        };
        let body = MemoryBody {
            title: "title".into(),
            content: "1970-01-01".into(),
        };
        let body_id = memory_body_id(&body.title, &body.content);
        let record = MemoryRecord {
            id: MemoryId([1; 32]),
            revision: 1,
            body_id,
            category: String::new(),
            memory_type: String::new(),
            authority_kind: String::new(),
            temporal_status: String::new(),
            scope: String::new(),
            lifecycle_state: String::new(),
            archived: false,
            parent_id: None,
            superseded_by: None,
            source_node_id: None,
            content_source_conversation_id: None,
            content_source_node_id: None,
            grounding_source_conversation_id: None,
            grounding_source_node_id: None,
            source_episode_id: None,
            source_ref: None,
            source_time_ns: Some(0),
            temporal_inference: Some(MemoryTemporalInference {
                body_id,
                source_time_ns: Some(1),
                inference: TemporalInference {
                    model: "old".into(),
                    contract_version: "stale".into(),
                    resolutions: vec![],
                },
            }),
            mutation_id: String::new(),
            created_at_ns: 0,
            updated_at_ns: 0,
            memory_version: 1,
        };
        let mentioned =
            TemporalFilter::new(TemporalBasis::Mentioned, 0, 86_400_000_000_000).unwrap();
        assert!(analyze_memory(&record, &body).is_err()); // Dream remains strict.
        assert_eq!(
            memory_temporal_evidence(&record, &body, &mentioned).len(),
            1
        );
        let wrong = MemoryBody {
            title: "different".into(),
            content: body.content.clone(),
        };
        assert!(memory_temporal_evidence(&record, &wrong, &mentioned).is_empty());
        // Source evidence only depends on the accepted revision metadata.
        let source = TemporalFilter::new(TemporalBasis::Source, 0, 1).unwrap();
        assert_eq!(
            memory_temporal_evidence(&record, &wrong, &source),
            vec![TemporalEvidence::Source { source_time_ns: 0 }]
        );
    }

    #[test]
    fn source_and_mentioned_evidence_respect_boundaries_and_reference_time() {
        let day = 86_400_000_000_000i64;
        let source = TemporalFilter::new(TemporalBasis::Source, 0, day).unwrap();
        assert_eq!(text_temporal_evidence("text", Some(0), &source).len(), 1);
        assert!(text_temporal_evidence("text", Some(day), &source).is_empty());
        let mentioned = TemporalFilter::new(TemporalBasis::Mentioned, 0, day).unwrap();
        assert_eq!(
            text_temporal_evidence("1970-01-01", None, &mentioned).len(),
            1
        );
        assert!(text_temporal_evidence("yesterday", None, &mentioned).is_empty());
        assert_eq!(
            text_temporal_evidence("yesterday", Some(day), &mentioned).len(),
            1
        );
        let next = TemporalFilter::new(TemporalBasis::Mentioned, day, 2 * day).unwrap();
        assert!(text_temporal_evidence("1970-01-01", None, &next).is_empty());
        assert_eq!(
            text_temporal_evidence("January 1970", None, &mentioned).len(),
            1
        );
    }
}
