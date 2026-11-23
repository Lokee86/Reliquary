use super::{
    GeneralDecisionEndpoint, IdentityOutcome, IdentityPairEvidence, compare_authored_pairs,
    pilot_runtime, sample,
};
use serde_json::Value;
use std::collections::BTreeSet;

const REGRESSIONS: &str = include_str!("regression.json");
const HELD_OUT: &str = include_str!("held_out.json");

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
    for source in [REGRESSIONS, HELD_OUT] {
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

fn run_same_model(source: &str) {
    let (_, general) = pilot_runtime();
    let decision = GeneralDecisionEndpoint::new(general.clone());
    let mut examples = cases(source);
    if let Ok(selection) = std::env::var("RELIQUARY_TRIAL_CASES") {
        let chosen: BTreeSet<_> = selection.split(',').map(str::trim).collect();
        examples.retain(|(id, _, _)| chosen.contains(id.as_str()));
        assert_eq!(
            examples.len(),
            chosen.len(),
            "unknown trial case in {selection}"
        );
    }
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
#[ignore = "uses live Sol on existing synthetic regression examples"]
fn same_model_on_regression_pairs() {
    run_same_model(REGRESSIONS);
}

#[test]
#[ignore = "uses live Sol on unseen synthetic pairs"]
fn same_model_on_held_out_pairs() {
    run_same_model(HELD_OUT);
}
