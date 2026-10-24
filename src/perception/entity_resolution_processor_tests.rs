use crate::entity_candidate_test_support::{
    entity_draft, install_phy_mention, install_rel_mention, publish_phy_memory, publish_rel_memory,
    temp_path,
};
use crate::entity_resolution_processor_test_support::BootstrapEndpoint;
use crate::{
    Cva, EntityCandidateConfig, EntityResolutionDecision, EntityResolutionReason, EntityResolver,
    MemoryEntityResolutionStatus, Phylactery, SimulatedGeneralEndpoint,
};
use serde_json::json;
use std::sync::atomic::Ordering;

#[test]
fn zero_entity_bootstrap_creates_splits_and_converges_after_reopen() {
    let path = temp_path("bootstrap.rel");
    let mut rel = Cva::create_project(&path).unwrap();

    let editor_first = publish_rel_memory(
        &mut rel,
        "editor-first",
        "Editor",
        "Helix is the code editor installed for Rust work.",
    );
    let editor_again = publish_rel_memory(
        &mut rel,
        "editor-again",
        "Editor settings",
        "Helix keybindings should map save to Ctrl-S.",
    );
    let service_first = publish_rel_memory(
        &mut rel,
        "service-first",
        "Telemetry",
        "Helix is the telemetry ingestion service for application events.",
    );
    let service_again = publish_rel_memory(
        &mut rel,
        "service-again",
        "Telemetry queue",
        "Helix telemetry queue latency increased after deployment.",
    );

    let keys = [
        install_rel_mention(&mut rel, editor_first, "Helix"),
        install_rel_mention(&mut rel, editor_again, "Helix"),
        install_rel_mention(&mut rel, service_first, "Helix"),
        install_rel_mention(&mut rel, service_again, "Helix"),
    ];

    let (endpoint, calls) = BootstrapEndpoint::new();
    let resolver = EntityResolver::new(endpoint);
    let config = EntityCandidateConfig::default();

    let first = rel
        .resolve_entity_mention(&resolver, keys[0], config, 10)
        .unwrap();
    assert!(first.entity_created);
    let editor = first.entity_id.unwrap();
    assert_eq!(rel.entities().len(), 1);
    assert_eq!(
        rel.entity_associations_for_memory(editor_first),
        vec![editor]
    );

    let second = rel
        .resolve_entity_mention(&resolver, keys[1], config, 11)
        .unwrap();
    assert_eq!(second.entity_id, Some(editor));
    assert!(!second.entity_created);
    assert_eq!(rel.entities().len(), 1);

    let third = rel
        .resolve_entity_mention(&resolver, keys[2], config, 12)
        .unwrap();
    let service = third.entity_id.unwrap();
    assert!(third.entity_created);
    assert_ne!(service, editor);
    assert_eq!(rel.entities().len(), 2);

    let fourth = rel
        .resolve_entity_mention(&resolver, keys[3], config, 13)
        .unwrap();
    assert_eq!(fourth.entity_id, Some(service));
    assert_eq!(rel.entities().len(), 2);
    assert_eq!(calls.load(Ordering::SeqCst), 5);

    for (key, expected) in keys.into_iter().zip([editor, editor, service, service]) {
        assert!(matches!(
            rel.entity_resolution(key).unwrap().status,
            MemoryEntityResolutionStatus::Resolved { entity_id, .. } if entity_id == expected
        ));
    }

    rel.sync().unwrap();
    drop(rel);
    let reopened = Cva::open(path).unwrap();
    assert_eq!(reopened.entities().len(), 2);
    assert_eq!(
        reopened.entity_associations_for_memory(editor_first),
        vec![editor]
    );
    assert_eq!(
        reopened.entity_associations_for_memory(service_first),
        vec![service]
    );
}

