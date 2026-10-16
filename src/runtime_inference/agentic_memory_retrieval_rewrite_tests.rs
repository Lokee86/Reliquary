use super::*;
use crate::{DecisionEndpointError, SimulatedDecisionEndpoint};
use std::sync::Mutex;

struct RewriteCapture(Mutex<Option<(Value, Value)>>);
impl DecisionEndpoint for RewriteCapture {
    fn model(&self) -> &str {
        "rewrite-capture"
    }
    fn evaluate(&self, state: &Value, questions: &Value) -> Result<Value, DecisionEndpointError> {
        *self.0.lock().unwrap() = Some((state.clone(), questions.clone()));
        Ok(json!({"version": 1, "query": "kids basketball game this week"}))
    }
}

#[test]
fn rewrite_reinterprets_complete_query_instead_of_appending() {
    let endpoint = RewriteCapture(Mutex::new(None));
    let revised = rewrite_query(
        &endpoint,
        "kids baseball game",
        "basketball instead, this week",
    )
    .unwrap();
    assert_eq!(revised, "kids basketball game this week");
    let (state, questions) = endpoint.0.lock().unwrap().clone().unwrap();
    assert_eq!(
        state,
        json!({
            "version": MEMORY_REWRITE_VERSION,
            "operation": MEMORY_REWRITE_OPERATION_V1,
            "current_query": "kids baseball game",
            "refinement": "basketball instead, this week"
        })
    );
    assert_eq!(questions["instruction"], MEMORY_REWRITE_INSTRUCTION_V1);
    assert_eq!(
        questions["response_shape"],
        json!({"version": 1, "query": "one complete revised query"})
    );
    assert_eq!(
        questions["schema"],
        serde_json::from_str::<Value>(MEMORY_REWRITE_SCHEMA_V1).unwrap()
    );
    assert_eq!(questions["schema"]["additionalProperties"], false);
    assert_eq!(
        questions["schema"]["properties"]["query"]["maxLength"],
        crate::MAX_MEMORY_SEARCH_QUERY_BYTES
    );
}

#[test]
fn rewrite_requires_one_versioned_nonempty_complete_query() {
    for response in [
        json!({"version": 1, "query": " "}),
        json!({"version": 2, "query": "new"}),
        json!({"version": 1, "query": "new", "alternatives": []}),
        json!({"version": 1, "query": "new", "commentary": "try this"}),
        json!({"version": 1, "query": ["new", "other"]}),
        json!("new query"),
        json!({"version": 1, "query": "x".repeat(crate::MAX_MEMORY_SEARCH_QUERY_BYTES + 1)}),
    ] {
        let endpoint = SimulatedDecisionEndpoint::new("invalid-rewrite", vec![response]);
        assert!(rewrite_query(&endpoint, "original", "correction").is_err());
    }
}
