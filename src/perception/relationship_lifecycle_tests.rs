use crate::owner_lifecycle::{HistoricalPayload, LifecycleState, TombstoneOutcome};
use crate::{
    Cva, EntityDraft, EntityRef, MemoryDraft, MemoryRef, Phylactery, RelationshipDraft,
    RelationshipError, RelationshipParticipant,
};

fn path(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "reliquary-relationship-lifecycle-{}-{name}",
        uuid::Uuid::new_v4()
    ))
}

fn memory_draft(mutation: &str) -> MemoryDraft {
    MemoryDraft {
        category: "preference".into(),
        memory_type: "user".into(),
        authority_kind: "direct".into(),
        temporal_status: "unknown".into(),
        title: "Relationship evidence".into(),
        content: "Independent retained Memory payload".into(),
        scope: "user".into(),
        lifecycle_state: "extracted".into(),
        archived: false,
        superseded_by: None,
        parent_id: None,
        source_node_id: None,
        content_source_conversation_id: None,
        content_source_node_id: None,
        grounding_source_conversation_id: None,
        grounding_source_node_id: None,
        source_episode_id: None,
        source_time_ns: None,
        mutation_id: mutation.into(),
        created_at_ns: 10,
        updated_at_ns: 10,
    }
}

fn entity_draft(mutation: &str) -> EntityDraft {
    EntityDraft {
        canonical_name: "Ada".into(),
        aliases: vec!["A".into()],
        kind: "person".into(),
        summary: "Independent retained Entity".into(),
        mutation_id: mutation.into(),
        created_at_ns: 11,
        updated_at_ns: 11,
    }
}

fn relationship_draft(
    owner_id: String,
    entity: crate::EntityId,
    memory: crate::MemoryId,
) -> RelationshipDraft {
    RelationshipDraft {
        kind: "association".into(),
        participants: vec![RelationshipParticipant {
            entity: EntityRef {
                owner_id: owner_id.clone(),
                entity_id: entity,
            },
            role: None,
        }],
        evidence: vec![MemoryRef {
            owner_id,
            memory_id: memory,
        }],
        summary: "Independent relationship".into(),
        mutation_id: "relationship".into(),
        created_at_ns: 12,
        updated_at_ns: 12,
    }
}

