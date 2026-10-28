use crate::{
    DecisionEndpoint, GeneralEndpoint, ReliquaryRuntimeRoutes, SimulatedDecisionEndpoint,
    SimulatedGeneralEndpoint,
};
use std::sync::Arc;

fn endpoint(model: &str) -> Arc<dyn GeneralEndpoint> {
    Arc::new(SimulatedGeneralEndpoint::new(model, Vec::new()))
}

fn decision_endpoint(model: &str) -> Arc<dyn DecisionEndpoint> {
    Arc::new(SimulatedDecisionEndpoint::new(model, Vec::new()))
}

#[test]
fn runtime_routes_keep_inference_capabilities_separate() {
    let routes = ReliquaryRuntimeRoutes::new(
        Some(endpoint("general")),
        Some(endpoint("insomnia")),
        Some(endpoint("metadata")),
        Some(endpoint("dream")),
        None,
    )
    .with_entity_routes(
        Some(endpoint("entity-extraction")),
        Some(endpoint("entity-resolution")),
    )
    .with_entity_resolution_decision(Some(decision_endpoint("entity-decision")))
    .with_temporal_decision(Some(decision_endpoint("chronos")));

    assert_eq!(routes.insomnia().unwrap().model(), "insomnia");
    assert_eq!(routes.insomnia_metadata().unwrap().model(), "metadata");
    assert_eq!(routes.insomnia_ownership().unwrap().model(), "metadata");
    assert_eq!(
        routes.entity_extraction().unwrap().model(),
        "entity-extraction"
    );
    assert_eq!(
        routes.entity_resolution_decision().unwrap().model(),
        "entity-decision"
    );
    assert_eq!(
        routes.entity_resolution().unwrap().model(),
        "entity-resolution"
    );
    assert_eq!(routes.temporal_decision().unwrap().model(), "chronos");
    assert_eq!(routes.dream().unwrap().model(), "dream");
}

#[test]
fn dedicated_routes_do_not_fall_back_to_other_capabilities() {
    let routes = ReliquaryRuntimeRoutes::new(Some(endpoint("general")), None, None, None, None);

    assert_eq!(routes.insomnia().unwrap().model(), "general");
    assert_eq!(routes.insomnia_ownership().unwrap().model(), "general");
    assert_eq!(routes.dream().unwrap().model(), "general");
    assert!(routes.insomnia_metadata().is_none());
    assert!(routes.entity_extraction().is_none());
    assert!(routes.entity_resolution_decision().is_none());
    assert!(routes.entity_resolution().is_none());
    assert!(routes.temporal_decision().is_none());
}
