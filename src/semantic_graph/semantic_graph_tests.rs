use crate::{
    Cva, EntityDraft, EntityId, EpisodeBoundary, EpisodeConfig, EpisodeOrigin, GraphDirection,
    GraphError, GraphRelationChange, GraphRelationKind, GraphRelationOrigin, MemoryDraft, MemoryId,
    SemanticGraphRelationChange, SemanticGraphRelationKind, SemanticNodeRef,
};
use std::{fs, path::PathBuf};

#[test]
fn typed_entity_association_survives_reopen_without_entering_memory_projection() {
    let file = path("entity-association.cva");
    let mut cva = Cva::create(&file).unwrap();
    let ep = episode(&mut cva);
    let a = memory(&mut cva, &ep, "a");
    let b = memory(&mut cva, &ep, "b");
    let entity_id = entity(&mut cva, "Reliquary");

    cva.set_memory_relation(a, b, GraphRelationKind::Factual, true, 0)
        .unwrap()
        .unwrap();
    let snapshot = cva.refresh_communities_leiden().unwrap();
    assert_eq!(snapshot.derived_graph_version, 1);

    let association = cva
        .set_entity_association(a, entity_id, true, 1)
        .unwrap()
        .unwrap();
    assert_eq!(
        association.kind,
        SemanticGraphRelationKind::EntityAssociation
    );
    assert_eq!(association.origin, GraphRelationOrigin::Perception);
    assert_eq!(cva.graph_version(), 2);
    assert_eq!(cva.memory_graph_version(), 1);
    assert_eq!(cva.graph_relations().len(), 1);
    assert_eq!(cva.semantic_graph_relations().len(), 2);
    assert_eq!(cva.entity_associations_for_memory(a), vec![entity_id]);
    assert_eq!(cva.memories_for_entity(entity_id), vec![a]);

    let semantic_neighbors = cva
        .semantic_graph_neighbors(SemanticNodeRef::memory(a), GraphDirection::Outgoing)
        .unwrap();
    assert!(semantic_neighbors.iter().any(|neighbor| {
        neighbor.node == SemanticNodeRef::entity(entity_id)
            && neighbor.kind == SemanticGraphRelationKind::EntityAssociation
    }));
    assert_eq!(
        cva.graph_neighbors(a, GraphDirection::Outgoing).unwrap()[0].memory_id,
        b
    );
    assert!(cva.community_stats().current);
    assert_eq!(
        cva.refresh_communities_leiden().unwrap().generation,
        snapshot.generation
    );

    cva.sync().unwrap();
    drop(cva);

    let reopened = Cva::open(&file).unwrap();
    assert_eq!(reopened.graph_version(), 2);
    assert_eq!(reopened.memory_graph_version(), 1);
    assert_eq!(reopened.entity_associations_for_memory(a), vec![entity_id]);
    assert_eq!(reopened.graph_stats().nodes, 3);
    assert_eq!(reopened.graph_stats().memory_nodes, 2);
    assert_eq!(reopened.graph_stats().active_relations, 2);
    assert_eq!(reopened.graph_stats().memory_active_relations, 1);
    assert!(reopened.community_stats().current);
}

#[test]
fn entity_only_graph_activity_does_not_create_memory_community_nodes() {
    let file = path("entity-only.cva");
    let mut cva = Cva::create(&file).unwrap();
    let ep = episode(&mut cva);
    let a = memory(&mut cva, &ep, "a");
    let entity_id = entity(&mut cva, "EntityOnly");

    cva.set_entity_association(a, entity_id, true, 0)
        .unwrap()
        .unwrap();

    assert_eq!(cva.graph_version(), 1);
    assert_eq!(cva.memory_graph_version(), 0);
    assert_eq!(cva.graph_stats().nodes, 2);
    assert_eq!(cva.graph_stats().memory_nodes, 0);
    assert!(cva.graph_relations().is_empty());
    let snapshot = cva.refresh_communities_leiden().unwrap();
    assert!(snapshot.communities.is_empty());
    assert_eq!(snapshot.derived_graph_version, 0);
}

