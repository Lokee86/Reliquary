use super::*;
use crate::{MAX_INSOMNIA_EVIDENCE_BYTES, MAX_INSOMNIA_EVIDENCE_TURNS};
use serde_json::json;

fn turn(node_id: &str, content: &str) -> InsomniaEvidenceTurn {
    InsomniaEvidenceTurn {
        conversation_id: "conversation".into(),
        node_id: node_id.into(),
        role: "user".into(),
        timestamp_ns: 7,
        content: content.into(),
    }
}

fn round() -> InsomniaEvidenceRound {
    InsomniaEvidenceRound {
        episode_payload: json!({}),
        initial_ledger: json!({}),
        requests: vec![EvidenceRequest {
            kind: "turn".into(),
            conversation_id: "conversation".into(),
            node_id: "n0".into(),
            start_node_id: String::new(),
            end_node_id: String::new(),
            query: String::new(),
            limit: 0,
        }],
    }
}

fn result(turns: Vec<InsomniaEvidenceTurn>) -> Vec<InsomniaEvidenceResult> {
    vec![InsomniaEvidenceResult {
        kind: "turn".into(),
        turns,
        error: None,
        truncated: false,
    }]
}

#[test]
fn accepts_coherent_response_with_exact_authorized_turns() {
    let evidence = vec![turn("n0", "authorized text")];
    assert!(validate_evidence_response(&round(), &result(evidence.clone()), &evidence).is_ok());
}

#[test]
fn evidence_round_rejects_changed_authoritative_input_before_model_call() {
    use reliquary_archive::{EpisodeBoundary, EpisodeOrigin};
    use reliquary_switchboard::SimulatedGeneralEndpoint;

    let episode = Episode {
        id: crate::EpisodeId([1; 32]),
        conversation_id: "conversation".into(),
        start_node_id: "n0".into(),
        end_node_id: "n0".into(),
        origin: EpisodeOrigin::Live,
        boundary: EpisodeBoundary::Explicit,
        source_through_ns: 7,
        finalized_at_ns: 8,
    };
    let mut turns = vec![ResolvedTurn {
        node_id: "n0".into(),
        role: "user".into(),
        principal_id: None,
        timestamp_ns: 7,
        content: "original".into(),
    }];
    let mut evidence_round = round();
    evidence_round.episode_payload = encode_episode_value(&episode, &turns).unwrap();
    turns[0].content = "changed".into();
    let extractor = InsomniaExtractor::new(SimulatedGeneralEndpoint::new("test", vec![]));
    let error = extractor
        .finish_evidence(&episode, &turns, evidence_round, vec![], vec![])
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("does not match the authoritative episode")
    );
}

#[test]
fn rejects_response_over_turn_or_byte_bounds() {
    let many: Vec<_> = (0..=MAX_INSOMNIA_EVIDENCE_TURNS)
        .map(|n| turn(&format!("n{n}"), "x"))
        .collect();
    assert!(validate_evidence_response(&round(), &result(many.clone()), &many).is_err());

    let huge = vec![turn("n0", &"x".repeat(MAX_INSOMNIA_EVIDENCE_BYTES + 1))];
    assert!(validate_evidence_response(&round(), &result(huge.clone()), &huge).is_err());
}

#[test]
fn rejects_incoherent_or_unmatched_result_shapes() {
    let evidence = vec![turn("n0", "text")];
    let mut wrong_kind = result(evidence.clone());
    wrong_kind[0].kind = "archive_search".into();
    assert!(validate_evidence_response(&round(), &wrong_kind, &evidence).is_err());

    let mut failed_with_turns = result(evidence.clone());
    failed_with_turns[0].error = Some("lookup failed".into());
    assert!(validate_evidence_response(&round(), &failed_with_turns, &evidence).is_err());

    let stranger = vec![turn("other", "unrequested")];
    assert!(validate_evidence_response(&round(), &result(stranger.clone()), &evidence).is_err());

    let repeated = vec![evidence[0].clone(); MAX_INSOMNIA_EVIDENCE_TURNS * 4 + 1];
    assert!(validate_evidence_response(&round(), &result(repeated), &evidence).is_err());
}
