use super::*;
use crate::{GeneralEndpointError, SimulatedGeneralEndpoint};
use serde_json::json;
use std::sync::Mutex;

struct RecordingGeneral {
    response: Result<Value, String>,
    requests: Mutex<Vec<(String, String, String, Value)>>,
}

impl GeneralEndpoint for RecordingGeneral {
    fn model(&self) -> &str {
        "general-model"
    }

    fn complete_json(
        &self,
        system_prompt: &str,
        user_payload: &str,
        schema_name: &str,
        schema: &Value,
    ) -> Result<Value, GeneralEndpointError> {
        self.requests.lock().unwrap().push((
            system_prompt.into(),
            user_payload.into(),
            schema_name.into(),
            schema.clone(),
        ));
        self.response.clone().map_err(GeneralEndpointError::Failure)
    }
}

fn recorder(response: Result<Value, &str>) -> Arc<RecordingGeneral> {
    Arc::new(RecordingGeneral {
        response: response.map_err(str::to_owned),
        requests: Mutex::new(Vec::new()),
    })
}

fn entity_questions() -> Value {
    json!({
        "resolution": {
            "type": "choice",
            "instructions": "Resolve only the supplied mention.",
            "criteria": {
                "candidate_0": "The mention matches candidate zero.",
                "unresolved": "Evidence is ambiguous."
            }
        }
    })
}

#[test]
fn choice_adapter_preserves_question_ids_and_allowed_choices_without_confidence() {
    let general = recorder(Ok(json!({
        "answers": {"resolution": {"type": "choice", "choice": "candidate_0"}}
    })));
    let adapter = GeneralDecisionEndpoint::new(general.clone());
    assert_eq!(adapter.model(), "general-model");
    let response = adapter
        .evaluate(&json!({"mention": "Ada"}), &entity_questions())
        .unwrap();
    assert_eq!(response["answers"]["resolution"]["choice"], "candidate_0");

    let requests = general.requests.lock().unwrap();
    let (prompt, payload, name, schema) = &requests[0];
    assert!(prompt.contains("Resolve only the supplied mention."));
    assert_eq!(name, "decision_answers");
    assert_eq!(
        serde_json::from_str::<Value>(payload).unwrap()["state"]["mention"],
        "Ada"
    );
    assert_eq!(
        schema["properties"]["answers"]["properties"]["resolution"]["properties"]["choice"]["enum"],
        json!(["candidate_0", "unresolved"])
    );
    assert_eq!(
        schema["properties"]["answers"]["properties"]["resolution"]["additionalProperties"],
        false
    );
    assert!(
        schema["properties"]["answers"]["properties"]["resolution"]["properties"]
            .get("probabilities")
            .is_none()
    );
}

#[test]
fn choice_adapter_rejects_unsolicited_confidence_unknown_answers_and_bad_choices() {
    for response in [
        json!({}),
        json!({"answers":{}}),
        json!({"answers":{"resolution":{"type":"choice"}}}),
        json!({"answers":{"resolution":{"type":"choice","choice":7}}}),
        json!({"answers":{"unknown":{"type":"choice","choice":"candidate_0"}}}),
        json!({"answers":{"resolution":{"type":"choice","choice":"candidate_0"}},"extra":true}),
        json!({"answers":{"resolution":{"type":"choice","choice":"candidate_0","probabilities":{"candidate_0":1.0}}}}),
        json!({"answers":{"resolution":{"type":"choice","choice":"candidate_0"},"extra":{"type":"choice","choice":"candidate_0"}}}),
        json!({"answers":{"resolution":{"type":"choice","choice":"invented"}}}),
        json!({"answers":{"resolution":{"type":"text","choice":"candidate_0"}}}),
    ] {
        let adapter = GeneralDecisionEndpoint::new(recorder(Ok(response)));
        assert!(matches!(
            adapter.evaluate(&json!({}), &entity_questions()),
            Err(DecisionEndpointError::InvalidResponse(_))
        ));
    }
}

#[test]
fn unsupported_or_malformed_choice_questions_fail_before_completion() {
    let general = recorder(Ok(json!({})));
    let adapter = GeneralDecisionEndpoint::new(general.clone());
    for questions in [
        json!({"q":{"type":"text","instructions":"x","criteria":{"a":"A"}}}),
        json!({"q":{"type":"choice","criteria":{"a":"A"}}}),
        json!({"q":{"type":"choice","instructions":"x","criteria":[]}}),
    ] {
        assert!(adapter.evaluate(&json!({}), &questions).is_err());
    }
    assert!(general.requests.lock().unwrap().is_empty());
}

#[test]
fn embedded_retrieval_schema_is_passed_through_exactly_and_response_is_preserved() {
    let schema = json!({
        "type":"object", "required":["version","approved"], "additionalProperties":false
    });
    let expected = json!({"version":1,"approved":[]});
    let general = recorder(Ok(expected.clone()));
    let adapter = GeneralDecisionEndpoint::new(general.clone());
    let questions = json!({"instruction":"Approve only supplied IDs.","schema":schema});
    assert_eq!(
        adapter
            .evaluate(&json!({"operation":"memory_query"}), &questions)
            .unwrap(),
        expected
    );
    let requests = general.requests.lock().unwrap();
    assert_eq!(requests[0].2, "decision_response");
    assert_eq!(requests[0].3, schema);
}

#[test]
fn general_transport_failures_are_mapped_without_retry() {
    let general = Arc::new(SpyFailure {
        calls: Mutex::new(0),
    });
    let adapter = GeneralDecisionEndpoint::new(general.clone());
    assert!(matches!(
        adapter.evaluate(&json!({}), &entity_questions()),
        Err(DecisionEndpointError::Failure(_))
    ));
    assert_eq!(*general.calls.lock().unwrap(), 1);
}

struct SpyFailure {
    calls: Mutex<usize>,
}

impl GeneralEndpoint for SpyFailure {
    fn model(&self) -> &str {
        "failure-model"
    }

    fn complete_json(
        &self,
        _: &str,
        _: &str,
        _: &str,
        _: &Value,
    ) -> Result<Value, GeneralEndpointError> {
        *self.calls.lock().unwrap() += 1;
        Err(GeneralEndpointError::Failure(
            "simulated transport error".into(),
        ))
    }
}

#[test]
fn simulated_general_endpoint_remains_usable_through_the_adapter() {
    let general: Arc<dyn GeneralEndpoint> = Arc::new(SimulatedGeneralEndpoint::new(
        "simulated",
        vec![json!({"answers":{"resolution":{"type":"choice","choice":"unresolved"}}})],
    ));
    let adapter = GeneralDecisionEndpoint::new(general);
    assert_eq!(adapter.model(), "simulated");
    assert_eq!(
        adapter.evaluate(&json!({}), &entity_questions()).unwrap()["answers"]["resolution"]["choice"],
        "unresolved"
    );
}
