//! Deterministic retained-vector replay and lexical evidence oracle; evaluation only.
use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};
pub(super) struct Replay {
    pub profile: crate::CompatibilityProfile,
    pub query: Mutex<Vec<f32>>,
    pub calls: AtomicUsize,
}
impl EmbeddingEndpoint for Replay {
    fn dimensions(&self) -> u32 {
        self.profile.dimensions
    }
    fn normalization(&self) -> crate::VectorNormalization {
        self.profile.normalization
    }
    fn embed(
        &self,
        mode: crate::EmbeddingMode,
        inputs: &[String],
    ) -> Result<Vec<Vec<f32>>, crate::EmbeddingEndpointError> {
        use crate::compatibility_profile_probe::{DOCUMENT_PROBES, QUERY_PROBES};
        let probes = match mode {
            crate::EmbeddingMode::Query => QUERY_PROBES,
            crate::EmbeddingMode::Document => DOCUMENT_PROBES,
        };
        if inputs.iter().map(String::as_str).eq(probes) {
            return Ok(self
                .profile
                .references
                .iter()
                .filter(|reference| reference.mode == mode)
                .map(|reference| reference.vector.clone())
                .collect());
        }
        if mode != crate::EmbeddingMode::Query {
            return Err(crate::EmbeddingEndpointError::Failure(
                "fixture replay forbids document generation".into(),
            ));
        }
        self.calls.fetch_add(inputs.len(), Ordering::SeqCst);
        Ok(inputs
            .iter()
            .map(|_| self.query.lock().unwrap().clone())
            .collect())
    }
}
/// 0 = approve all, 1 = decline all, 2 = literal retained-fact evidence oracle.
pub(super) struct Oracle {
    pub mode: AtomicUsize,
    pub calls: Mutex<Vec<Value>>,
}
impl DecisionEndpoint for Oracle {
    fn model(&self) -> &str {
        "retained-fact-oracle-v1"
    }
    fn evaluate(&self, state: &Value, _: &Value) -> Result<Value, crate::DecisionEndpointError> {
        self.calls.lock().unwrap().push(state.clone());
        if state["operation"] == "refine_query_rewrite" {
            return Ok(json!({"version":1,"query":state["refinement"]}));
        }
        let query = state["query"].as_str().unwrap().to_lowercase();
        let mode = self.mode.load(Ordering::SeqCst);
        let approved = state["candidates"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|candidate| match mode {
                0 => true,
                1 => false,
                _ => format!(
                    "{} {}",
                    candidate["title"].as_str().unwrap(),
                    candidate["content"].as_str().unwrap()
                )
                .to_lowercase()
                .contains(&query),
            })
            .map(|candidate| candidate["memory"].clone())
            .collect::<Vec<_>>();
        Ok(json!({"version":1,"approved":approved}))
    }
}