#[test]
fn entity_association_validates_endpoint_shape_owner_and_origin() {
    let file = path("entity-association-rules.cva");
    let mut cva = Cva::create(&file).unwrap();
    let ep = episode(&mut cva);
    let a = memory(&mut cva, &ep, "a");
    let entity_id = entity(&mut cva, "Rules");

    assert!(matches!(
        cva.set_entity_association(a, EntityId([99; 32]), true, 0),
        Err(GraphError::MissingEntity(_))
    ));

    let reversed = SemanticGraphRelationChange {
        source: SemanticNodeRef::entity(entity_id),
        target: SemanticNodeRef::memory(a),
        kind: SemanticGraphRelationKind::EntityAssociation,
        active: true,
    };
    assert!(matches!(
        cva.set_semantic_relations_with_origin(&[reversed], GraphRelationOrigin::Perception, 0,),
        Err(GraphError::InvalidRelationShape)
    ));

    let valid = SemanticGraphRelationChange::entity_association(a, entity_id, true);
    assert!(matches!(
        cva.set_semantic_relations_with_origin(&[valid], GraphRelationOrigin::Dream, 0),
        Err(GraphError::InvalidRelationOrigin)
    ));
}

#[test]
fn memory_endpoint_error_precedes_invalid_origin() {
    let file = path("memory-endpoint-origin-order.cva");
    let mut cva = Cva::create(&file).unwrap();
    let missing = GraphRelationChange {
        source: MemoryId([98; 32]),
        target: MemoryId([99; 32]),
        kind: GraphRelationKind::Factual,
        active: true,
    };

    assert!(matches!(
        cva.set_memory_relations_with_origin(&[missing], GraphRelationOrigin::Perception, 0),
        Err(GraphError::MissingMemory(id)) if id == missing.source
    ));
}

fn path(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "continuity-semantic-graph-{}",
        uuid::Uuid::new_v4()
    ));
    fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

fn episode(cva: &mut Cva) -> crate::Episode {
    cva.append_node(
        "u".into(),
        "c".into(),
        None,
        "user".into(),
        10,
        "connect semantic nodes",
    )
    .unwrap();
    cva.append_node(
        "a".into(),
        "c".into(),
        Some("u".into()),
        "assistant".into(),
        20,
        "ok",
    )
    .unwrap();
    cva.materialize_path_episodes(
        "c",
        "a",
        EpisodeConfig::default(),
        EpisodeOrigin::Live,
        Some((EpisodeBoundary::Inactivity, 30)),
    )
    .unwrap()
    .created
    .into_iter()
    .next()
    .unwrap()
}

fn memory(cva: &mut Cva, episode: &crate::Episode, id: &str) -> MemoryId {
    cva.publish_memory(
        None,
        0,
        MemoryDraft {
            category: "fact".into(),
            memory_type: "project".into(),
            authority_kind: "unknown".into(),
            temporal_status: "unknown".into(),
            title: id.into(),
            content: format!("memory {id}"),
            scope: "private".into(),
            lifecycle_state: "extracted".into(),
            archived: false,
            superseded_by: None,
            parent_id: None,
            source_node_id: Some("u".into()),
            content_source_conversation_id: None,
            content_source_node_id: None,
            grounding_source_conversation_id: None,
            grounding_source_node_id: None,
            source_episode_id: Some(episode.id),
            source_time_ns: None,
            mutation_id: id.into(),
            created_at_ns: 30,
            updated_at_ns: 30,
        },
    )
    .unwrap()
    .0
    .id
}

fn entity(cva: &mut Cva, name: &str) -> EntityId {
    cva.publish_entity(
        None,
        0,
        EntityDraft {
            canonical_name: name.into(),
            aliases: vec![],
            kind: "project".into(),
            summary: format!("{name} entity"),
            mutation_id: format!("entity-{name}"),
            created_at_ns: 30,
            updated_at_ns: 30,
        },
    )
    .unwrap()
    .0
    .id
}
