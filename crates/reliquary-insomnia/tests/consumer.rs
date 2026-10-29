use reliquary_archive::{EpisodeBoundary, EpisodeOrigin};
use reliquary_insomnia::{
    Episode, EpisodeId, INSOMNIA_EXTRACTOR_CONTRACT_VERSION, InsomniaEvidenceResult,
    InsomniaEvidenceTurn, InsomniaExtractionStage, InsomniaExtractor, MAX_INSOMNIA_CANDIDATES,
    ResolvedTurn, insomnia_schema,
};
use reliquary_switchboard::SimulatedGeneralEndpoint;
use serde_json::json;

fn episode() -> Episode {
    Episode {
        id: EpisodeId([7; 32]),
        conversation_id: "conversation".into(),
        start_node_id: "u0".into(),
        end_node_id: "u0".into(),
        origin: EpisodeOrigin::Live,
        boundary: EpisodeBoundary::Explicit,
        source_through_ns: 10,
        finalized_at_ns: 11,
    }
}

fn turns() -> Vec<ResolvedTurn> {
    vec![ResolvedTurn {
        node_id: "u0".into(),
        role: "user".into(),
        principal_id: None,
        timestamp_ns: 10,
        content: "I prefer concise answers.".into(),
    }]
}

fn retained_ledger() -> serde_json::Value {
    json!({"turns": {"u0": [{
        "disposition": "retain", "authority_kind": "direct",
        "category": "preference", "type": "communication",
        "lifecycle": "current", "proposition": "The user prefers concise answers.",
        "authority_source_node_id": "", "grounding_source_node_id": "",
        "reason": "durable preference"
    }]}, "evidence_requests": []})
}

fn omitted_ledger(requests: serde_json::Value) -> serde_json::Value {
    json!({"turns": {"u0": [{
        "disposition": "omit", "authority_kind": "none",
        "category": "none", "type": "none", "lifecycle": "none",
        "proposition": "", "authority_source_node_id": "",
        "grounding_source_node_id": "", "reason": "not durable"
    }]}, "evidence_requests": requests})
}

#[test]
fn external_consumer_runs_real_semantic_extraction_without_root() {
    assert_eq!(INSOMNIA_EXTRACTOR_CONTRACT_VERSION, "v3-4");
    assert_eq!(MAX_INSOMNIA_CANDIDATES, 64);
    assert!(insomnia_schema().is_object());
    let endpoint = SimulatedGeneralEndpoint::new(
        "consumer-model",
        vec![
            retained_ledger(),
            json!({"groups": {"g000": {
                "title": "Concise responses",
                "content": "The user prefers concise answers."
            }}}),
        ],
    );
    let result = InsomniaExtractor::new(endpoint)
        .extract(&episode(), &turns())
        .unwrap();
    assert_eq!(result.model, "consumer-model");
    assert_eq!(result.candidates.len(), 1);
    assert!(result.rejected.is_empty());
    let candidate = &result.candidates[0];
    assert_eq!(candidate.authority_kind, "direct");
    assert_eq!(candidate.category, "preference");
    assert_eq!(candidate.memory_type, "communication");
    assert_eq!(candidate.source_node_id, "u0");
    assert_eq!(candidate.content, "The user prefers concise answers.");
    assert!(candidate.authority_source_node_id.is_none());
}

#[test]
fn external_consumer_completes_one_bounded_evidence_round() {
    let request = json!({
        "kind": "turn", "conversation_id": "older", "node_id": "a0",
        "start_node_id": "", "end_node_id": "", "query": "", "limit": 0
    });
    let endpoint = SimulatedGeneralEndpoint::new(
        "consumer-model",
        vec![omitted_ledger(json!([request])), omitted_ledger(json!([]))],
    );
    let extractor = InsomniaExtractor::new(endpoint);
    let InsomniaExtractionStage::Evidence(round) = extractor.start(&episode(), &turns()).unwrap()
    else {
        panic!("expected one read-only evidence round");
    };
    assert_eq!(round.requests().len(), 1);
    let turn = InsomniaEvidenceTurn {
        conversation_id: "older".into(),
        node_id: "a0".into(),
        role: "assistant".into(),
        timestamp_ns: 5,
        content: "Earlier evidence.".into(),
    };
    let result = extractor
        .finish_evidence(
            &episode(),
            &turns(),
            round,
            vec![InsomniaEvidenceResult {
                kind: "turn".into(),
                turns: vec![turn.clone()],
                error: None,
                truncated: false,
            }],
            vec![turn],
        )
        .unwrap();
    assert!(result.candidates.is_empty());
    assert_eq!(result.evidence_turns.len(), 1);
}

#[test]
fn external_consumer_rejects_archive_fallback_for_external_input() {
    let endpoint = SimulatedGeneralEndpoint::new(
        "consumer-model",
        vec![omitted_ledger(
            json!([{"kind": "turn", "conversation_id": "older", "node_id": "a0"}]),
        )],
    );
    let error = InsomniaExtractor::new(endpoint)
        .extract_external_input([9; 32], "feed", "conversation", &turns())
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("external evidence must be in the authorized window")
    );
}
