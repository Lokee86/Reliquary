use super::*;
use crate::{evaluate_identity_pair, model_evaluator::identity_evaluator::tests::side};
use reliquary_perception_store::identity::{
    IdentityCitation, IdentityOutcome, IdentityReason, IdentitySide,
};
use reliquary_switchboard::{
    DecisionEndpoint, DecisionEndpointError, GeneralEndpoint, GeneralEndpointError,
};
use serde_json::{Value, json};
use std::{collections::VecDeque, sync::Mutex};

const BASELINE: &str = "entity_identity_comparison";
const INTERPRET: &str = "identity_trial_interpret";
const DECIDE: &str = "verdict";
const SUPPORT: &str = "witnesses";
const ANCHOR: &str = "Project Meridian Contract";
const LEFT_QUOTE: &str = "Alex Chen signed Project Meridian Contract in April.";
const RIGHT_QUOTE: &str = "The register shows Alex Chen signed Project Meridian Contract.";
const LEFT_CITATION: IdentityCitation = IdentityCitation::Relationship {
    side: IdentitySide::Left,
    relationship: 0,
    memory: 0,
};
const RIGHT_CITATION: IdentityCitation = IdentityCitation::Relationship {
    side: IdentitySide::Right,
    relationship: 0,
    memory: 0,
};

type Reply = Result<Value, &'static str>;

#[derive(Default)]
struct Scripted {
    replies: Mutex<VecDeque<Reply>>,
    calls: Mutex<Vec<(&'static str, Value)>>,
}

impl Scripted {
    fn new(replies: impl IntoIterator<Item = Reply>) -> Self {
        Self {
            replies: Mutex::new(replies.into_iter().collect()),
            ..Self::default()
        }
    }

    fn reply(&self) -> Reply {
        self.replies
            .lock()
            .unwrap()
            .pop_front()
            .expect("unexpected endpoint call")
    }

    fn assert_calls(&self, expected: &[&str]) {
        let calls = self.calls.lock().unwrap();
        assert_eq!(
            calls.iter().map(|(name, _)| *name).collect::<Vec<_>>(),
            expected
        );
        assert!(self.replies.lock().unwrap().is_empty());
    }
}

impl GeneralEndpoint for Scripted {
    fn model(&self) -> &str {
        "identity-trial-fixture"
    }

    fn complete_json(
        &self,
        prompt: &str,
        payload: &str,
        schema_name: &str,
        schema: &Value,
    ) -> Result<Value, GeneralEndpointError> {
        assert!(!prompt.trim().is_empty());
        assert_eq!(schema["type"], "object");
        let name = [BASELINE, INTERPRET]
            .into_iter()
            .find(|name| *name == schema_name)
            .unwrap_or_else(|| panic!("unexpected schema {schema_name}"));
        let payload = serde_json::from_str(payload).expect("valid JSON payload");
        self.calls.lock().unwrap().push((name, payload));
        self.reply()
            .map_err(|error| GeneralEndpointError::Failure(error.into()))
    }
}

impl DecisionEndpoint for Scripted {
    fn model(&self) -> &str {
        "identity-trial-decision-fixture"
    }

