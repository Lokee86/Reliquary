use crate::owner_lifecycle::{HistoricalPayload, LifecycleState, TombstoneOutcome};
use crate::{
    EntityDraft, EntityError, EntityRef, MemoryDraft, MemoryRef, Phylactery, RelationshipDraft,
    RelationshipParticipant,
};

fn path(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "reliquary-entity-lifecycle-{}-{name}",
        uuid::Uuid::new_v4()
    ))
}

fn entity_draft(name: &str, mutation: &str) -> EntityDraft {
    EntityDraft {
        canonical_name: name.into(),
        aliases: vec![format!("{name} alias")],
        kind: "person".into(),
        summary: format!("{name} summary"),
        mutation_id: mutation.into(),
        created_at_ns: 10,
        updated_at_ns: 10,
    }
}

fn memory_draft(mutation: &str) -> MemoryDraft {
    MemoryDraft {
        category: "fact".into(),
        memory_type: "user".into(),
        authority_kind: "direct".into(),
        temporal_status: "unknown".into(),
        title: "Entity evidence".into(),
        content: "Independent Memory survives Entity deletion.".into(),
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
        created_at_ns: 11,
        updated_at_ns: 11,
    }
}

#[test]
fn direct_entity_tombstone_is_terminal_idempotent_and_reopens_historically() {
    let file = path("direct.phy");
    let mut phy = Phylactery::create(&file).unwrap();

    let entity = phy
        .publish_entity(None, 0, entity_draft("Ada", "entity-direct"))
        .unwrap()
        .0;
    let memory = phy
        .publish_memory(None, 0, memory_draft("entity-evidence"))
        .unwrap()
        .0;
    let owner_id = phy.owner_id().unwrap();
    let relationship = phy
        .publish_relationship(
            None,
            0,
            RelationshipDraft {
                kind: "association".into(),
                participants: vec![RelationshipParticipant {
                    entity: EntityRef {
                        owner_id: owner_id.clone(),
                        entity_id: entity.id,
                    },
                    role: None,
                }],
                evidence: vec![MemoryRef {
                    owner_id,
                    memory_id: memory.id,
                }],
                summary: "Relationship survives direct Entity deletion".into(),
                mutation_id: "entity-direct-relationship".into(),
                created_at_ns: 12,
                updated_at_ns: 12,
            },
        )
        .unwrap()
        .0;

    phy.set_entity_association(memory.id, entity.id, true, phy.graph_version())
        .unwrap();
    let graph_cut = phy.graph_version();
    assert!(!phy.semantic_graph_relations().is_empty());
    assert_eq!(
        phy.entity_candidates_for_surface("Ada", 8),
        vec![entity.clone()]
    );

    let global_before = phy.container.latest_version();
    let entity_before = phy.entity_version();
    let outcome = phy.tombstone_entity(entity.id, entity.revision).unwrap();
    let published = match outcome {
        TombstoneOutcome::Published(coordinates) => coordinates,
        other => panic!("expected published tombstone, got {other:?}"),
    };

    assert_eq!(phy.container.latest_version(), global_before + 1);
    assert_eq!(phy.entity_version(), entity_before + 1);
    assert_eq!(published.global_version, Some(global_before + 1));
    assert_eq!(published.domain_version, Some(entity_before + 1));
    assert!(matches!(
        phy.entity(entity.id),
        Err(EntityError::MissingEntity)
    ));
    assert!(phy.entity_candidates_for_surface("Ada", 8).is_empty());
    assert!(phy.semantic_graph_relations().is_empty());
    assert_eq!(phy.graph_version(), graph_cut);

    assert_eq!(phy.memory(memory.id).unwrap(), memory);
    assert_eq!(phy.relationship(relationship.id).unwrap(), relationship);
    assert!(matches!(
        phy.historical_entity_revision(entity.id, entity.revision)
            .unwrap(),
        HistoricalPayload::Tombstoned(value) if value == entity
    ));
    assert!(matches!(
        phy.entity_lifecycle_state(entity.id).unwrap(),
        LifecycleState::Tombstoned(coordinates) if coordinates == published
    ));

    let retained = phy.entity_retention_descriptor(entity.id).unwrap();
    assert_eq!(retained.id, entity.id);
    assert_eq!(retained.tombstone_revision, entity.revision + 1);
    assert_eq!(retained.tombstone_global_version, global_before + 1);
    assert_eq!(retained.tombstone_entity_version, entity_before + 1);
    assert_eq!(retained.replacement_id, None);
    assert_eq!(retained.revisions.len(), 1);
    assert_eq!(retained.revisions[0].global_id, entity.global_id);

    assert!(matches!(
        phy.publish_entity(
            Some(entity.id),
            entity.revision + 1,
            entity_draft("Ada Returned", "entity-direct-republish")
        ),
        Err(EntityError::TerminalTombstone)
    ));

    let retry_global = phy.container.latest_version();
    let retry_entity = phy.entity_version();
    assert_eq!(
        phy.tombstone_entity(entity.id, entity.revision).unwrap(),
        TombstoneOutcome::AlreadyTombstoned(published)
    );
    assert_eq!(phy.container.latest_version(), retry_global);
    assert_eq!(phy.entity_version(), retry_entity);
    assert!(matches!(
        phy.tombstone_entity(entity.id, 99),
        Err(EntityError::RevisionConflict)
    ));

    phy.sync().unwrap();
    drop(phy);

    let mut reopened = Phylactery::open(&file).unwrap();
    assert!(matches!(
        reopened.entity(entity.id),
        Err(EntityError::MissingEntity)
    ));
    assert!(reopened.entity_candidates_for_surface("Ada", 8).is_empty());
    assert!(reopened.semantic_graph_relations().is_empty());
    assert_eq!(reopened.graph_version(), graph_cut);
    assert_eq!(reopened.memory(memory.id).unwrap(), memory);
    assert_eq!(
        reopened.relationship(relationship.id).unwrap(),
        relationship
    );
    assert_eq!(
        reopened
            .entity_revision(entity.id, entity.revision)
            .unwrap(),
        entity
    );
    assert_eq!(
        reopened.entity_retention_descriptor(entity.id).unwrap(),
        retained
    );
    assert!(matches!(
        reopened.entity_lifecycle_state(entity.id).unwrap(),
        LifecycleState::Tombstoned(coordinates) if coordinates == published
    ));
}

