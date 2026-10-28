use super::processor::{PreparedApplication, PreparedMemory};
use super::temporal::{assess_draft, infer_prepared};
use crate::{
    DecisionEndpoint, DecisionEndpointError, MemoryDraft, TemporalIndicationKind,
    TemporalResolutionStatus,
};
use serde_json::{Value, json};
use std::sync::atomic::{AtomicUsize, Ordering};
use time::{Date, Month};

#[test]
fn insomnia_draft_assessment_uses_authoritative_source_time() {
    let source_time_ns = timestamp_ns(2026, Month::September, 16);
    let draft = draft("Review tomorrow.", Some(source_time_ns));
    let assessment = assess_draft(&draft);

    assert_eq!(
        assessment.analysis.source_timestamp_ns,
        Some(source_time_ns)
    );
    assert_eq!(
        assessment.resolution.status,
        TemporalResolutionStatus::FullyResolved
    );
    assert!(assessment.resolution.unresolved_indications.is_empty());
}

#[test]
fn insomnia_draft_assessment_carries_only_unresolved_residue() {
    let source_time_ns = timestamp_ns(2026, Month::September, 16);
    let draft = draft(
        "Deadline 2026-10-01; schedule a biweekly review.",
        Some(source_time_ns),
    );
    let assessment = assess_draft(&draft);

    assert_eq!(
        assessment.resolution.status,
        TemporalResolutionStatus::Unresolved
    );
    assert!(
        assessment
            .resolution
            .unresolved_indications
            .iter()
            .any(|value| {
                value.kind == TemporalIndicationKind::Recurrence
                    && value.evidence.eq_ignore_ascii_case("biweekly")
            })
    );
    assert!(
        !assessment
            .resolution
            .unresolved_indications
            .iter()
            .any(|value| {
                value.kind == TemporalIndicationKind::Explicit && value.evidence == "2026-10-01"
            })
    );
}

#[test]
fn insomnia_temporal_inference_is_called_only_for_unresolved_prepared_memories() {
    let resolved_draft = draft(
        "Deadline 2026-10-01.",
        Some(timestamp_ns(2026, Month::September, 16)),
    );
    let unresolved_draft = draft(
        "Review biweekly.",
        Some(timestamp_ns(2026, Month::September, 16)),
    );
    let endpoint = CountingEndpoint::new(json!({
        "answers": {"u000": {"type":"choice", "choice":"u000c00"}}
    }));
    let mut prepared = PreparedApplication {
        project_drafts: vec![
            PreparedMemory {
                temporal: assess_draft(&resolved_draft),
                draft: resolved_draft,
                temporal_inference: None,
                routing_metadata: None,
            },
            PreparedMemory {
                temporal: assess_draft(&unresolved_draft),
                draft: unresolved_draft,
                temporal_inference: None,
                routing_metadata: None,
            },
        ],
        user_drafts: Vec::new(),
        rejected: Vec::new(),
        model: "extractor".into(),
    };

    assert_eq!(infer_prepared(Some(&endpoint), &mut prepared).unwrap(), 1);
    assert_eq!(endpoint.calls.load(Ordering::SeqCst), 1);
    assert!(prepared.project_drafts[0].temporal_inference.is_none());
    assert!(prepared.project_drafts[1].temporal_inference.is_some());
}

#[test]
fn insomnia_explicit_recurrence_definition_resolves_without_a_decision_call() {
    let explicit_draft = draft(
        "Review biweekly, meaning every two weeks.",
        Some(timestamp_ns(2026, Month::September, 16)),
    );
    let assessment = assess_draft(&explicit_draft);
    assert_eq!(
        assessment.resolution.status,
        TemporalResolutionStatus::FullyResolved
    );
    assert!(assessment.resolution.unresolved_indications.is_empty());
    assert!(assessment.analysis.patterns.iter().any(|pattern| {
        pattern.frequency == crate::TemporalFrequency::Weekly
            && pattern.interval == 2
            && pattern.occurrences_per_period == 1
            && pattern.weekday.is_none()
            && pattern.month_day.is_none()
    }));

    let endpoint = CountingEndpoint::new(json!({"answers": {}}));
    let mut prepared = PreparedApplication {
        project_drafts: vec![PreparedMemory {
            temporal: assessment,
            draft: explicit_draft,
            temporal_inference: None,
            routing_metadata: None,
        }],
        user_drafts: Vec::new(),
        rejected: Vec::new(),
        model: "extractor".into(),
    };
    assert_eq!(infer_prepared(Some(&endpoint), &mut prepared).unwrap(), 0);
    assert_eq!(endpoint.calls.load(Ordering::SeqCst), 0);
    assert!(prepared.project_drafts[0].temporal_inference.is_none());
}

