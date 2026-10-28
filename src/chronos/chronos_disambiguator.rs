use crate::{DecisionEndpoint, DecisionEndpointError, TemporalInference};

#[derive(Debug)]
pub enum TemporalDecisionError {
    Endpoint(DecisionEndpointError),
    InvalidOutput(String),
}
impl std::fmt::Display for TemporalDecisionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Endpoint(e) => write!(f, "{e}"),
            Self::InvalidOutput(e) => write!(f, "invalid Chronos selection: {e}"),
        }
    }
}
impl std::error::Error for TemporalDecisionError {}

pub struct TemporalDisambiguator<E> {
    endpoint: E,
}
impl<E: DecisionEndpoint> TemporalDisambiguator<E> {
    pub fn new(endpoint: E) -> Self {
        Self { endpoint }
    }
    pub fn select(
        &self,
        text: &str,
        reference: Option<i64>,
    ) -> Result<Option<TemporalInference>, TemporalDecisionError> {
        let choices = reliquary_chronos::chronos_choices::assess_choices(text, reference);
        if choices.sets.is_empty() {
            return Ok(None);
        }
        let (state, questions) =
            reliquary_chronos::chronos_choices::selection_request(text, reference, &choices);
        let response = self
            .endpoint
            .evaluate(&state, &questions)
            .map_err(TemporalDecisionError::Endpoint)?;
        reliquary_chronos::chronos_choices::accept_selection(
            text,
            reference,
            self.endpoint.model(),
            &response,
        )
        .map_err(|e| TemporalDecisionError::InvalidOutput(e.to_string()))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::atomic::{AtomicUsize, Ordering};
    struct Endpoint {
        calls: AtomicUsize,
        response: serde_json::Value,
    }
    impl DecisionEndpoint for Endpoint {
        fn model(&self) -> &str {
            "test"
        }
        fn evaluate(
            &self,
            _: &serde_json::Value,
            _: &serde_json::Value,
        ) -> Result<serde_json::Value, DecisionEndpointError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(self.response.clone())
        }
    }
    #[test]
    fn selection_calls_only_for_candidates_and_never_invents_a_choice() {
        let endpoint = Endpoint {
            calls: AtomicUsize::new(0),
            response: json!({"answers":{"u000":{"type":"choice","choice":"u000c01"}}}),
        };
        let disambiguator = TemporalDisambiguator::new(&endpoint);
        assert!(disambiguator.select("today", None).unwrap().is_none());
        assert!(disambiguator.select("2027-03-04", None).unwrap().is_none());
        assert!(disambiguator.select("biweekly", None).unwrap().is_some());
        assert_eq!(endpoint.calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn multiple_indications_are_batched_and_abstention_is_not_persisted() {
        let endpoint = Endpoint {
            calls: AtomicUsize::new(0),
            response: json!({"answers": {
                "u000": {"type":"choice", "choice":"u000c00"},
                "u001": {"type":"choice", "choice":"abstain"}
            }}),
        };
        let inference = TemporalDisambiguator::new(&endpoint)
            .select("biweekly and biweekly", None)
            .unwrap()
            .unwrap();
        assert_eq!(endpoint.calls.load(Ordering::SeqCst), 1);
        assert_eq!(inference.resolutions.len(), 1);
        assert_eq!(inference.resolutions[0].start_byte, 0);
    }

    #[test]
    fn context_free_labeled_cases_abstain_without_invented_results() {
        let endpoint = Endpoint {
            calls: AtomicUsize::new(0),
            response: json!({"answers":{"u000":{"type":"choice","choice":"abstain"}}}),
        };
        let selector = TemporalDisambiguator::new(&endpoint);
        for text in ["Review biweekly", "réunion biweekly"] {
            assert!(selector.select(text, None).unwrap().is_none());
        }
        for text in [
            "Review semiweekly",
            "Deadline 2027-03-04",
            "next week",
            "spring 2027",
        ] {
            assert!(selector.select(text, None).unwrap().is_none());
        }
        assert_eq!(endpoint.calls.load(Ordering::SeqCst), 2);
    }
}
