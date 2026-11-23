use super::{IdentityOutcome, IdentityPairEvidence, Trial, pilot_cases, start_trial};
use crate::identity_trial::grounded_citations;
use reliquary_perception_store::identity::IdentitySide;
use reliquary_switchboard::{
    ConfiguredRoutes, DecisionConsumer, DecisionEndpoint, DecisionEndpointError, GeneralEndpoint,
    GeneralEndpointError,
};
use serde_json::{Value, json};
use std::path::Path;

struct QuoteFixture<'a>(&'a IdentityPairEvidence);

impl GeneralEndpoint for QuoteFixture<'_> {
    fn model(&self) -> &str {
        "quote-fixture"
    }

    fn complete_json(
        &self,
        _prompt: &str,
        payload: &str,
        schema_name: &str,
        _schema: &Value,
    ) -> Result<Value, GeneralEndpointError> {
        assert_eq!(schema_name, "identity_trial_interpret");
        assert_eq!(Some(payload), self.0.model_input());
        let items: Vec<_> = grounded_citations(self.0)
            .into_iter()
            .map(|citation| {
                let side = match citation.side {
                    IdentitySide::Left => "left",
                    IdentitySide::Right => "right",
                };
                json!({
                    "side": side,
                    "relationship": citation.relationship,
                    "memory": citation.memory,
                    "excerpt": self.0.quote(citation).expect("grounded source quote"),
                    "interpretation": "The excerpt is the full source quote."
                })
            })
            .collect();
        Ok(json!({"items": items}))
    }
}

struct FixedProposal<'a> {
    endpoint: &'a dyn DecisionEndpoint,
    verdict: &'static str,
}

impl DecisionEndpoint for FixedProposal<'_> {
    fn model(&self) -> &str {
        self.endpoint.model()
    }

    fn evaluate(&self, state: &Value, questions: &Value) -> Result<Value, DecisionEndpointError> {
        if questions.get("verdict").is_some() {
            return Ok(json!({"answers": {"verdict": {"type": "choice", "choice": self.verdict}}}));
        }
        assert_eq!(state["verdict"], self.verdict);
        let reply = self.endpoint.evaluate(state, questions)?;
        for item in questions.as_object().expect("witness questions").keys() {
            let answer = &reply["answers"][item];
            let choice = match answer["choice"].as_str() {
                Some("supports") => "yes",
                Some("does_not_support") => "no",
                _ => "invalid",
            };
            let probabilities = &answer["probabilities"];
            eprintln!(
                "  {item}: {choice}, yes={:?} no={:?}",
                probabilities["supports"].as_f64(),
                probabilities["does_not_support"].as_f64()
            );
        }
        Ok(reply)
    }
}

/// Conditional attribution diagnostic with supplied quotes and provisional verdicts.
/// Only the final attribution batch is sent to the configured provider.
#[test]
#[ignore = "uses the native Entity decision provider with fixed synthetic proposals"]
fn configured_support_on_fixed_proposals() {
    let path = std::env::var_os("RELIQUARY_IDENTITY_TRIAL_CONFIG")
        .expect("select a configured trial profile");
    let routes = ConfiguredRoutes::from_path(Path::new(&path)).expect("open trial routes");
    let selection = routes.readiness();
    assert!(
        selection.contains("decision selection: ")
            && (selection.contains("entity_resolution=entity_resolution_decision,")
                || selection.contains("entity_resolution=decision,")),
        "a native Entity decision route is required"
    );
    let endpoint = routes
        .decision_endpoint(DecisionConsumer::EntityResolution)
        .expect("configured Entity decision route");
    eprintln!(
        "Fixed provisional choices, attribution only via {}",
        endpoint.model()
    );
    let mut errors = 0;
    let mut mismatches = 0;
    for (case, desired, pair) in pilot_cases() {
        let verdict = match case {
            "marker" => "same_identity",
            "side-by-side" | "version" => "related_distinct",
            _ => continue,
        };
        eprintln!("{case}: proposed={verdict} desired={desired:?}");
        let general = QuoteFixture(&pair);
        let decision = FixedProposal {
            endpoint: endpoint.as_ref(),
            verdict,
        };
        let observed = start_trial(&pair, &general, &decision).and_then(|trial| {
            let Trial::Support(prepared) = trial else {
                panic!("the fixed proposal must reach attribution");
            };
            prepared.evaluate()
        });
        match observed {
            Ok(judgment) => {
                eprintln!(
                    "  outcome={:?} citations={:?}",
                    judgment.outcome, judgment.citations
                );
                mismatches += usize::from(judgment.outcome != desired);
                if judgment.outcome != IdentityOutcome::Unresolved {
                    assert_eq!(judgment.citations.len(), 2);
                }
            }
            Err(error) => {
                errors += 1;
                eprintln!("  outcome unavailable: {error:?}");
            }
        }
    }
    assert_eq!(errors, 0, "attribution request failures");
    assert_eq!(
        mismatches, 0,
        "attribution outcome differs from labeled evidence"
    );
}