#[test]
fn relationship_tombstone_is_terminal_idempotent_and_reopens_historically() {
    let file = path("terminal.phy");
    let mut phy = Phylactery::create(&file).unwrap();

    let memory = phy
        .publish_memory(None, 0, memory_draft("relationship-evidence"))
        .unwrap()
        .0;
    let entity = phy
        .publish_entity(None, 0, entity_draft("relationship-entity"))
        .unwrap()
        .0;
    let draft = relationship_draft(phy.owner_id().unwrap(), entity.id, memory.id);
    let relationship = phy.publish_relationship(None, 0, draft.clone()).unwrap().0;

    assert_eq!(phy.relationship(relationship.id).unwrap(), relationship);
    assert_eq!(
        phy.relationships_for_entity(&EntityRef {
            owner_id: phy.owner_id().unwrap(),
            entity_id: entity.id,
        }),
        vec![relationship.clone()]
    );

    let global_before = phy.container.latest_version();
    let relationship_before = phy.relationship_version();

    let outcome = phy
        .tombstone_relationship(relationship.id, relationship.revision)
        .unwrap();
    let published = match outcome {
        TombstoneOutcome::Published(coordinates) => coordinates,
        other => panic!("expected published tombstone, got {other:?}"),
    };

    assert_eq!(phy.container.latest_version(), global_before + 1);
    assert_eq!(phy.relationship_version(), relationship_before + 1);
    assert_eq!(published.global_version, Some(global_before + 1));
    assert_eq!(published.domain_version, Some(relationship_before + 1));
    assert!(matches!(
        phy.relationship(relationship.id),
        Err(RelationshipError::MissingRelationship)
    ));
    assert!(phy.relationships().is_empty());
    assert!(
        phy.relationships_for_entity(&EntityRef {
            owner_id: phy.owner_id().unwrap(),
            entity_id: entity.id,
        })
        .is_empty()
    );

    assert_eq!(phy.memory(memory.id).unwrap(), memory);
    assert_eq!(phy.entity(entity.id).unwrap(), entity);

    assert!(matches!(
        phy.historical_relationship_revision(relationship.id, relationship.revision)
            .unwrap(),
        HistoricalPayload::Tombstoned(value) if value == relationship
    ));
    assert!(matches!(
        phy.relationship_lifecycle_state(relationship.id).unwrap(),
        LifecycleState::Tombstoned(coordinates) if coordinates == published
    ));

    let retained = phy
        .relationship_retention_descriptor(relationship.id)
        .unwrap();
    assert_eq!(retained.id, relationship.id);
    assert_eq!(retained.tombstone_revision, relationship.revision + 1);
    assert_eq!(retained.tombstone_global_version, global_before + 1);
    assert_eq!(
        retained.tombstone_relationship_version,
        relationship_before + 1
    );
    assert_eq!(retained.revisions.len(), 1);
    assert_eq!(retained.revisions[0].revision, relationship.revision);
    assert_eq!(
        retained.revisions[0].global_version,
        relationship.global_version
    );
    assert_eq!(
        retained.revisions[0].relationship_version,
        relationship.relationship_version
    );
    assert_eq!(
        retained.revisions[0].participants,
        relationship.participants
    );
    assert_eq!(retained.revisions[0].evidence, relationship.evidence);

    assert!(matches!(
        phy.publish_relationship(Some(relationship.id), relationship.revision + 1, draft),
        Err(RelationshipError::TerminalTombstone)
    ));

    let retry_global = phy.container.latest_version();
    let retry_relationship = phy.relationship_version();
    assert_eq!(
        phy.tombstone_relationship(relationship.id, relationship.revision)
            .unwrap(),
        TombstoneOutcome::AlreadyTombstoned(published)
    );
    assert_eq!(phy.container.latest_version(), retry_global);
    assert_eq!(phy.relationship_version(), retry_relationship);
    assert!(matches!(
        phy.tombstone_relationship(relationship.id, 99),
        Err(RelationshipError::RevisionConflict)
    ));

    phy.sync().unwrap();
    drop(phy);

    let mut reopened = Phylactery::open(&file).unwrap();
    assert!(matches!(
        reopened.relationship(relationship.id),
        Err(RelationshipError::MissingRelationship)
    ));
    assert!(reopened.relationships().is_empty());
    assert_eq!(reopened.memory(memory.id).unwrap(), memory);
    assert_eq!(reopened.entity(entity.id).unwrap(), entity);
    assert_eq!(
        reopened
            .relationship_revision(relationship.id, relationship.revision)
            .unwrap(),
        relationship
    );
    assert_eq!(
        reopened
            .relationship_retention_descriptor(relationship.id)
            .unwrap(),
        retained
    );
    assert!(matches!(
        reopened.relationship_lifecycle_state(relationship.id).unwrap(),
        LifecycleState::Tombstoned(coordinates) if coordinates == published
    ));
}

#[test]
fn rel_relationship_tombstone_is_archive_passive_and_direct_only() {
    let file = path("passive.rel");
    let mut rel = Cva::create_project(&file).unwrap();

    rel.append_node(
        "n1".into(),
        "conversation".into(),
        None,
        "user".into(),
        1,
        "Archive history must not move when a Relationship is tombstoned.",
    )
    .unwrap();

    let memory = rel
        .publish_memory(None, 0, memory_draft("rel-relationship-evidence"))
        .unwrap()
        .0;
    let entity = rel
        .publish_entity(None, 0, entity_draft("rel-relationship-entity"))
        .unwrap()
        .0;
    let relationship = rel
        .publish_relationship(
            None,
            0,
            relationship_draft(rel.owner_id().unwrap(), entity.id, memory.id),
        )
        .unwrap()
        .0;

    let archive_cut = rel.archive_version();
    let activity_cut = rel.archive().rel_turn_count();
    let memory_cut = rel.memory_version();
    let entity_cut = rel.entity_version();

    rel.tombstone_relationship(relationship.id, relationship.revision)
        .unwrap();

    assert_eq!(rel.archive_version(), archive_cut);
    assert_eq!(rel.archive().rel_turn_count(), activity_cut);
    assert_eq!(rel.memory_version(), memory_cut);
    assert_eq!(rel.entity_version(), entity_cut);
    assert_eq!(rel.memory(memory.id).unwrap(), memory);
    assert_eq!(rel.entity(entity.id).unwrap(), entity);
    assert!(rel.relationships().is_empty());

    rel.sync().unwrap();
    drop(rel);

    let mut reopened = Cva::open(&file).unwrap();
    assert_eq!(reopened.archive_version(), archive_cut);
    assert_eq!(reopened.archive().rel_turn_count(), activity_cut);
    assert_eq!(reopened.memory(memory.id).unwrap(), memory);
    assert_eq!(reopened.entity(entity.id).unwrap(), entity);
    assert_eq!(
        reopened
            .relationship_revision(relationship.id, relationship.revision)
            .unwrap(),
        relationship
    );
    assert!(reopened.relationships().is_empty());
}
