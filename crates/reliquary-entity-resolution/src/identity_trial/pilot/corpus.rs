use super::{
    GeneralDecisionEndpoint, IdentityOutcome, IdentityPairEvidence, compare_authored_pairs,
    evaluate_identity_pair, pilot_runtime, sample,
};
use crate::{EntityResolverError, model_evaluator::identity_evaluator::tests::Scripted};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

const REGRESSIONS: &str = include_str!("regression.json");
const HELD_OUT: &str = include_str!("held_out.json");
const CORROBORATION: &str = include_str!("corroboration.json");

// One-call response.json files from the first attempt of each held-out case in
// the three 2026-11-23 Sol batches, matched by the one-call schema and run order.
const HELD_OUT_ONE_CALL_REPLIES: &str = include_str!("held_out_one_call_replies.json");

#[test]
fn replays_held_out_one_call_first_attempts() {
    let replies: BTreeMap<String, Value> =
        serde_json::from_str(HELD_OUT_ONE_CALL_REPLIES).expect("valid replay fixture");
    assert_eq!(replies.len(), 24);
    let mut evidence: BTreeMap<_, _> = cases(HELD_OUT)
        .into_iter()
        .map(|(id, label, pair)| (id, (label, pair)))
        .collect();
    let mut correct = 0;
    for (id, output) in replies {
        let (label, pair) = evidence
            .remove(&id)
            .unwrap_or_else(|| panic!("missing evidence for captured reply {id}"));
        let result = evaluate_identity_pair(&pair, &Scripted::reply(output))
            .map(|judgment| judgment.outcome);
        match id.as_str() {
            "briar" => assert!(
                matches!(
                    result,
                    Err(EntityResolverError::InvalidOutput(ref message))
                        if message == "duplicate identity citation"
                ),
                "{id}: {result:?}"
            ),
            _ => {
                assert_eq!(result.expect(&id), label, "{id}");
                correct += 1;
            }
        }
    }
    assert_eq!(correct, 23);
}

fn cases(source: &str) -> Vec<(String, IdentityOutcome, IdentityPairEvidence)> {
    let rows: Vec<Value> = serde_json::from_str(source).expect("valid identity fixture array");
    rows.into_iter()
        .map(|row| {
            let field = |key: &str| {
                row[key]
                    .as_str()
                    .filter(|v| !v.trim().is_empty())
                    .unwrap_or_else(|| panic!("missing {key} in identity fixture {}", row["id"]))
            };
            let label = match field("expected") {
                "same_identity" => IdentityOutcome::SameIdentity,
                "related_distinct" => IdentityOutcome::RelatedDistinct,
                "unresolved" => IdentityOutcome::Unresolved,
                other => panic!("unknown identity label {other}"),
            };
            field("rationale");
            let pair = sample(
                field("kind"),
                (field("left_name"), field("left_context"), field("left")),
                (field("right_name"), field("right_context"), field("right")),
            );
            (field("id").to_owned(), label, pair)
        })
        .collect()
}

#[test]
fn broader_fixtures_have_unique_grounded_inputs() {
    let mut ids = BTreeSet::new();
    let mut inputs = BTreeSet::new();
    for source in [REGRESSIONS, HELD_OUT, CORROBORATION] {
        let examples = cases(source);
        assert!(!examples.is_empty(), "empty trial corpus");
        for (id, _, pair) in examples {
            assert!(ids.insert(id.clone()), "duplicate case {id}");
            let input = pair.model_input().expect("bounded trial input").to_string();
            assert!(inputs.insert(input), "duplicate evidence in {id}");
            assert!(
                crate::deterministic_identity_judgment(&pair).is_none(),
                "{id} never reaches the model"
            );
        }
    }
}

fn select_cases(examples: &mut Vec<(String, IdentityOutcome, IdentityPairEvidence)>) {
    if let Ok(selection) = std::env::var("RELIQUARY_TRIAL_CASES") {
        let chosen: BTreeSet<_> = selection.split(',').map(str::trim).collect();
        examples.retain(|(id, _, _)| chosen.contains(id.as_str()));
        assert_eq!(
            examples.len(),
            chosen.len(),
            "unknown trial case in {selection}"
        );
    }
}

fn run_same_model(source: &str) {
    let (_, general) = pilot_runtime();
    let decision = GeneralDecisionEndpoint::new(general.clone());
    let mut examples = cases(source);
    select_cases(&mut examples);
    let scores = compare_authored_pairs(general.as_ref(), &decision, examples);
    assert_eq!(
        (scores.baseline_errors, scores.staged_errors),
        (0, 0),
        "incomplete paired trial"
    );
    assert_eq!(
        (scores.baseline_correct, scores.staged_correct),
        (scores.cases, scores.cases),
        "completed evaluators missed a labeled case"
    );
}

#[test]
#[ignore = "uses the selected live Entity route once per synthetic pair"]
fn one_call_on_synthetic_pairs() {
    let mut examples = cases(HELD_OUT);
    examples.extend(cases(REGRESSIONS));
    examples.extend(cases(CORROBORATION));
    select_cases(&mut examples);
    let (_, general) = pilot_runtime();
    let count = examples.len();
    let mut correct = 0;
    let mut errors = 0;
    for (id, label, pair) in examples {
        let result = evaluate_identity_pair(&pair, general.as_ref());
        eprintln!("{id}: expected={label:?} one_call={result:?}");
        correct += usize::from(
            result
                .as_ref()
                .is_ok_and(|judgment| judgment.outcome == label),
        );
        errors += usize::from(result.is_err());
    }
    eprintln!("Cases={count} one_call_correct={correct} one_call_errors={errors}");
    assert_eq!(errors, 0, "incomplete one-call trial");
    assert_eq!(correct, count, "one-call evaluator missed a labeled case");
}

#[test]
#[ignore = "uses live Sol on existing synthetic regression examples"]
fn same_model_on_regression_pairs() {
    run_same_model(REGRESSIONS);
}

#[test]
#[ignore = "uses live Sol on unseen synthetic pairs"]
fn same_model_on_held_out_pairs() {
    run_same_model(HELD_OUT);
}
