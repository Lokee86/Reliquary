mod fixed;
mod signed_in;

use super::{Trial, start_trial};
use crate::{
    IdentityPairEvidence, evaluate_identity_pair, model_evaluator::identity_evaluator::tests::side,
};
use reliquary_perception_store::identity::IdentityOutcome;
use reliquary_switchboard::{
    ConfiguredRoutes, DecisionConsumer, DecisionEndpoint, DecisionEndpointError,
    GeneralDecisionEndpoint, GeneralEndpoint,
};
use serde_json::Value;

struct ObservedDecision<'a>(&'a dyn DecisionEndpoint);
impl DecisionEndpoint for ObservedDecision<'_> {
    fn model(&self) -> &str {
        self.0.model()
    }
    fn evaluate(&self, state: &Value, questions: &Value) -> Result<Value, DecisionEndpointError> {
        let reply = self.0.evaluate(state, questions)?;
        if let Some(items) = questions.as_object() {
            for key in items.keys() {
                let answer = &reply["answers"][key];
                if let Some(choice) = answer["choice"]
                    .as_str()
                    .filter(|choice| items[key]["criteria"].get(choice).is_some())
                {
                    eprintln!(
                        "  {key} = {choice}, score {:?}",
                        answer["probabilities"][choice].as_f64()
                    );
                }
            }
        }
        Ok(reply)
    }
}
use std::{path::Path, time::Instant};

// Each source is authored for this pilot. Labels are used only after both evaluators run.
fn sample(name: &str, kind: &str, context: &str, left: &str, right: &str) -> IdentityPairEvidence {
    let witness = |owner, quote| {
        let mut evidence = side(owner, owner, quote);
        evidence.endpoint.canonical_name = name.into();
        evidence.endpoint.kind = kind.into();
        evidence.endpoint.aliases.clear();
        let rel = &mut evidence.relationships[0];
        rel.kind = "recorded_in".into();
        rel.participants[0].entity = evidence.endpoint.clone();
        rel.participants[0].roles = vec!["subject".into()];
        rel.participants[1].entity.canonical_name = context.into();
        rel.participants[1].entity.kind = "record".into();
        rel.participants[1].roles = vec!["reference".into()];
        evidence
    };
    IdentityPairEvidence::new(witness(1, left), witness(2, right))
}

fn pilot_runtime() -> (ConfiguredRoutes, std::sync::Arc<dyn GeneralEndpoint>) {
    let path = std::env::var_os("RELIQUARY_IDENTITY_TRIAL_CONFIG")
        .expect("select a configured trial profile");
    let routes = ConfiguredRoutes::from_path(Path::new(&path)).expect("open trial routes");
    let mut general = routes.entity_endpoint().expect("configured Entity route");
    if let Some(executable) = std::env::var_os("RELIQUARY_TRIAL_CODEX") {
        let scratch = Path::new(
            &std::env::var_os("RELIQUARY_TRIAL_SCRATCH").expect("trial scratch directory"),
        )
        .canonicalize()
        .expect("resolve trial scratch directory");
        general = std::sync::Arc::new(signed_in::SignedInSol {
            executable: executable.into(),
            scratch,
            model: general.model().to_owned(),
        });
        eprintln!("Interpretation transport: signed-in Codex client");
    }
    (routes, general)
}

#[test]
#[ignore = "uses the selected provider routes with synthetic evidence"]
fn configured_models_on_authored_pairs() {
    let (routes, general) = pilot_runtime();
    let decision = routes
        .decision_endpoint(DecisionConsumer::EntityResolution)
        .expect("configured Entity decision route");
    compare_authored_pairs(general.as_ref(), decision.as_ref());
}

#[test]
#[ignore = "uses the same live general model for every stage with synthetic evidence"]
fn same_model_on_authored_pairs() {
    let (_, general) = pilot_runtime();
    let decision = GeneralDecisionEndpoint::new(general.clone());
    compare_authored_pairs(general.as_ref(), &decision);
}

fn compare_authored_pairs(general: &dyn GeneralEndpoint, decision: &dyn DecisionEndpoint) {
    eprintln!("Models: {} / {}", general.model(), decision.model());
    let cases = pilot_cases();
    let mut staged_correct = 0;
    for (case, label, pair) in cases {
        eprintln!("Case {case}");
        let started = Instant::now();
        let baseline = evaluate_identity_pair(&pair, general);
        let baseline_time = started.elapsed();
        let started = Instant::now();
        let observed = ObservedDecision(decision);
        let staged = start_trial(&pair, general, &observed).and_then(|step| match step {
            Trial::Complete(judgment) => Ok(judgment),
            Trial::Support(support) => support.evaluate(),
        });
        eprintln!(
            "{case}: expected={label:?} current={:?} ({baseline_time:?}) staged={:?} ({:?})",
            baseline.as_ref().map(|j| (j.outcome, &j.citations)),
            staged.as_ref().map(|j| (j.outcome, &j.citations)),
            started.elapsed()
        );
        staged_correct += usize::from(staged.is_ok_and(|j| j.outcome == label));
    }
    assert_eq!(
        staged_correct, 4,
        "the proposed evaluator missed a labeled case or a route failed"
    );
}

fn pilot_cases() -> [(&'static str, IdentityOutcome, IdentityPairEvidence); 4] {
    use IdentityOutcome::{RelatedDistinct, SameIdentity, Unresolved};
    [
        (
            "marker",
            SameIdentity,
            sample(
                "South Beacon",
                "place",
                "Survey Monument K17",
                "The register identifies South Beacon as the summit at Survey Monument K17.",
                "The field survey locates the summit known as South Beacon at Survey Monument K17.",
            ),
        ),
        (
            "side-by-side",
            RelatedDistinct,
            sample(
                "Relay",
                "application",
                "Depot Inventory",
                "Depot Inventory lists two installed programs called Relay. This entry identifies the Relay audio editor, installed beside the backup utility.",
                "Depot Inventory records both packages under Relay. This entry identifies the Relay backup utility installed alongside the audio editor.",
            ),
        ),
        (
            "version",
            Unresolved,
            sample(
                "Relay",
                "application",
                "Depot Inventory",
                "Depot Inventory lists Relay version 1.4 on the workstation in March.",
                "Depot Inventory lists Relay version 2.0 on the workstation in September.",
            ),
        ),
        (
            "conflict",
            Unresolved,
            sample(
                "Alex Chen",
                "person",
                "Town Records",
                "Town Records give Alex Chen's birthplace as Meridian City.",
                "Town Records give Alex Chen's birthplace as Lantern City.",
            ),
        ),
    ]
}
