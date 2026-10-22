use crate::cva_reconcile_graph_test_support::{create_workspace, diverge_archive, test_dir};
use crate::entity_principal::{PRINCIPAL_ENTITY_KIND, principal_entity_id};
use crate::{Cva, EntityDraft, GraphRelationOrigin, SemanticGraphRelationKind};
use std::fs;

#[test]
fn reconcile_replays_right_only_entity_association() {
    let dir = test_dir();
    let left = dir.join("left.cva");
    let right = dir.join("right.cva");
    let output = dir.join("merged.cva");
    let ids = create_workspace(&left, &["a"]);
    fs::copy(&left, &right).unwrap();

    let mut left_cva = Cva::open(&left).unwrap();
    diverge_archive(&mut left_cva, "left");
    left_cva.sync().unwrap();

    let mut right_cva = Cva::open(&right).unwrap();
    diverge_archive(&mut right_cva, "right");
    let (entity, _) = right_cva
        .publish_entity(
            None,
            0,
            EntityDraft {
                canonical_name: "Reliquary".into(),
                aliases: vec!["REL".into()],
                kind: "project".into(),
                summary: "Project referent.".into(),
                mutation_id: "reconcile-entity".into(),
                created_at_ns: 1,
                updated_at_ns: 1,
            },
        )
        .unwrap();
    right_cva
        .set_entity_association(ids[0], entity.id, true, right_cva.graph_version())
        .unwrap();
    right_cva.sync().unwrap();
    drop((left_cva, right_cva));

    let result = Cva::reconcile(&left, &right, &output).unwrap();
    assert_eq!(result.replayed_entity_revisions, 1);
    assert_eq!(result.replayed_graph_transactions, 1);
    assert_eq!(result.replayed_graph_mutations, 1);

    let merged = Cva::open(output).unwrap();
    assert_eq!(
        merged.entity(entity.id).unwrap().canonical_name,
        "Reliquary"
    );
    assert_eq!(
        merged.entity_associations_for_memory(ids[0]),
        vec![entity.id]
    );
    let relation = merged
        .semantic_graph_relations()
        .into_iter()
        .find(|relation| relation.kind == SemanticGraphRelationKind::EntityAssociation)
        .unwrap();
    assert_eq!(relation.origin, GraphRelationOrigin::Perception);
}

#[test]
fn reconcile_replays_right_only_principal_association_without_exposing_it_generically() {
    let dir = test_dir();
    let left = dir.join("left-principal.cva");
    let right = dir.join("right-principal.cva");
    let output = dir.join("merged-principal.cva");
    let ids = create_workspace(&left, &["a"]);
    fs::copy(&left, &right).unwrap();

    let mut left_cva = Cva::open(&left).unwrap();
    diverge_archive(&mut left_cva, "left-principal");
    left_cva.sync().unwrap();

    let mut right_cva = Cva::open(&right).unwrap();
    diverge_archive(&mut right_cva, "right-principal");
    let principal = "phy-00000000-0000-0000-0000-000000000321";
    let entity_id = principal_entity_id(principal);
    right_cva
        .publish_entity(
            Some(entity_id),
            0,
            EntityDraft {
                canonical_name: principal.into(),
                aliases: vec!["User 321".into()],
                kind: PRINCIPAL_ENTITY_KIND.into(),
                summary: "User principal.".into(),
                mutation_id: "reconcile-principal".into(),
                created_at_ns: 1,
                updated_at_ns: 1,
            },
        )
        .unwrap();
    right_cva
        .set_principal_association(ids[0], entity_id, true, right_cva.graph_version())
        .unwrap();
    right_cva.sync().unwrap();
    drop((left_cva, right_cva));

    let result = Cva::reconcile(&left, &right, &output).unwrap();
    assert_eq!(result.replayed_entity_revisions, 1);
    assert_eq!(result.replayed_graph_transactions, 1);
    assert_eq!(result.replayed_graph_mutations, 1);

    let merged = Cva::open(output).unwrap();
    assert_eq!(merged.principal_entity(principal).unwrap().id, entity_id);
    assert_eq!(
        merged.principal_associations_for_memory(ids[0]),
        vec![entity_id]
    );
    assert_eq!(merged.memories_for_phy_principal(principal), vec![ids[0]]);
    assert!(merged.semantic_graph_relations().is_empty());
}
