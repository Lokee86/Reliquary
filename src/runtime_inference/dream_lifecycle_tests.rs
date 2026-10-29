use crate::dream_candidate_test_support::{memory, memory_extracted, test_path};
use crate::{Cva, GraphRelationKind};

#[test]
fn successful_active_extracted_source_advances_to_knowledge() {
    let mut cva = Cva::create(test_path("lifecycle-knowledge.cva")).unwrap();
    let source = memory_extracted(&mut cva, "source", "Source", "Source memory.", 100);

    let result = cva.reconcile_dream_lifecycle(source).unwrap();

    assert!(result.promoted_to_knowledge);
    assert!(result.archived.is_empty());
    assert_eq!(result.source.lifecycle_state, "knowledge");
    assert!(!result.source.archived);
}

#[test]
fn extracted_duplicate_archives_when_component_has_active_representative() {
    let mut cva = Cva::create(test_path("lifecycle-duplicate.cva")).unwrap();
    let representative = memory(&mut cva, "rep", "Same", "Same memory.", 100, false);
    let source = memory_extracted(&mut cva, "dup", "Same", "Same memory.", 110);
    cva.set_memory_relation(
        source,
        representative,
        GraphRelationKind::DuplicateOf,
        true,
        0,
    )
    .unwrap();

    let result = cva.reconcile_dream_lifecycle(source).unwrap();

    assert!(!result.promoted_to_knowledge);
    assert_eq!(result.source.lifecycle_state, "archived");
    assert!(result.source.archived);
    assert_eq!(result.archived, vec![source]);
    assert!(!cva.memory(representative).unwrap().archived);
}

#[test]
fn extracted_duplicate_remains_active_when_all_other_observations_are_archived() {
    let mut cva = Cva::create(test_path("lifecycle-duplicate-root.cva")).unwrap();
    let old = memory(&mut cva, "old", "Same", "Same memory.", 100, true);
    let source = memory_extracted(&mut cva, "source", "Same", "Same memory.", 110);
    cva.set_memory_relation(source, old, GraphRelationKind::DuplicateOf, true, 0)
        .unwrap();

    let result = cva.reconcile_dream_lifecycle(source).unwrap();

    assert!(result.promoted_to_knowledge);
    assert_eq!(result.source.lifecycle_state, "knowledge");
    assert!(!result.source.archived);
}

#[test]
fn supersedes_archives_target_and_projects_unique_superseder() {
    let mut cva = Cva::create(test_path("lifecycle-supersedes.cva")).unwrap();
    let old = memory(&mut cva, "old", "Old", "Old rule.", 100, false);
    let source = memory_extracted(&mut cva, "new", "New", "New rule.", 110);
    cva.set_memory_relation(source, old, GraphRelationKind::Supersedes, true, 0)
        .unwrap();

    let result = cva.reconcile_dream_lifecycle(source).unwrap();
    let old = cva.memory(old).unwrap();

    assert!(result.promoted_to_knowledge);
    assert!(old.archived);
    assert_eq!(old.lifecycle_state, "archived");
    assert_eq!(old.superseded_by, Some(source));
    assert_eq!(result.source.lifecycle_state, "knowledge");
}

#[test]
fn superseded_source_archives_instead_of_advancing_to_knowledge() {
    let mut cva = Cva::create(test_path("lifecycle-source-superseded.cva")).unwrap();
    let source = memory_extracted(&mut cva, "old", "Old", "Old rule.", 100);
    let newer = memory(&mut cva, "new", "New", "New rule.", 110, false);
    cva.set_memory_relation(newer, source, GraphRelationKind::Supersedes, true, 0)
        .unwrap();

    let result = cva.reconcile_dream_lifecycle(source).unwrap();

    assert!(!result.promoted_to_knowledge);
    assert!(result.source.archived);
    assert_eq!(result.source.lifecycle_state, "archived");
    assert_eq!(result.source.superseded_by, Some(newer));
}

#[test]
fn lifecycle_reconciliation_is_idempotent() {
    let mut cva = Cva::create(test_path("lifecycle-idempotent.cva")).unwrap();
    let source = memory_extracted(&mut cva, "source", "Source", "Source memory.", 100);
    let first = cva.reconcile_dream_lifecycle(source).unwrap();
    let memory_version = cva.memory_version();
    let second = cva.reconcile_dream_lifecycle(source).unwrap();

    assert!(first.promoted_to_knowledge);
    assert!(!second.promoted_to_knowledge);
    assert!(second.revised.is_empty());
    assert_eq!(cva.memory_version(), memory_version);
}

#[test]
fn superseded_duplicate_peer_does_not_archive_the_source() {
    let mut cva = Cva::create(test_path("lifecycle-peer-archived-first.cva")).unwrap();
    let old = memory(&mut cva, "old", "Old", "Old memory.", 100, false);
    let source = memory_extracted(&mut cva, "new", "New", "New memory.", 110);
    cva.set_memory_relation(source, old, GraphRelationKind::DuplicateOf, true, 0)
        .unwrap();
    cva.set_memory_relation(
        source,
        old,
        GraphRelationKind::Supersedes,
        true,
        cva.graph_version(),
    )
    .unwrap();
    let result = cva.reconcile_dream_lifecycle(source).unwrap();
    assert!(cva.memory(old).unwrap().archived);
    assert!(!result.source.archived);
    assert_eq!(result.source.lifecycle_state, "knowledge");
}
