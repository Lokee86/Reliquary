use reliquary_perception::{
    ContributionState, HitVerdict, NATIVE_DECISION_MODEL, NativeDecisionError, ScopeChoice,
    TemporalChoice, VerificationState, VerificationVerdict, contribute, verify,
};
use reliquary_switchboard::{DecisionEndpoint, DecisionEndpointError};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Deserialize)]
struct Recording<State> {
    state: State,
    questions: Value,
    reply: Value,
}

struct Endpoint {
    model: &'static str,
    state: Value,
    questions: Value,
    reply: Value,
    calls: AtomicUsize,
}

impl Endpoint {
    fn new(recording: &Recording<impl serde::Serialize>) -> Self {
        Self {
            model: NATIVE_DECISION_MODEL,
            state: serde_json::to_value(&recording.state).unwrap(),
            questions: recording.questions.clone(),
            reply: recording.reply.clone(),
            calls: AtomicUsize::new(0),
        }
    }

    fn calls(&self) -> usize {
        self.calls.load(Ordering::Relaxed)
    }
}

impl DecisionEndpoint for Endpoint {
    fn model(&self) -> &str {
        self.model
    }

    fn evaluate(&self, state: &Value, questions: &Value) -> Result<Value, DecisionEndpointError> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        assert_eq!(state, &self.state);
        assert_eq!(questions, &self.questions);
        Ok(self.reply.clone())
    }
}

#[test]
fn one_contribution_request_replays_exact_recorded_state_and_questions() {
    let case: Recording<ContributionState> =
        serde_json::from_str(include_str!("fixtures/contribution.json")).unwrap();
    let mut endpoint = Endpoint::new(&case);
    let recorded = contribute(&endpoint, &case.state).unwrap();
    assert_eq!(endpoint.calls(), 1);
    assert_eq!(recorded.hits.len(), 5);
    assert!(!recorded.triggers_review());
    let later = recorded.hits["jev-02-b"];
    assert_eq!(later.verdict, HitVerdict::MaterialQualification);
    assert_eq!(later.time, TemporalChoice::LaterChangeOnly);
    assert!(!later.triggers_review());

    let answer = |choice| json!({"type": "choice", "choice": choice});
    let answers = endpoint.reply["answers"].as_object_mut().unwrap();
    answers.insert("jev-02-a".into(), answer("material_challenge"));
    answers.insert("jev-02-c".into(), answer("material_challenge"));
    answers.insert("jev-02-c-time".into(), answer("first_effective_after"));
    answers.insert("jev-02-c-scope".into(), answer("overlapping_scope"));
    answers.insert("jev-02-d".into(), answer("material_qualification"));
    answers.insert("jev-02-d-scope".into(), answer("unclear_scope"));
    let checked = contribute(&endpoint, &case.state).unwrap();
    assert_eq!(endpoint.calls(), 2);
    assert!(checked.triggers_review());
    assert_eq!(
        checked.hits["jev-02-a"].scope,
        ScopeChoice::ExplicitlySeparateScope
    );
    assert!(!checked.hits["jev-02-a"].triggers_review());
    assert!(!checked.hits["jev-02-b"].triggers_review());
    assert_eq!(
        checked.hits["jev-02-c"].time,
        TemporalChoice::FirstEffectiveAfter
    );
    assert!(checked.hits["jev-02-c"].triggers_review());
    assert!(checked.hits["jev-02-d"].triggers_review());
    assert!(!checked.hits["jev-02-e"].triggers_review());

    endpoint.reply["answers"]["jev-02-b"] = answer("material_challenge");
    let checked = contribute(&endpoint, &case.state).unwrap();
    assert!(checked.hits["jev-02-b"].triggers_review());
    assert_eq!(endpoint.calls(), 3);
}

#[test]
fn one_verification_request_preserves_frozen_state_for_each_draft_and_applies_veto() {
    let mut case: Recording<VerificationState> =
        serde_json::from_str(include_str!("fixtures/verification.json")).unwrap();
    let endpoint = Endpoint::new(&case);
    let initial = verify(&endpoint, &case.state).unwrap();
    assert_eq!(initial.verdict, VerificationVerdict::StandsAsWritten);
    assert_eq!(initial.hits["p2"], HitVerdict::NoMaterialEffect);
    assert_eq!(endpoint.calls(), 1);

    case.state.claim = "The Observatory held the contingency session indoors for 23 guests.".into();
    let mut endpoint = Endpoint::new(&case);
    for disposition in ["material_challenge", "material_qualification"] {
        endpoint.reply["answers"]["hit-p2"] = json!({"type": "choice", "choice": disposition});
        let successor = verify(&endpoint, &case.state).unwrap();
        assert_eq!(successor.overall, VerificationVerdict::StandsAsWritten);
        assert_eq!(successor.verdict, VerificationVerdict::Unresolved);
    }
    endpoint.reply["answers"]["verify"] = json!({"type": "choice", "choice": "needs_revision"});
    assert_eq!(
        verify(&endpoint, &case.state).unwrap().verdict,
        VerificationVerdict::NeedsRevision
    );
    assert_eq!(endpoint.calls(), 3);
}

