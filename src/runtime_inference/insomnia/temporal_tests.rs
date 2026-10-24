use super::native_processor::{PreparedApplication, PreparedMemory};
use super::temporal::{assess_draft, infer_prepared};
use crate::{
    GeneralEndpoint, GeneralEndpointError, MemoryDraft, TemporalIndicationKind,
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
        "resolutions": [{"id":"u000","canonical_expression":"every two weeks"}]
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

    assert_eq!(infer_prepared(&endpoint, &mut prepared).unwrap(), 1);
    assert_eq!(endpoint.calls.load(Ordering::SeqCst), 1);
    assert!(prepared.project_drafts[0].temporal_inference.is_none());
    assert!(prepared.project_drafts[1].temporal_inference.is_some());
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

impl GeneralEndpoint for CountingEndpoint {
    fn model(&self) -> &str {
        "chronos-test"
    }

    fn complete_json(
        &self,
        _system_prompt: &str,
        _user_payload: &str,
        _schema_name: &str,
        _schema: &Value,
    ) -> Result<Value, GeneralEndpointError> {
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
