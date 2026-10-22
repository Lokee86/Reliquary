use crate::entity_candidate_test_support::{
    entity_draft, install_rel_mention, publish_rel_memory, temp_path,
};
use crate::{Cva, EntityResolutionReason};

#[test]
fn reports_zero_degree_and_surface_collisions() {
    let mut rel = Cva::create_project(temp_path("entity-audit.rel")).unwrap();
    let memory = publish_rel_memory(&mut rel, "audit", "School", "Eastwood is a school.");
    let key = install_rel_mention(&mut rel, memory, "Eastwood");
    let first = rel
        .publish_entity(None, 0, entity_draft("Eastwood", &[], "first", 1))
        .unwrap()
        .0;
    let _second = rel
        .publish_entity(None, 0, entity_draft("east-wood", &[], "second", 2))
        .unwrap()
        .0;
    rel.set_entity_association(memory, first.id, true, rel.graph_version())
        .unwrap();
    rel.put_entity_resolved(key, 0, first.id, EntityResolutionReason::ContextMatch, 3)
        .unwrap();

    let report = rel.audit_entities();
    assert_eq!(report.entity_count, 2);
    assert_eq!(report.association_count, 1);
    assert_eq!(report.zero_degree_entities.len(), 1);
    assert_eq!(report.normalized_surface_collisions.len(), 1);
    assert!(!report.is_clean());
}

#[test]
fn stale_pending_candidate_requires_explicit_repair_after_reopen() {
    let path = temp_path("entity-audit-stale-pending.rel");
    let mut rel = Cva::create_project(&path).unwrap();
    let memory = publish_rel_memory(
        &mut rel,
        "stale-pending",
        "Server",
        "The game server owns live gameplay.",
    );
    let key = install_rel_mention(&mut rel, memory, "game server");
    let survivor = rel
        .publish_entity(None, 0, entity_draft("game server", &[], "survivor", 1))
        .unwrap()
        .0;
    let retired = rel
        .publish_entity(
            None,
            0,
            entity_draft("services/game-server", &[], "retired", 2),
        )
        .unwrap()
        .0;

    rel.put_entity_unresolved(
        key,
        0,
        vec![retired.id],
        EntityResolutionReason::Ambiguous,
        [1; 32],
        [2; 32],
        3,
    )
    .unwrap();

    // Simulate a legacy/incomplete merge that retired the Entity without
    // retargeting the pending resolution candidate.
    rel.entities
        .tombstone(
            &mut rel.container,
            retired.id,
            retired.revision,
            survivor.id,
        )
        .unwrap();

    let report = rel.audit_entities();
    assert_eq!(report.missing_candidate_targets.len(), 1);
    assert!(!report.is_clean());

    rel.sync().unwrap();
    let before = rel.container.physical_digest().unwrap();
    drop(rel);

    let mut reopened = Cva::open(&path).unwrap();
    assert_eq!(reopened.container.physical_digest().unwrap(), before);
    assert_eq!(reopened.entity_resolution(key).unwrap().revision, 1);
    assert_eq!(
        reopened.entity_resolution_maintenance().unwrap(),
        crate::EntityResolutionMaintenance::RetiredEntityReferences { revisions: 1 }
    );
    assert_eq!(reopened.repair_retired_entity_references().unwrap(), 1);
    drop(reopened);

    let reopened = Cva::open(&path).unwrap();
    let resolution = reopened.entity_resolution(key).unwrap();
    assert_eq!(resolution.revision, 2);
    assert!(matches!(
        &resolution.status,
        crate::MemoryEntityResolutionStatus::Pending(value)
            if value.candidate_entity_ids == vec![survivor.id]
                && value.candidate_set_fingerprint == [0; 32]
    ));
    assert!(
        reopened
            .audit_entities()
            .missing_candidate_targets
            .is_empty()
    );
}
