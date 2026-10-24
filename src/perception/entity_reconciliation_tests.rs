use crate::entity_candidate_test_support::{entity_draft, publish_rel_memory, temp_path};
use crate::{Cva, EntityId, SimulatedGeneralEndpoint};
use serde_json::json;

fn publish(rel: &mut Cva, name: &str, summary: &str, seed: i64) -> EntityId {
    rel.publish_entity(None, 0, entity_draft(name, &[], summary, seed))
        .unwrap()
        .0
        .id
}

#[test]
fn exact_identity_collision_is_model_verified_before_merge() {
    let mut rel = Cva::create_project(temp_path("reconcile-exact.rel")).unwrap();
    let a = publish(&mut rel, "Eastwood", "A school.", 1);
    let b = publish(&mut rel, "eastwood", "The same school.", 2);
    let left = publish_rel_memory(&mut rel, "left", "School", "Eastwood workshop.");
    let right = publish_rel_memory(&mut rel, "right", "School", "Eastwood event.");
    rel.set_entity_association(left, a, true, rel.graph_version())
        .unwrap();
    rel.set_entity_association(right, b, true, rel.graph_version())
        .unwrap();

    let endpoint =
        SimulatedGeneralEndpoint::new("reconciler", vec![json!({"relation":"same_identity"})]);
    let report = rel.reconcile_entities(&endpoint, 100).unwrap();

    assert_eq!(report.deterministic_merges, 0);
    assert_eq!(report.model_merges, 1);
    assert_eq!(rel.entities().len(), 1);
}

#[test]
fn lexical_identity_pair_can_be_merged_by_model() {
    let mut rel = Cva::create_project(temp_path("reconcile-model.rel")).unwrap();
    let a = publish(
        &mut rel,
        "family garden",
        "The family's continuing backyard vegetable garden.",
        1,
    );
    let b = publish(
        &mut rel,
        "backyard garden",
        "The continuing family garden with raised beds.",
        2,
    );
    let left = publish_rel_memory(
        &mut rel,
        "left",
        "Garden",
        "The family garden has tomatoes.",
    );
    let right = publish_rel_memory(
        &mut rel,
        "right",
        "Garden",
        "The backyard garden has raised beds.",
    );
    rel.set_entity_association(left, a, true, rel.graph_version())
        .unwrap();
    rel.set_entity_association(right, b, true, rel.graph_version())
        .unwrap();

    let endpoint =
        SimulatedGeneralEndpoint::new("reconciler", vec![json!({"relation":"same_identity"})]);
    let report = rel.reconcile_entities(&endpoint, 100).unwrap();

    assert_eq!(report.model_merges, 1);
    assert_eq!(rel.entities().len(), 1);
}

#[test]
fn failed_reconsideration_restores_rejected_mention_after_reopen() {
    use crate::entity_candidate_test_support::install_rel_mention;
    use crate::{EntityResolutionReason, MemoryEntityResolutionStatus};
    let path = temp_path("reconsider-failure.rel");
    let mut rel = Cva::create_project(&path).unwrap();
    let memory = publish_rel_memory(
        &mut rel,
        "late-school",
        "School",
        "Eastwood is the durable school attended by the family.",
    );
    let key = install_rel_mention(&mut rel, memory, "Eastwood");
    rel.put_entity_rejected(key, 0, EntityResolutionReason::GenericRole, 10)
        .unwrap();
    let mut draft = entity_draft("Eastwood", &[], "late-school-entity", 20);
    draft.created_at_ns = 20;
    rel.publish_entity(None, 0, draft).unwrap();
    let endpoint =
        SimulatedGeneralEndpoint::new("invalid-resolver", vec![json!({"unexpected":true})]);
    assert!(rel.reconcile_entities(&endpoint, 100).is_err());
    assert!(matches!(
        rel.entity_resolution(key).unwrap().status,
        MemoryEntityResolutionStatus::Rejected {
            reason: EntityResolutionReason::GenericRole
        }
    ));
    rel.sync().unwrap();
    drop(rel);
    let rel = Cva::open(path).unwrap();
    assert!(matches!(
        rel.entity_resolution(key).unwrap().status,
        MemoryEntityResolutionStatus::Rejected {
            reason: EntityResolutionReason::GenericRole
        }
    ));
}

#[test]
fn reset_failure_returns_without_attempting_rejection_restoration() {
    use crate::entity_candidate_test_support::install_rel_mention;
    use crate::{EntityResolutionReason, MemoryEntityResolutionStatus};
    let path = temp_path("reconsider-read-only.rel");
    let mut rel = Cva::create_project(&path).unwrap();
    let memory = publish_rel_memory(
        &mut rel,
        "late-school-read-only",
        "School",
        "Eastwood is the durable school attended by the family.",
    );
    let key = install_rel_mention(&mut rel, memory, "Eastwood");
    rel.put_entity_rejected(key, 0, EntityResolutionReason::GenericRole, 10)
        .unwrap();
    let mut draft = entity_draft("Eastwood", &[], "late-read-only-entity", 20);
    draft.created_at_ns = 20;
    rel.publish_entity(None, 0, draft).unwrap();
    rel.sync().unwrap();
    drop(rel);
    let before = std::fs::read(&path).unwrap();
    let mut rel = Cva::open_read_only(&path).unwrap();
    let endpoint = SimulatedGeneralEndpoint::new("unused-resolver", vec![]);
    let error = rel.reconcile_entities(&endpoint, 100).unwrap_err();
    assert!(!error.to_string().contains("failed to restore rejection"));
    assert!(matches!(
        rel.entity_resolution(key).unwrap().status,
        MemoryEntityResolutionStatus::Rejected {
            reason: EntityResolutionReason::GenericRole
        }
    ));
    assert_eq!(std::fs::read(&path).unwrap(), before);
}