#[test]
fn verification_accepts_an_empty_backcheck_and_uses_choices_without_a_score_cutoff() {
    let mut case: Recording<VerificationState> =
        serde_json::from_str(include_str!("fixtures/verification.json")).unwrap();
    case.state.backcheck_hits.clear();
    case.questions.as_object_mut().unwrap().remove("hit-p2");
    case.reply["answers"]
        .as_object_mut()
        .unwrap()
        .remove("hit-p2");
    case.reply["answers"]["verify"] = json!({
        "type": "choice",
        "choice": "stands_as_written",
        "confidence": 0.0,
        "probabilities": {"stands_as_written": 0.0, "unresolved": 1.0}
    });
    let endpoint = Endpoint::new(&case);
    let decision = verify(&endpoint, &case.state).unwrap();
    assert_eq!(decision.verdict, VerificationVerdict::StandsAsWritten);
    assert!(decision.hits.is_empty());
    assert_eq!(decision.diagnostics["verify"].confidence, Some(0.0));
    assert_eq!(endpoint.calls(), 1);
}

#[test]
fn malformed_native_replies_never_yield_a_partial_decision() {
    let case: Recording<VerificationState> =
        serde_json::from_str(include_str!("fixtures/verification.json")).unwrap();
    let mut endpoint = Endpoint::new(&case);
    for malformed in [
        json!({"type":"different", "choice":"stands_as_written"}),
        json!({"type":"choice", "choice":"material_challenge"}),
        json!({"type":"choice", "choice":"stands_as_written", "probabilities":{"unresolved":0.8}}),
        json!({"type":"choice", "choice":"stands_as_written", "probabilities":{"stands_as_written":1.1}}),
        json!({"type":"choice", "choice":"stands_as_written", "probabilities":{"stands_as_written":0.1,"unknown":0.9}}),
        json!({"type":"choice", "choice":"stands_as_written", "probabilities":null}),
    ] {
        endpoint.reply["answers"]["verify"] = malformed;
        assert!(matches!(
            verify(&endpoint, &case.state),
            Err(NativeDecisionError::InvalidReply(_))
        ));
    }
    endpoint.reply = case.reply.clone();
    endpoint.reply["answers"]["unexpected"] = endpoint.reply["answers"]["verify"].clone();
    assert!(matches!(
        verify(&endpoint, &case.state),
        Err(NativeDecisionError::InvalidReply(_))
    ));
    endpoint.reply["answers"]
        .as_object_mut()
        .unwrap()
        .remove("hit-p2");
    assert!(matches!(
        verify(&endpoint, &case.state),
        Err(NativeDecisionError::InvalidReply(_))
    ));

    endpoint.reply = case.reply.clone();
    endpoint.reply["extra"] = json!(" ".repeat(16 * 1024));
    assert!(matches!(
        verify(&endpoint, &case.state),
        Err(NativeDecisionError::InvalidReply(_))
    ));

    let case: Recording<ContributionState> =
        serde_json::from_str(include_str!("fixtures/contribution.json")).unwrap();
    let mut endpoint = Endpoint::new(&case);
    endpoint.reply["answers"]
        .as_object_mut()
        .unwrap()
        .remove("jev-02-e-scope");
    assert!(matches!(
        contribute(&endpoint, &case.state),
        Err(NativeDecisionError::InvalidReply(_))
    ));
}

#[test]
fn invalid_capture_and_wrong_model_fail_before_dispatch() {
    let mut case: Recording<VerificationState> =
        serde_json::from_str(include_str!("fixtures/verification.json")).unwrap();
    let mut endpoint = Endpoint::new(&case);
    endpoint.model = "general-model";
    assert!(matches!(
        verify(&endpoint, &case.state),
        Err(NativeDecisionError::WrongModel)
    ));
    endpoint.model = NATIVE_DECISION_MODEL;
    case.state.backcheck_hits[0].before = "é".repeat(4096);
    assert!(matches!(
        verify(&endpoint, &case.state),
        Err(NativeDecisionError::InvalidRequest(_))
    ));

    let mut case: Recording<ContributionState> =
        serde_json::from_str(include_str!("fixtures/contribution.json")).unwrap();
    case.state.existing_observation = "a".repeat(64 * 1024);
    assert!(matches!(
        contribute(&endpoint, &case.state),
        Err(NativeDecisionError::InvalidRequest(_))
    ));
    case.state.existing_observation = "A currently applies.".into();
    case.state.evidence_hits = [
        ("hit".into(), "Exact source text.".into()),
        ("hit-time".into(), "Another exact source.".into()),
    ]
    .into();
    assert!(matches!(
        contribute(&endpoint, &case.state),
        Err(NativeDecisionError::InvalidRequest(_))
    ));
    assert_eq!(endpoint.calls(), 0);
}
