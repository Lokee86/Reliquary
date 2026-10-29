use crate::dream_candidate_test_support::test_path;
use crate::dream_canonical_test_support::{authority_anchor, draft_from, publish};
use crate::{Cva, GraphRelationKind};

#[test]
fn direct_authoritative_decision_promotes_immediately_to_canonical() {
    let mut cva = Cva::create(test_path("canonical-direct.cva")).unwrap();
    let anchor = authority_anchor(&mut cva, "direct", 100);
    let source = publish(
        &mut cva,
        "direct",
        &anchor,
        "decision",
        "project",
        "direct",
        "extracted",
        "Use fibre-cement siding.",
    );

    let result = cva.reconcile_dream_lifecycle(source).unwrap();

    assert_eq!(result.source.lifecycle_state, "canonical");
    assert_eq!(result.canonicalized, vec![source]);
    assert!(!result.promoted_to_knowledge);
}

#[test]
fn ordinary_direct_fact_stays_knowledge_until_corroborated() {
    let mut cva = Cva::create(test_path("canonical-fact.cva")).unwrap();
    let anchor = authority_anchor(&mut cva, "fact", 100);
    let source = publish(
        &mut cva,
        "fact",
        &anchor,
        "fact",
        "identity",
        "direct",
        "extracted",
        "The client is Acme.",
    );

    let result = cva.reconcile_dream_lifecycle(source).unwrap();

    assert_eq!(result.source.lifecycle_state, "knowledge");
    assert!(result.canonicalized.is_empty());
    assert!(result.promoted_to_knowledge);
}

#[test]
fn independent_duplicate_authority_promotes_active_representative() {
    let mut cva = Cva::create(test_path("canonical-corroborated.cva")).unwrap();
    let first_anchor = authority_anchor(&mut cva, "first", 100);
    let second_anchor = authority_anchor(&mut cva, "second", 200);
    let first = publish(
        &mut cva,
        "first",
        &first_anchor,
        "fact",
        "identity",
        "direct",
        "knowledge",
        "The client is Acme.",
    );
    let second = publish(
        &mut cva,
        "second",
        &second_anchor,
        "fact",
        "identity",
        "direct",
        "extracted",
        "The client is Acme.",
    );
    cva.set_memory_relation(second, first, GraphRelationKind::DuplicateOf, true, 0)
        .unwrap();

    let result = cva.reconcile_dream_lifecycle(second).unwrap();

    assert!(result.source.archived);
    assert_eq!(cva.memory(first).unwrap().lifecycle_state, "canonical");
    assert_eq!(result.canonicalized, vec![first]);
}

#[test]
fn corroborated_representative_uses_post_archive_duplicate_members() {
    let mut cva = Cva::create(test_path("canonical-post-archive-representative.cva")).unwrap();
    let first_anchor = authority_anchor(&mut cva, "post-archive-first", 100);
    let source_anchor = authority_anchor(&mut cva, "post-archive-source", 200);
    let third_anchor = authority_anchor(&mut cva, "post-archive-third", 300);
    let first = publish(
        &mut cva,
        "post-archive-first",
        &first_anchor,
        "fact",
        "identity",
        "direct",
        "knowledge",
        "The client is Acme.",
    );
    let source = publish(
        &mut cva,
        "post-archive-source",
        &source_anchor,
        "fact",
        "identity",
        "direct",
        "extracted",
        "The client is Acme.",
    );
    let third = publish(
        &mut cva,
        "post-archive-third",
        &third_anchor,
        "fact",
        "identity",
        "direct",
        "knowledge",
        "The client is Acme.",
    );
    cva.set_memory_relation(source, first, GraphRelationKind::DuplicateOf, true, 0)
        .unwrap();
    cva.set_memory_relation(
        third,
        first,
        GraphRelationKind::DuplicateOf,
        true,
        cva.graph_version(),
    )
    .unwrap();
    cva.set_memory_relation(
        source,
        first,
        GraphRelationKind::Supersedes,
        true,
        cva.graph_version(),
    )
    .unwrap();

    let result = cva.reconcile_dream_lifecycle(source).unwrap();

    assert!(cva.memory(first).unwrap().archived);
    assert!(result.source.archived);
    assert_eq!(cva.memory(third).unwrap().lifecycle_state, "canonical");
    assert_eq!(result.canonicalized, vec![third]);
}

