use crate::owner_lifecycle::{HistoricalPayload, LifecycleState, TombstoneOutcome};
use crate::{
    Cva, EntityRef, GraphRelationKind, MemoryDraft, MemoryError, MemoryRef, Phylactery,
    RelationshipDraft, RelationshipError, RelationshipParticipant,
};

fn path(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "reliquary-memory-lifecycle-{}-{name}",
        uuid::Uuid::new_v4()
    ))
}

fn memory_draft(mutation: &str, title: &str, content: &str) -> MemoryDraft {
    MemoryDraft {
        category: "preference".into(),
        memory_type: "user".into(),
        authority_kind: "direct".into(),
        temporal_status: "unknown".into(),
        title: title.into(),
        content: content.into(),
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

#[test]
fn memory_tombstone_is_terminal_idempotent_and_reopens_with_historical_payload() {
    let file = path("terminal.phy");
    let mut phy = Phylactery::create(&file).unwrap();
    let deleted = phy
        .publish_memory(
            None,
            0,
            memory_draft(
                "deleted",
                "Deleted lifecycle memory",
                "Only the deleted Memory contains cobalt-jellyfish.",
            ),
        )
        .unwrap()
        .0;
    let survivor = phy
        .publish_memory(
            None,
            0,
            memory_draft(
                "survivor",
                "Surviving lifecycle memory",
                "The surviving Memory is unrelated.",
            ),
        )
        .unwrap()
        .0;

    phy.set_memory_relation(deleted.id, survivor.id, GraphRelationKind::Topical, true, 0)
        .unwrap();
    assert_eq!(
        phy.memory_lexical_candidates("cobalt-jellyfish", 10)
            .unwrap()
            .len(),
        1
    );

    let graph_cut = phy.graph_version();
    let global_before = phy.container.latest_version();
    let memory_before = phy.memory_version();

    let outcome = phy.tombstone_memory(deleted.id, deleted.revision).unwrap();
    let published = match outcome {
        TombstoneOutcome::Published(coordinates) => coordinates,
        other => panic!("expected published tombstone, got {other:?}"),
    };

    assert_eq!(phy.container.latest_version(), global_before + 1);
    assert_eq!(phy.memory_version(), memory_before + 1);
    assert_eq!(published.global_version, Some(global_before + 1));
    assert_eq!(published.domain_version, Some(memory_before + 1));
    assert!(matches!(
        phy.memory(deleted.id),
        Err(MemoryError::MissingMemory)
    ));
    assert_eq!(phy.memory(survivor.id).unwrap(), survivor);
    assert!(!phy.memory_ids().contains(&deleted.id));
    assert_eq!(
        phy.memory_lexical_candidates("cobalt-jellyfish", 10)
            .unwrap()
            .len(),
        0
    );
    assert!(phy.graph_relations().is_empty());
    assert_eq!(phy.graph_version(), graph_cut);

    assert!(matches!(
        phy.historical_memory_revision(deleted.id, deleted.revision)
            .unwrap(),
        HistoricalPayload::Tombstoned(value) if value == deleted
    ));
    assert!(matches!(
        phy.memory_lifecycle_state(deleted.id).unwrap(),
        LifecycleState::Tombstoned(_)
    ));
    let retained = phy.memory_retention_descriptor(deleted.id).unwrap();
    assert_eq!(retained.id, deleted.id);
    assert_eq!(retained.tombstone_revision, deleted.revision + 1);
    assert_eq!(retained.tombstone_global_version, global_before + 1);
    assert_eq!(retained.tombstone_memory_version, memory_before + 1);
    assert_eq!(retained.revisions.len(), 1);
    assert_eq!(retained.revisions[0].global_version, deleted.global_version);

    assert!(matches!(
        phy.publish_memory(
            Some(deleted.id),
            deleted.revision + 1,
            memory_draft(
                "resurrect",
                "Deleted lifecycle memory",
                "Only the deleted Memory contains cobalt-jellyfish.",
            ),
        ),
        Err(MemoryError::TerminalTombstone)
    ));

    let retry_global = phy.container.latest_version();
    let retry_memory = phy.memory_version();
    assert_eq!(
        phy.tombstone_memory(deleted.id, deleted.revision).unwrap(),
        TombstoneOutcome::AlreadyTombstoned(published)
    );
    assert_eq!(phy.container.latest_version(), retry_global);
    assert_eq!(phy.memory_version(), retry_memory);
    assert!(matches!(
        phy.tombstone_memory(deleted.id, 99),
        Err(MemoryError::RevisionConflict)
    ));

    phy.sync().unwrap();
    drop(phy);

    let mut reopened = Phylactery::open(&file).unwrap();
    assert!(matches!(
        reopened.memory(deleted.id),
        Err(MemoryError::MissingMemory)
    ));
    assert_eq!(reopened.memory(survivor.id).unwrap(), survivor);
    assert_eq!(reopened.graph_version(), graph_cut);
    assert!(reopened.graph_relations().is_empty());
    assert_eq!(
        reopened
            .memory_lexical_candidates("cobalt-jellyfish", 10)
            .unwrap()
            .len(),
        0
    );
    assert_eq!(
        reopened.memory_retention_descriptor(deleted.id).unwrap(),
        retained
    );
    assert_eq!(
        reopened
            .memory_revision(deleted.id, deleted.revision)
            .unwrap(),
        deleted
    );
    assert!(matches!(
        reopened.memory_lifecycle_state(deleted.id).unwrap(),
        LifecycleState::Tombstoned(coordinates) if coordinates == published
    ));
}

#[test]
fn rel_memory_tombstone_is_archive_passive_and_surviving_history_reopens() {
    let file = path("passive.rel");
    let mut rel = Cva::create_project(&file).unwrap();

    rel.append_node(
        "n1".into(),
        "conversation".into(),
        None,
        "user".into(),
        1,
        "Archive history must not move when a Memory is tombstoned.",
    )
    .unwrap();

    let memory = rel
        .publish_memory(
            None,
            0,
            memory_draft(
                "rel-memory",
                "REL tombstone source",
                "Historical relationship evidence survives deletion.",
            ),
        )
        .unwrap()
        .0;

    let entity = rel
        .publish_entity(
            None,
            0,
            crate::EntityDraft {
                canonical_name: "Ada".into(),
                aliases: Vec::new(),
                kind: "person".into(),
                summary: "Surviving entity".into(),
                mutation_id: "entity".into(),
                created_at_ns: 11,
                updated_at_ns: 11,
            },
        )
        .unwrap()
        .0;

    let stored_file = rel
        .store_file(
            "memory-evidence.txt".into(),
            Some("text/plain".into()),
            b"historical file-memory link",
        )
        .unwrap();
    rel.link_file_to_memory(stored_file.id, memory.id).unwrap();

    let relationship = rel
        .publish_relationship(
            None,
            0,
            RelationshipDraft {
                kind: "association".into(),
                participants: vec![RelationshipParticipant {
                    entity: EntityRef {
                        owner_id: rel.owner_id().unwrap(),
                        entity_id: entity.id,
                    },
                    role: None,
                }],
                evidence: vec![MemoryRef {
                    owner_id: rel.owner_id().unwrap(),
                    memory_id: memory.id,
                }],
                summary: "Surviving relationship".into(),
                mutation_id: "relationship".into(),
                created_at_ns: 12,
                updated_at_ns: 12,
            },
        )
        .unwrap()
        .0;

    let archive_cut = rel.archive_version();
    let activity_cut = rel.archive().rel_turn_count();

    rel.tombstone_memory(memory.id, memory.revision).unwrap();

    assert_eq!(rel.archive_version(), archive_cut);
    assert_eq!(rel.archive().rel_turn_count(), activity_cut);
    assert!(rel.memory_ids().is_empty());
    assert_eq!(rel.relationship(relationship.id).unwrap(), relationship);
    assert_eq!(rel.files_for_memory(memory.id), vec![stored_file.clone()]);

    assert!(matches!(
        rel.publish_relationship(
            None,
            0,
            RelationshipDraft {
                kind: "association".into(),
                participants: vec![RelationshipParticipant {
                    entity: EntityRef {
                        owner_id: rel.owner_id().unwrap(),
                        entity_id: entity.id,
                    },
                    role: None,
                }],
                evidence: vec![MemoryRef {
                    owner_id: rel.owner_id().unwrap(),
                    memory_id: memory.id,
                }],
                summary: "New writes still require live Memory evidence".into(),
                mutation_id: "relationship-after-delete".into(),
                created_at_ns: 13,
                updated_at_ns: 13,
            },
        ),
        Err(RelationshipError::MissingLocalMemory(id)) if id == memory.id
    ));

    rel.sync().unwrap();
    drop(rel);

    let mut reopened = Cva::open(&file).unwrap();
    assert_eq!(reopened.archive_version(), archive_cut);
    assert_eq!(reopened.archive().rel_turn_count(), activity_cut);
    assert!(reopened.memory_ids().is_empty());
    assert_eq!(
        reopened
            .memory_revision(memory.id, memory.revision)
            .unwrap(),
        memory
    );
    assert_eq!(
        reopened.relationship(relationship.id).unwrap(),
        relationship
    );
    assert_eq!(reopened.files_for_memory(memory.id), vec![stored_file]);
}

#[test]
fn mutation_retry_cannot_return_a_tombstoned_memory_under_another_id() {
    let file = path("mutation-target.phy");
    let mut phy = Phylactery::create(&file).unwrap();
    let draft = memory_draft("mutation-target", "Original", "Original body");
    let memory = phy.publish_memory(None, 0, draft.clone()).unwrap().0;
    phy.tombstone_memory(memory.id, memory.revision).unwrap();
    let global = phy.container.latest_version();
    let version = phy.memory_version();
    let other = crate::MemoryId([31; 32]);
    assert_ne!(other, memory.id);
    assert!(matches!(
        phy.publish_memory(Some(other), 0, draft.clone()),
        Err(MemoryError::MutationConflict)
    ));
    assert_eq!(phy.container.latest_version(), global);
    assert_eq!(phy.memory_version(), version);
    assert!(phy.memory_ids().is_empty());
    phy.sync().unwrap();
    drop(phy);
    let mut reopened = Phylactery::open(file).unwrap();
    assert!(matches!(
        reopened.publish_memory(Some(other), 0, draft),
        Err(MemoryError::MutationConflict)
    ));
    assert_eq!(reopened.memory_version(), version);
    assert!(reopened.memory_ids().is_empty());
}