    fn evaluate(&self, state: &Value, questions: &Value) -> Result<Value, DecisionEndpointError> {
        let definitions = questions.as_object().expect("typed choice questions");
        let name = if definitions.contains_key("verdict") {
            assert_eq!(definitions.len(), 1);
            DECIDE
        } else {
            let count = state["items"].as_array().unwrap().len();
            assert_eq!(definitions.len(), count);
            assert!((0..count).all(|index| definitions.contains_key(&format!("witness_{index}"))));
            SUPPORT
        };
        for definition in definitions.values() {
            assert_eq!(definition["type"], "choice");
            assert!(
                definition["instructions"]
                    .as_str()
                    .is_some_and(|value| !value.trim().is_empty())
            );
        }
        let expected_criteria = if name == DECIDE {
            &["related_distinct", "same_identity", "unresolved"][..]
        } else {
            &["does_not_support", "supports"][..]
        };
        for definition in definitions.values() {
            assert_eq!(
                definition["criteria"]
                    .as_object()
                    .unwrap()
                    .keys()
                    .map(String::as_str)
                    .collect::<Vec<_>>(),
                expected_criteria
            );
        }
        self.calls
            .lock()
            .unwrap()
            .push((name, json!({"state": state, "questions": questions})));
        self.reply()
            .map_err(|error| DecisionEndpointError::Failure(error.into()))
    }
}

fn fixture(left: &str, right: &str) -> IdentityPairEvidence {
    let left = side(1, 1, left);
    let mut right = side(2, 2, right);
    right.relationships[0].memories[0].reference.id = [8; 32];
    IdentityPairEvidence::new(left, right)
}

fn interpretation(left: &str, right: &str) -> Value {
    json!({"items": [
        {"side": "left", "relationship": 0, "memory": 0, "excerpt": left, "interpretation": "The first source identifies its referent."},
        {"side": "right", "relationship": 0, "memory": 0, "excerpt": right, "interpretation": "The second source identifies its referent."}
    ]})
}

fn selection(choice: &str) -> Value {
    json!({"answers": {"verdict": {"type": "choice", "choice": choice}}})
}

fn attribution(left: bool, right: bool) -> Value {
    let choice = |supports| {
        if supports {
            "supports"
        } else {
            "does_not_support"
        }
    };
    json!({"answers": {
        "witness_0": {"type": "choice", "choice": choice(left)},
        "witness_1": {"type": "choice", "choice": choice(right)}
    }})
}

fn baseline_answer(left: &str, right: &str) -> Value {
    json!({"outcome": "same_identity", "reason": "corroborated", "citations": [
        {"side": "left", "relationship": 0, "memory": 0, "fact": left, "bearing": "corroborates_identity"},
        {"side": "right", "relationship": 0, "memory": 0, "fact": right, "bearing": "corroborates_identity"},
    ]})
}

#[test]
fn supported_pair_preserves_grounded_citations_in_plain_and_scored_modes() {
    let pair = fixture(LEFT_QUOTE, RIGHT_QUOTE);
    let baseline = Scripted::new([Ok(baseline_answer(ANCHOR, ANCHOR))]);
    let expected = evaluate_identity_pair(&pair, &baseline).unwrap();
    baseline.assert_calls(&[BASELINE]);
    assert_eq!(expected.outcome, IdentityOutcome::SameIdentity);
    assert_eq!(expected.reason, IdentityReason::Corroborated);
    assert_eq!(expected.citations, [LEFT_CITATION, RIGHT_CITATION]);
    assert_eq!(pair.quote(LEFT_CITATION), Some(LEFT_QUOTE));
    assert_eq!(pair.quote(RIGHT_CITATION), Some(RIGHT_QUOTE));
    let (_, left, _) = pair.witness(LEFT_CITATION).unwrap();
    let (_, right, _) = pair.witness(RIGHT_CITATION).unwrap();
    assert_ne!(left.reference.id, right.reference.id);
    assert_ne!(left.lineage, right.lineage);

    let mut scored = attribution(true, true);
    scored["answers"]["witness_0"]["probabilities"] =
        json!({"supports": 0.08, "does_not_support": 0.02});
    scored["answers"]["witness_1"]["probabilities"] =
        json!({"supports": 0.25, "does_not_support": 0.15});
    for reply in [attribution(true, true), scored] {
        let endpoint = Scripted::new([
            Ok(interpretation(ANCHOR, ANCHOR)),
            Ok(selection("same_identity")),
            Ok(reply),
        ]);
        let Trial::Support(prepared) = start_trial(&pair, &endpoint, &endpoint).unwrap() else {
            panic!("selected outcome should reach attribution");
        };
        assert_eq!(endpoint.calls.lock().unwrap().len(), 2);
        assert_eq!(prepared.evaluate().unwrap(), expected);
        endpoint.assert_calls(&[INTERPRET, DECIDE, SUPPORT]);
        let calls = endpoint.calls.lock().unwrap();
        let raw: Value = serde_json::from_str(pair.model_input().unwrap()).unwrap();
        assert_eq!(calls[0].1, raw);
        let selection_state = &calls[1].1["state"];
        let support_state = &calls[2].1["state"];
        assert_eq!(selection_state["evidence"], raw);
        assert_eq!(support_state["evidence"], raw);
        assert_eq!(selection_state["items"], support_state["items"]);
        assert_eq!(support_state["verdict"], "same_identity");
        assert!(selection_state.get("verdict").is_none());
        let items = selection_state["items"].as_array().unwrap();
        assert_eq!(items.len(), 2);
        for (index, side) in ["left", "right"].into_iter().enumerate() {
            assert_eq!(items[index]["item"], index);
            assert_eq!(items[index]["side"], side);
            assert_eq!(items[index]["relationship"], 0);
            assert_eq!(items[index]["memory"], 0);
            assert_eq!(items[index]["excerpt"], ANCHOR);
            assert!(items[index]["source"].is_u64());
            assert!(items[index]["lineage"].is_array());
        }
        assert_ne!(items[0]["source"], items[1]["source"]);
        assert_ne!(items[0]["lineage"], items[1]["lineage"]);
    }
}

#[test]
fn scores_rank_independent_support_without_changing_the_verdict() {
    let pair = fixture(LEFT_QUOTE, RIGHT_QUOTE);
    let mut right = pair.right.clone();
    let mut extra = right.relationships[0].memories[0].clone();
    extra.reference.id = [9; 32];
    extra.lineage = vec![[3; 32]];
    right.relationships[0].memories.push(extra);
    let pair = IdentityPairEvidence::new(pair.left, right);

    let mut interpreted = interpretation(ANCHOR, ANCHOR);
    let mut extra = interpreted["items"][1].clone();
    extra["memory"] = json!(1);
    interpreted["items"].as_array_mut().unwrap().push(extra);
    let mut plain = attribution(true, true);
    plain["answers"]["witness_2"] = json!({"type": "choice", "choice": "supports"});
    let mut scored = plain.clone();
    scored["answers"]["witness_1"]["probabilities"] = json!({"supports": 0.2});
    scored["answers"]["witness_2"]["probabilities"] = json!({"supports": 0.8});

    for (reply, expected) in [
        (plain, RIGHT_CITATION),
        (
            scored,
            IdentityCitation::Relationship {
                memory: 1,
                side: IdentitySide::Right,
                relationship: 0,
            },
        ),
    ] {
        let endpoint = Scripted::new([
            Ok(interpreted.clone()),
            Ok(selection("same_identity")),
            Ok(reply),
        ]);
        let Trial::Support(prepared) = start_trial(&pair, &endpoint, &endpoint).unwrap() else {
            panic!("selected outcome should reach attribution");
        };
        let judgment = prepared.evaluate().unwrap();
        assert_eq!(judgment.outcome, IdentityOutcome::SameIdentity);
        assert_eq!(judgment.citations, [LEFT_CITATION, expected]);
        endpoint.assert_calls(&[INTERPRET, DECIDE, SUPPORT]);
    }
}

#[test]
fn affirmative_selection_requires_support_on_both_sides() {
    let pair = fixture(LEFT_QUOTE, RIGHT_QUOTE);
    for (choice, witnesses) in [
        ("same_identity", attribution(true, false)),
        ("related_distinct", attribution(false, false)),
    ] {
        let endpoint = Scripted::new([
            Ok(interpretation(ANCHOR, ANCHOR)),
            Ok(selection(choice)),
            Ok(witnesses),
        ]);
        let Trial::Support(prepared) = start_trial(&pair, &endpoint, &endpoint).unwrap() else {
            panic!("affirmative selection should still be evaluated for support");
        };
        let judgment = prepared.evaluate().unwrap();
        assert_eq!(judgment.outcome, IdentityOutcome::Unresolved);
        assert_eq!(judgment.reason, IdentityReason::Ambiguity);
        assert!(judgment.citations.is_empty());
        endpoint.assert_calls(&[INTERPRET, DECIDE, SUPPORT]);
    }
}

#[test]
fn version_only_claims_stay_unresolved_when_attribution_rejects_them() {
    let pair = fixture(
        "Alex Chen used the v2 release.",
        "The register lists the v2 release.",
    );
    let baseline = Scripted::new([Ok(baseline_answer("v2", "v2"))]);
    let current = evaluate_identity_pair(&pair, &baseline).unwrap();
    baseline.assert_calls(&[BASELINE]);
    assert_eq!(current.outcome, IdentityOutcome::Unresolved);

    let endpoint = Scripted::new([
        Ok(interpretation("v2", "v2")),
        Ok(selection("same_identity")),
        Ok(attribution(false, false)),
    ]);
    let Trial::Support(prepared) = start_trial(&pair, &endpoint, &endpoint).unwrap() else {
        panic!("affirmative selection should still be evaluated for support");
    };
    let trial = prepared.evaluate().unwrap();
    assert_eq!(trial.outcome, current.outcome);
    endpoint.assert_calls(&[INTERPRET, DECIDE, SUPPORT]);
}

#[test]
#[ignore = "comparison experiment with deliberately mistaken attribution"]
fn observe_version_only_false_affirmative() {
    let pair = fixture(
        "Alex Chen used the v2 release.",
        "The register lists the v2 release.",
    );
    let baseline = Scripted::new([Ok(baseline_answer("v2", "v2"))]);
    let current = evaluate_identity_pair(&pair, &baseline).unwrap();
    baseline.assert_calls(&[BASELINE]);
    let endpoint = Scripted::new([
        Ok(interpretation("v2", "v2")),
        Ok(selection("same_identity")),
        Ok(attribution(true, true)),
    ]);
    let Trial::Support(prepared) = start_trial(&pair, &endpoint, &endpoint).unwrap() else {
        panic!("affirmative selection should still be evaluated for support");
    };
    let trial = prepared.evaluate().unwrap();
    endpoint.assert_calls(&[INTERPRET, DECIDE, SUPPORT]);
    eprintln!(
        "Version-only evidence with mistaken attribution. Baseline: {current:?}. Trial: {trial:?}."
    );
}

#[test]
fn invented_or_omitted_evidence_stops_before_selection() {
    let pair = fixture(LEFT_QUOTE, RIGHT_QUOTE);
    let mut omitted = interpretation(ANCHOR, ANCHOR);
    omitted["items"].as_array_mut().unwrap().pop();
    for result in [
        interpretation("a fact absent from the source", ANCHOR),
        omitted,
    ] {
        let endpoint = Scripted::new([Ok(result)]);
        assert!(matches!(
            start_trial(&pair, &endpoint, &endpoint),
            Err(TrialError::Malformed(Stage::Interpret))
        ));
        endpoint.assert_calls(&[INTERPRET]);
    }
}

#[test]
fn uncertain_selection_completes_without_attribution() {
    let pair = fixture(LEFT_QUOTE, RIGHT_QUOTE);
    let endpoint = Scripted::new([
        Ok(interpretation(ANCHOR, ANCHOR)),
        Ok(selection("unresolved")),
    ]);
    let Trial::Complete(judgment) = start_trial(&pair, &endpoint, &endpoint).unwrap() else {
        panic!("unresolved selection should skip attribution");
    };
    assert_eq!(judgment.outcome, IdentityOutcome::Unresolved);
    endpoint.assert_calls(&[INTERPRET, DECIDE]);
}

#[test]
fn invalid_or_unavailable_selector_leaves_trial_pending() {
    let pair = fixture(LEFT_QUOTE, RIGHT_QUOTE);
    for (reply, transport) in [
        (Ok(selection("maybe")), false),
        (Err("provider detail"), true),
    ] {
        let endpoint = Scripted::new([Ok(interpretation(ANCHOR, ANCHOR)), reply]);
        let result = start_trial(&pair, &endpoint, &endpoint);
        if transport {
            assert!(matches!(result, Err(TrialError::Endpoint(Stage::Decide))));
        } else {
            assert!(matches!(result, Err(TrialError::Malformed(Stage::Decide))));
        }
        endpoint.assert_calls(&[INTERPRET, DECIDE]);
    }
}

#[test]
fn attribution_retry_reuses_the_same_evidence_and_selection() {
    let pair = fixture(LEFT_QUOTE, RIGHT_QUOTE);
    let endpoint = Scripted::new([
        Ok(interpretation(ANCHOR, ANCHOR)),
        Ok(selection("same_identity")),
        Err("provider detail"),
        Ok(json!({"answers": {"witness_0": {"type": "choice", "choice": "supports"}}})),
        Ok(attribution(true, true)),
    ]);
    let Trial::Support(prepared) = start_trial(&pair, &endpoint, &endpoint).unwrap() else {
        panic!("selected outcome should reach attribution");
    };
    assert!(matches!(
        prepared.evaluate(),
        Err(TrialError::Endpoint(Stage::Support))
    ));
    assert!(matches!(
        prepared.evaluate(),
        Err(TrialError::Malformed(Stage::Support))
    ));
    assert_eq!(
        prepared.evaluate().unwrap().citations,
        [LEFT_CITATION, RIGHT_CITATION]
    );
    endpoint.assert_calls(&[INTERPRET, DECIDE, SUPPORT, SUPPORT, SUPPORT]);
    let calls = endpoint.calls.lock().unwrap();
    assert_eq!(calls[2].1, calls[3].1);
    assert_eq!(calls[2].1, calls[4].1);
}

#[test]
fn shared_provenance_completes_locally() {
    let pair = IdentityPairEvidence::new(side(1, 1, LEFT_QUOTE), side(2, 1, RIGHT_QUOTE));
    let endpoint = Scripted::default();
    let baseline = evaluate_identity_pair(&pair, &endpoint).unwrap();
    let Trial::Complete(judgment) = start_trial(&pair, &endpoint, &endpoint).unwrap() else {
        panic!("preflight should skip the endpoint");
    };
    assert_eq!(judgment, baseline);
    assert_eq!(judgment.reason, IdentityReason::Ambiguity);
    endpoint.assert_calls(&[]);
}
