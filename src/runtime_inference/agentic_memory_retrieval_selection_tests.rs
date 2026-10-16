use super::*;
use crate::{
    AgenticMemoryContent, AgenticMemoryError, AgenticMemoryOperation,
    AgenticMemoryRelationEvidence, DecisionEndpointError, GraphDirection, GraphRelationKind,
    MemoryId, MemoryRef,
};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::sync::Mutex;

struct SelectionEndpoint {
    response: Value,
    observed: Mutex<Vec<(Value, Value)>>,
}

impl DecisionEndpoint for SelectionEndpoint {
    fn model(&self) -> &str {
        "selection-contract-test"
    }

    fn evaluate(&self, state: &Value, questions: &Value) -> Result<Value, DecisionEndpointError> {
        self.observed
            .lock()
            .unwrap()
            .push((state.clone(), questions.clone()));
        Ok(self.response.clone())
    }
}

fn memory(owner: &str, byte: u8, title: &str, content: &str) -> AgenticMemoryContent {
    AgenticMemoryContent {
        memory: MemoryRef {
            owner_id: owner.into(),
            memory_id: MemoryId([byte; 32]),
        },
        title: title.into(),
        content: content.into(),
    }
}

fn identity(owner: &str, byte: u8) -> Value {
    json!({"owner_id": owner, "memory_id": format!("{:02x}", byte).repeat(32)})
}

fn endpoint(response: Value) -> SelectionEndpoint {
    SelectionEndpoint {
        response,
        observed: Mutex::new(Vec::new()),
    }
}

fn valid_response(approved: Vec<Value>) -> Value {
    json!({"version": MEMORY_SELECTION_VERSION, "approved": approved})
}

#[test]
fn versioned_request_and_schema_are_exact_for_all_three_operations() {
    assert_eq!(MEMORY_SELECTION_VERSION, 1);
    let schema: Value = serde_json::from_str(MEMORY_SELECTION_SCHEMA_V1).unwrap();
    assert_eq!(schema["additionalProperties"], false);
    assert_eq!(
        schema["properties"]["approved"]["items"]["additionalProperties"],
        false
    );
    let operations = [
        (AgenticMemoryOperation::Query, MEMORY_QUERY_OPERATION_V1),
        (
            AgenticMemoryOperation::Refinement,
            MEMORY_REFINEMENT_OPERATION_V1,
        ),
        (AgenticMemoryOperation::Traversal, MEMORY_GRAPH_OPERATION_V1),
    ];
    for (operation, operation_name) in operations {
        let candidate = memory("owner-A", 0x2a, "Research note", "complete query context");
        let anchor = MemoryRef {
            owner_id: "owner-A".into(),
            memory_id: MemoryId([0x11; 32]),
        };
        let relation = AgenticMemoryRelationEvidence {
            neighbor: candidate.memory.clone(),
            kind: GraphRelationKind::Causal,
            direction: GraphDirection::Incoming,
        };
        let mock = endpoint(valid_response(vec![identity("owner-A", 0x2a)]));
        let selected = select_memories(
            &mock,
            operation,
            "complete query context",
            std::slice::from_ref(&candidate),
            Some(&anchor),
            std::slice::from_ref(&relation),
        )
        .unwrap();
        assert_eq!(selected, BTreeSet::from([candidate.memory.clone()]));
        let observed = mock.observed.lock().unwrap();
        let (state, questions) = &observed[0];
        assert_eq!(
            state,
            &json!({
                "version": 1,
                "operation": operation_name,
                "query": "complete query context",
                "anchor": {"owner_id": "owner-A", "memory_id": "11".repeat(32)},
                "relations": [{
                    "neighbor": identity("owner-A", 0x2a),
                    "kind": "Causal",
                    "direction": "Incoming"
                }],
                "candidates": [{
                    "memory": identity("owner-A", 0x2a),
                    "title": "Research note",
                    "content": "complete query context"
                }]
            })
        );
        assert_eq!(questions["instruction"], MEMORY_SELECTION_INSTRUCTION_V1);
        assert_eq!(questions["schema"], schema);
        assert_eq!(questions["response_shape"]["version"], 1);
    }
}

#[test]
fn query_and_refinement_requests_allow_absent_graph_context() {
    for operation in [
        AgenticMemoryOperation::Query,
        AgenticMemoryOperation::Refinement,
    ] {
        let candidate = memory("owner-B", 0x31, "Selected", "evidence");
        let mock = endpoint(valid_response(vec![]));
        select_memories(
            &mock,
            operation,
            "whole revised information need",
            &[candidate],
            None,
            &[],
        )
        .unwrap();
        let observed = mock.observed.lock().unwrap();
        let state = &observed[0].0;
        assert_eq!(state["query"], "whole revised information need");
        assert!(state["anchor"].is_null());
        assert_eq!(state["relations"], json!([]));
    }
}

#[test]
fn strict_parser_rejects_unknown_fields_duplicates_wrong_version_and_unprovided_ids() {
    let candidate = memory("owner-C", 0x42, "Evidence", "body");
    let malformed = [
        (
            json!({"version": 1, "approved": [], "extra": true}),
            "unknown response field",
        ),
        (
            json!({"version": 1, "approved": [{"owner_id":"owner-C", "memory_id":format!("{}", "42".repeat(32)), "extra":true}]}),
            "unknown identity field",
        ),
        (
            valid_response(vec![identity("owner-C", 0x42), identity("owner-C", 0x42)]),
            "duplicate identity",
        ),
        (json!({"version": 2, "approved": []}), "wrong version"),
        (
            valid_response(vec![identity("other-owner", 0x42)]),
            "unprovided owner-qualified identity",
        ),
    ];
    for (response, label) in malformed {
        let mock = endpoint(response);
        let error = select_memories(
            &mock,
            AgenticMemoryOperation::Query,
            "complete query",
            std::slice::from_ref(&candidate),
            None,
            &[],
        )
        .unwrap_err();
        assert!(
            matches!(error, AgenticMemoryError::InvalidDecision(_)),
            "{label}: {error:?}"
        );
    }
}
