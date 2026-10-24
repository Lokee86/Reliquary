use crate::entity_candidate_test_support::{
    entity_draft, install_rel_mention, publish_rel_memory, temp_path,
};
use crate::entity_resolution_processor_test_support::BootstrapEndpoint;
use crate::{Cva, EntityCandidateConfig, EntityResolver};
use std::sync::atomic::Ordering;

#[test]
fn unresolved_same_surface_context_retries_only_when_evidence_changes() {
    let mut rel = Cva::create_project(temp_path("surface-wake.rel")).unwrap();
    let ambiguous = publish_rel_memory(
        &mut rel,
        "surface-ambiguous",
        "Entrypoint",
        "main.go is an ambiguous executable entry point.",
    );
    let key = install_rel_mention(&mut rel, ambiguous, "main.go");
    let (endpoint, calls) = BootstrapEndpoint::new();
    let resolver = EntityResolver::new(endpoint);

    rel.resolve_entity_mention(&resolver, key, EntityCandidateConfig::default(), 50)
        .unwrap();
    rel.resolve_entity_mention(&resolver, key, EntityCandidateConfig::default(), 51)
        .unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 1);

    let context = publish_rel_memory(
        &mut rel,
        "surface-context",
        "Server entrypoint",
        "The server repository uses main.go as its namespaced executable entry point.",
    );
    install_rel_mention(&mut rel, context, "main.go");
    assert!(
        rel.pending_entity_mentions_for_surface("MAIN.GO")
            .contains(&key)
    );

    rel.resolve_entity_mention(&resolver, key, EntityCandidateConfig::default(), 52)
        .unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    rel.resolve_entity_mention(&resolver, key, EntityCandidateConfig::default(), 53)
        .unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[test]
fn unresolved_candidate_retries_after_candidate_evidence_changes() {
    let mut rel = Cva::create_project(temp_path("candidate-wake.rel")).unwrap();
    let entity_id = rel
        .publish_entity(
            None,
            0,
            entity_draft("Helix", &[], "candidate-wake-entity", 1),
        )
        .unwrap()
        .0
        .id;
    let ambiguous = publish_rel_memory(
        &mut rel,
        "candidate-ambiguous",
        "Ambiguous tool",
        "Helix remains ambiguous in this context.",
    );
    let key = install_rel_mention(&mut rel, ambiguous, "Helix");
    let (endpoint, calls) = BootstrapEndpoint::new();
    let resolver = EntityResolver::new(endpoint);

    rel.resolve_entity_mention(&resolver, key, EntityCandidateConfig::default(), 60)
        .unwrap();
    assert!(
        rel.pending_entity_mentions_for_candidate(entity_id)
            .contains(&key)
    );
    rel.resolve_entity_mention(&resolver, key, EntityCandidateConfig::default(), 61)
        .unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 1);

    let support = publish_rel_memory(
        &mut rel,
        "candidate-support",
        "Editor",
        "Helix is the code editor used for Rust work.",
    );
    rel.set_entity_association(support, entity_id, true, rel.graph_version())
        .unwrap();

    rel.resolve_entity_mention(&resolver, key, EntityCandidateConfig::default(), 62)
        .unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    rel.resolve_entity_mention(&resolver, key, EntityCandidateConfig::default(), 63)
        .unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[test]
fn prepared_resolution_cannot_commit_to_a_different_owner() {
    let mut source = Cva::create_project(temp_path("owner-source.rel")).unwrap();
    let memory = publish_rel_memory(
        &mut source,
        "owner-source",
        "Editor",
        "Helix is the code editor installed for Rust work.",
    );
    let key = install_rel_mention(&mut source, memory, "Helix");
    let (endpoint, _) = BootstrapEndpoint::new();
    let resolver = EntityResolver::new(endpoint);
    let prepared = match source
        .prepare_entity_resolution(key, EntityCandidateConfig::default())
        .unwrap()
    {
        crate::EntityResolutionPreparation::Ready(value) => value,
        crate::EntityResolutionPreparation::Complete(_) => panic!("expected prepared work"),
    };
    let evaluation = resolver.core.evaluate_prepared(&prepared).unwrap();

    let mut other = Cva::create_project(temp_path("owner-other.rel")).unwrap();
    assert_ne!(source.owner_id(), other.owner_id());
    assert!(
        other
            .commit_entity_resolution(prepared, evaluation, 80)
            .unwrap()
            .is_none()
    );
    assert!(other.entities().is_empty());
}

#[test]
fn stale_prepared_resolution_is_rejected_before_persistence() {
    let mut rel = Cva::create_project(temp_path("stale-prepared.rel")).unwrap();
    let memory = publish_rel_memory(
        &mut rel,
        "stale-source",
        "Editor",
        "Helix is the code editor installed for Rust work.",
    );
    let key = install_rel_mention(&mut rel, memory, "Helix");
    let (endpoint, _) = BootstrapEndpoint::new();
    let resolver = EntityResolver::new(endpoint);
    let prepared = match rel
        .prepare_entity_resolution(key, EntityCandidateConfig::default())
        .unwrap()
    {
        crate::EntityResolutionPreparation::Ready(value) => value,
        crate::EntityResolutionPreparation::Complete(_) => panic!("expected prepared work"),
    };
    let evaluation = resolver.core.evaluate_prepared(&prepared).unwrap();

    publish_rel_memory(
        &mut rel,
        "stale-intervening",
        "Unrelated",
        "An unrelated Memory changes the owner Memory version.",
    );

    assert!(
        rel.commit_entity_resolution(prepared, evaluation, 70)
            .unwrap()
            .is_none()
    );
    assert!(rel.entity_resolution(key).is_none());
    assert!(rel.entities().is_empty());
}