#[test]
fn unresolved_and_rejected_states_are_persisted_and_do_not_repeat_inference() {
    let mut rel = Cva::create_project(temp_path("terminal-and-pending.rel")).unwrap();
    let ambiguous = publish_rel_memory(
        &mut rel,
        "ambiguous",
        "Entrypoint",
        "main.go is an ambiguous executable entry point.",
    );
    let rejected = publish_rel_memory(
        &mut rel,
        "rejected",
        "Role",
        "Operator is not a durable referent in this sentence.",
    );
    let ambiguous_key = install_rel_mention(&mut rel, ambiguous, "main.go");
    let rejected_key = install_rel_mention(&mut rel, rejected, "Operator");

    let (endpoint, calls) = BootstrapEndpoint::new();
    let resolver = EntityResolver::new(endpoint);
    let config = EntityCandidateConfig::default();

    let pending = rel
        .resolve_entity_mention(&resolver, ambiguous_key, config, 20)
        .unwrap();
    assert_eq!(pending.decision, EntityResolutionDecision::Unresolved);
    assert!(pending.resolution_changed);
    assert!(matches!(
        rel.entity_resolution(ambiguous_key).unwrap().status,
        MemoryEntityResolutionStatus::Pending(_)
    ));

    let repeated = rel
        .resolve_entity_mention(&resolver, ambiguous_key, config, 21)
        .unwrap();
    assert_eq!(repeated.decision, EntityResolutionDecision::Unresolved);
    assert!(!repeated.resolution_changed);

    let rejected_outcome = rel
        .resolve_entity_mention(&resolver, rejected_key, config, 22)
        .unwrap();
    assert_eq!(rejected_outcome.decision, EntityResolutionDecision::Reject);
    assert!(matches!(
        rel.entity_resolution(rejected_key).unwrap().status,
        MemoryEntityResolutionStatus::Rejected {
            reason: EntityResolutionReason::GenericRole
        }
    ));

    let repeated_reject = rel
        .resolve_entity_mention(&resolver, rejected_key, config, 23)
        .unwrap();
    assert!(!repeated_reject.resolution_changed);
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[test]
fn admission_rejection_reason_cannot_remain_pending() {
    let mut rel = Cva::create_project(temp_path("admission-terminal-reason.rel")).unwrap();
    let memory = publish_rel_memory(
        &mut rel,
        "logger-role",
        "Logger",
        "project logger writes task diagnostics.",
    );
    let key = install_rel_mention(&mut rel, memory, "project logger");
    let resolver = EntityResolver::new(SimulatedGeneralEndpoint::new(
        "terminal-reason-test",
        vec![json!({
            "decision": "unresolved",
            "reason": "generic_role",
            "promotion_policy": "none",
            "entity_kind": "unknown",
            "entity_summary": ""
        })],
    ));

    let result = rel
        .resolve_entity_mention(&resolver, key, EntityCandidateConfig::default(), 24)
        .unwrap();

    assert_eq!(result.decision, EntityResolutionDecision::Reject);
    assert_eq!(result.reason, EntityResolutionReason::GenericRole);
    assert!(matches!(
        rel.entity_resolution(key).unwrap().status,
        MemoryEntityResolutionStatus::Rejected {
            reason: EntityResolutionReason::GenericRole
        }
    ));
}

#[test]
fn candidate_rejection_reason_cannot_remain_pending() {
    let mut rel = Cva::create_project(temp_path("candidate-terminal-reason.rel")).unwrap();
    rel.publish_entity(
        None,
        0,
        entity_draft("Project Logger", &[], "logger service", 1),
    )
    .unwrap();
    let memory = publish_rel_memory(
        &mut rel,
        "logger-role",
        "Logger",
        "Project Logger writes task diagnostics.",
    );
    let key = install_rel_mention(&mut rel, memory, "Project Logger");
    let resolver = EntityResolver::new(SimulatedGeneralEndpoint::new(
        "terminal-reason-test",
        vec![json!({
            "decision": "unresolved",
            "reason": "generic_role",
            "target_candidate_index": -1,
            "identity_relation": "uncertain",
            "mention_kind": "unknown"
        })],
    ));

    let result = rel
        .resolve_entity_mention(&resolver, key, EntityCandidateConfig::default(), 25)
        .unwrap();

    assert_eq!(result.decision, EntityResolutionDecision::Reject);
    assert_eq!(result.reason, EntityResolutionReason::GenericRole);
    assert!(matches!(
        rel.entity_resolution(key).unwrap().status,
        MemoryEntityResolutionStatus::Rejected {
            reason: EntityResolutionReason::GenericRole
        }
    ));
}

#[test]
fn existing_source_association_recovers_partial_resolution_write() {
    let mut rel = Cva::create_project(temp_path("association-recovery.rel")).unwrap();
    let memory = publish_rel_memory(
        &mut rel,
        "recovery",
        "Editor restart",
        "Zephyr editor restart should restore the previous workspace.",
    );
    let key = install_rel_mention(&mut rel, memory, "Zephyr");

    let entity_id = rel
        .publish_entity(
            None,
            0,
            entity_draft("Editor Product", &[], "partial-entity", 1),
        )
        .unwrap()
        .0
        .id;
    rel.set_entity_association(memory, entity_id, true, rel.graph_version())
        .unwrap();

    let endpoint = SimulatedGeneralEndpoint::new(
        "resolver-test",
        vec![json!({
            "decision": "resolve_existing",
            "reason": "context_match",
            "target_candidate_index": 0
        })],
    );
    let resolver = EntityResolver::new(endpoint);
    let result = rel
        .resolve_entity_mention(&resolver, key, EntityCandidateConfig::default(), 30)
        .unwrap();

    assert_eq!(result.entity_id, Some(entity_id));
    assert!(!result.association_changed);
    assert!(result.resolution_changed);
}

#[test]
fn phylactery_uses_same_zero_entity_create_path() {
    let mut phy = Phylactery::create(temp_path("bootstrap.phy")).unwrap();
    let memory = publish_phy_memory(&mut phy, "Zephyr is a durable local editor.");
    let key = install_phy_mention(&mut phy, memory, "Zephyr");
    let resolver = EntityResolver::new(SimulatedGeneralEndpoint::new(
        "resolver-test",
        vec![json!({
            "decision": "create_new",
            "reason": "first_seen_identity",
            "promotion_policy": "immediate",
            "entity_kind": "tool",
            "entity_summary": "The Zephyr local editor."
        })],
    ));

    let result = phy
        .resolve_entity_mention(&resolver, key, EntityCandidateConfig::default(), 40)
        .unwrap();

    let entity_id = result.entity_id.unwrap();
    assert!(result.entity_created);
    assert_eq!(phy.entities().len(), 1);
    assert_eq!(phy.entity_associations_for_memory(memory), vec![entity_id]);
    assert!(matches!(
        phy.entity_resolution(key).unwrap().status,
        MemoryEntityResolutionStatus::Resolved { entity_id: id, .. } if id == entity_id
    ));
}

#[test]
fn transient_historical_occurrences_are_rejected_before_model_inference() {
    let mut rel = Cva::create_project(temp_path("transient-occurrence.rel")).unwrap();
    let memory = publish_rel_memory(
        &mut rel,
        "commit-memory",
        "Rollback",
        "Rollback to commit 6ed8e7a is final.",
    );
    let key = install_rel_mention(&mut rel, memory, "commit 6ed8e7a");

    let bogus = rel
        .publish_entity(
            None,
            0,
            entity_draft("commit 6ed8e7a", &[], "bad-commit-entity", 1),
        )
        .unwrap()
        .0
        .id;

    let resolver = EntityResolver::new(SimulatedGeneralEndpoint::new(
        "must-not-be-called",
        Vec::new(),
    ));
    let result = rel
        .resolve_entity_mention(&resolver, key, EntityCandidateConfig::default(), 50)
        .unwrap();

    assert_eq!(result.decision, EntityResolutionDecision::Reject);
    assert_eq!(result.reason, EntityResolutionReason::TransientValue);
    assert_eq!(result.entity_id, None);
    assert!(rel.entity_associations_for_memory(memory).is_empty());
    assert!(matches!(
        rel.entity_resolution(key).unwrap().status,
        MemoryEntityResolutionStatus::Rejected {
            reason: EntityResolutionReason::TransientValue
        }
    ));
    assert_eq!(rel.entity(bogus).unwrap().canonical_name, "commit 6ed8e7a");
}

#[test]
fn confirmed_surface_variant_is_learned_as_entity_alias() {
    let mut rel = Cva::create_project(temp_path("learned-alias.rel")).unwrap();
    let entity_id = rel
        .publish_entity(
            None,
            0,
            entity_draft("PlayerHuePresenter", &[], "presenter-entity", 1),
        )
        .unwrap()
        .0
        .id;
    let memory = publish_rel_memory(
        &mut rel,
        "presenter-use",
        "Presenter",
        "Player Hue Presenter owns player hue presentation.",
    );
    let key = install_rel_mention(&mut rel, memory, "Player Hue Presenter");

    let resolver = EntityResolver::new(SimulatedGeneralEndpoint::new(
        "resolver-test",
        vec![json!({
            "decision": "resolve_existing",
            "reason": "context_match",
            "target_candidate_index": 0
        })],
    ));
    let result = rel
        .resolve_entity_mention(&resolver, key, EntityCandidateConfig::default(), 60)
        .unwrap();

    assert_eq!(result.entity_id, Some(entity_id));
    let entity = rel.entity(entity_id).unwrap();
    assert_eq!(entity.revision, 2);
    assert!(
        entity
            .aliases
            .iter()
            .any(|alias| alias == "Player Hue Presenter")
    );
    assert_eq!(
        rel.entity_candidates_for_surface("Player Hue Presenter", 8)
            .into_iter()
            .map(|entity| entity.id)
            .collect::<Vec<_>>(),
        vec![entity_id]
    );
}

#[test]
fn fine_grained_entity_waits_for_recurrence_then_promotes_and_backfills() {
    let mut rel = Cva::create_project(temp_path("recurrence-promotion.rel")).unwrap();
    let first_memory = publish_rel_memory(
        &mut rel,
        "first-symbol",
        "Random helper",
        "randomRange chooses a spawn offset.",
    );
    let first_key = install_rel_mention(&mut rel, first_memory, "randomRange");

    let endpoint = SimulatedGeneralEndpoint::new(
        "recurrence-test",
        vec![
            json!({
                "decision": "create_new",
                "reason": "first_seen_identity",
                "promotion_policy": "immediate",
                "entity_kind": "code_symbol",
                "entity_summary": "The randomRange helper function."
            }),
            json!({
                "decision": "create_new",
                "reason": "first_seen_identity",
                "promotion_policy": "requires_recurrence",
                "entity_kind": "code_symbol",
                "entity_summary": "The randomRange helper function used for spawn offsets."
            }),
            json!({
                "decision": "resolve_existing",
                "reason": "context_match",
                "target_candidate_index": 0
            }),
        ],
    );
    let resolver = EntityResolver::new(endpoint);
    let config = EntityCandidateConfig::default();

    let first = rel
        .resolve_entity_mention(&resolver, first_key, config, 70)
        .unwrap();
    assert_eq!(first.decision, EntityResolutionDecision::Unresolved);
    assert_eq!(first.reason, EntityResolutionReason::RecurrenceRequired);
    assert_eq!(rel.entities().len(), 0);
    assert!(matches!(
        rel.entity_resolution(first_key).unwrap().status,
        MemoryEntityResolutionStatus::Pending(ref value)
            if value.reason == EntityResolutionReason::RecurrenceRequired
    ));

    let second_memory = publish_rel_memory(
        &mut rel,
        "second-symbol",
        "Random helper reuse",
        "randomRange is also used when selecting asteroid spawn offsets.",
    );
    let second_key = install_rel_mention(&mut rel, second_memory, "randomRange");
    let second = rel
        .resolve_entity_mention(&resolver, second_key, config, 71)
        .unwrap();
    assert!(second.entity_created);
    let entity_id = second.entity_id.unwrap();
    assert_eq!(rel.entities().len(), 1);
    assert_eq!(
        rel.entity_associations_for_memory(second_memory),
        vec![entity_id]
    );

    let backfill = rel
        .resolve_entity_mention(&resolver, first_key, config, 72)
        .unwrap();
    assert_eq!(
        backfill.decision,
        EntityResolutionDecision::ResolveExisting(entity_id)
    );
    assert_eq!(
        rel.entity_associations_for_memory(first_memory),
        vec![entity_id]
    );
}

#[test]
fn candidate_path_create_new_still_enforces_hard_recurrence() {
    let mut rel = Cva::create_project(temp_path("candidate-recurrence-gate.rel")).unwrap();
    let existing = rel
        .publish_entity(
            None,
            0,
            entity_draft("BackgroundMusic", &[], "existing music node", 1),
        )
        .unwrap()
        .0
        .id;
    let memory = publish_rel_memory(
        &mut rel,
        "candidate-symbol",
        "Music binding",
        "background_music_player is a separate code symbol used beside BackgroundMusic.",
    );
    rel.set_entity_association(memory, existing, true, rel.graph_version())
        .unwrap();
    let key = install_rel_mention(&mut rel, memory, "background_music_player");
    let resolver = EntityResolver::new(SimulatedGeneralEndpoint::new(
        "candidate-recurrence-test",
        vec![
            json!({
                "decision": "create_new",
                "reason": "context_conflict_new_identity",
                "target_candidate_index": -1
            }),
            json!({
                "decision": "create_new",
                "reason": "first_seen_identity",
                "promotion_policy": "immediate",
                "entity_kind": "code_symbol",
                "entity_summary": "The background_music_player code symbol."
            }),
        ],
    ));

    let result = rel
        .resolve_entity_mention(&resolver, key, EntityCandidateConfig::default(), 75)
        .unwrap();

    assert_eq!(result.decision, EntityResolutionDecision::Unresolved);
    assert_eq!(result.reason, EntityResolutionReason::RecurrenceRequired);
    assert_eq!(rel.entities().len(), 1);
    assert!(matches!(
        rel.entity_resolution(key).unwrap().status,
        MemoryEntityResolutionStatus::Pending(ref value)
            if value.reason == EntityResolutionReason::RecurrenceRequired
    ));
}

#[test]
fn candidate_path_create_new_can_promote_after_recurrence() {
    let mut rel = Cva::create_project(temp_path("candidate-recurrence-promote.rel")).unwrap();
    let existing = rel
        .publish_entity(
            None,
            0,
            entity_draft("BackgroundMusic", &[], "existing music node", 1),
        )
        .unwrap()
        .0
        .id;
    publish_rel_memory(
        &mut rel,
        "earlier-symbol",
        "Music binding",
        "background_music_player stores the scene music player reference.",
    );
    let memory = publish_rel_memory(
        &mut rel,
        "candidate-symbol",
        "Music binding reuse",
        "background_music_player is reused for playback control beside BackgroundMusic.",
    );
    rel.set_entity_association(memory, existing, true, rel.graph_version())
        .unwrap();
    let key = install_rel_mention(&mut rel, memory, "background_music_player");
    let resolver = EntityResolver::new(SimulatedGeneralEndpoint::new(
        "candidate-recurrence-test",
        vec![
            json!({
                "decision": "create_new",
                "reason": "context_conflict_new_identity",
                "target_candidate_index": -1
            }),
            json!({
                "decision": "create_new",
                "reason": "first_seen_identity",
                "promotion_policy": "requires_recurrence",
                "entity_kind": "code_symbol",
                "entity_summary": "The background_music_player code symbol."
            }),
        ],
    ));

    let result = rel
        .resolve_entity_mention(&resolver, key, EntityCandidateConfig::default(), 76)
        .unwrap();

    assert!(result.entity_created);
    assert_eq!(rel.entities().len(), 2);
}

#[test]
fn first_seen_high_level_entity_can_still_promote_immediately() {
    let mut rel = Cva::create_project(temp_path("immediate-promotion.rel")).unwrap();
    let memory = publish_rel_memory(
        &mut rel,
        "service",
        "Telemetry service",
        "TelemetryHub is the persistent service that owns event ingestion.",
    );
    let key = install_rel_mention(&mut rel, memory, "TelemetryHub");
    let resolver = EntityResolver::new(SimulatedGeneralEndpoint::new(
        "immediate-test",
        vec![json!({
            "decision": "create_new",
            "reason": "first_seen_identity",
            "promotion_policy": "immediate",
            "entity_kind": "service",
            "entity_summary": "The persistent TelemetryHub event-ingestion service."
        })],
    ));

    let result = rel
        .resolve_entity_mention(&resolver, key, EntityCandidateConfig::default(), 80)
        .unwrap();
    assert!(result.entity_created);
    assert_eq!(rel.entities().len(), 1);
}

#[test]
fn conflicting_create_plan_persists_ambiguous_reason() {
    let mut rel = Cva::create_project(temp_path("commit-conflict-reason.rel")).unwrap();
    rel.publish_entity(None, 0, entity_draft("Zephyr", &[], "existing-zephyr", 1))
        .unwrap();
    let memory = publish_rel_memory(
        &mut rel,
        "zephyr-mention",
        "Editor",
        "Zephyr is a durable local editor.",
    );
    let key = install_rel_mention(&mut rel, memory, "Zephyr");
    let crate::EntityResolutionPreparation::Ready(prepared) = rel
        .prepare_entity_resolution(key, EntityCandidateConfig::default())
        .unwrap()
    else {
        panic!("expected pending resolution");
    };
    let outcome = rel
        .commit_entity_resolution(
            prepared,
            crate::EntityResolutionEvaluation {
                output: crate::EntityResolverOutput {
                    decision: EntityResolutionDecision::CreateNew,
                    reason: EntityResolutionReason::ContextConflictNewIdentity,
                },
                materialization: Some(crate::EntityMaterialization {
                    kind: "test".into(),
                    summary: "Conflicting editor identity.".into(),
                }),
            },
            100,
        )
        .unwrap()
        .unwrap();
    assert_eq!(outcome.decision, EntityResolutionDecision::Unresolved);
    assert_eq!(outcome.reason, EntityResolutionReason::Ambiguous);
    assert!(
        matches!(&rel.entity_resolution(key).unwrap().status, MemoryEntityResolutionStatus::Pending(value) if value.reason == EntityResolutionReason::Ambiguous)
    );
    assert_eq!(rel.entities().len(), 1);
}
