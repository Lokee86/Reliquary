use super::{
    DREAM_COMMUNITY_NAMING_RETRY_LIMIT, DREAM_COMMUNITY_NAMING_SYSTEM_PROMPT,
    DreamCommunityNameEvaluationError, DreamCommunityNameEvaluator, dream_community_naming_schema,
    parse_name,
};
use reliquary_switchboard::{GeneralEndpoint, GeneralEndpointError};
use serde_json::{Value, json};
use std::sync::atomic::{AtomicUsize, Ordering};

struct FakeEndpoint {
    calls: AtomicUsize,
    responses: Vec<Value>,
    backpressure: bool,
}

impl GeneralEndpoint for FakeEndpoint {
    fn model(&self) -> &str {
        "fake"
    }

    fn complete_json(
        &self,
        _: &str,
        _: &str,
        _: &str,
        _: &Value,
    ) -> Result<Value, GeneralEndpointError> {
        let call = self.calls.fetch_add(1, Ordering::SeqCst);
        if self.backpressure {
            return Err(GeneralEndpointError::backpressure("busy", None));
        }
        self.responses
            .get(call)
            .cloned()
            .ok_or_else(|| GeneralEndpointError::Failure("response exhausted".into()))
    }
}

#[test]
fn public_naming_contract_has_prompt_schema_and_retry_policy() {
    assert!(DREAM_COMMUNITY_NAMING_SYSTEM_PROMPT.contains("concrete noun phrase"));
    assert_eq!(dream_community_naming_schema()["required"][0], "name");
    assert_eq!(DREAM_COMMUNITY_NAMING_RETRY_LIMIT, 2);
}

#[test]
fn name_validation_trims_and_rejects_bad_output() {
    assert_eq!(
        parse_name(&json!({"name": "  Alpha Work  "})).unwrap(),
        "Alpha Work"
    );
    assert!(parse_name(&json!({"name": "\n"})).is_err());
    assert!(parse_name(&json!({"name": "unsafe\u{0007}name"})).is_err());
}

#[test]
fn evaluator_retries_invalid_and_transient_results_but_stops_for_backpressure() {
    let evaluator = DreamCommunityNameEvaluator::new(FakeEndpoint {
        calls: AtomicUsize::new(0),
        responses: vec![
            json!({"name": ""}),
            json!({"name": "also\u{0007}bad"}),
            json!({"name": "Accepted Name"}),
        ],
        backpressure: false,
    });
    assert_eq!(evaluator.generate_name(&[]).unwrap(), "Accepted Name");

    let endpoint = FakeEndpoint {
        calls: AtomicUsize::new(0),
        responses: vec![],
        backpressure: true,
    };
    let evaluator = DreamCommunityNameEvaluator::new(endpoint);
    assert!(matches!(
        evaluator.generate_name(&[]),
        Err(DreamCommunityNameEvaluationError::Inference(_))
    ));
    assert_eq!(evaluator.endpoint.calls.load(Ordering::SeqCst), 1);
}
