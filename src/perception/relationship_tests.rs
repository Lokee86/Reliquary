use crate::relationship_test_support::{draft, entity, temp_path};
use crate::{
    Cva, EntityId, EntityRef, MemoryId, MemoryRef, Phylactery, RelationshipDraft,
    RelationshipError, RelationshipParticipant,
};

#[test]
fn relationship_owner_reopens_with_cross_owner_references() {
    let path = temp_path("relationship-owner.rel");
    let mut rel = Cva::create_project(&path).unwrap();
    let local_owner = rel.owner_id().unwrap();
    let local = entity(&mut rel, "Local", 1);
    let external_entity = EntityId([7; 32]);

    let (relationship, created) = rel
        .publish_relationship(
            None,
            0,
            RelationshipDraft {
                kind: " collaboration ".into(),
                participants: vec![
                    RelationshipParticipant {
                        entity: EntityRef {
                            owner_id: local_owner.clone(),
                            entity_id: local,
                        },
                        role: Some(" maintainer ".into()),
                    },
                    RelationshipParticipant {
                        entity: EntityRef {
                            owner_id: "phy-11111111-1111-1111-1111-111111111111".into(),
                            entity_id: external_entity,
                        },
                        role: None,
                    },
                ],
                evidence: vec![MemoryRef {
                    owner_id: "rel-22222222-2222-2222-2222-222222222222".into(),
                    memory_id: MemoryId([8; 32]),
                }],
                summary: " Shared work. ".into(),
                mutation_id: "relationship-test-create".into(),
                created_at_ns: 1,
                updated_at_ns: 1,
            },
        )
        .unwrap();
    assert!(created);
    assert_eq!(relationship.kind, "collaboration");
    let local_participant = relationship
        .participants
        .iter()
        .find(|participant| {
            participant.entity.owner_id == local_owner && participant.entity.entity_id == local
        })
        .unwrap();
    assert_eq!(local_participant.role.as_deref(), Some("maintainer"));

    rel.sync().unwrap();
    drop(rel);

    let reopened = Cva::open(&path).unwrap();
    assert_eq!(
        reopened.relationship(relationship.id).unwrap(),
        relationship
    );
    assert_eq!(reopened.relationship_stats().relationships, 1);
    assert_eq!(reopened.relationship_version(), 1);
}

#[test]
fn same_relationship_identity_is_independent_across_owners() {
    let rel_path = temp_path("relationship-independent.rel");
    let phy_path = temp_path("relationship-independent.phy");
    let mut rel = Cva::create_project(&rel_path).unwrap();
    let mut phy = Phylactery::create(&phy_path).unwrap();
    let participants = vec![
        RelationshipParticipant {
            entity: EntityRef {
                owner_id: "rel-aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa".into(),
                entity_id: EntityId([1; 32]),
            },
            role: Some("source".into()),
        },
        RelationshipParticipant {
            entity: EntityRef {
                owner_id: "phy-bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb".into(),
                entity_id: EntityId([2; 32]),
            },
            role: Some("target".into()),
        },
    ];

    let rel_relationship = rel
        .publish_relationship(
            None,
            0,
            draft(participants.clone(), "shared-id", "REL view"),
        )
        .unwrap()
        .0;
    let phy_relationship = phy
        .publish_relationship(None, 0, draft(participants, "shared-id", "PHY view"))
        .unwrap()
        .0;

    assert_eq!(rel_relationship.id, phy_relationship.id);
    assert_ne!(rel_relationship.summary, phy_relationship.summary);
    assert_eq!(rel.relationships().len(), 1);
    assert_eq!(phy.relationships().len(), 1);
}

#[test]
fn local_participant_must_resolve_in_relationship_owner() {
    let path = temp_path("relationship-local-validation.rel");
    let mut rel = Cva::create_project(&path).unwrap();
    let owner_id = rel.owner_id().unwrap();
    let result = rel.publish_relationship(
        None,
        0,
        draft(
            vec![RelationshipParticipant {
                entity: EntityRef {
                    owner_id,
                    entity_id: EntityId([99; 32]),
                },
                role: None,
            }],
            "missing-local",
            "invalid",
        ),
    );
    assert!(matches!(
        result,
        Err(RelationshipError::MissingLocalEntity(id)) if id == EntityId([99; 32])
    ));
}

#[test]
fn relationship_store_keeps_typed_participant_while_owner_rejects_missing_local_entity_atomically()
{
    let direct_path = temp_path("relationship-store-local.rel");
    let mut direct = Cva::create_project(&direct_path).unwrap();
    let owner_id = direct.owner_id().unwrap();
    let missing_entity = EntityId([98; 32]);
    let direct_value = direct
        .relationships
        .publish(
            &mut direct.container,
            None,
            0,
            draft(
                vec![RelationshipParticipant {
                    entity: EntityRef {
                        owner_id: owner_id.clone(),
                        entity_id: missing_entity,
                    },
                    role: None,
                }],
                "store-local-missing-target",
                "direct owner fixture",
            ),
        )
        .unwrap()
        .0;
    assert_eq!(
        direct.relationships.relationship(direct_value.id).unwrap(),
        direct_value
    );

    let composed_path = temp_path("relationship-caller-atomic.rel");
    let mut composed = Cva::create_project(&composed_path).unwrap();
    let before = composed.container.latest_version();
    let owner_id = composed.owner_id().unwrap();
    assert!(matches!(
        composed.publish_relationship(
            None,
            0,
            draft(
                vec![RelationshipParticipant {
                    entity: EntityRef {
                        owner_id,
                        entity_id: missing_entity,
                    },
                    role: None,
                }],
                "caller-rejects-missing-target",
                "must not publish",
            ),
        ),
        Err(RelationshipError::MissingLocalEntity(id)) if id == missing_entity
    ));
    assert_eq!(composed.container.latest_version(), before);
    assert!(composed.relationships().is_empty());
}