#[test]
fn insomnia_no_route_assessment_resolves_explicit_recurrence_deterministically() {
    let explicit_draft = draft(
        "Review biweekly (every two weeks).",
        Some(timestamp_ns(2026, Month::September, 16)),
    );
    let assessment = assess_draft(&explicit_draft);

    assert_eq!(
        assessment.resolution.status,
        TemporalResolutionStatus::FullyResolved
    );
    assert!(assessment.resolution.unresolved_indications.is_empty());
    assert!(assessment.analysis.patterns.iter().any(|pattern| {
        pattern.frequency == crate::TemporalFrequency::Weekly && pattern.interval == 2
    }));

    let mut prepared = PreparedApplication {
        project_drafts: vec![PreparedMemory {
            temporal: assessment.clone(),
            draft: explicit_draft,
            temporal_inference: None,
            routing_metadata: None,
        }],
        user_drafts: Vec::new(),
        rejected: Vec::new(),
        model: "extractor".into(),
    };
    assert_eq!(infer_prepared(None, &mut prepared).unwrap(), 0);
    assert_eq!(prepared.project_drafts[0].temporal, assessment);
    assert!(prepared.project_drafts[0].temporal_inference.is_none());
}

#[test]
fn insomnia_mixed_explicit_and_ambiguous_recurrences_only_request_the_ambiguous_one() {
    let mixed_draft = draft(
        "Review biweekly (every two weeks); audit bimonthly.",
        Some(timestamp_ns(2026, Month::September, 16)),
    );
    let assessment = assess_draft(&mixed_draft);
    assert_eq!(
        assessment.resolution.status,
        TemporalResolutionStatus::Unresolved
    );
    assert_eq!(assessment.resolution.unresolved_indications.len(), 1);
    assert_eq!(
        assessment.resolution.unresolved_indications[0].evidence,
        "bimonthly"
    );

    let endpoint = CountingEndpoint::new(json!({
        "answers": {"u000": {"type":"choice", "choice":"u000c00"}}
    }));
    let mut prepared = PreparedApplication {
        project_drafts: vec![PreparedMemory {
            temporal: assessment,
            draft: mixed_draft,
            temporal_inference: None,
            routing_metadata: None,
        }],
        user_drafts: Vec::new(),
        rejected: Vec::new(),
        model: "extractor".into(),
    };

    assert_eq!(infer_prepared(Some(&endpoint), &mut prepared).unwrap(), 1);
    assert_eq!(endpoint.calls.load(Ordering::SeqCst), 1);
    assert!(prepared.project_drafts[0].temporal_inference.is_some());
}

struct CountingEndpoint {
    calls: AtomicUsize,
    response: Value,
}

impl CountingEndpoint {
    fn new(response: Value) -> Self {
        Self {
            calls: AtomicUsize::new(0),
            response,
        }
    }
}

impl DecisionEndpoint for CountingEndpoint {
    fn model(&self) -> &str {
        "chronos-test"
    }

    fn evaluate(&self, _state: &Value, _questions: &Value) -> Result<Value, DecisionEndpointError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(self.response.clone())
    }
}

fn draft(content: &str, source_time_ns: Option<i64>) -> MemoryDraft {
    MemoryDraft {
        category: "fact".into(),
        memory_type: "project".into(),
        authority_kind: "direct".into(),
        temporal_status: "current".into(),
        title: "Temporal test".into(),
        content: content.into(),
        scope: "test".into(),
        lifecycle_state: "extracted".into(),
        archived: false,
        superseded_by: None,
        parent_id: None,
        source_node_id: None,
        content_source_conversation_id: None,
        content_source_node_id: None,
        grounding_source_conversation_id: None,
        grounding_source_node_id: None,
        source_episode_id: None,
        source_time_ns,
        mutation_id: "insomnia-temporal-test".into(),
        created_at_ns: 1,
        updated_at_ns: 1,
    }
}

fn timestamp_ns(year: i32, month: Month, day: u8) -> i64 {
    let date = Date::from_calendar_date(year, month, day).unwrap();
    i64::try_from(
        date.with_hms(12, 0, 0)
            .unwrap()
            .assume_utc()
            .unix_timestamp_nanos(),
    )
    .unwrap()
}
