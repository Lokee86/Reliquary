use super::*;
use crate::{DecisionEndpointError, SimulatedDecisionEndpoint};
use std::sync::Mutex;

struct RewriteCapture(Mutex<Option<Value>>);
impl DecisionEndpoint for RewriteCapture {
    fn model(&self) -> &str {
        "rewrite-capture"
    }
    fn evaluate(&self, state: &Value, _: &Value) -> Result<Value, DecisionEndpointError> {
        *self.0.lock().unwrap() = Some(state.clone());
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
    let state = endpoint.0.lock().unwrap().clone().unwrap();
    assert_eq!(state["current_query"], "kids baseball game");
    assert_eq!(state["refinement"], "basketball instead, this week");
}

#[test]
fn rewrite_requires_one_versioned_nonempty_complete_query() {
    for response in [
        json!({"version": 1, "query": " "}),
        json!({"version": 2, "query": "new"}),
        json!({"version": 1, "query": "new", "alternatives": []}),
        json!({"version": 1, "query": ["new", "other"]}),
        json!("new query"),
        json!({"version": 1, "query": "x".repeat(crate::MAX_MEMORY_SEARCH_QUERY_BYTES + 1)}),
    ] {
        let endpoint = SimulatedDecisionEndpoint::new("invalid-rewrite", vec![response]);
        assert!(rewrite_query(&endpoint, "original", "correction").is_err());
    }
}