#[test]
fn merge_retirement_uses_shared_terminal_lifecycle_and_keeps_replacement() {
    let file = path("merge.phy");
    let mut phy = Phylactery::create(&file).unwrap();

    let survivor = phy
        .publish_entity(None, 0, entity_draft("Canonical", "entity-survivor"))
        .unwrap()
        .0;
    let retired = phy
        .publish_entity(None, 0, entity_draft("Duplicate", "entity-retired"))
        .unwrap()
        .0;

    let outcome = phy.merge_entities(survivor.id, retired.id, 20).unwrap();
    assert!(outcome.changed);
    assert_eq!(outcome.survivor_id, survivor.id);
    assert_eq!(outcome.retired_id, retired.id);
    assert_eq!(
        phy.retired_entity_replacement(retired.id),
        Some(survivor.id)
    );

    let state = phy.entity_lifecycle_state(retired.id).unwrap();
    let coordinates = match state {
        LifecycleState::Tombstoned(coordinates) => coordinates,
        other => panic!("expected tombstoned lifecycle, got {other:?}"),
    };
    let retained = phy.entity_retention_descriptor(retired.id).unwrap();
    assert_eq!(retained.replacement_id, Some(survivor.id));
    assert_eq!(
        retained.tombstone_global_version,
        coordinates.global_version.unwrap()
    );
    assert_eq!(
        retained.tombstone_entity_version,
        coordinates.domain_version.unwrap()
    );
    assert_eq!(
        phy.entity_revision(retired.id, retired.revision).unwrap(),
        retired
    );
    assert!(matches!(
        phy.publish_entity(
            Some(retired.id),
            retired.revision + 1,
            entity_draft("Duplicate Returned", "entity-retired-republish")
        ),
        Err(EntityError::TerminalTombstone)
    ));

    let retry = phy.merge_entities(survivor.id, retired.id, 21).unwrap();
    assert!(!retry.changed);

    phy.sync().unwrap();
    drop(phy);

    let reopened = Phylactery::open(&file).unwrap();
    assert_eq!(
        reopened.retired_entity_replacement(retired.id),
        Some(survivor.id)
    );
    assert!(matches!(
        reopened.entity_lifecycle_state(retired.id).unwrap(),
        LifecycleState::Tombstoned(value) if value == coordinates
    ));
    assert_eq!(
        reopened.entity_retention_descriptor(retired.id).unwrap(),
        retained
    );
}