#[test]
fn duplicate_memories_from_same_authority_anchor_do_not_corroborate() {
    let mut cva = Cva::create(test_path("canonical-same-anchor.cva")).unwrap();
    let anchor = authority_anchor(&mut cva, "shared", 100);
    let first = publish(
        &mut cva,
        "same-a",
        &anchor,
        "fact",
        "identity",
        "direct",
        "knowledge",
        "The client is Acme.",
    );
    let second = publish(
        &mut cva,
        "same-b",
        &anchor,
        "fact",
        "identity",
        "direct",
        "extracted",
        "The client is Acme.",
    );
    cva.set_memory_relation(second, first, GraphRelationKind::DuplicateOf, true, 0)
        .unwrap();

    let result = cva.reconcile_dream_lifecycle(second).unwrap();

    assert!(result.source.archived);
    assert_eq!(cva.memory(first).unwrap().lifecycle_state, "knowledge");
    assert!(result.canonicalized.is_empty());
}

#[test]
fn superseding_a_canonical_memory_inherits_canonical_status() {
    let mut cva = Cva::create(test_path("canonical-supersession.cva")).unwrap();
    let old_anchor = authority_anchor(&mut cva, "old", 100);
    let new_anchor = authority_anchor(&mut cva, "new", 200);
    let old = publish(
        &mut cva,
        "old",
        &old_anchor,
        "fact",
        "identity",
        "direct",
        "canonical",
        "The client is Acme.",
    );
    let new = publish(
        &mut cva,
        "new",
        &new_anchor,
        "fact",
        "identity",
        "direct",
        "extracted",
        "The client is Beacon.",
    );
    cva.set_memory_relation(new, old, GraphRelationKind::Supersedes, true, 0)
        .unwrap();

    let result = cva.reconcile_dream_lifecycle(new).unwrap();

    assert_eq!(result.source.lifecycle_state, "canonical");
    assert_eq!(result.canonicalized, vec![new]);
    let old = cva.memory(old).unwrap();
    assert!(old.archived);
    assert_eq!(old.superseded_by, Some(new));
}

#[test]
fn canonical_supersession_recovers_after_target_archival_precedes_promotion() {
    let mut cva = Cva::create(test_path("canonical-supersession-recovery.cva")).unwrap();
    let old_anchor = authority_anchor(&mut cva, "old-recovery", 100);
    let new_anchor = authority_anchor(&mut cva, "new-recovery", 200);
    let old = publish(
        &mut cva,
        "old-recovery",
        &old_anchor,
        "fact",
        "identity",
        "direct",
        "canonical",
        "The client is Acme.",
    );
    let new = publish(
        &mut cva,
        "new-recovery",
        &new_anchor,
        "fact",
        "identity",
        "direct",
        "extracted",
        "The client is Beacon.",
    );
    cva.set_memory_relation(new, old, GraphRelationKind::Supersedes, true, 0)
        .unwrap();

    let old_current = cva.memory(old).unwrap();
    let mut archived = draft_from(&old_current, "archived", "simulate-partial-supersession");
    archived.archived = true;
    archived.superseded_by = Some(new);
    cva.publish_memory(Some(old), old_current.revision, archived)
        .unwrap();

    let result = cva.reconcile_dream_lifecycle(new).unwrap();

    assert_eq!(result.source.lifecycle_state, "canonical");
    assert_eq!(result.canonicalized, vec![new]);
}

#[test]
fn canonical_reconciliation_is_idempotent() {
    let mut cva = Cva::create(test_path("canonical-idempotent.cva")).unwrap();
    let anchor = authority_anchor(&mut cva, "idempotent", 100);
    let source = publish(
        &mut cva,
        "idempotent",
        &anchor,
        "decision",
        "project",
        "direct",
        "extracted",
        "Use fibre-cement siding.",
    );
    cva.reconcile_dream_lifecycle(source).unwrap();
    let memory_version = cva.memory_version();

    let second = cva.reconcile_dream_lifecycle(source).unwrap();

    assert!(second.canonicalized.is_empty());
    assert!(second.revised.is_empty());
    assert_eq!(cva.memory_version(), memory_version);
}
